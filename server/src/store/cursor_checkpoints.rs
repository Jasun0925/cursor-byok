//! Stores Cursor presentation state alongside its immutable canonical checkpoint.
use crate::{model::CheckpointId, Result};

use super::Store;

impl Store {
    pub(crate) async fn save_cursor_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
        state: &[u8],
    ) -> Result<()> {
        let _write = self.writes.lock().await;
        sqlx::query(
            "INSERT INTO cursor_checkpoints(checkpoint_id, state) VALUES (?, ?)
             ON CONFLICT(checkpoint_id) DO UPDATE SET state = excluded.state",
        )
        .bind(checkpoint_id.0)
        .bind(state)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn cursor_checkpoint(
        &self,
        checkpoint_id: CheckpointId,
    ) -> Result<Option<Vec<u8>>> {
        Ok(sqlx::query_scalar(
            "WITH RECURSIVE lineage(checkpoint_id, parent_checkpoint_id, depth) AS (
                 SELECT checkpoint_id, parent_checkpoint_id, 0
                 FROM conversation_checkpoints WHERE checkpoint_id = ?
                 UNION ALL
                 SELECT c.checkpoint_id, c.parent_checkpoint_id, l.depth + 1
                 FROM conversation_checkpoints c JOIN lineage l ON c.checkpoint_id = l.parent_checkpoint_id
             )
             SELECT s.state FROM lineage l JOIN cursor_checkpoints s ON s.checkpoint_id = l.checkpoint_id
             ORDER BY l.depth LIMIT 1",
        )
        .bind(checkpoint_id.0)
        .fetch_optional(&self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use sqlx::{
        migrate::Migrator,
        sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    };

    use super::Store;
    use crate::model::{CanonicalMessage, CheckpointId, ConversationId, Origin, Role};

    #[tokio::test]
    async fn upgrade_preserves_history_and_recovers_checkpoint_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("checkpoints.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        let all = sqlx::migrate!("./migrations");
        let prior = Migrator {
            migrations: Cow::Owned(
                all.iter()
                    .filter(|migration| migration.version <= 10)
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        prior.run(&pool).await.unwrap();
        let old = Store {
            pool,
            writes: Default::default(),
        };
        let conversation = ConversationId::new("preserved-conversation");
        let root = old.ensure_conversation(&conversation).await.unwrap();
        let history = vec![CanonicalMessage::text(
            "old-user",
            Role::User,
            Origin::User,
            "Keep this history.",
        )];
        let child = old
            .import_checkpoint(&conversation, &history)
            .await
            .unwrap();
        old.pool.close().await;

        let store = Store::connect(&format!("sqlite://{}", path.display()))
            .await
            .unwrap();
        assert_eq!(
            store.load_checkpoint_messages(child).await.unwrap(),
            history
        );
        assert_eq!(store.cursor_checkpoint(child).await.unwrap(), None);
        store
            .save_cursor_checkpoint(root, b"root state")
            .await
            .unwrap();
        assert_eq!(
            store.cursor_checkpoint(child).await.unwrap().as_deref(),
            Some(b"root state".as_slice())
        );
        store
            .save_cursor_checkpoint(child, b"child state")
            .await
            .unwrap();
        store
            .save_cursor_checkpoint(child, b"settled child state")
            .await
            .unwrap();
        assert_eq!(
            store.cursor_checkpoint(child).await.unwrap().as_deref(),
            Some(b"settled child state".as_slice())
        );
        assert!(store
            .save_cursor_checkpoint(CheckpointId(i64::MAX), b"invalid")
            .await
            .is_err());
        let other = store
            .ensure_conversation(&ConversationId::new("other-conversation"))
            .await
            .unwrap();
        assert_eq!(store.cursor_checkpoint(other).await.unwrap(), None);
        store.pool.close().await;
        let reopened = Store::connect(&format!("sqlite://{}", path.display()))
            .await
            .unwrap();
        assert_eq!(
            reopened.cursor_checkpoint(child).await.unwrap().as_deref(),
            Some(b"settled child state".as_slice())
        );
        assert_eq!(
            reopened.load_checkpoint_messages(child).await.unwrap(),
            history
        );
    }
}

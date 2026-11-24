//! Deduplicates Cursor inputs against their Conversation base.
use crate::{
    model::{CheckpointId, ConversationId},
    Result,
};

use super::{now_ms, Store};

impl Store {
    pub async fn anchor_input(
        &self,
        conversation_id: &ConversationId,
        input_id: &str,
        base_checkpoint_id: CheckpointId,
    ) -> Result<CheckpointId> {
        let _write = self.writes.lock().await;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "INSERT INTO input_anchors
             (conversation_id, input_id, base_checkpoint_id, created_at_ms)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(conversation_id, input_id) DO NOTHING",
        )
        .bind(conversation_id.as_str())
        .bind(input_id)
        .bind(base_checkpoint_id.0)
        .bind(now_ms())
        .execute(&mut *tx)
        .await?;
        let anchored = sqlx::query_scalar::<_, i64>(
            "SELECT base_checkpoint_id FROM input_anchors
             WHERE conversation_id = ? AND input_id = ?",
        )
        .bind(conversation_id.as_str())
        .bind(input_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(CheckpointId(anchored))
    }

    /// Moves an input from a base that can no longer be sent to a model onto
    /// the base Cursor sent with this attempt. The move only happens while the
    /// input is still anchored to `from`, so a concurrent move is not undone.
    pub async fn reanchor_input(
        &self,
        conversation_id: &ConversationId,
        input_id: &str,
        from: CheckpointId,
        to: CheckpointId,
    ) -> Result<CheckpointId> {
        let _write = self.writes.lock().await;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query(
            "UPDATE input_anchors SET base_checkpoint_id = ?
             WHERE conversation_id = ? AND input_id = ? AND base_checkpoint_id = ?",
        )
        .bind(to.0)
        .bind(conversation_id.as_str())
        .bind(input_id)
        .bind(from.0)
        .execute(&mut *tx)
        .await?;
        let anchored = sqlx::query_scalar::<_, i64>(
            "SELECT base_checkpoint_id FROM input_anchors
             WHERE conversation_id = ? AND input_id = ?",
        )
        .bind(conversation_id.as_str())
        .bind(input_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(CheckpointId(anchored))
    }
}

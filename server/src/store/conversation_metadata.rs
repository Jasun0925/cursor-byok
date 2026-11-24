//! Stores presentation metadata independently of immutable conversation history.
use super::Store;
use crate::{model::ConversationId, Error, Result};

#[cfg(test)]
#[path = "conversation_metadata_tests.rs"]
mod tests;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationMetadata {
    pub name: Option<String>,
    pub workspace_paths: Vec<String>,
}

impl Store {
    pub async fn conversation_metadata(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationMetadata>> {
        let row: Option<(Option<String>, String)> = sqlx::query_as(
            "SELECT name, workspace_paths_json FROM conversation_metadata WHERE conversation_id = ?"
        ).bind(id.as_str()).fetch_optional(&self.pool).await?;
        row.map(|(name, paths)| {
            Ok(ConversationMetadata {
                name,
                workspace_paths: serde_json::from_str(&paths)?,
            })
        })
        .transpose()
    }

    pub async fn update_conversation_metadata(
        &self,
        id: &ConversationId,
        name: Option<&str>,
        only_if_empty: bool,
        paths: &[String],
    ) -> Result<()> {
        if id.as_str().trim().is_empty() {
            return Err(Error::Protocol("conversation_id is required".into()));
        }
        let _write = self.writes.lock().await;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row: Option<(Option<String>, String)> = sqlx::query_as(
            "SELECT name, workspace_paths_json FROM conversation_metadata WHERE conversation_id = ?"
        ).bind(id.as_str()).fetch_optional(&mut *tx).await?;
        let original = row
            .map(|(name, paths)| -> Result<_> {
                Ok(ConversationMetadata {
                    name,
                    workspace_paths: serde_json::from_str(&paths)?,
                })
            })
            .transpose()?
            .unwrap_or_default();
        let mut next = original.clone();
        if let Some(name) = name {
            if !only_if_empty
                || next
                    .name
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
            {
                next.name = Some(name.trim().to_owned());
            }
        }
        let mut normalized = Vec::new();
        for path in paths
            .iter()
            .map(|path| path.trim())
            .filter(|path| !path.is_empty())
        {
            if !normalized.iter().any(|existing| existing == path) {
                normalized.push(path.to_owned());
            }
        }
        // The protobuf repeated field cannot distinguish absent from empty.
        // A name-only request must not erase known workspace paths.
        if !normalized.is_empty() {
            next.workspace_paths = normalized;
        }
        if next != original {
            sqlx::query("INSERT INTO conversation_metadata (conversation_id, name, workspace_paths_json) VALUES (?, ?, ?) ON CONFLICT(conversation_id) DO UPDATE SET name = excluded.name, workspace_paths_json = excluded.workspace_paths_json")
                .bind(id.as_str()).bind(&next.name).bind(serde_json::to_string(&next.workspace_paths)?)
                .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

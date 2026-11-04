//! Removes content-addressed blobs that no saved conversation can reach.
//!
//! Every blob that a stored Cursor checkpoint, a stored tool result image, a
//! trace artifact, or a recent write may still need is marked first. Only
//! unmarked blobs are deleted. Blobs written since the oldest running turn
//! started are kept, because a turn may hold blob ids in memory until it saves
//! its checkpoint. A checkpoint or message that cannot be decoded aborts the
//! whole cleanup before anything is deleted. Request-context blobs (rules,
//! skills, subagents, MCPs) are not referenced by checkpoints; Cursor resends
//! them when they are missing, and recent ones are kept by the grace window.

use std::collections::HashSet;

use prost::Message;
use serde::Serialize;
use sqlx::Row;

use crate::{
    cursor::protocol::proto::agent::v1 as pb,
    model::{CanonicalMessage, MessageContent},
    Error, Result,
};

use crate::store::{now_ms, BlobId, Store};

/// Blobs written in this window are kept. A run may persist blobs before the
/// checkpoint that references them is saved.
pub(crate) const RECENT_BLOB_GRACE_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct UnreferencedBlobs {
    pub blob_count: i64,
    pub byte_count: i64,
}

impl Store {
    /// Counts what [`Store::clear_unreferenced_blobs`] would delete, without deleting.
    pub async fn unreferenced_blobs(&self) -> Result<UnreferencedBlobs> {
        let _write = self.writes.lock().await;
        let candidates = self.unreferenced_blob_ids(now_ms()).await?;
        self.measure_blobs(&candidates).await
    }

    /// Deletes unreachable blobs. The caller must hold the write lock.
    pub(super) async fn delete_unreferenced_blobs(
        &self,
        cutoff_now_ms: i64,
    ) -> Result<UnreferencedBlobs> {
        let candidates = self.unreferenced_blob_ids(cutoff_now_ms).await?;
        let removed = self.measure_blobs(&candidates).await?;
        if candidates.is_empty() {
            return Ok(removed);
        }
        let mut transaction = self.pool.begin().await?;
        for id in &candidates {
            // Edges from this blob describe its children; edges into it can only
            // come from other unreachable blobs, because reachability follows edges.
            sqlx::query("DELETE FROM blob_edges WHERE parent_blob_id = ? OR child_blob_id = ?")
                .bind(id.as_bytes().as_slice())
                .bind(id.as_bytes().as_slice())
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM blobs WHERE blob_id = ?")
                .bind(id.as_bytes().as_slice())
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(removed)
    }

    async fn measure_blobs(&self, ids: &[BlobId]) -> Result<UnreferencedBlobs> {
        let mut total = UnreferencedBlobs::default();
        for id in ids {
            let bytes: Option<i64> =
                sqlx::query_scalar("SELECT length(data) FROM blobs WHERE blob_id = ?")
                    .bind(id.as_bytes().as_slice())
                    .fetch_optional(&self.pool)
                    .await?;
            if let Some(bytes) = bytes {
                total.blob_count += 1;
                total.byte_count += bytes;
            }
        }
        Ok(total)
    }

    /// Returns blobs that are not reachable from any retained root.
    async fn unreferenced_blob_ids(&self, now: i64) -> Result<Vec<BlobId>> {
        // A running turn may hold blob ids only in memory until it saves its
        // checkpoint. Keep every blob written since the oldest running turn
        // started, in addition to the normal recent-write window.
        let keep_since = match self.oldest_running_run_started_at().await? {
            Some(started) => started.min(now - RECENT_BLOB_GRACE_MS),
            None => now - RECENT_BLOB_GRACE_MS,
        };
        let mut roots = self.checkpoint_blob_roots().await?;
        roots.extend(self.message_image_roots().await?);
        roots.extend(self.trace_artifact_roots().await?);
        roots.extend(self.recent_blob_roots(keep_since).await?);
        let reachable: HashSet<BlobId> = self
            .reachable_from(roots.into_iter().collect())
            .await?
            .into_iter()
            .collect();

        let mut candidates = Vec::new();
        let rows = sqlx::query("SELECT blob_id FROM blobs")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let id = BlobId::from_bytes(row.get::<Vec<u8>, _>(0).as_slice())?;
            if !reachable.contains(&id) {
                candidates.push(id);
            }
        }
        candidates.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        Ok(candidates)
    }

    async fn oldest_running_run_started_at(&self) -> Result<Option<i64>> {
        Ok(sqlx::query_scalar(
            "SELECT MIN(r.created_at_ms) FROM conversations c
             JOIN runs r ON r.run_id = c.active_run_id
             WHERE r.status = 'running'",
        )
        .fetch_one(&self.pool)
        .await?)
    }

    async fn checkpoint_blob_roots(&self) -> Result<HashSet<BlobId>> {
        let mut roots = HashSet::new();
        let rows = sqlx::query("SELECT checkpoint_id, state FROM cursor_checkpoints")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let checkpoint_id: i64 = row.get(0);
            let state: Vec<u8> = row.get(1);
            let state = pb::ConversationStateStructure::decode(state.as_slice()).map_err(|error| {
                Error::Store(format!(
                    "Cursor checkpoint {checkpoint_id} cannot be decoded, so no blob was removed: {error}"
                ))
            })?;
            collect_state_roots(&state, &mut roots).map_err(|error| {
                Error::Store(format!(
                    "Cursor checkpoint {checkpoint_id} has an invalid blob reference, so no blob was removed: {error}"
                ))
            })?;
        }
        Ok(roots)
    }

    async fn message_image_roots(&self) -> Result<HashSet<BlobId>> {
        let mut roots = HashSet::new();
        let rows = sqlx::query(
            "SELECT conversation_id, message_id, payload_json FROM messages
             WHERE payload_json LIKE '%\"blob_id\"%'",
        )
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let payload: String = row.get(2);
            let message = serde_json::from_str::<CanonicalMessage>(&payload).map_err(|error| {
                Error::Store(format!(
                    "message {}/{} cannot be decoded, so no blob was removed: {error}",
                    row.get::<String, _>(0),
                    row.get::<String, _>(1)
                ))
            })?;
            if let MessageContent::ToolResult(result) = message.content {
                if let Some(image) = result.image {
                    roots.insert(BlobId::from_base64(&image.blob_id)?);
                }
            }
        }
        Ok(roots)
    }

    async fn trace_artifact_roots(&self) -> Result<HashSet<BlobId>> {
        let mut roots = HashSet::new();
        let rows = sqlx::query("SELECT DISTINCT blob_id FROM cursor_run_trace_artifacts")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            roots.insert(BlobId::from_bytes(row.get::<Vec<u8>, _>(0).as_slice())?);
        }
        Ok(roots)
    }

    async fn recent_blob_roots(&self, since_ms: i64) -> Result<HashSet<BlobId>> {
        let mut roots = HashSet::new();
        let rows = sqlx::query("SELECT blob_id FROM blobs WHERE created_at_ms >= ?")
            .bind(since_ms)
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            roots.insert(BlobId::from_bytes(row.get::<Vec<u8>, _>(0).as_slice())?);
        }
        Ok(roots)
    }

    /// Follows `blob_edges` from every root and returns the full reachable set.
    async fn reachable_from(&self, roots: Vec<BlobId>) -> Result<Vec<BlobId>> {
        let mut seen = HashSet::new();
        let mut stack = roots;
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            let rows = sqlx::query("SELECT child_blob_id FROM blob_edges WHERE parent_blob_id = ?")
                .bind(id.as_bytes().as_slice())
                .fetch_all(&self.pool)
                .await?;
            for row in rows {
                let child = BlobId::from_bytes(row.get::<Vec<u8>, _>(0).as_slice())?;
                if !seen.contains(&child) {
                    stack.push(child);
                }
            }
        }
        Ok(seen.into_iter().collect())
    }
}

/// Collects every blob id a Cursor checkpoint state refers to directly.
fn collect_state_roots(
    state: &pb::ConversationStateStructure,
    roots: &mut HashSet<BlobId>,
) -> Result<()> {
    let mut add = |raw: &[u8]| -> Result<()> {
        roots.insert(BlobId::from_bytes(raw)?);
        Ok(())
    };
    for raw in state
        .root_prompt_messages_json
        .iter()
        .chain(&state.turns)
        .chain(&state.todos)
        .chain(&state.summary_archives)
    {
        add(raw)?;
    }
    for raw in [&state.summary, &state.plan, &state.summary_archive]
        .into_iter()
        .flatten()
    {
        add(raw)?;
    }
    for file in state.file_states_v2.values() {
        for raw in [&file.content, &file.initial_content].into_iter().flatten() {
            add(raw)?;
        }
    }
    for raw in state.subagent_state_refs.values() {
        add(raw)?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn collect_state_roots_for_test(
    state: &pb::ConversationStateStructure,
    roots: &mut HashSet<BlobId>,
) {
    collect_state_roots(state, roots).unwrap();
}

//! Admits each background completion once before importing state or waking a Run.
use std::{collections::HashSet, sync::Arc};

use parking_lot::Mutex;
use prost::Message;

use crate::{
    cursor::{compile, protocol::proto::agent::v1 as pb},
    model::{project_messages, CheckpointId, ConversationId},
    store::{BlobId, Store},
    Result,
};

type Key = (ConversationId, String);

pub(super) async fn pending_base_checkpoint(
    store: &Store,
    conversation_id: &ConversationId,
    prepared: CheckpointId,
    incoming_state: Option<&pb::ConversationStateStructure>,
) -> Result<(CheckpointId, Option<pb::ConversationStateStructure>)> {
    let current = store.ensure_conversation(conversation_id).await?;
    let state = store
        .cursor_checkpoint(current)
        .await?
        .map(|bytes| pb::ConversationStateStructure::decode(bytes.as_slice()))
        .transpose()?;
    let incoming = project_messages(&store.load_checkpoint_messages(prepared).await?)?;
    let local = project_messages(&store.load_checkpoint_messages(current).await?)?;
    // Imported assistant/tool IDs differ. An incoming prefix can use the local
    // canonical IDs; a newer upstream history must still be imported intact.
    let local_extends_incoming = local.len() >= incoming.len()
        && incoming
            .iter()
            .zip(&local)
            .all(|(a, b)| a.role == b.role && a.content == b.content);
    let mut summarized = false;
    if let (Some(incoming), Some(current)) = (incoming_state, state.as_ref()) {
        let roots = incoming
            .root_prompt_messages_json
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>();
        // Compaction deliberately replaces a prefix. Its archived roots prove
        // whether a late notification refers to that replaced history.
        for archive_id in &current.summary_archives {
            if let Some(bytes) = store.get_blob(&BlobId::from_bytes(archive_id)?).await? {
                let archive = pb::ConversationSummaryArchive::decode(bytes.as_slice())?;
                if archive.summarized_messages.starts_with(&roots) {
                    summarized = true;
                    break;
                }
            }
        }
    }
    if local_extends_incoming || summarized {
        Ok((current, state))
    } else {
        Ok((prepared, None))
    }
}

#[derive(Clone, Default)]
pub(super) struct BackgroundCompletions(Arc<Mutex<HashSet<Key>>>);

pub(super) enum Admission {
    Ordinary,
    Ignored,
    New(CompletionGuard),
    Duplicate,
}

pub(super) struct CompletionGuard {
    pending: BackgroundCompletions,
    keys: Vec<Key>,
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        let mut pending = self.pending.0.lock();
        for key in &self.keys {
            pending.remove(key);
        }
    }
}

impl BackgroundCompletions {
    pub async fn admit(
        &self,
        store: &Store,
        request_id: &str,
        request: &mut pb::AgentRunRequest,
    ) -> Result<Admission> {
        let Some(action) = request
            .action
            .as_ref()
            .and_then(|action| action.action.as_ref())
        else {
            return Ok(Admission::Ordinary);
        };
        if !matches!(
            action,
            pb::conversation_action::Action::BackgroundTaskCompletionAction(_)
        ) {
            return Ok(Admission::Ordinary);
        }
        let events = compile::background_completion_events(request)?;
        if events.is_empty() {
            return Ok(Admission::Ignored);
        }
        let conversation = ConversationId::new(
            request
                .conversation_id
                .clone()
                .unwrap_or_else(|| request_id.into()),
        );
        let mut guard = CompletionGuard {
            pending: self.clone(),
            keys: Vec::new(),
        };
        let claimed = {
            let mut pending = self.0.lock();
            events
                .into_iter()
                .filter_map(|(index, event)| {
                    let key = (conversation.clone(), event);
                    if pending.insert(key.clone()) {
                        guard.keys.push(key.clone());
                        Some((index, key))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut admitted = HashSet::new();
        for (index, key) in claimed {
            // Each task has a durable canonical identity, independent of how
            // Cursor batches or retries completion notifications.
            if store
                .message(&key.0, &format!("runtime:{}", key.1))
                .await?
                .is_none()
            {
                admitted.insert(index);
            }
        }
        if admitted.is_empty() {
            return Ok(Admission::Duplicate);
        }
        let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(action)) = request
            .action
            .as_mut()
            .and_then(|action| action.action.as_mut())
        else {
            unreachable!()
        };
        let mut index = 0;
        action.completions.retain(|_| {
            let keep = admitted.contains(&index);
            index += 1;
            keep
        });
        Ok(Admission::New(guard))
    }
}

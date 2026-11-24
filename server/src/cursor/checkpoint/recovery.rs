//! Restores Conversation Messages and pending Tool state from a checkpoint.
use std::collections::HashSet;

use crate::{
    cursor::{checkpoint::messages, protocol::proto::agent::v1 as pb},
    model::{CanonicalMessage, MessageContent, Origin, Role, ToolResultContent},
    run::interruption::CANCELLED_TOOL_MESSAGE,
    store::BlobId,
    Error, Result,
};

use super::CheckpointBuilder;

impl CheckpointBuilder {
    pub async fn import_prefetched(&self, blobs: &[pb::PreFetchedBlob]) -> Result<()> {
        for blob in blobs {
            let expected = BlobId::from_bytes(&blob.id)?;
            let actual = self.store.put_blob(&blob.value, &[]).await?;
            if expected != actual {
                return Err(Error::Protocol(format!(
                    "prefetched Blob hash mismatch: {}",
                    expected.to_base64()
                )));
            }
        }
        Ok(())
    }

    pub async fn hydrate_messages(
        &self,
        state: Option<&pb::ConversationStateStructure>,
    ) -> Result<Vec<CanonicalMessage>> {
        let mut messages = Vec::new();
        let Some(state) = state else {
            return Ok(messages);
        };
        for (ordinal, raw_id) in state.root_prompt_messages_json.iter().enumerate() {
            let id = BlobId::from_bytes(raw_id)?;
            let Some(data) = self.sync.get(&id).await? else {
                return Err(Error::Protocol(format!(
                    "missing message Blob {}",
                    id.to_base64()
                )));
            };
            messages.push(messages::decode(
                &data,
                format!("cursor-root:{}:{ordinal}", id.to_base64()),
            )?);
        }
        Ok(settle_interrupted_tool_calls(messages))
    }
}

/// Cursor can keep an assistant tool call without its result when the user
/// stops a turn while parallel tools are running. Every provider requires a
/// result for each call, so a call that the conversation has already moved
/// past gets the same cancelled result BYOK writes for its own stopped tools.
/// A round at the end of the history is left as is: it may still be running.
fn settle_interrupted_tool_calls(messages: Vec<CanonicalMessage>) -> Vec<CanonicalMessage> {
    let total = messages.len();
    let mut settled = Vec::with_capacity(total);
    let mut messages = messages.into_iter().peekable();
    let mut consumed = 0;
    while let Some(message) = messages.next() {
        consumed += 1;
        let round_id = match &message.content {
            MessageContent::Assistant {
                tool_round_id: Some(round_id),
                tool_calls,
                ..
            } if !tool_calls.is_empty() => round_id.clone(),
            _ => {
                settled.push(message);
                continue;
            }
        };
        let mut calls = Vec::new();
        let mut answered = HashSet::new();
        let mut next = Some(message);
        while let Some(assistant) = next.take() {
            if let MessageContent::Assistant { tool_calls, .. } = &assistant.content {
                calls.extend(tool_calls.iter().cloned());
            }
            settled.push(assistant);
            while let Some(result) = messages.next_if(|candidate| match &candidate.content {
                MessageContent::ToolResult(result) => {
                    calls.iter().any(|call| call.call_id == result.call_id)
                }
                _ => false,
            }) {
                consumed += 1;
                if let MessageContent::ToolResult(content) = &result.content {
                    answered.insert(content.call_id.clone());
                }
                settled.push(result);
            }
            next = messages.next_if(|candidate| match &candidate.content {
                MessageContent::Assistant {
                    tool_round_id: Some(candidate_round),
                    tool_calls,
                    ..
                } => candidate_round == &round_id && !tool_calls.is_empty(),
                _ => false,
            });
            if next.is_some() {
                consumed += 1;
            }
        }
        if consumed == total {
            continue;
        }
        calls.sort_by_key(|call| call.index);
        for call in calls
            .into_iter()
            .filter(|call| !answered.contains(&call.call_id))
        {
            tracing::warn!(
                round_id = %round_id.as_str(),
                call_id = call.call_id,
                tool = call.name,
                "Cursor history has a tool call without a result; marking it cancelled"
            );
            settled.push(CanonicalMessage {
                message_id: format!("{}:cancelled:{}", round_id.as_str(), call.index),
                role: Role::Tool,
                origin: Origin::Tool,
                content: MessageContent::ToolResult(ToolResultContent {
                    call_id: call.call_id,
                    name: call.name,
                    content: CANCELLED_TOOL_MESSAGE.into(),
                    is_error: true,
                    image: None,
                    provider_parts: Vec::new(),
                }),
                runtime_event_id: None,
            });
        }
    }
    settled
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::{project_messages, ToolCallContent, ToolRoundId};

    fn assistant(id: &str, calls: &[&str]) -> CanonicalMessage {
        CanonicalMessage {
            message_id: id.into(),
            role: Role::Assistant,
            origin: Origin::Assistant,
            content: MessageContent::Assistant {
                text: String::new(),
                thinking: String::new(),
                tool_round_id: Some(ToolRoundId::from(format!("{id}:tool-round"))),
                replay_state: None,
                tool_calls: calls
                    .iter()
                    .enumerate()
                    .map(|(index, call_id)| ToolCallContent {
                        index,
                        call_id: (*call_id).into(),
                        name: "Grep".into(),
                        arguments: json!({}),
                    })
                    .collect(),
            },
            runtime_event_id: None,
        }
    }

    fn result(id: &str, call_id: &str) -> CanonicalMessage {
        CanonicalMessage {
            message_id: id.into(),
            role: Role::Tool,
            origin: Origin::Tool,
            content: MessageContent::ToolResult(ToolResultContent {
                call_id: call_id.into(),
                name: "Grep".into(),
                content: "ok".into(),
                is_error: false,
                image: None,
                provider_parts: Vec::new(),
            }),
            runtime_event_id: None,
        }
    }

    fn user(id: &str) -> CanonicalMessage {
        CanonicalMessage::text(id, Role::User, Origin::User, "next question")
    }

    #[test]
    fn cancels_a_parallel_call_that_the_conversation_moved_past() {
        let history = vec![
            assistant("cursor-root:a:1", &["grep", "dynamic"]),
            result("cursor-root:a:2", "grep"),
            user("cursor-root:a:3"),
        ];
        assert!(project_messages(&history).is_err());

        let settled = settle_interrupted_tool_calls(history);
        assert_eq!(settled.len(), 4);
        let MessageContent::ToolResult(cancelled) = &settled[2].content else {
            panic!("expected a cancelled tool result");
        };
        assert_eq!(cancelled.call_id, "dynamic");
        assert!(cancelled.is_error);
        assert_eq!(cancelled.content, CANCELLED_TOOL_MESSAGE);
        assert_eq!(
            settled[2].message_id,
            "cursor-root:a:1:tool-round:cancelled:1"
        );
        assert!(matches!(settled[3].content, MessageContent::Parts { .. }));
        assert!(project_messages(&settled).is_ok());
    }

    #[test]
    fn keeps_complete_rounds_unchanged() {
        let history = vec![
            assistant("cursor-root:a:1", &["grep", "dynamic"]),
            result("cursor-root:a:2", "grep"),
            result("cursor-root:a:3", "dynamic"),
            user("cursor-root:a:4"),
        ];
        assert_eq!(settle_interrupted_tool_calls(history.clone()), history);
    }

    #[test]
    fn leaves_a_trailing_round_for_the_running_turn() {
        let history = vec![
            user("cursor-root:a:1"),
            assistant("cursor-root:a:2", &["grep", "dynamic"]),
            result("cursor-root:a:3", "grep"),
        ];
        assert_eq!(settle_interrupted_tool_calls(history.clone()), history);
    }
}

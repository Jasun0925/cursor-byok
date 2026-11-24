//! Projects canonical Messages into provider-visible model input.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Result BYOK gives the model for a tool that was stopped before it finished.
pub const CANCELLED_TOOL_MESSAGE: &str = "Tool execution was cancelled before completion.";

use super::{
    normalize_tool_name, CanonicalMessage, ContentPart, MessageContent, ProviderReplayState, Role,
    ToolCallContent, ToolResultContent,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProjectedContent {
    Parts(Vec<ContentPart>),
    Assistant {
        text: String,
        thinking: String,
        replay_state: Option<ProviderReplayState>,
        calls: Vec<ToolCallContent>,
    },
    ToolResult(ToolResultContent),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProjectedMessage {
    pub message_id: String,
    pub role: Role,
    pub content: ProjectedContent,
}

/// Projects history for a model request. A tool round that the conversation
/// has moved past is completed for the model: a call without a result gets a
/// cancelled result, and an identical repeated result is sent once.
pub fn project_messages(messages: &[CanonicalMessage]) -> Result<Vec<ProjectedMessage>> {
    project(messages, Projection::Model)
}

/// Projects history one message per stored message, for writing it back to
/// Cursor. Cursor compares this list with its own by position, so nothing is
/// added or removed, even where a tool round is incomplete.
pub fn project_stored_messages(messages: &[CanonicalMessage]) -> Result<Vec<ProjectedMessage>> {
    project(messages, Projection::Stored)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Projection {
    Model,
    Stored,
}

fn project(messages: &[CanonicalMessage], mode: Projection) -> Result<Vec<ProjectedMessage>> {
    let mut projected = Vec::new();
    let mut index = 0;
    while index < messages.len() {
        if let Some((group, next)) = project_tool_round(messages, index, mode)? {
            projected.extend(group);
            index = next;
        } else {
            projected.push(project_message(&messages[index]));
            index += 1;
        }
    }
    Ok(projected)
}

/// Removes provider-visible image parts for models configured as text-only.
/// Canonical messages remain unchanged so the setting can be changed without
/// losing the original conversation input.
pub fn strip_images(messages: &mut [ProjectedMessage]) {
    for message in messages {
        if let ProjectedContent::Parts(parts) = &mut message.content {
            parts.retain(|part| !matches!(part, ContentPart::Image { .. }));
        }
    }
}

fn project_tool_round(
    messages: &[CanonicalMessage],
    start: usize,
    mode: Projection,
) -> Result<Option<(Vec<ProjectedMessage>, usize)>> {
    let MessageContent::Assistant {
        tool_round_id: Some(group_id),
        tool_calls,
        ..
    } = &messages[start].content
    else {
        return Ok(None);
    };
    if tool_calls.is_empty() {
        return Ok(None);
    }

    let mut cursor = start;
    let mut text = String::new();
    let mut thinking = String::new();
    let mut replay_state = None;
    let mut calls = Vec::new();
    let mut results = Vec::new();
    let mut result_ids = HashSet::new();

    while cursor < messages.len() {
        let MessageContent::Assistant {
            text: part_text,
            thinking: part_thinking,
            tool_round_id: Some(candidate_group),
            replay_state: part_replay,
            tool_calls: part_calls,
        } = &messages[cursor].content
        else {
            break;
        };
        if candidate_group != group_id || part_calls.is_empty() {
            break;
        }
        text.push_str(part_text);
        thinking.push_str(part_thinking);
        if replay_state.is_none() {
            replay_state = part_replay.clone();
        } else if part_replay.is_some() {
            return Err(Error::Protocol(
                "tool round repeats provider replay state".into(),
            ));
        }
        calls.extend(part_calls.iter().map(normalized_tool_call));
        cursor += 1;

        while cursor < messages.len() {
            let MessageContent::ToolResult(result) = &messages[cursor].content else {
                break;
            };
            if !calls.iter().any(|call| call.call_id == result.call_id) {
                break;
            }
            if !result_ids.insert(result.call_id.clone()) {
                if mode == Projection::Stored {
                    results.push((messages[cursor].message_id.clone(), result.clone()));
                    cursor += 1;
                    continue;
                }
                // Cursor can save the same tool result twice. Models accept
                // one result per call, so a repeat that matches the first one
                // is left out of the model input; stored history keeps it.
                let first = results
                    .iter()
                    .find(|(_, existing): &&(String, ToolResultContent)| {
                        existing.call_id == result.call_id
                    })
                    .map(|(_, existing)| existing);
                if first != Some(result) {
                    return Err(Error::Protocol(format!(
                        "duplicate tool result call_id: {}",
                        result.call_id
                    )));
                }
                tracing::warn!(
                    call_id = result.call_id,
                    "dropping a repeated identical tool result from model input"
                );
                cursor += 1;
                continue;
            }
            results.push((messages[cursor].message_id.clone(), result.clone()));
            cursor += 1;
        }
    }

    calls.sort_by_key(|call| call.index);
    let round_continues = cursor < messages.len();
    for call in &calls {
        if result_ids.contains(&call.call_id) {
            continue;
        }
        if mode == Projection::Stored {
            continue;
        }
        // A round at the end of the history may still be running, so a
        // missing result there is an error. When the conversation has moved
        // past the round, Cursor lost the result of an interrupted call;
        // the model receives the same cancelled result BYOK writes for its
        // own stopped tools. Stored history is not changed.
        if !round_continues {
            return Err(Error::Protocol(format!(
                "assistant tool call has no result call_id: {}",
                call.call_id
            )));
        }
        tracing::warn!(
            call_id = call.call_id,
            tool = call.name,
            "tool call has no result in history; sending it to the model as cancelled"
        );
        results.push((
            format!("{}:cancelled:{}", messages[start].message_id, call.index),
            ToolResultContent {
                call_id: call.call_id.clone(),
                name: call.name.clone(),
                content: CANCELLED_TOOL_MESSAGE.into(),
                is_error: true,
                image: None,
                provider_parts: Vec::new(),
            },
        ));
    }

    let mut output = Vec::with_capacity(results.len() + 1);
    output.push(ProjectedMessage {
        message_id: messages[start].message_id.clone(),
        role: Role::Assistant,
        content: ProjectedContent::Assistant {
            text,
            thinking,
            replay_state,
            calls,
        },
    });
    output.extend(
        results
            .into_iter()
            .map(|(message_id, result)| ProjectedMessage {
                message_id,
                role: Role::Tool,
                content: ProjectedContent::ToolResult(normalized_tool_result(&result)),
            }),
    );
    Ok(Some((output, cursor)))
}

fn project_message(message: &CanonicalMessage) -> ProjectedMessage {
    let content = match &message.content {
        MessageContent::Parts { parts } => ProjectedContent::Parts(parts.clone()),
        MessageContent::Assistant {
            text,
            thinking,
            replay_state,
            tool_calls,
            ..
        } => ProjectedContent::Assistant {
            text: text.clone(),
            thinking: thinking.clone(),
            replay_state: replay_state.clone(),
            calls: tool_calls.iter().map(normalized_tool_call).collect(),
        },
        MessageContent::ToolResult(result) => {
            ProjectedContent::ToolResult(normalized_tool_result(result))
        }
    };
    ProjectedMessage {
        message_id: message.message_id.clone(),
        role: message.role.clone(),
        content,
    }
}

fn normalized_tool_call(call: &ToolCallContent) -> ToolCallContent {
    let mut call = call.clone();
    call.name = normalize_tool_name(&call.name);
    call
}

fn normalized_tool_result(result: &ToolResultContent) -> ToolResultContent {
    let mut result = result.clone();
    result.name = normalize_tool_name(&result.name);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Origin, ToolRoundId};
    use serde_json::json;

    fn round(calls: &[&str]) -> CanonicalMessage {
        CanonicalMessage {
            message_id: "cursor-root:a:1".into(),
            role: Role::Assistant,
            origin: Origin::Assistant,
            content: MessageContent::Assistant {
                text: String::new(),
                thinking: String::new(),
                tool_round_id: Some(ToolRoundId::from("cursor-root:a:1:tool-round")),
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

    fn result(id: &str, call_id: &str, content: &str) -> CanonicalMessage {
        CanonicalMessage {
            message_id: id.into(),
            role: Role::Tool,
            origin: Origin::Tool,
            content: MessageContent::ToolResult(ToolResultContent {
                call_id: call_id.into(),
                name: "Grep".into(),
                content: content.into(),
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

    fn result_ids(projected: &[ProjectedMessage]) -> Vec<(&str, &str)> {
        projected
            .iter()
            .filter_map(|message| match &message.content {
                ProjectedContent::ToolResult(result) => {
                    Some((message.message_id.as_str(), result.content.as_str()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_call_the_conversation_moved_past_reaches_the_model_as_cancelled() {
        let history = [
            round(&["grep", "dynamic"]),
            result("cursor-root:a:2", "grep", "ok"),
            user("cursor-root:a:3"),
        ];

        let model = project_messages(&history).unwrap();
        assert_eq!(model.len(), 4);
        assert_eq!(
            result_ids(&model),
            vec![
                ("cursor-root:a:2", "ok"),
                ("cursor-root:a:1:cancelled:1", CANCELLED_TOOL_MESSAGE),
            ]
        );
        let ProjectedContent::ToolResult(cancelled) = &model[2].content else {
            panic!("expected the cancelled result");
        };
        assert_eq!(cancelled.call_id, "dynamic");
        assert!(cancelled.is_error);

        let stored = project_stored_messages(&history).unwrap();
        assert_eq!(stored.len(), history.len());
        assert_eq!(result_ids(&stored), vec![("cursor-root:a:2", "ok")]);
    }

    #[test]
    fn a_missing_result_at_the_end_of_history_is_still_an_error() {
        let history = [
            user("cursor-root:a:0"),
            round(&["grep", "dynamic"]),
            result("cursor-root:a:2", "grep", "ok"),
        ];
        assert!(project_messages(&history).is_err());
        assert_eq!(project_stored_messages(&history).unwrap().len(), 3);
    }

    #[test]
    fn an_identical_repeated_result_reaches_the_model_once() {
        let history = [
            round(&["grep"]),
            result("cursor-root:a:2", "grep", "ok"),
            result("cursor-root:a:3", "grep", "ok"),
            user("cursor-root:a:4"),
        ];

        let model = project_messages(&history).unwrap();
        assert_eq!(result_ids(&model), vec![("cursor-root:a:2", "ok")]);
        assert_eq!(model.len(), 3);

        let stored = project_stored_messages(&history).unwrap();
        assert_eq!(stored.len(), history.len());
        assert_eq!(
            result_ids(&stored),
            vec![("cursor-root:a:2", "ok"), ("cursor-root:a:3", "ok")]
        );
    }

    #[test]
    fn a_conflicting_repeated_result_is_rejected() {
        let history = [
            round(&["grep"]),
            result("cursor-root:a:2", "grep", "ok"),
            result("cursor-root:a:3", "grep", "different"),
            user("cursor-root:a:4"),
        ];
        assert!(project_messages(&history).is_err());
        assert_eq!(project_stored_messages(&history).unwrap().len(), 4);
    }

    #[test]
    fn complete_rounds_project_the_same_both_ways() {
        let history = [
            round(&["grep", "dynamic"]),
            result("cursor-root:a:2", "grep", "ok"),
            result("cursor-root:a:3", "dynamic", "ok"),
            user("cursor-root:a:4"),
        ];
        assert_eq!(
            project_messages(&history).unwrap(),
            project_stored_messages(&history).unwrap()
        );
    }

    #[test]
    fn tool_names_are_normalized_before_provider_dispatch() {
        let messages = [CanonicalMessage {
            message_id: "assistant-1".into(),
            role: Role::Assistant,
            origin: Origin::Assistant,
            content: MessageContent::Assistant {
                text: String::new(),
                thinking: String::new(),
                tool_round_id: None,
                replay_state: None,
                tool_calls: vec![ToolCallContent {
                    index: 0,
                    call_id: "call-1".into(),
                    name: "multi_tool_use.parallel".into(),
                    arguments: json!({}),
                }],
            },
            runtime_event_id: None,
        }];

        let projected = project_messages(&messages).unwrap();
        let ProjectedContent::Assistant { calls, .. } = &projected[0].content else {
            panic!("expected assistant projection");
        };
        assert_eq!(calls[0].name, "multi_tool_use_parallel");
    }

    #[test]
    fn strip_images_only_changes_provider_projection() {
        let mut projected = vec![ProjectedMessage {
            message_id: "user-1".into(),
            role: Role::User,
            content: ProjectedContent::Parts(vec![
                ContentPart::Text {
                    text: "look".into(),
                },
                ContentPart::Image {
                    mime_type: "image/png".into(),
                    data: vec![1, 2],
                },
            ]),
        }];

        strip_images(&mut projected);

        assert_eq!(
            projected[0].content,
            ProjectedContent::Parts(vec![ContentPart::Text {
                text: "look".into()
            },])
        );
    }
}

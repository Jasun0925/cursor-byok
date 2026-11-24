//! Defines provider-independent model requests and streaming responses.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ModelSpec, ProjectedContent, ProjectedMessage, ToolDefinition};

// Keep the provider-visible ID within the strictest provider limit. The
// canonical conversation keeps the original ID; this only changes the
// projected request copy passed through NormalizedProvider.
const PROVIDER_TOOL_CALL_ID_MAX_BYTES: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PromptSpec {
    pub instructions: String,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelRequest {
    pub prompt: PromptSpec,
    pub model: ModelSpec,
    pub history: Vec<ProjectedMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelInvocation {
    pub call_id: String,
    pub run_id: String,
    pub conversation_id: String,
    pub prompt_cache_enabled: bool,
    pub provider_call_index: u64,
    pub request: ModelRequest,
}

impl ModelInvocation {
    /// Stable across turns, tool rounds, and retries within one conversation.
    pub(crate) fn cache_key(&self) -> Option<String> {
        self.prompt_cache_enabled
            .then(|| format!("cursor:{}", self.conversation_id))
    }
}

pub(crate) fn normalize_provider_tool_call_ids(history: &mut [ProjectedMessage]) {
    for message in history {
        match &mut message.content {
            ProjectedContent::Assistant { calls, .. } => {
                for call in calls {
                    normalize_tool_call_id(&mut call.call_id);
                }
            }
            ProjectedContent::ToolResult(result) => {
                normalize_tool_call_id(&mut result.call_id);
            }
            ProjectedContent::Parts(_) => {}
        }
    }
}

fn normalize_tool_call_id(call_id: &mut String) {
    if call_id.len() <= PROVIDER_TOOL_CALL_ID_MAX_BYTES {
        return;
    }
    // Hash the complete ID instead of truncating it. Truncation makes two
    // provider IDs with the same prefix refer to the same tool result.
    *call_id = hex::encode(Sha256::digest(call_id.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Role, ToolCallContent, ToolResultContent};
    use serde_json::json;

    fn history_with_tool_id(call_id: &str) -> Vec<ProjectedMessage> {
        vec![
            ProjectedMessage {
                message_id: "assistant-1".into(),
                role: Role::Assistant,
                content: ProjectedContent::Assistant {
                    text: String::new(),
                    thinking: String::new(),
                    replay_state: None,
                    calls: vec![ToolCallContent {
                        index: 0,
                        call_id: call_id.into(),
                        name: "Shell".into(),
                        arguments: json!({}),
                    }],
                },
            },
            ProjectedMessage {
                message_id: "tool-1".into(),
                role: Role::Tool,
                content: ProjectedContent::ToolResult(ToolResultContent {
                    call_id: call_id.into(),
                    name: "Shell".into(),
                    content: "ok".into(),
                    is_error: false,
                    image: None,
                    provider_parts: Vec::new(),
                }),
            },
        ]
    }

    #[test]
    fn long_tool_call_ids_are_stable_and_match_across_call_and_result() {
        let original_id = format!("{}-suffix-a", "a".repeat(PROVIDER_TOOL_CALL_ID_MAX_BYTES));
        let mut history = history_with_tool_id(&original_id);

        normalize_provider_tool_call_ids(&mut history);

        let ProjectedContent::Assistant { calls, .. } = &history[0].content else {
            panic!("expected assistant content");
        };
        let ProjectedContent::ToolResult(result) = &history[1].content else {
            panic!("expected tool result content");
        };
        assert_ne!(calls[0].call_id, original_id);
        assert_eq!(calls[0].call_id, result.call_id);
        assert!(calls[0].call_id.is_ascii());
        assert!(calls[0].call_id.len() <= PROVIDER_TOOL_CALL_ID_MAX_BYTES);
    }

    #[test]
    fn long_tool_call_ids_with_the_same_prefix_do_not_collide() {
        let prefix = "a".repeat(PROVIDER_TOOL_CALL_ID_MAX_BYTES);
        let first = format!("{prefix}-first");
        let second = format!("{prefix}-second");
        let mut history = history_with_tool_id(&first);
        let mut other = history_with_tool_id(&second);

        normalize_provider_tool_call_ids(&mut history);
        normalize_provider_tool_call_ids(&mut other);

        let ProjectedContent::Assistant { calls, .. } = &history[0].content else {
            panic!("expected assistant content");
        };
        let ProjectedContent::Assistant {
            calls: other_calls, ..
        } = &other[0].content
        else {
            panic!("expected assistant content");
        };
        assert_ne!(calls[0].call_id, other_calls[0].call_id);
    }

    #[test]
    fn short_ascii_tool_call_ids_are_preserved() {
        let original_id = "call_123";
        let mut history = history_with_tool_id(original_id);

        normalize_provider_tool_call_ids(&mut history);

        let ProjectedContent::Assistant { calls, .. } = &history[0].content else {
            panic!("expected assistant content");
        };
        assert_eq!(calls[0].call_id, original_id);
    }

    #[test]
    fn non_ascii_ids_are_normalized_by_encoded_length() {
        let original_id = "工".repeat(PROVIDER_TOOL_CALL_ID_MAX_BYTES);
        let mut history = history_with_tool_id(&original_id);

        normalize_provider_tool_call_ids(&mut history);

        let ProjectedContent::Assistant { calls, .. } = &history[0].content else {
            panic!("expected assistant content");
        };
        assert!(calls[0].call_id.is_ascii());
        assert!(calls[0].call_id.len() <= PROVIDER_TOOL_CALL_ID_MAX_BYTES);
    }
}

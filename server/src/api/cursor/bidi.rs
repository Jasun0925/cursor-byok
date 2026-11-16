//! Accepts ordered Cursor Bidi append requests and routes them by request_id.
use std::borrow::Cow;

use prost::Message;

use crate::{
    cursor::{
        conversation::TransportCommand,
        protocol::{
            events,
            proto::{agent::v1 as agent, aiserver::v1 as ai},
        },
        transport::{TransportParent, TransportRegistry},
    },
    Error, Result,
};

pub struct DecodedAppend {
    pub request_id: String,
    pub seqno: i64,
    pub message: agent::AgentClientMessage,
}

impl DecodedAppend {
    pub fn model_id(&self) -> Option<&str> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        request
            .requested_model
            .as_ref()
            .map(|model| model.model_id.as_str())
            .filter(|model| !model.is_empty())
            .or_else(|| {
                request
                    .model_details
                    .as_ref()
                    .map(|model| model.model_id.as_str())
                    .filter(|model| !model.is_empty())
            })
    }

    pub fn conversation_id(&self) -> Option<&str> {
        let agent::agent_client_message::Message::RunRequest(request) =
            self.message.message.as_ref()?
        else {
            return None;
        };
        request.conversation_id.as_deref()
    }

    pub fn is_background_task_completion(&self) -> bool {
        let Some(agent::agent_client_message::Message::RunRequest(request)) =
            self.message.message.as_ref()
        else {
            return false;
        };
        matches!(
            request
                .action
                .as_ref()
                .and_then(|action| action.action.as_ref()),
            Some(agent::conversation_action::Action::BackgroundTaskCompletionAction(_))
        )
    }

    pub fn trace_metadata(&self) -> serde_json::Value {
        let Some(message) = self.message.message.as_ref() else {
            return serde_json::json!({
                "append_seqno": self.seqno,
                "message_type": "empty",
            });
        };
        let agent::agent_client_message::Message::RunRequest(request) = message else {
            return serde_json::json!({
                "append_seqno": self.seqno,
                "message_type": client_message_type(message),
            });
        };
        let (action_type, history_messages, history_images) = request
            .action
            .as_ref()
            .and_then(|action| action.action.as_ref())
            .map(|action| match action {
                agent::conversation_action::Action::UserMessageAction(action) => {
                    let history = action.conversation_history.as_ref();
                    (
                        "user_message",
                        history.map_or(0, |history| history.messages.len()),
                        history.map_or(0, history_image_count),
                    )
                }
                agent::conversation_action::Action::BackgroundTaskCompletionAction(_) => {
                    ("background_task_completion", 0, 0)
                }
                agent::conversation_action::Action::ExecutePlanAction(_) => ("execute_plan", 0, 0),
                agent::conversation_action::Action::SummarizeAction(_) => ("summarize", 0, 0),
                _ => ("other", 0, 0),
            })
            .unwrap_or(("none", 0, 0));
        let state = request.conversation_state.as_ref();
        serde_json::json!({
            "append_seqno": self.seqno,
            "message_type": "run_request",
            "conversation_id": request.conversation_id,
            "model_id": self.model_id(),
            "action_type": action_type,
            "conversation_history_messages": history_messages,
            "conversation_history_images": history_images,
            "root_message_count": state.map_or(0, |state| state.root_prompt_messages_json.len()),
            "turn_count": state.map_or(0, |state| state.turns.len()),
            "prefetched_blob_count": request.pre_fetched_blobs.len(),
        })
    }
}

fn client_message_type(message: &agent::agent_client_message::Message) -> &'static str {
    use agent::agent_client_message::Message;
    match message {
        Message::RunRequest(_) => "run_request",
        Message::ExecClientMessage(_) => "exec_client_message",
        Message::ExecClientControlMessage(_) => "exec_client_control_message",
        Message::KvClientMessage(_) => "kv_client_message",
        Message::ConversationAction(_) => "conversation_action",
        Message::InteractionResponse(_) => "interaction_response",
        Message::ClientHeartbeat(_) => "client_heartbeat",
        Message::PrewarmRequest(_) => "prewarm_request",
    }
}

fn history_image_count(history: &agent::ConversationHistory) -> usize {
    use agent::{
        conversation_history_message::Message,
        conversation_history_tool_result_content::Content as ToolContent,
        conversation_history_user_content::Content as UserContent,
    };
    history
        .messages
        .iter()
        .map(|message| match message.message.as_ref() {
            Some(Message::User(user)) => user
                .content
                .iter()
                .filter(|content| matches!(content.content, Some(UserContent::Image(_))))
                .count(),
            Some(Message::Tool(tool)) => tool
                .content
                .iter()
                .filter(|content| matches!(content.content, Some(ToolContent::Image(_))))
                .count(),
            _ => 0,
        })
        .sum()
}

pub fn decode(request: &ai::BidiAppendRequest) -> Result<DecodedAppend> {
    let request_id = request
        .request_id
        .as_ref()
        .map(|id| id.request_id.as_str())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| Error::Protocol("BidiAppend request_id is required".into()))?;
    // Older Cursor clients send hex text in `data`; newer ones send raw bytes in `data_binary`.
    let payload: Cow<[u8]> = match (request.data.is_empty(), request.data_binary.is_empty()) {
        (false, true) => Cow::Owned(
            hex::decode(&request.data)
                .map_err(|error| Error::Protocol(format!("invalid BidiAppend hex: {error}")))?,
        ),
        (true, false) => Cow::Borrowed(&request.data_binary[..]),
        (true, true) => {
            return Err(Error::Protocol(
                "BidiAppend contains no AgentClientMessage".into(),
            ))
        }
        (false, false) => {
            return Err(Error::Protocol(
                "BidiAppend sets both data and data_binary".into(),
            ))
        }
    };
    let message = agent::AgentClientMessage::decode(&payload[..]).inspect_err(|error| {
        tracing::warn!(
            request_id,
            seqno = request.append_seqno,
            payload_len = payload.len(),
            %error,
            "cannot decode BidiAppend AgentClientMessage"
        );
    })?;
    Ok(DecodedAppend {
        request_id: request_id.into(),
        seqno: request.append_seqno,
        message,
    })
}

pub async fn append(
    registry: &TransportRegistry,
    request: DecodedAppend,
    parent: Option<TransportParent>,
) -> Result<ai::BidiAppendResponse> {
    let replace_closing = request.model_id().is_some();
    let handle = registry
        .get_or_create_for_append(&request.request_id, replace_closing)
        .await?;
    let _admission = handle.admit()?;
    if let Some(conversation_id) = request.conversation_id() {
        handle.set_conversation_id(conversation_id)?;
    }
    if let Some(parent) = parent {
        handle.set_parent(parent)?;
    }
    if matches!(
        request.message.message.as_ref(),
        Some(agent::agent_client_message::Message::ClientHeartbeat(_))
    ) {
        handle.emit(&events::heartbeat())?;
    }
    handle
        .command(TransportCommand::Append {
            seqno: request.seqno,
            message: Box::new(request.message),
        })
        .await?;
    Ok(ai::BidiAppendResponse {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn decode_failure_logs_metadata_without_payload() {
        let mut log = tempfile::tempfile().unwrap();
        let writer = log.try_clone().unwrap();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.try_clone().unwrap())
            .finish();
        // Field 1 must be length-delimited, not a 32-bit value.
        let mut payload = vec![0x0d];
        payload.extend_from_slice(b"private-conversation-content");
        let request = ai::BidiAppendRequest {
            request_id: Some(ai::BidiRequestId {
                request_id: "decode-failure-test".into(),
            }),
            append_seqno: 7,
            data: hex::encode(&payload),
            ..Default::default()
        };

        tracing::subscriber::with_default(subscriber, || {
            assert!(decode(&request).is_err());
        });
        log.seek(SeekFrom::Start(0)).unwrap();
        let mut output = String::new();
        log.read_to_string(&mut output).unwrap();
        assert!(output.contains("cannot decode BidiAppend AgentClientMessage"));
        assert!(output.contains("decode-failure-test"));
        assert!(output.contains("seqno=7"));
        assert!(output.contains(&format!("payload_len={}", payload.len())));
        assert!(!output.contains("private-conversation-content"));
        assert!(!output.contains(&hex::encode(&payload)));
        assert!(!output.contains("payload_prefix"));
    }

    fn run_request_bytes() -> Vec<u8> {
        agent::AgentClientMessage {
            message: Some(agent::agent_client_message::Message::RunRequest(
                agent::AgentRunRequest {
                    conversation_id: Some("binary-conversation".into()),
                    ..Default::default()
                },
            )),
        }
        .encode_to_vec()
    }

    fn append_request(data: String, data_binary: Vec<u8>) -> ai::BidiAppendRequest {
        ai::BidiAppendRequest {
            request_id: Some(ai::BidiRequestId {
                request_id: "data-encoding-test".into(),
            }),
            append_seqno: 3,
            data,
            data_binary,
        }
    }

    #[test]
    fn decodes_hex_data_and_raw_data_binary_alike() {
        let bytes = run_request_bytes();
        for request in [
            append_request(hex::encode(&bytes), Vec::new()),
            append_request(String::new(), bytes.clone()),
        ] {
            let decoded = decode(&request).unwrap();
            assert_eq!(decoded.request_id, "data-encoding-test");
            assert_eq!(decoded.seqno, 3);
            assert_eq!(decoded.conversation_id(), Some("binary-conversation"));
        }
    }

    #[test]
    fn rejects_empty_or_ambiguous_payloads() {
        let bytes = run_request_bytes();
        assert!(decode(&append_request(String::new(), Vec::new())).is_err());
        assert!(decode(&append_request(hex::encode(&bytes), bytes)).is_err());
    }
}

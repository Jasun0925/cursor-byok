//! Controlled HTTP 503 reproduction; a simulated Cursor client drives the real run loop.
//! Copy to server/tests/issue174_probe.rs, run that target, then remove the copy.
#[path = "support/fixtures.rs"]
mod fixtures;

use axum::{body::Bytes, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use cursor_server::{
    config::{ProviderConfig, ProviderKind},
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
        TransportCommand, TransportRegistry,
    },
    model::ConversationId,
    provider::OpenAiChatProvider,
};
use prost::Message;
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[tokio::test]
async fn repeated_http_503_keeps_request_bytes_and_mode_reminders_unchanged() {
    let started = Instant::now();
    let captured = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let capture = captured.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/v1/chat/completions", post(move |body: Bytes| {
        let capture = capture.clone();
        async move {
            let count = {
                let mut requests = capture.lock().unwrap();
                requests.push(body.to_vec());
                requests.len()
            };
            if count <= 2 {
                return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":{"message":"controlled outage"}}))).into_response();
            }
            let sse = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"RECOVERED_174\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
            ([("content-type", "text/event-stream")], sse).into_response()
        }
    }));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let provider = OpenAiChatProvider::new(
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        ProviderConfig {
            kind: ProviderKind::OpenAiChat,
            request_url: format!("http://{address}/v1/chat/completions"),
            api_key: "isolated-test-key".into(),
            custom_headers: Default::default(),
            max_output_tokens: None,
            request_timeout: Duration::from_secs(5),
            allowed_body_fields: None,
        },
    );
    let (_directory, store) = fixtures::temp_store().await;
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry.get_or_create("issue174-request").await.unwrap();
    let mut output = handle.subscribe();
    handle
        .command(TransportCommand::Append {
            seqno: 0,
            message: Box::new(pb::AgentClientMessage {
                message: Some(pb::agent_client_message::Message::RunRequest(
                    pb::AgentRunRequest {
                        conversation_id: Some("issue174-conversation".into()),
                        run_id: Some("issue174-run".into()),
                        requested_model: Some(pb::RequestedModel {
                            model_id: "test-model".into(),
                            ..Default::default()
                        }),
                        action: Some(pb::ConversationAction {
                            action: Some(pb::conversation_action::Action::UserMessageAction(
                                pb::UserMessageAction {
                                    user_message: Some(pb::UserMessage {
                                        text: "Reply with RECOVERED_174.".into(),
                                        message_id: "issue174-user".into(),
                                        mode: pb::AgentMode::Agent as i32,
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                },
                            )),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
            }),
        })
        .await
        .unwrap();
    let mut seqno = 1;
    let mut text = String::new();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(20), output.recv())
            .await
            .unwrap()
            .unwrap();
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            assert_eq!(
                serde_json::from_slice::<Value>(&payload).unwrap(),
                json!({})
            );
            break;
        }
        match pb::AgentServerMessage::decode(payload).unwrap().message {
            Some(pb::agent_server_message::Message::KvServerMessage(kv)) => {
                assert!(matches!(
                    kv.message,
                    Some(pb::kv_server_message::Message::SetBlobArgs(_))
                ));
                handle
                    .command(TransportCommand::Append {
                        seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(pb::agent_client_message::Message::KvClientMessage(
                                pb::KvClientMessage {
                                    id: kv.id,
                                    message: Some(pb::kv_client_message::Message::SetBlobResult(
                                        pb::SetBlobResult { error: None },
                                    )),
                                },
                            )),
                        }),
                    })
                    .await
                    .unwrap();
                seqno += 1;
            }
            Some(pb::agent_server_message::Message::InteractionUpdate(update)) => {
                if let Some(pb::interaction_update::Message::TextDelta(delta)) = update.message {
                    text.push_str(&delta.text);
                }
            }
            _ => {}
        }
    }
    assert_eq!(text, "RECOVERED_174");
    let requests = captured.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert!(
        requests.windows(2).all(|r| r[0] == r[1]),
        "HTTP request body must remain byte-identical during empty 503 retries"
    );
    let body: Value = serde_json::from_slice(&requests[0]).unwrap();
    let messages = body["messages"].to_string();
    assert_eq!(messages.matches("<system_reminder>").count(), 2);
    assert_eq!(messages.matches("You are now in Agent mode.").count(), 1);
    assert_eq!(
        messages.matches("You are still in **Agent Mode**").count(),
        1
    );
    let stored = store
        .load_current_messages(&ConversationId::new("issue174-conversation"))
        .await
        .unwrap();
    let stored_json = serde_json::to_string(&stored).unwrap();
    assert_eq!(stored_json.matches("<system_reminder>").count(), 2);
    let status: String = sqlx::query_scalar("SELECT status FROM runs WHERE cursor_request_id = ?")
        .bind("issue174-request")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(status, "completed");
    println!(
        "ISSUE174_EVIDENCE {}",
        json!({
            "http_statuses":[503,503,200], "requests":requests.len(),
            "request_body_bytes":requests.iter().map(Vec::len).collect::<Vec<_>>(),
            "request_bodies_byte_identical":true, "mode_reminder_blocks_per_request":[2,2,2],
            "stored_mode_reminder_blocks":2, "run_status":status,
            "streamed_text":text, "elapsed_ms":started.elapsed().as_millis(),
        })
    );
    registry.shutdown().await;
    server.abort();
}

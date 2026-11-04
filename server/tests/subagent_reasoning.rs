//! Checks Issue #337 reasoning parameters at the actual provider HTTP boundary.
//! Cursor requests are synthetic; these tests do not validate Cursor's UI labels.
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{sync::Arc, time::Duration};

use axum::{routing::post, Json, Router};
use cursor_server::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
        TransportCommand, TransportRegistry,
    },
    model::{ModelConfigInput, OPENAI_CHAT_ENDPOINT, OPENAI_RESPONSES_ENDPOINT},
    network::NetworkClients,
    plugin::{PluginRegistry, PluginRuntime},
    provider::ProviderRouter,
};
use prost::Message;
use serde_json::{json, Value};

#[tokio::test]
async fn subagent_reasoning_reaches_openai_chat_without_using_display_labels() {
    check_reasoning(OPENAI_CHAT_ENDPOINT).await;
}

#[tokio::test]
async fn subagent_reasoning_reaches_responses_without_using_display_labels() {
    check_reasoning(OPENAI_RESPONSES_ENDPOINT).await;
}

async fn check_reasoning(endpoint: &'static str) {
    let (captured, mut requests) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new().route(
        endpoint,
        post(move |Json(body): Json<Value>| {
            let captured = captured.clone();
            async move {
                captured.send(body).unwrap();
                let response = if endpoint == OPENAI_CHAT_ENDPOINT {
                    "data: {\"choices\":[{\"delta\":{\"content\":\"DONE\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
                } else {
                    "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"delta\":\"DONE\"}\n\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n"
                };
                ([("content-type", "text/event-stream")], response)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let (_directory, store) = fixtures::temp_store().await;
    let plugins = PluginRegistry::managed(
        store.clone(),
        PluginRuntime::managed().unwrap(),
        "test".into(),
    )
    .unwrap();
    let provider = ProviderRouter::new(
        store.clone(),
        plugins,
        NetworkClients::new(store.clone()),
        Duration::from_secs(5),
        Duration::from_secs(5),
    );
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );

    // The child request is authoritative. Display labels must not silently
    // replace its explicit High selection; no client-side inheritance is simulated.
    for (index, (child, requested, configured, expected)) in [
        (false, Some("max"), "low", Some("max")),
        (true, Some("high"), "low", Some("high")),
        (true, Some("max"), "low", Some("max")),
        (true, None, "medium", Some("medium")),
        (true, Some("max"), "none", None),
    ]
    .into_iter()
    .enumerate()
    {
        let input: ModelConfigInput = serde_json::from_value(json!({
            "display_name":format!("Configured label High {index}"),
            "type":"openai", "base_url":format!("http://{address}/v1"),
            "api_key":"test-only", "tooltip_data":"fixture", "model_id":"gpt-test",
            "openai_endpoint":endpoint, "reasoning_effort":configured
        }))
        .unwrap();
        let model = store.create_model(&input).await.unwrap();
        let id = format!("reasoning-{index}");
        let handle = registry.get_or_create(&id).await.unwrap();
        let mut output = handle.subscribe();
        let request = pb::AgentRunRequest {
            conversation_id: Some(id.clone()),
            run_id: Some(id.clone()),
            subagent_type_name: child.then(|| "generalPurpose".into()),
            model_details: Some(pb::ModelDetails {
                model_id: model.model_hash.clone(),
                display_name: "Untrusted display label Max".into(),
                ..Default::default()
            }),
            requested_model: Some(pb::RequestedModel {
                model_id: model.model_hash,
                max_mode: true,
                parameters: requested
                    .into_iter()
                    .map(|effort| pb::requested_model::ModelParameterValue {
                        id: "reasoning".into(),
                        value: effort.into(),
                    })
                    .collect(),
                ..Default::default()
            }),
            action: Some(pb::ConversationAction {
                action: Some(pb::conversation_action::Action::UserMessageAction(
                    pb::UserMessageAction {
                        user_message: Some(pb::UserMessage {
                            message_id: format!("user-{index}"),
                            text: "Reply DONE".into(),
                            mode: pb::AgentMode::Agent as i32,
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        };
        handle
            .command(TransportCommand::Append {
                seqno: 0,
                message: Box::new(pb::AgentClientMessage {
                    message: Some(pb::agent_client_message::Message::RunRequest(request)),
                }),
            })
            .await
            .unwrap();
        let mut seqno = 1;
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(5), output.recv())
                .await
                .unwrap()
                .expect("Run finishes with EndStream");
            let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
            if flags & connect::END_STREAM_FLAG != 0 {
                assert_eq!(payload.as_ref(), b"{}", "provider request must succeed");
                break;
            }
            if let Some(pb::agent_server_message::Message::KvServerMessage(kv)) =
                pb::AgentServerMessage::decode(payload).unwrap().message
            {
                handle
                    .command(TransportCommand::Append {
                        seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(pb::agent_client_message::Message::KvClientMessage(
                                pb::KvClientMessage {
                                    id: kv.id,
                                    message: Some(pb::kv_client_message::Message::SetBlobResult(
                                        pb::SetBlobResult::default(),
                                    )),
                                },
                            )),
                        }),
                    })
                    .await
                    .unwrap();
                seqno += 1;
            }
        }
        let body = requests.try_recv().expect("one actual HTTP request");
        assert_eq!(body["model"], "gpt-test");
        let effort = if endpoint == OPENAI_CHAT_ENDPOINT {
            body.get("reasoning_effort")
        } else {
            body.pointer("/reasoning/effort")
        };
        assert_eq!(effort, expected.map(Value::from).as_ref(), "{body}");
        if expected.is_none() && endpoint == OPENAI_RESPONSES_ENDPOINT {
            assert!(body.get("reasoning").is_none(), "{body}");
        }
        assert!(
            requests.try_recv().is_err(),
            "no duplicate provider request"
        );
    }
    registry.shutdown().await;
    server.abort();
}

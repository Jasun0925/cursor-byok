//! Checks signed thinking through HTTP streaming, Cursor checkpoints and replay.
use std::time::Duration;

use axum::{http::header, routing::post, Json, Router};
use cursor_server::{
    config::{ProviderConfig, ProviderKind},
    cursor::checkpoint::messages::{decode, stable_messages},
    model::{
        project_messages, CanonicalMessage, MessageContent, ModelInvocation, ModelRequest,
        ModelSpec, Origin, PromptSpec, ProviderReplayState, Role, ToolCallContent,
        ToolResultContent,
    },
    provider::{AnthropicProvider, Provider},
    run::{consume_model_cycle, ModelCycleResult},
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

struct Mock {
    provider: AnthropicProvider,
    requests: tokio::sync::mpsc::UnboundedReceiver<Value>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn mock(events: Vec<Value>) -> Mock {
    let sse = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect::<String>();
    let (sender, requests) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new().route(
        "/v1/messages",
        post(move |Json(body): Json<Value>| {
            let sse = sse.clone();
            let sender = sender.clone();
            async move {
                sender.send(body).unwrap();
                ([(header::CONTENT_TYPE, "text/event-stream")], sse)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Mock {
        provider: AnthropicProvider::new(
            reqwest::Client::builder().no_proxy().build().unwrap(),
            ProviderConfig {
                kind: ProviderKind::Anthropic,
                request_url: format!("http://{address}/v1/messages"),
                api_key: "test-only".into(),
                custom_headers: Default::default(),
                max_output_tokens: None,
                request_timeout: Duration::from_secs(5),
                allowed_body_fields: None,
            },
        ),
        requests,
        server,
    }
}

fn thinking(index: usize, text: &str, signatures: &[&str]) -> Vec<Value> {
    let mut events = vec![
        json!({"type":"content_block_start","index":index,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":index,"delta":{"type":"thinking_delta","thinking":text}}),
    ];
    for signature in signatures {
        events.push(json!({"type":"content_block_delta","index":index,"delta":{"type":"signature_delta","signature":signature}}));
    }
    events.push(json!({"type":"content_block_stop","index":index}));
    events
}

fn finish(mut events: Vec<Value>, terminal: bool) -> Vec<Value> {
    events.extend([
        json!({"type":"content_block_start","index":10,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":10,"delta":{"type":"text_delta","text":"answer"}}),
        json!({"type":"content_block_stop","index":10}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
    ]);
    if terminal {
        events.push(json!({"type":"message_stop"}));
    }
    events
}

async fn call(mock: &mut Mock, messages: &[CanonicalMessage]) -> (ModelCycleResult, Value) {
    let mut model = ModelSpec::new("test-claude");
    model.reasoning.enabled = true;
    let invocation = ModelInvocation {
        call_id: "signature-call".into(),
        run_id: "signature-run".into(),
        conversation_id: "signature-conversation".into(),
        prompt_cache_enabled: true,
        provider_call_index: 0,
        request: ModelRequest {
            prompt: PromptSpec {
                instructions: "test".into(),
                tools: vec![],
            },
            model,
            history: project_messages(messages).unwrap(),
        },
    };
    let cancellation = CancellationToken::new();
    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        consume_model_cycle(
            mock.provider.stream(invocation, cancellation.clone()),
            &sender,
            &cancellation,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    (result, mock.requests.try_recv().unwrap())
}

fn assistant(state: Option<ProviderReplayState>, thinking: &str) -> CanonicalMessage {
    CanonicalMessage {
        message_id: "assistant".into(),
        role: Role::Assistant,
        origin: Origin::Assistant,
        runtime_event_id: None,
        content: MessageContent::Assistant {
            text: "answer".into(),
            thinking: thinking.into(),
            tool_round_id: None,
            replay_state: state,
            tool_calls: vec![],
        },
    }
}

fn round_trip(message: CanonicalMessage) -> CanonicalMessage {
    let encoded = stable_messages("", std::slice::from_ref(&message), "test-claude").unwrap();
    let recovered = decode(&encoded[0], message.message_id.clone()).unwrap();
    assert_eq!(recovered, message);
    recovered
}

#[tokio::test]
async fn streamed_signature_survives_checkpoint_and_next_http_request() {
    for terminal in [true, false] {
        let mut mock = mock(finish(
            thinking(0, "reasoning", &["sig+/", "=opaque"]),
            terminal,
        ))
        .await;
        let (cycle, _) = call(&mut mock, &[]).await;
        assert_eq!(cycle.reasoning, "reasoning");
        let recovered = round_trip(assistant(cycle.replay_state, &cycle.reasoning));
        let (_, body) = call(&mut mock, &[recovered]).await;
        assert_eq!(
            body["messages"][0]["content"],
            json!([
                {"type":"thinking","thinking":"reasoning","signature":"sig+/=opaque"},
                {"type":"text","text":"answer"},
            ])
        );
    }
}

#[tokio::test]
async fn unsigned_thinking_is_displayable_but_never_replayed_as_anthropic_thinking() {
    let mut mock = mock(finish(thinking(0, "unsigned", &[]), true)).await;
    let (cycle, _) = call(&mut mock, &[]).await;
    assert_eq!(cycle.reasoning, "unsigned");
    assert!(cycle.replay_state.is_none());
    let recovered = round_trip(assistant(cycle.replay_state, &cycle.reasoning));
    let (_, body) = call(&mut mock, &[recovered]).await;
    assert_eq!(
        body["messages"][0]["content"],
        json!([{"type":"text","text":"answer"}])
    );
}

#[tokio::test]
async fn multiple_signed_and_redacted_blocks_remain_in_order() {
    let redacted = json!({"type":"redacted_thinking","data":"opaque-redacted-data"});
    let mut events = thinking(0, "first", &["signature-1"]);
    events.extend([
        json!({"type":"content_block_start","index":1,"content_block":redacted}),
        json!({"type":"content_block_stop","index":1}),
    ]);
    events.extend(thinking(2, "second", &["signature-2"]));
    let mut mock = mock(finish(events, true)).await;
    let (cycle, _) = call(&mut mock, &[]).await;
    let recovered = round_trip(assistant(cycle.replay_state, &cycle.reasoning));
    let (_, body) = call(&mut mock, &[recovered]).await;
    assert_eq!(
        body["messages"][0]["content"],
        json!([
            {"type":"thinking","thinking":"first","signature":"signature-1"},
            redacted,
            {"type":"thinking","thinking":"second","signature":"signature-2"},
            {"type":"text","text":"answer"},
        ])
    );
}

#[tokio::test]
async fn terminal_closes_a_signed_thinking_block_without_block_stop() {
    for terminal in [true, false] {
        let mut events = thinking(0, "reasoning", &["sig"]);
        events.pop();
        events.push(json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}));
        if terminal {
            events.push(json!({"type":"message_stop"}));
        }
        let mut mock = mock(events).await;
        let (cycle, _) = call(&mut mock, &[]).await;
        assert_eq!(
            cycle.replay_state.unwrap().value,
            json!({"blocks":[
                {"type":"thinking","thinking":"reasoning","signature":"sig"}
            ]})
        );
    }
}

#[tokio::test]
async fn foreign_provider_reasoning_is_not_sent_as_unsigned_anthropic_thinking() {
    let mut mock = mock(finish(vec![], true)).await;
    for kind in ["cursor_opaque", "openai_chat", "openai_responses"] {
        let state = ProviderReplayState {
            provider_kind: kind.into(),
            value: json!({"blocks":[{"type":"thinking","thinking":"foreign"}]}),
        };
        let message = assistant(Some(state), "foreign");
        let (_, body) = call(&mut mock, &[message]).await;
        assert_eq!(
            body["messages"][0]["content"],
            json!([{"type":"text","text":"answer"}])
        );
    }
}

#[tokio::test]
async fn signed_thinking_is_preserved_before_tools_and_across_followup_turns() {
    let mut events = thinking(0, "first", &["sig-1"]);
    events.extend(thinking(1, "second", &["sig-2"]));
    let mut mock = mock(finish(events, true)).await;
    let (cycle, _) = call(&mut mock, &[]).await;
    let mut message = assistant(cycle.replay_state, &cycle.reasoning);
    if let MessageContent::Assistant { tool_calls, .. } = &mut message.content {
        tool_calls.push(ToolCallContent {
            index: 1,
            call_id: "tool-1".into(),
            name: "Read".into(),
            arguments: json!({"path":"test.txt"}),
        });
    }
    let encoded = stable_messages("", &[message], "test-claude").unwrap();
    let recovered = decode(&encoded[0], "assistant".into()).unwrap();
    let mut history = vec![
        recovered,
        CanonicalMessage {
            message_id: "result".into(),
            role: Role::Tool,
            origin: Origin::Tool,
            runtime_event_id: None,
            content: MessageContent::ToolResult(ToolResultContent {
                call_id: "tool-1".into(),
                name: "Read".into(),
                content: "file text".into(),
                is_error: false,
                image: None,
                provider_parts: vec![],
            }),
        },
    ];
    let (_, first) = call(&mut mock, &history).await;
    assert_eq!(
        first["messages"][0]["content"].as_array().unwrap()[..2],
        [
            json!({"type":"thinking","thinking":"first","signature":"sig-1"}),
            json!({"type":"thinking","thinking":"second","signature":"sig-2"}),
        ]
    );
    assert_eq!(first["messages"][0]["content"][3]["type"], "tool_use");
    assert_eq!(first["messages"][1]["content"][0]["tool_use_id"], "tool-1");
    let mut next_assistant = assistant(None, "");
    next_assistant.message_id = "assistant-next".into();
    history.push(next_assistant);
    history.push(CanonicalMessage::text(
        "followup",
        Role::User,
        Origin::User,
        "continue",
    ));
    let (_, next) = call(&mut mock, &history).await;
    assert_eq!(first["messages"][0], next["messages"][0]);
}

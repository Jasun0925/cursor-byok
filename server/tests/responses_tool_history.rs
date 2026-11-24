//! Checks issue #102 across streamed tool calls, persisted history and Responses replay.
use std::time::Duration;

use axum::{routing::post, Json, Router};
use cursor_server::{
    config::{ProviderConfig, ProviderKind},
    cursor::checkpoint::messages::{decode, stable_messages},
    model::{
        project_messages, CanonicalMessage, MessageContent, ModelInvocation, ModelRequest,
        ModelSpec, Origin, PromptSpec, Role, ToolCallContent, ToolResultContent,
    },
    provider::{OpenAiChatProvider, OpenAiResponsesProvider, Provider},
    run::{consume_model_cycle, ModelCycleResult},
    store::Store,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

const CALL_ID: &str = "call_issue102_read";

struct Endpoint {
    url: String,
    requests: tokio::sync::mpsc::UnboundedReceiver<Value>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Endpoint {
    async fn new(events: Vec<Value>) -> Self {
        let sse = events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect::<String>();
        let (sender, requests) = tokio::sync::mpsc::unbounded_channel();
        let app = Router::new().route(
            "/",
            post(move |Json(body): Json<Value>| {
                let sender = sender.clone();
                let sse = sse.clone();
                async move {
                    sender.send(body).unwrap();
                    ([("content-type", "text/event-stream")], sse)
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            url,
            requests,
            server,
        }
    }

    fn provider(&self, kind: ProviderKind) -> Box<dyn Provider> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let config = ProviderConfig {
            kind: kind.clone(),
            request_url: self.url.clone(),
            api_key: "test-only".into(),
            custom_headers: Default::default(),
            max_output_tokens: None,
            request_timeout: Duration::from_secs(5),
            allowed_body_fields: None,
        };
        match kind {
            ProviderKind::OpenAiChat => Box::new(OpenAiChatProvider::new(client, config)),
            ProviderKind::OpenAiResponses => Box::new(OpenAiResponsesProvider::new(client, config)),
            ProviderKind::Anthropic => unreachable!(),
        }
    }
}

async fn cycle(provider: &dyn Provider, messages: &[CanonicalMessage]) -> ModelCycleResult {
    let invocation = ModelInvocation {
        call_id: "model-call".into(),
        run_id: "run".into(),
        conversation_id: "issue102".into(),
        prompt_cache_enabled: true,
        provider_call_index: 0,
        request: ModelRequest {
            prompt: PromptSpec {
                instructions: "test".into(),
                tools: vec![],
            },
            model: ModelSpec::new("gpt-test"),
            history: project_messages(messages).unwrap(),
        },
    };
    let cancel = CancellationToken::new();
    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    tokio::time::timeout(
        Duration::from_secs(5),
        consume_model_cycle(
            provider.stream(invocation, cancel.clone()),
            &sender,
            &cancel,
        ),
    )
    .await
    .unwrap()
    .unwrap()
}

async fn verify_replay(kind: ProviderKind, events: Vec<Value>) {
    let source = Endpoint::new(events).await;
    let result = cycle(source.provider(kind).as_ref(), &[]).await;
    assert_eq!(result.calls.len(), 1);
    let call = &result.calls[0];
    assert_eq!(
        call.call_id, CALL_ID,
        "Responses item id must never replace call_id"
    );
    assert_eq!(call.arguments, json!({"path":"test.txt"}));
    let messages = vec![
        CanonicalMessage::text("user", Role::User, Origin::User, "Read test.txt"),
        CanonicalMessage {
            message_id: "assistant".into(),
            role: Role::Assistant,
            origin: Origin::Assistant,
            runtime_event_id: None,
            content: MessageContent::Assistant {
                text: result.text,
                thinking: result.reasoning,
                tool_round_id: None,
                replay_state: result.replay_state,
                tool_calls: vec![ToolCallContent {
                    index: call.index,
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                }],
            },
        },
        CanonicalMessage {
            message_id: "tool-result".into(),
            role: Role::Tool,
            origin: Origin::Tool,
            runtime_event_id: None,
            content: MessageContent::ToolResult(ToolResultContent {
                call_id: call.call_id.clone(),
                name: call.name.clone(),
                content: "file contents".into(),
                is_error: false,
                image: None,
                provider_parts: vec![],
            }),
        },
    ];

    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", directory.path().join("history.db").display());
    let store = Store::connect(&url).await.unwrap();
    let checkpoint = store
        .import_checkpoint(&"issue102".into(), &messages)
        .await
        .unwrap();
    store.pool().close().await;
    let store = Store::connect(&url).await.unwrap();
    let saved = store.load_checkpoint_messages(checkpoint).await.unwrap();
    assert_eq!(saved, messages);
    store.pool().close().await;

    let encoded = stable_messages("", &saved, "gpt-test").unwrap();
    let mut recovered = encoded
        .iter()
        .zip(&saved)
        .map(|(bytes, original)| decode(bytes, original.message_id.clone()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(recovered.len(), messages.len());
    assert_eq!(
        project_messages(&recovered).unwrap(),
        project_messages(&messages).unwrap()
    );

    let mut destination = Endpoint::new(vec![
        json!({"type":"response.output_text.delta","output_index":0,"delta":"accepted"}),
        json!({"type":"response.completed","response":{}}),
    ])
    .await;
    let provider = destination.provider(ProviderKind::OpenAiResponses);
    assert_eq!(cycle(provider.as_ref(), &recovered).await.text, "accepted");
    let first = destination.requests.try_recv().unwrap();
    let input = first["input"].as_array().unwrap();
    let calls = input
        .iter()
        .filter(|item| item["type"] == "function_call")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["call_id"], CALL_ID);
    assert_eq!(calls[0]["name"], "Read");
    assert_eq!(
        serde_json::from_str::<Value>(calls[0]["arguments"].as_str().unwrap()).unwrap(),
        json!({"path":"test.txt"})
    );
    assert!(
        calls[0].get("id").is_none(),
        "never emit a Chat call id as a Responses item id"
    );
    let outputs = input
        .iter()
        .filter(|item| item["type"] == "function_call_output")
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0]["call_id"], CALL_ID);
    assert_eq!(outputs[0]["output"], "file contents");

    recovered.push(CanonicalMessage::text(
        "next-user",
        Role::User,
        Origin::User,
        "Summarize the work",
    ));
    cycle(provider.as_ref(), &recovered).await;
    let next = destination.requests.try_recv().unwrap();
    assert!(next["input"].as_array().unwrap().starts_with(input));
    assert_eq!(next["input"].as_array().unwrap().len(), input.len() + 1);
}

#[tokio::test]
async fn chat_tool_id_survives_restart_checkpoint_and_switch_to_responses() {
    verify_replay(ProviderKind::OpenAiChat, vec![
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":CALL_ID,"type":"function","function":{"name":"Read","arguments":"{\"path\":\"test.txt\"}"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
    ]).await;
}

#[tokio::test]
async fn responses_item_id_is_distinct_from_call_id_through_history_replay() {
    verify_replay(ProviderKind::OpenAiResponses, vec![
        json!({"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc_issue102_item","call_id":CALL_ID,"name":"Read","arguments":""}}),
        json!({"type":"response.function_call_arguments.delta","output_index":0,"item_id":"fc_issue102_item","delta":"{\"path\":\"test.txt\"}"}),
        json!({"type":"response.output_item.done","output_index":0,"item":{"type":"function_call","id":"fc_issue102_item","call_id":CALL_ID,"name":"Read","arguments":"{\"path\":\"test.txt\"}"}}),
        json!({"type":"response.completed","response":{}}),
    ]).await;
}

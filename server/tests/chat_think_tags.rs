//! Keeps literal think tags separate from protocol reasoning through HTTP/SSE and Run events.
use std::time::Duration;

use axum::{routing::post, Router};
use cursor_server::{
    config::{ProviderConfig, ProviderKind},
    model::{ModelInvocation, ModelRequest, ModelSpec, PromptSpec},
    provider::{FinishReason, OpenAiChatProvider, Provider},
    run::{consume_model_cycle, ModelCycleFailure, ModelCycleResult, RunEvent, RunFailure},
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

async fn cycle(
    deltas: Vec<Value>,
    completed: bool,
) -> (
    Result<ModelCycleResult, Box<ModelCycleFailure>>,
    Vec<RunEvent>,
) {
    let mut sse = deltas
        .into_iter()
        .map(|delta| {
            let chunk = json!({"choices":[{"index":0,"delta":delta,"finish_reason":null}]});
            format!("data: {chunk}\n\n")
        })
        .collect::<String>();
    if completed {
        sse.push_str("data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n");
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move || {
            let sse = sse.clone();
            async move { ([("content-type", "text/event-stream")], sse) }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let provider = OpenAiChatProvider::new(
        reqwest::Client::builder().no_proxy().build().unwrap(),
        ProviderConfig {
            kind: ProviderKind::OpenAiChat,
            request_url: format!("http://{address}/v1/chat/completions"),
            api_key: "test-only".into(),
            custom_headers: Default::default(),
            max_output_tokens: None,
            request_timeout: Duration::from_secs(5),
            allowed_body_fields: None,
        },
    );
    let invocation = ModelInvocation {
        call_id: "think-tag-call".into(),
        run_id: "think-tag-run".into(),
        conversation_id: "think-tag-conversation".into(),
        prompt_cache_enabled: true,
        provider_call_index: 0,
        request: ModelRequest {
            prompt: PromptSpec {
                instructions: String::new(),
                tools: vec![],
            },
            model: ModelSpec::new("test-model"),
            history: vec![],
        },
    };
    let cancel = CancellationToken::new();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(128);
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        consume_model_cycle(
            provider.stream(invocation, cancel.clone()),
            &sender,
            &cancel,
        ),
    )
    .await;
    server.abort();
    drop(sender);
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        events.push(event);
    }
    (result.unwrap(), events)
}

async fn assert_literal(parts: &[&str]) {
    let (result, events) = cycle(parts.iter().map(|s| json!({"content":s})).collect(), true).await;
    let result = result.unwrap();
    assert_eq!(result.text, parts.concat());
    assert_eq!(result.reasoning, "");
    assert_eq!(result.replay_state, None);
    assert_eq!(result.finish_reason, FinishReason::Stop);
    assert_eq!(events.len(), parts.len() + 2);
    assert!(matches!(events.first(), Some(RunEvent::TextStart)));
    assert!(matches!(events.last(), Some(RunEvent::TextEnd)));
    for (event, part) in events[1..events.len() - 1].iter().zip(parts) {
        assert!(
            matches!(event, RunEvent::TextDelta(text) if text == part),
            "client must receive each fragment as text: {event:?}"
        );
    }
}

#[tokio::test]
async fn complete_think_tags_remain_literal_text() {
    assert_literal(&["<think>reasoning</think>", "正文仍然完整。"]).await;
}

#[tokio::test]
async fn tags_split_across_sse_deltas_do_not_swallow_following_text() {
    assert_literal(&["<thi", "nk>", "推理文本", "</th", "ink>", "后续正文。"]).await;
}

#[tokio::test]
async fn unclosed_think_tag_and_quote_do_not_end_the_cycle() {
    assert_literal(&["<think>", "'", "后续正文仍然到达。"]).await;
}

#[tokio::test]
async fn lone_open_tag_is_preserved_when_provider_explicitly_finishes() {
    assert_literal(&["<think>"]).await;
}

#[tokio::test]
async fn structured_reasoning_and_literal_tags_use_separate_channels() {
    let (result, events) = cycle(
        vec![
            json!({"reasoning_content":"structured reasoning"}),
            json!({"content":"<think>literal text</think>"}),
            json!({"content":"正文。"}),
        ],
        true,
    )
    .await;
    let result = result.unwrap();
    assert_eq!(result.reasoning, "structured reasoning");
    assert_eq!(result.text, "<think>literal text</think>正文。");
    assert_eq!(result.finish_reason, FinishReason::Stop);
    assert_eq!(
        result.replay_state.unwrap().value,
        json!({"reasoning_content":"structured reasoning"})
    );
    assert!(matches!(events.as_slice(), [
        RunEvent::ThinkingStart,
        RunEvent::ThinkingDelta(reasoning),
        RunEvent::ThinkingEnd { .. },
        RunEvent::TextStart,
        RunEvent::TextDelta(tag),
        RunEvent::TextDelta(text),
        RunEvent::TextEnd,
    ] if reasoning == "structured reasoning" && tag == "<think>literal text</think>" && text == "正文。"));
}

#[tokio::test]
async fn stream_truncation_is_an_error_and_retains_partial_text() {
    let (result, _) = cycle(
        vec![json!({"content":"<think>"}), json!({"content":"'"})],
        false,
    )
    .await;
    let failure = result.unwrap_err();
    assert_eq!(failure.partial_text, "<think>'");
    assert_eq!(failure.partial_reasoning, "");
    assert!(
        matches!(failure.failure, RunFailure::Provider(message) if message.contains("stream ended without finish_reason"))
    );
}

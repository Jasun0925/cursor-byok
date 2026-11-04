//! Exercises text reconciliation through actual Responses HTTP/SSE and model cycles.
use std::{sync::Arc, time::Duration};

use axum::{http::header, routing::post, Router};
use cursor_server::{
    config::{ProviderConfig, ProviderKind},
    model::{ModelInvocation, ModelRequest, ModelSpec, PromptSpec},
    provider::{FinishReason, OpenAiResponsesProvider, Provider},
    run::{consume_model_cycle, ModelCycleResult},
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

async fn cycle(mut events: Vec<Value>, completed: bool) -> ModelCycleResult {
    if completed {
        events.push(json!({"type":"response.completed","response":{"usage":{"input_tokens":3,"output_tokens":2}}}));
    }
    let sse = events
        .iter()
        .map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap()))
        .collect::<String>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/responses",
        post(move || {
            let sse = sse.clone();
            async move { ([(header::CONTENT_TYPE, "text/event-stream")], sse) }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let provider = Arc::new(OpenAiResponsesProvider::new(
        reqwest::Client::builder().no_proxy().build().unwrap(),
        ProviderConfig {
            kind: ProviderKind::OpenAiResponses,
            request_url: format!("http://{address}/v1/responses"),
            api_key: "test-only".into(),
            custom_headers: Default::default(),
            max_output_tokens: None,
            request_timeout: Duration::from_secs(5),
            allowed_body_fields: None,
        },
    ));
    let invocation = ModelInvocation {
        call_id: "text-call".into(),
        run_id: "text-run".into(),
        conversation_id: "text-conversation".into(),
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
    let (sender, _receiver) = tokio::sync::mpsc::channel(128);
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
    result.unwrap().unwrap()
}

fn delta(index: u64, content: u64, text: &str) -> Value {
    json!({"type":"response.output_text.delta","output_index":index,"content_index":content,"delta":text})
}

fn text_done(index: u64, content: u64, text: &str) -> Value {
    json!({"type":"response.output_text.done","output_index":index,"content_index":content,"text":text})
}

fn item_done(index: u64, texts: &[&str]) -> Value {
    json!({"type":"response.output_item.done","output_index":index,"item":{"type":"message","id":format!("msg_{index}"),"role":"assistant","content":texts.iter().map(|text|json!({"type":"output_text","text":text})).collect::<Vec<_>>()}})
}


fn completed_snapshot(text: &str) -> Value {
    json!({
        "type":"response.completed",
        "response":{
            "status":"completed",
            "output":[{"type":"message","id":"msg_review","role":"assistant","status":"completed","content":[{"type":"output_text","text":text,"annotations":[]}]}],
            "usage":{"input_tokens":3,"output_tokens":2}
        }
    })
}

#[tokio::test]
async fn completed_snapshot_only_keeps_final_review_text() {
    let result = cycle(vec![completed_snapshot("Review complete.")], false).await;
    assert_eq!(result.finish_reason, FinishReason::Stop);
    assert_eq!(result.text, "Review complete.");
}

#[tokio::test]
async fn completed_snapshot_repairs_a_missing_review_suffix() {
    let result = cycle(vec![delta(0, 0, "Review "), completed_snapshot("Review complete.")], false).await;
    assert_eq!(result.text, "Review complete.");
}

#[tokio::test]
async fn normal_item_events_plus_snapshot_do_not_repeat_review_text() {
    let result = cycle(vec![delta(0, 0, "Review "), text_done(0, 0, "Review complete."), item_done(0, &["Review complete."]), completed_snapshot("Review complete.")], false).await;
    assert_eq!(result.text, "Review complete.");
}

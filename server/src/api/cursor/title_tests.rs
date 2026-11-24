//! Wire regressions for local titles and conversation metadata.
use std::sync::{Arc, Mutex};

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    routing::post,
    Router,
};
use prost::Message;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use crate::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::proto::agent::v1 as agent,
        transport::TransportRegistry,
    },
    model::{ModelConfigInput, ModelInvocation},
    network::NetworkClients,
    provider::{FinishReason, ModelEvent, Provider, ProviderStream},
    store::Store,
};

#[derive(Clone, PartialEq, Message)]
struct NameTabRequest {
    #[prost(message, repeated, tag = "1")]
    messages: Vec<TitleMessage>,
    #[prost(string, tag = "6")]
    conversation_id: String,
}
#[derive(Clone, PartialEq, Message)]
struct TitleMessage {
    #[prost(string, tag = "1")]
    text: String,
    #[prost(int32, tag = "2")]
    r#type: i32,
}
#[derive(Clone, PartialEq, Message)]
struct NameTabResponse {
    #[prost(string, tag = "1")]
    name: String,
}

const NAME: &str = "/aiserver.v1.AiService/NameTab";
const META: &str = "/agent.v1.AgentService/UpdateConversationMetadata";

#[derive(Clone, Default)]
struct TitleProvider {
    invocations: Arc<Mutex<Vec<ModelInvocation>>>,
    script: Arc<Mutex<Option<ProviderStream>>>,
    cancellations: Arc<Mutex<Vec<CancellationToken>>>,
}

impl Provider for TitleProvider {
    fn stream(
        &self,
        invocation: ModelInvocation,
        cancellation: CancellationToken,
    ) -> ProviderStream {
        self.invocations.lock().unwrap().push(invocation);
        self.cancellations.lock().unwrap().push(cancellation);
        if let Some(stream) = self.script.lock().unwrap().take() {
            return stream;
        }
        Box::pin(futures_util::stream::iter([
            Ok(ModelEvent::TextDelta(
                "```\n标题：\"分析启动缓慢\"。\n```".into(),
            )),
            Ok(ModelEvent::Done(FinishReason::Stop)),
        ]))
    }
}

async fn router(store: Store, provider: TitleProvider) -> Router {
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let clients = NetworkClients::new(store.clone());
    let registry = TransportRegistry::new(store, Arc::new(provider), PromptCompiler::new(assets));
    super::router(registry, clients).unwrap()
}

async fn post_proto(
    router: Router,
    path: &str,
    message: impl Message,
    upstream: &str,
    local: bool,
) -> axum::response::Response {
    router
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/proto")
                .header(
                    header::AUTHORIZATION,
                    if local {
                        crate::local_app::local_cursor_authorization()
                    } else {
                        "Bearer official-test".into()
                    },
                )
                .header(crate::api::cursor::proxy::UPSTREAM_URL_HEADER, upstream)
                .body(Body::from(message.encode_to_vec()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn model(store: &Store) -> String {
    let input: ModelConfigInput = serde_json::from_value(serde_json::json!({
        "sort_order": 0, "display_name": "Title test", "type": "openai",
        "base_url": "http://127.0.0.1:1", "api_key": "test", "tooltip_data": "Title test", "model_id": "test-model"
    })).unwrap();
    store.create_model(&input).await.unwrap().model_hash
}

#[tokio::test]
async fn local_name_tab_generates_an_isolated_short_title() {
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let model_id = model(&store).await;
    let provider = TitleProvider::default();
    let response = post_proto(
        router(store.clone(), provider.clone()).await,
        NAME,
        NameTabRequest {
            conversation_id: "existing-conversation".into(),
            messages: vec![TitleMessage {
                text: "请帮我分析项目启动速度很慢的原因".into(),
                r#type: 1,
            }],
        },
        "http://127.0.0.1:1",
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    let title = NameTabResponse::decode(body).unwrap();
    assert_eq!(title.name, "分析启动缓慢");
    let requests = provider.invocations.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_ne!(request.conversation_id, "existing-conversation");
    assert!(request.request.prompt.tools.is_empty());
    assert_eq!(request.request.model.model_id, model_id);
    assert!(request.request.model.max_output_tokens.unwrap() <= 128);
    assert!(!request.request.model.reasoning.enabled);
    assert!(request.request.model.reasoning.effort.is_none());
    assert_eq!(request.request.history.len(), 1);
}

#[tokio::test]
async fn metadata_before_a_run_is_saved_without_creating_a_checkpoint() {
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let response = post_proto(
        router(store.clone(), TitleProvider::default()).await,
        META,
        agent::UpdateConversationMetadataRequest {
            conversation_id: "early".into(),
            name: Some("保存标题".into()),
            only_set_name_if_empty: Some(true),
            workspace_paths: vec!["/workspace".into()],
        },
        "http://127.0.0.1:1",
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='conversation_metadata'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        exists, 1,
        "successful metadata replies must persist the request"
    );
    let saved: (String, String) = sqlx::query_as("SELECT name, workspace_paths_json FROM conversation_metadata WHERE conversation_id = 'early'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(saved, ("保存标题".into(), "[\"/workspace\"]".into()));
    assert!(store.conversation(&"early".into()).await.unwrap().is_none());
}

fn title_request(text: &str) -> NameTabRequest {
    NameTabRequest {
        conversation_id: "title-test".into(),
        messages: vec![TitleMessage {
            text: text.into(),
            r#type: 1,
        }],
    }
}

#[tokio::test]
async fn official_requests_use_the_proxy_without_local_generation_or_writes() {
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let provider = TitleProvider::default();
    let router = router(store.clone(), provider.clone()).await;
    // An invalid upstream override makes proxy dispatch observable without
    // contacting an official account or external network.
    for (path, message) in [
        (NAME, title_request("subject").encode_to_vec()),
        (
            META,
            agent::UpdateConversationMetadataRequest {
                conversation_id: "official".into(),
                name: Some("not local".into()),
                ..Default::default()
            }
            .encode_to_vec(),
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::post(path)
                    .header(header::AUTHORIZATION, "Bearer official-test")
                    .header(
                        crate::api::cursor::proxy::UPSTREAM_URL_HEADER,
                        "http://127.0.0.1:1",
                    )
                    .body(Body::from(message))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 8192).await.unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("upstream URL must target a Cursor HTTPS host")
        );
    }
    assert!(provider.invocations.lock().unwrap().is_empty());
    assert!(store
        .conversation_metadata(&"official".into())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn title_source_is_bounded_and_uses_the_commit_model_when_selected() {
    use crate::{
        model::{ContentPart, ProjectedContent},
        store::CommitSettings,
    };
    let store = Store::connect("sqlite::memory:").await.unwrap();
    model(&store).await;
    let input: ModelConfigInput = serde_json::from_value(serde_json::json!({
        "sort_order": 9, "display_name": "Second", "type": "openai", "base_url": "http://127.0.0.1:1", "api_key": "test", "tooltip_data": "Second", "model_id": "second"
    })).unwrap();
    let selected = store.create_model(&input).await.unwrap();
    store
        .set_commit_settings(CommitSettings {
            model_id: selected.model_hash.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    let provider = TitleProvider::default();
    let mut request = title_request(&"中".repeat(10_000));
    request.messages.insert(
        0,
        TitleMessage {
            text: "do not title the assistant reply".into(),
            r#type: 2,
        },
    );
    let response = post_proto(
        router(store, provider.clone()).await,
        NAME,
        request,
        "http://127.0.0.1:1",
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let calls = provider.invocations.lock().unwrap();
    assert_eq!(calls[0].request.model.model_id, selected.model_hash);
    let ProjectedContent::Parts(parts) = &calls[0].request.history[0].content else {
        panic!("user parts")
    };
    let ContentPart::Text { text } = &parts[0] else {
        panic!("text")
    };
    assert_eq!(text.matches('中').count(), 2_000);
    assert!(!text.contains("assistant reply"));
    assert!(!calls[0].prompt_cache_enabled);
}

#[tokio::test]
async fn empty_missing_model_and_failed_titles_do_not_report_success() {
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let provider = TitleProvider::default();
    let app = router(store.clone(), provider.clone()).await;
    for text in ["", "subject"] {
        let response = post_proto(
            app.clone(),
            NAME,
            title_request(text),
            "http://127.0.0.1:1",
            true,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(provider.invocations.lock().unwrap().is_empty());
    model(&store).await;
    for events in [
        vec![Ok(ModelEvent::Done(FinishReason::Stop))],
        vec![Ok(ModelEvent::TextDelta("partial".into()))],
        vec![Ok(ModelEvent::ToolCallStart {
            index: 0,
            call_id: "forbidden".into(),
            name: "Shell".into(),
        })],
        vec![Err(crate::Error::ProviderRefusal("title refused".into()))],
    ] {
        *provider.script.lock().unwrap() = Some(Box::pin(futures_util::stream::iter(events)));
        let response = post_proto(
            app.clone(),
            NAME,
            title_request("subject"),
            "http://127.0.0.1:1",
            true,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(provider
            .cancellations
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .is_cancelled());
    }
    let response = post_proto(
        app,
        META,
        agent::UpdateConversationMetadataRequest::default(),
        "http://127.0.0.1:1",
        true,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn title_deadline_cancels_a_stalled_provider() {
    let store = Store::connect("sqlite::memory:").await.unwrap();
    model(&store).await;
    let provider = TitleProvider::default();
    *provider.script.lock().unwrap() = Some(Box::pin(futures_util::stream::pending()));
    let app = router(store, provider.clone()).await;
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        app.oneshot(
            Request::post(NAME)
                .header(
                    header::AUTHORIZATION,
                    crate::local_app::local_cursor_authorization(),
                )
                .header("connect-timeout-ms", "20")
                .body(Body::from(title_request("subject").encode_to_vec()))
                .unwrap(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(provider.cancellations.lock().unwrap()[0].is_cancelled());
}

#[tokio::test]
async fn anthropic_title_wire_has_no_tools_or_disabled_thinking_parameter() {
    use crate::{plugin::PluginRegistry, provider::ProviderRouter};
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let (send, mut recv) = tokio::sync::mpsc::channel(1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let send = send.clone();
            async move {
                send.send(body).await.unwrap();
                ([("content-type", "text/event-stream")], "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Startup analysis\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n")
            }
        }))).await.unwrap();
    });
    let input: ModelConfigInput = serde_json::from_value(serde_json::json!({
        "display_name": "Anthropic title", "type": "anthropic", "base_url": url,
        "use_full_url": true, "api_key": "test", "tooltip_data": "test", "model_id": "claude-test",
        "anthropic_extra_params_enabled": true,
        "anthropic_extra_params": {"max_tokens": 30000, "thinking": {"type": "adaptive"}}
    }))
    .unwrap();
    store.create_model(&input).await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let plugins = PluginRegistry::for_test(store.clone(), directory.path()).unwrap();
    let provider = Arc::new(ProviderRouter::new(
        store.clone(),
        plugins,
        NetworkClients::new(store.clone()),
        std::time::Duration::from_secs(3),
        std::time::Duration::from_secs(3),
    ));
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let clients = NetworkClients::new(store.clone());
    let registry = TransportRegistry::new(store, provider, PromptCompiler::new(assets));
    let response = post_proto(
        super::router(registry, clients).unwrap(),
        NAME,
        title_request("Analyze startup"),
        "http://127.0.0.1:1",
        true,
    )
    .await;
    task.abort();
    assert_eq!(response.status(), StatusCode::OK);
    let wire = recv.recv().await.unwrap();
    assert!(wire.get("thinking").is_none());
    assert!(wire
        .get("tools")
        .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty)));
    assert_eq!(wire["max_tokens"], 128);
    assert_eq!(wire["model"], "claude-test");
    assert_eq!(wire["messages"].as_array().unwrap().len(), 1);
}

//! Exercises persisted TAB targets against loopback HTTP services only.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body, Bytes},
    extract::Extension,
    http::{header, HeaderMap, Request, Response, StatusCode},
    Router,
};
use cursor_server::{
    api::cursor::proxy::{CursorProxy, UPSTREAM_URL_HEADER},
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        services::tab::{self, TAB_PATHS},
        transport::TransportRegistry,
    },
    network::NetworkClients,
    store::{ProxyMode, ProxySettingsInput, Store, TabMode, TabSettings},
};
use futures_util::StreamExt;
use tokio::sync::{Mutex, Notify};
use tower::ServiceExt;

const REQUEST_BYTES: &[u8] = b"\0\0\0\0\x04test";
const FIRST_FRAME: &[u8] = b"\0\0\0\0\x03one";
const LAST_FRAME: &[u8] = b"\x02\0\0\0\x02{}";

struct Received {
    uri: String,
    headers: HeaderMap,
    body: Bytes,
}

struct Endpoint {
    url: String,
    requests: Arc<Mutex<Vec<Received>>>,
    finish_stream: Arc<Notify>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn endpoint() -> Endpoint {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let finish_stream = Arc::new(Notify::new());
    let captured = requests.clone();
    let finish = finish_stream.clone();
    let app = Router::new().fallback(move |request: Request<Body>| {
        let captured = captured.clone();
        let finish = finish.clone();
        async move {
            let (parts, body) = request.into_parts();
            assert_eq!(parts.method, "POST");
            let streaming = parts.headers.contains_key("x-fixture-stream");
            captured.lock().await.push(Received {
                uri: parts.uri.to_string(),
                headers: parts.headers,
                body: to_bytes(body, 1024).await.unwrap(),
            });
            let body = if streaming {
                Body::from_stream(async_stream::stream! {
                    yield Ok::<_, Infallible>(Bytes::from_static(FIRST_FRAME));
                    finish.notified().await;
                    yield Ok::<_, Infallible>(Bytes::from_static(LAST_FRAME));
                })
            } else {
                Body::from(FIRST_FRAME)
            };
            Response::builder()
                .status(StatusCode::ACCEPTED)
                .header(header::CONTENT_TYPE, "application/connect+proto")
                .header("x-tab-fixture", "received")
                .body(body)
                .unwrap()
        }
    });
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Endpoint {
        url,
        requests,
        finish_stream,
        task,
    }
}

async fn router(store: Store) -> Router {
    // An explicit proxy disables ambient proxy discovery; its loopback bypass
    // keeps every test request local without changing the process environment.
    store
        .set_proxy_settings(ProxySettingsInput {
            mode: ProxyMode::Custom,
            address: "http://127.0.0.1:1".into(),
            auth_enabled: false,
            username: String::new(),
            password: None,
        })
        .await
        .unwrap();
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let clients = NetworkClients::new(store.clone());
    let registry = TransportRegistry::new(
        store,
        Arc::new(fake_provider::FakeProvider::default()),
        PromptCompiler::new(assets),
    );
    tab::router()
        .with_state(registry)
        .layer(Extension(CursorProxy::cursor(clients)))
}

fn request(path: &str, streaming: bool) -> Request<Body> {
    let mut request = Request::post(format!("{path}?cursor=a%2Fb&n=7"))
        .header(header::HOST, "api2.cursor.sh")
        .header(header::AUTHORIZATION, "Bearer synthetic-tab-token")
        .header(header::CONTENT_TYPE, "application/connect+proto")
        .header(UPSTREAM_URL_HEADER, "http://127.0.0.1:1/do-not-use");
    if streaming {
        request = request.header("x-fixture-stream", "true");
    }
    request.body(Body::from(REQUEST_BYTES)).unwrap()
}

async fn call(router: &Router, path: &str, streaming: bool) -> Response<Body> {
    tokio::time::timeout(
        Duration::from_secs(5),
        router.clone().oneshot(request(path, streaming)),
    )
    .await
    .expect("TAB forwarding must finish its response headers")
    .unwrap()
}

async fn custom(store: &Store, address: String) -> TabSettings {
    store
        .set_tab_settings(TabSettings {
            mode: TabMode::Custom,
            address,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn persisted_custom_base_forwards_every_tab_rpc_with_prefix_query_and_binary_body() {
    let (directory, store) = fixtures::temp_store().await;
    let target = endpoint().await;
    let base = format!("{}/custom/tab", target.url);
    let saved = custom(&store, format!("  {base}///  ")).await;
    assert_eq!(saved.address, base);
    drop(store);
    let reopened = Store::connect(&format!(
        "sqlite://{}",
        directory.path().join("test.db").display()
    ))
    .await
    .unwrap();
    assert_eq!(reopened.tab_settings().await.unwrap(), saved);
    let app = router(reopened).await;

    for path in TAB_PATHS {
        let response = call(&app, path, false).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED, "{path}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/connect+proto"
        );
        assert_eq!(response.headers()["x-tab-fixture"], "received");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            FIRST_FRAME
        );
    }
    let received = target.requests.lock().await;
    assert_eq!(received.len(), TAB_PATHS.len());
    for (request, path) in received.iter().zip(TAB_PATHS) {
        assert_eq!(request.uri, format!("/custom/tab{path}?cursor=a%2Fb&n=7"));
        assert_eq!(request.body, REQUEST_BYTES);
        assert_eq!(
            request.headers[header::AUTHORIZATION],
            "Bearer synthetic-tab-token"
        );
        assert_eq!(
            request.headers[header::HOST],
            target.url.strip_prefix("http://").unwrap()
        );
        assert!(!request.headers.contains_key(UPSTREAM_URL_HEADER));
    }
}

#[tokio::test]
async fn changing_custom_target_takes_effect_on_the_existing_router() {
    let (_directory, store) = fixtures::temp_store().await;
    let first = endpoint().await;
    let second = endpoint().await;
    custom(&store, first.url.clone()).await;
    let app = router(store.clone()).await;
    assert_eq!(
        call(&app, TAB_PATHS[0], false).await.status(),
        StatusCode::ACCEPTED
    );
    custom(&store, format!("{}/replacement", second.url)).await;
    assert_eq!(
        call(&app, TAB_PATHS[0], false).await.status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(first.requests.lock().await.len(), 1);
    assert_eq!(second.requests.lock().await.len(), 1);

    store
        .set_tab_settings(TabSettings {
            mode: TabMode::Direct,
            address: second.url.clone(),
        })
        .await
        .unwrap();
    // Direct mode rejects the deliberately invalid official-upstream header,
    // instead of contacting the saved custom target or the public service.
    let direct = call(&app, TAB_PATHS[0], false).await;
    assert_eq!(direct.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(direct.into_body(), 4096).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("Cursor HTTPS host"));
    assert_eq!(second.requests.lock().await.len(), 1);
}

#[tokio::test]
async fn custom_tab_response_stream_is_not_buffered_until_completion() {
    let (_directory, store) = fixtures::temp_store().await;
    let target = endpoint().await;
    custom(&store, target.url.clone()).await;
    let app = router(store).await;
    let response = call(&app, "/aiserver.v1.AiService/StreamCpp", true).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let mut stream = response.into_body().into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(first, FIRST_FRAME);
    target.finish_stream.notify_one();
    let tail = tokio::time::timeout(Duration::from_secs(5), async {
        let mut tail = Vec::new();
        while let Some(chunk) = stream.next().await {
            tail.extend_from_slice(&chunk.unwrap());
        }
        tail
    })
    .await
    .unwrap();
    assert_eq!(tail, LAST_FRAME);
}

#[tokio::test]
async fn invalid_custom_addresses_leave_the_last_valid_setting_unchanged() {
    let (_directory, store) = fixtures::temp_store().await;
    let expected = custom(&store, "https://example.test/tab".into()).await;
    for address in [
        "",
        "not a URL",
        "ftp://example.test",
        "https://",
        "https://example.test/?key=value",
        "https://example.test/#fragment",
    ] {
        assert!(
            store
                .set_tab_settings(TabSettings {
                    mode: TabMode::Custom,
                    address: address.into()
                })
                .await
                .is_err(),
            "{address}"
        );
        assert_eq!(store.tab_settings().await.unwrap(), expected);
    }
}

//! Tests routing changes in child processes without changing the host proxy.
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{
    net::SocketAddr,
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::ConnectInfo,
    http::{HeaderMap, Uri},
    response::IntoResponse,
    Router,
};
use cursor_server::{
    network::NetworkClients,
    store::{ProxyMode, ProxySettingsInput},
};
use serde_json::{json, Value};

const PROXY_ENV: &[&str] = &[
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
    "REQUEST_METHOD",
];
const REFRESH: Duration = Duration::from_millis(2100);

fn isolated(name: &str) -> bool {
    if std::env::var("CURSOR_BYOK_PROXY_TEST").as_deref() == Ok(name) {
        return true;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", name, "--nocapture"])
        .env("CURSOR_BYOK_PROXY_TEST", name);
    for key in PROXY_ENV {
        command.env_remove(key);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

fn set_proxy(address: &str, bypass: &str) {
    // Only called in the isolated child on a current-thread runtime, between
    // completed HTTP operations. No other test shares this process environment.
    std::env::set_var("HTTP_PROXY", address);
    std::env::set_var("HTTPS_PROXY", address);
    std::env::set_var("NO_PROXY", bypass);
}

struct Endpoint {
    url: String,
    requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn endpoint(label: &'static str) -> Endpoint {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let app = Router::new().fallback(move |ConnectInfo(peer): ConnectInfo<SocketAddr>, uri: Uri, headers: HeaderMap| {
        let count = count.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            if uri.path() == "/stream" {
                let body = axum::body::Body::from_stream(async_stream::stream! {
                    yield Ok::<_, std::convert::Infallible>(bytes::Bytes::from_static(label.as_bytes()));
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    yield Ok::<_, std::convert::Infallible>(bytes::Bytes::from_static(b"complete"));
                });
                return body.into_response();
            }
            if uri.path() == "/slow" { tokio::time::sleep(Duration::from_millis(500)).await; }
            if uri.path() == "/redirect" {
                return (axum::http::StatusCode::FOUND, [("location", "/target")]).into_response();
            }
            axum::Json(json!({"route":label,"connection":peer.port(),"auth":headers.get("proxy-authorization").and_then(|h| h.to_str().ok())})).into_response()
        }
    });
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    Endpoint {
        url,
        requests,
        task,
    }
}

async fn clients(network: &NetworkClients) -> Vec<reqwest::Client> {
    vec![
        network.default_client().await.unwrap(),
        network.cursor_client().await.unwrap(),
        network
            .provider_client(Duration::from_secs(3))
            .await
            .unwrap(),
    ]
}

async fn routes(network: &NetworkClients, url: &str, expected: &str) -> Vec<Value> {
    let mut responses = Vec::new();
    for client in clients(network).await {
        let body: Value = client
            .get(url)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            body["route"], expected,
            "cached client used the wrong route"
        );
        responses.push(body);
    }
    responses
}

#[test]
fn system_proxy_changes_refresh_all_client_roles() {
    if !isolated("system_proxy_changes_refresh_all_client_roles") {
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (_directory, store) = fixtures::temp_store().await;
            let a = endpoint("A").await;
            let b = endpoint("B").await;
            let direct = endpoint("direct").await;
            set_proxy(&a.url, "");
            let network = NetworkClients::new(store);
            let initial = routes(&network, &direct.url, "A").await;
            let old_client = network.default_client().await.unwrap();
            tokio::time::sleep(REFRESH).await;
            assert_eq!(
                routes(&network, &direct.url, "A").await,
                initial,
                "unchanged routing must reuse connections"
            );

            let mut stream = old_client
                .get(format!("{}/stream", direct.url))
                .send()
                .await
                .unwrap();
            assert_eq!(stream.chunk().await.unwrap().unwrap(), "A");
            set_proxy(&b.url, "");
            tokio::time::sleep(REFRESH).await;
            routes(&network, &direct.url, "B").await;
            assert_eq!(
                stream.chunk().await.unwrap().unwrap(),
                "complete",
                "cache refresh must not cancel an in-flight response"
            );
            assert!(stream.chunk().await.unwrap().is_none());

            set_proxy(&b.url, "127.0.0.1");
            tokio::time::sleep(REFRESH).await;
            routes(&network, &direct.url, "direct").await;
            set_proxy(&a.url, "");
            tokio::time::sleep(REFRESH).await;
            routes(&network, &direct.url, "A").await;

            let dead = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let dead_url = format!("http://{}", dead.local_addr().unwrap());
            drop(dead);
            set_proxy(&dead_url, "");
            tokio::time::sleep(REFRESH).await;
            let direct_count = direct.requests.load(Ordering::SeqCst);
            for client in clients(&network).await {
                assert!(client
                    .get(&direct.url)
                    .timeout(Duration::from_secs(2))
                    .send()
                    .await
                    .is_err());
            }
            assert_eq!(
                direct.requests.load(Ordering::SeqCst),
                direct_count,
                "an enabled dead proxy must not silently fall back to direct access"
            );

            set_proxy(&b.url, "");
            tokio::time::sleep(REFRESH).await;
            let redirect = format!("{}/redirect", direct.url);
            assert_eq!(
                network
                    .cursor_client()
                    .await
                    .unwrap()
                    .get(&redirect)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                302
            );
            assert_eq!(
                network
                    .default_client()
                    .await
                    .unwrap()
                    .get(&redirect)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                200
            );
            let error = network
                .provider_client(Duration::from_millis(50))
                .await
                .unwrap()
                .get(format!("{}/slow", direct.url))
                .send()
                .await
                .unwrap_err();
            assert!(error.is_timeout());
        });
}

#[test]
fn custom_proxy_refresh_preserves_auth_and_ignores_system_changes() {
    if !isolated("custom_proxy_refresh_preserves_auth_and_ignores_system_changes") {
        return;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (_directory, store) = fixtures::temp_store().await;
            let a = endpoint("A").await;
            let b = endpoint("B").await;
            let custom = |address: &str, password: &str| ProxySettingsInput {
                mode: ProxyMode::Custom,
                address: address.into(),
                auth_enabled: true,
                username: "test-user".into(),
                password: Some(password.into()),
            };
            store
                .set_proxy_settings(custom(&a.url, "first"))
                .await
                .unwrap();
            set_proxy(&a.url, "");
            let network = NetworkClients::new(store.clone());
            let target = "http://proxy-test.invalid/probe";
            let first = routes(&network, target, "A").await;
            assert!(first.iter().all(|response| response["auth"]
                .as_str()
                .is_some_and(|value| value.starts_with("Basic "))));

            set_proxy(&b.url, "*");
            tokio::time::sleep(REFRESH).await;
            assert_eq!(routes(&network, target, "A").await, first);

            store
                .set_proxy_settings(custom(&a.url, "second"))
                .await
                .unwrap();
            tokio::time::sleep(REFRESH).await;
            let updated = routes(&network, target, "A").await;
            assert!(updated
                .iter()
                .zip(&first)
                .all(|(after, before)| after["auth"] != before["auth"]));
            store
                .set_proxy_settings(custom(&b.url, "second"))
                .await
                .unwrap();
            tokio::time::sleep(REFRESH).await;
            routes(&network, target, "B").await;

            // The settings API calls invalidate after saving: this must remain immediate.
            store
                .set_proxy_settings(custom(&a.url, "third"))
                .await
                .unwrap();
            network.invalidate().await;
            let before_error = routes(&network, target, "A").await;
            store.pool().close().await;
            tokio::time::sleep(REFRESH).await;
            assert_eq!(
                routes(&network, target, "A").await,
                before_error,
                "a settings read failure must preserve working cached clients"
            );
        });
}

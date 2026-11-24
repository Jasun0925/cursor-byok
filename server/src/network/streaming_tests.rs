//! Verifies the Cursor relay's TLS negotiation and concurrent request/response flow.
use std::convert::Infallible;

use axum::{
    body::Body,
    extract::Extension,
    http::{Request, Response, Version},
};
use futures_util::StreamExt;
use hudsucker::{
    hyper::service::service_fn,
    hyper_util::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto::Builder,
    },
    rustls::{crypto::aws_lc_rs, pki_types::PrivateKeyDer, ServerConfig},
};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair, KeyUsagePurpose};
use tokio_stream::wrappers::ReceiverStream;

use super::*;

enum ResponseLifetime {
    Complete,
    DropBeforeRead,
    DropAfterRead,
}

async fn relay_exchange(protocol: &[u8], lifetime: ResponseLifetime) {
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::from_ca_cert_pem(&ca.pem(), ca_key).unwrap();
    let leaf_key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec!["api2.cursor.sh".into()])
        .unwrap()
        .signed_by(&leaf_key, &issuer)
        .unwrap();
    let mut tls = ServerConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf.der().clone()],
            PrivateKeyDer::try_from(leaf_key.serialize_der()).unwrap(),
        )
        .unwrap();
    // Supporting both protocols lets the server expose the actual negotiated version.
    tls.alpn_protocols = if protocol == b"h2" {
        vec![b"h2".to_vec(), b"http/1.1".to_vec()]
    } else {
        vec![b"http/1.1".to_vec()]
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (versions, mut observed) = tokio::sync::mpsc::unbounded_channel();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let tls = tokio_rustls::TlsAcceptor::from(Arc::new(tls))
            .accept(tcp)
            .await
            .unwrap();
        let service = service_fn(move |request: Request<hudsucker::hyper::body::Incoming>| {
            versions.send(request.version()).unwrap();
            async move {
                Ok::<_, Infallible>(Response::new(Body::from_stream(
                    Body::new(request.into_body()).into_data_stream(),
                )))
            }
        });
        let _ = Builder::new(TokioExecutor::new())
            .serve_connection(TokioIo::new(tls), service)
            .await;
    });
    let directory = tempfile::tempdir().unwrap();
    let store = Store::connect(&format!(
        "sqlite://{}",
        directory.path().join("network.db").display()
    ))
    .await
    .unwrap();
    let clients = NetworkClients::new(store.clone());
    clients.refresh_proxy_config().await;
    // Use the production builder. Only trust and DNS are isolated to this test endpoint.
    let client = client_builder(&store)
        .await
        .unwrap()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .add_root_certificate(reqwest::Certificate::from_pem(ca.pem().as_bytes()).unwrap())
        .resolve("api2.cursor.sh", address)
        .build()
        .unwrap();
    clients.cache.write().await.cursor = Some(client);
    let proxy = crate::api::cursor::proxy::CursorProxy::cursor(clients);
    let (upload, chunks) =
        tokio::sync::mpsc::channel::<std::result::Result<bytes::Bytes, std::io::Error>>(2);
    upload
        .send(Ok(bytes::Bytes::from_static(b"first")))
        .await
        .unwrap();
    let request = Request::builder()
        .method("POST")
        .uri("/duplex-probe")
        .header(
            crate::api::cursor::proxy::UPSTREAM_URL_HEADER,
            format!("https://api2.cursor.sh:{}/duplex-probe", address.port()),
        )
        .body(Body::from_stream(ReceiverStream::new(chunks)))
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let response = crate::api::cursor::proxy::forward(Extension(proxy), request)
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        if matches!(lifetime, ResponseLifetime::DropBeforeRead) {
            drop(response);
            upload.closed().await;
            return;
        }
        let mut response = response.into_body().into_data_stream();
        assert_eq!(response.next().await.unwrap().unwrap(), "first");
        if matches!(lifetime, ResponseLifetime::DropAfterRead) {
            drop(response);
            upload.closed().await;
            return;
        }
        upload
            .send(Ok(bytes::Bytes::from_static(b"second")))
            .await
            .unwrap();
        assert_eq!(response.next().await.unwrap().unwrap(), "second");
        drop(upload);
        while let Some(chunk) = response.next().await {
            assert!(chunk.unwrap().is_empty());
        }
    })
    .await;
    server.abort();
    assert!(result.is_ok(), "Cursor relay stopped duplex progress");
    let actual_version = observed.try_recv().unwrap();
    let expected_version = if protocol == b"h2" {
        Version::HTTP_2
    } else {
        Version::HTTP_11
    };
    assert_eq!(
        actual_version, expected_version,
        "Cursor relay did not negotiate the upstream protocol"
    );
}

#[tokio::test]
async fn cursor_relay_negotiates_http2_for_bidirectional_streaming() {
    relay_exchange(b"h2", ResponseLifetime::Complete).await;
}

#[tokio::test]
async fn cursor_relay_still_accepts_http1_only_upstreams() {
    relay_exchange(b"http/1.1", ResponseLifetime::Complete).await;
}

#[tokio::test]
async fn cursor_relay_cancels_upload_when_response_is_dropped() {
    relay_exchange(b"h2", ResponseLifetime::DropAfterRead).await;
}

#[tokio::test]
async fn cursor_relay_cancels_upload_when_unpolled_response_is_dropped() {
    relay_exchange(b"h2", ResponseLifetime::DropBeforeRead).await;
}

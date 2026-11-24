//! Exercises a real CONNECT/TLS/H2 client against the production takeover proxy.
use std::time::Duration;

use axum::{body::Body as AxumBody, routing::post, Router};
use http_body_util::BodyExt;
use hudsucker::{
    hyper::{body::Bytes, client::conn::http2, Version},
    hyper_util::rt::{TokioExecutor, TokioIo},
    rustls::{pki_types::ServerName, ClientConfig, RootCertStore},
};
use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair, KeyUsagePurpose};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::wrappers::ReceiverStream;

use super::*;

fn test_ca() -> (LoadedCa, RootCertStore) {
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let cert = params.self_signed(&key).unwrap();
    let mut roots = RootCertStore::empty();
    roots.add(cert.der().clone()).unwrap();
    let ca = LoadedCa {
        issuer: Issuer::from_ca_cert_pem(&cert.pem(), key).unwrap(),
    };
    (ca, roots)
}

async fn connect_h2(port: u16, roots: RootCertStore) -> http2::SendRequest<Body> {
    let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    tcp.write_all(b"CONNECT api2.cursor.sh:443 HTTP/1.1\r\nHost: api2.cursor.sh:443\r\n\r\n")
        .await
        .unwrap();
    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        response.push(tcp.read_u8().await.unwrap());
        assert!(response.len() < 4096);
    }
    assert!(response.starts_with(b"HTTP/1.1 200"));
    let mut config = ClientConfig::builder_with_provider(Arc::new(aws_lc_rs::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let tls = tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(ServerName::try_from("api2.cursor.sh").unwrap(), tcp)
        .await
        .unwrap();
    assert_eq!(tls.get_ref().1.alpn_protocol(), Some(b"h2".as_slice()));
    let (sender, connection) = http2::handshake(TokioExecutor::new(), TokioIo::new(tls))
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    sender
}

#[tokio::test]
async fn takeover_keeps_upload_open_after_the_first_response_chunk() {
    let (versions, mut observed) = tokio::sync::mpsc::unbounded_channel();
    let router = Router::new().route(
        "/duplex-probe",
        post(move |request: axum::http::Request<AxumBody>| {
            let versions = versions.clone();
            async move {
                versions.send(request.version()).unwrap();
                // Reply while the upload is open; every new upload chunk must still arrive.
                AxumBody::from_stream(request.into_body().into_data_stream())
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let backend = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (ca, roots) = test_ca();
    let mut proxy = ProxyRuntime::default();
    let (_, port) = proxy
        .start(address, ca, 0, Arc::new(RwLock::new(TabMode::Direct)))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut client = connect_h2(port, roots).await;
        let (upload, chunks) =
            tokio::sync::mpsc::channel::<std::result::Result<Bytes, std::io::Error>>(2);
        upload.send(Ok(Bytes::from_static(b"first"))).await.unwrap();
        let request = Request::builder()
            .method(Method::POST)
            .version(Version::HTTP_2)
            .uri("https://api2.cursor.sh/duplex-probe")
            .body(Body::from_stream(ReceiverStream::new(chunks)))
            .unwrap();
        let mut response = client.send_request(request).await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response
                .body_mut()
                .frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap(),
            "first"
        );
        upload
            .send(Ok(Bytes::from_static(b"second")))
            .await
            .unwrap();
        assert_eq!(
            response
                .body_mut()
                .frame()
                .await
                .unwrap()
                .unwrap()
                .into_data()
                .unwrap(),
            "second"
        );
        drop(upload);
        while let Some(frame) = response.body_mut().frame().await {
            assert!(frame.unwrap().into_data().unwrap().is_empty());
        }
    })
    .await;
    proxy.stop().await;
    backend.abort();
    let version = observed.try_recv().ok();
    assert!(
        result.is_ok(),
        "duplex stream stalled; backend HTTP version: {version:?}"
    );
}

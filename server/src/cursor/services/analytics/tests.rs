use axum::body::to_bytes;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use super::*;

async fn config_from_response(response: Response<Body>, framed: bool) -> Value {
    assert_eq!(response.status(), StatusCode::OK);
    let length = response.headers()[header::CONTENT_LENGTH]
        .to_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.len(), length);
    let (actual_framed, payload) = unary_payload(&body).unwrap();
    assert_eq!(actual_framed, framed);
    let message = BootstrapStatsigResponse::decode(payload).unwrap();
    serde_json::from_str(&message.config).unwrap()
}

#[tokio::test]
async fn upstream_bootstrap_enables_background_jobs_and_preserves_other_config() {
    for (hash, retries_key, jobs_key) in [
        ("none", "nal_agent_retries", "long_running_jobs"),
        ("djb2", "101914982", "119294905"),
        (
            "sha256",
            "ifvjhhnfrCEApPQP1Pgv02EJdaWKslFkfOhSOAezhxk=",
            "TY6yhlx6Yr6dYcXUYxqRr3v+AsH1iJrVoj44YBdYwSw=",
        ),
    ] {
        for framed in [false, true] {
            let original = json!({
                "hash_used": hash,
                "user": {"userID": "upstream-user", "customIDs": {"device": "test"}},
                "feature_gates": {
                    "unrelated": {"value": false, "rule_id": "keep"},
                    jobs_key: {"value": false}
                },
                "dynamic_configs": {"keep": {"value": 7}},
                "layer_configs": {"keep": {"value": "original"}}
            });
            let message = BootstrapStatsigResponse {
                config: original.to_string(),
                generated_at_ms: 123,
            };
            let response = proxy::BufferedResponse {
                status: StatusCode::OK,
                headers: Default::default(),
                body: encode_unary(&message, framed),
            };

            let patched = config_from_response(patch_upstream(response).unwrap(), framed).await;

            assert_eq!(patched["feature_gates"][jobs_key]["value"], true);
            assert_eq!(patched["feature_gates"][retries_key]["value"], true);
            assert_eq!(patched["feature_gates"].as_object().unwrap().len(), 3);
            for field in ["user", "dynamic_configs", "layer_configs", "hash_used"] {
                assert_eq!(patched[field], original[field]);
            }
            assert_eq!(
                patched["feature_gates"]["unrelated"],
                original["feature_gates"]["unrelated"]
            );
        }
    }
}

fn authorization(claims: Value) -> String {
    format!(
        "Bearer e30.{}.test-signature",
        URL_SAFE_NO_PAD.encode(claims.to_string())
    )
}

async fn unavailable_bootstrap(authorization: Option<&str>) -> Value {
    let directory = tempfile::tempdir().unwrap();
    let store = crate::store::Store::connect(&format!(
        "sqlite://{}",
        directory.path().join("test.db").display()
    ))
    .await
    .unwrap();
    let upstream = proxy::CursorProxy::cursor(crate::network::NetworkClients::new(store));
    // Rejected by URL validation before any network access, exercising local bootstrap.
    let mut request = Request::builder()
        .uri(BOOTSTRAP_STATSIG_PATH)
        .header(proxy::UPSTREAM_URL_HEADER, "http://localhost/unused");
    if let Some(value) = authorization {
        request = request.header(header::AUTHORIZATION, value);
    }
    let response = bootstrap_statsig(Extension(upstream), request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    config_from_response(response, false).await
}

#[tokio::test]
async fn unavailable_bootstrap_matches_each_request_identity() {
    for subject in ["test-user-a", "test-user-b", "cursor-local-user"] {
        let auth = authorization(json!({"sub": subject, "email": "not-copied@example.test"}));
        let config = unavailable_bootstrap(Some(&auth)).await;

        assert_eq!(config["user"], json!({"userID": subject}));
        assert_eq!(config["feature_gates"]["long_running_jobs"]["value"], true);
        assert_eq!(config["feature_gates"]["nal_agent_retries"]["value"], true);
        assert!(config["sdkParams"].get("stableID").is_none());
        assert!(!config.to_string().contains("not-copied@example.test"));
    }
}

#[tokio::test]
async fn missing_or_invalid_identity_does_not_invent_an_account() {
    for auth in [
        None,
        Some("Bearer not-a-jwt".to_owned()),
        Some("Basic e30.e30.test-signature".to_owned()),
        Some(authorization(json!({"sub": ""}))),
        Some(authorization(json!({"sub": 123}))),
        Some(authorization(json!({"email": "unused@example.test"}))),
    ] {
        let config = unavailable_bootstrap(auth.as_deref()).await;
        assert_eq!(config["user"], json!({}));
    }
}

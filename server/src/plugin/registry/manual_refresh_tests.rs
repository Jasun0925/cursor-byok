//! Exercise the actual Grok resource functions through the host refresh entry point.
use super::{
    auth_retry_tests::{fixture, stop_workers, worker_observations},
    *,
};

async fn add_grok_account(registry: &PluginRegistry, mode: &str) -> ResourceRecord {
    registry.inner.state.upsert_resources("dev.refresh", "grok-account", vec![ResourceDraft {
        key: "grok:test".into(),
        private_data: serde_json::json!({
            "accessToken": format!("opaque-{mode}"), "refreshToken": format!("refresh-{mode}"),
            "displayName": "test", "quota": null,
        }),
        state: None,
    }]).await.unwrap();
    registry
        .inner
        .state
        .resources("dev.refresh", "grok-account")
        .await
        .unwrap()
        .remove(0)
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn opaque_token_manual_refresh_recovers_credentials_and_quota_in_one_action() {
    let (_root, registry, entry, executable, _) = fixture().await;
    let record = add_grok_account(&registry, "ok").await;
    let result = registry
        .refresh_resource("dev.refresh", "grok-account", &record.id)
        .await;
    let saved = registry
        .find_record("dev.refresh", "grok-account", &record.id)
        .await
        .unwrap();
    let observations = worker_observations(&registry, &entry, &executable).await;
    stop_workers(&registry).await;
    result.unwrap();
    assert!(saved.state.is_ready(now_ms()), "{:?}", saved.state);
    assert_eq!(saved.private_data["accessToken"], "opaque-ok-rotated");
    assert_eq!(saved.private_data["refreshToken"], "rotated-ok");
    assert_eq!(saved.private_data["quota"]["remainingPercent"], 75.0);
    assert_eq!(
        observations["grokCalls"],
        serde_json::json!(["Bearer opaque-ok", "token", "Bearer opaque-ok-rotated"])
    );
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn quota_retry_failure_keeps_rotation_and_stops_after_one_refresh() {
    for mode in ["reject", "unavailable"] {
        let (_root, registry, entry, executable, _) = fixture().await;
        let record = add_grok_account(&registry, mode).await;
        let result = registry
            .refresh_resource("dev.refresh", "grok-account", &record.id)
            .await;
        let saved = registry
            .find_record("dev.refresh", "grok-account", &record.id)
            .await
            .unwrap();
        let observations = worker_observations(&registry, &entry, &executable).await;
        stop_workers(&registry).await;
        assert!(result.is_err(), "quota failure must be reported");
        assert_eq!(
            saved.private_data["refreshToken"],
            format!("rotated-{mode}")
        );
        assert_eq!(
            observations["grokCalls"],
            serde_json::json!([
                format!("Bearer opaque-{mode}"),
                "token",
                format!("Bearer opaque-{mode}-rotated")
            ])
        );
    }
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn concurrent_manual_refreshes_do_not_rotate_the_same_rejected_token_twice() {
    let (_root, registry, entry, executable, _) = fixture().await;
    let record = add_grok_account(&registry, "ok").await;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let (registry, id) = (registry.clone(), record.id.clone());
        tasks.push(tokio::spawn(async move {
            registry
                .refresh_resource("dev.refresh", "grok-account", &id)
                .await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let observed = worker_observations(&registry, &entry, &executable).await;
    stop_workers(&registry).await;
    assert_eq!(
        observed["grokCalls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| *call == "token")
            .count(),
        1
    );
}

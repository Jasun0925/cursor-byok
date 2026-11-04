use super::*;
use crate::{
    model::{ModelRequest, ModelSpec, NewLlmCall, PromptSpec, ProviderType},
    plugin::{asset::RuntimeAsset, installation},
};
use futures_util::StreamExt;
use tracing::instrument::WithSubscriber;
#[test]
fn authentication_retries_once_and_never_after_output() {
    assert!(should_retry_auth("auth-error", 0, false));
    assert!(!should_retry_auth("auth-error", 1, false));
    assert!(!should_retry_auth("auth-error", 0, true));
    assert!(!should_retry_auth("request-error", 0, false));
}

pub(super) async fn fixture() -> (
    tempfile::TempDir,
    PluginRegistry,
    PluginEntry,
    std::path::PathBuf,
    ResourceRecord,
) {
    let executable = std::path::PathBuf::from(
        std::env::var_os("DENO_TEST_EXECUTABLE").expect("set DENO_TEST_EXECUTABLE"),
    );
    let root = tempfile::tempdir().unwrap();
    // Deno checks module URLs against the read allowlist. macOS temp paths
    // have a /var -> /private/var alias; use the same canonical root for both.
    let path = root.path().canonicalize().unwrap().join("plugins");
    let directory = path.join("installed/test");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("plugin.json"), r#"{"apiVersion":2,"id":"dev.refresh","name":"Refresh test","version":"0.1.0","minAppVersion":"0.1.0","entry":"main.ts","icon":"icon.svg","permissions":{"network":[]}}"#).unwrap();
    std::fs::write(
        directory.join("icon.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .unwrap();
    std::fs::write(
        directory.join("main.ts"),
        include_str!("refresh_fixture.ts").replace(
            "../../../plugins/build-in/grok-auth/resources.ts",
            "./grok_resources.ts",
        ),
    )
    .unwrap();
    std::fs::write(
        directory.join("grok_resources.ts"),
        include_str!("../../../plugins/build-in/grok-auth/resources.ts"),
    )
    .unwrap();
    let deno = std::process::Command::new(&executable)
        .args(["eval", "console.log(Deno.execPath())"])
        .output()
        .unwrap();
    assert!(deno.status.success());
    let deno = String::from_utf8(deno.stdout).unwrap();
    let runtime_root = path.join("runtime");
    let installed =
        installation::runtime_executable(&runtime_root, RuntimeAsset::current().unwrap());
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::copy(deno.trim(), &installed).unwrap();
    std::fs::write(installed.parent().unwrap().join(".ready"), "test runtime").unwrap();
    let runtime = PluginRuntime::new(runtime_root).unwrap();
    let store = Store::connect(&format!(
        "sqlite://{}",
        root.path().join("test.db").display()
    ))
    .await
    .unwrap();
    let state = PluginStateStore::new(PluginDataStore::for_test(path.join("data")).unwrap());
    state
        .upsert_resources(
            "dev.refresh",
            "account",
            vec![ResourceDraft {
                key: "one".into(),
                private_data: serde_json::json!({"token":"expired"}),
                state: None,
            }],
        )
        .await
        .unwrap();
    let record = state
        .resources("dev.refresh", "account")
        .await
        .unwrap()
        .remove(0);
    let catalog = PluginCatalog::for_test(path.clone());
    let entry = catalog
        .entries(&executable)
        .with_subscriber(tracing_subscriber::fmt().with_test_writer().finish())
        .await
        .pop()
        .expect("test plugin loads");
    let registry = PluginRegistry {
        inner: Arc::new(RegistryInner {
            root: path,
            operations: RwLock::new(()),
            cancellation: parking_lot::RwLock::new(CancellationToken::new()),
            reset_lock: Mutex::new(()),
            store,
            state,
            catalog,
            runtime,
            entries: RwLock::new(Some(vec![entry.clone()])),
            workers: Default::default(),
            oauth_sessions: Default::default(),
            preparation_locks: Default::default(),
            pending_preparations: Default::default(),
        }),
    };
    (root, registry, entry, executable, record)
}

pub(super) async fn stop_workers(registry: &PluginRegistry) {
    for worker in registry.inner.workers.lock().await.values() {
        worker.stop().await;
    }
}

async fn model_stream(
    registry: &PluginRegistry,
    scenario: &str,
    cancellation: CancellationToken,
) -> ProviderStream {
    registry
        .inner
        .state
        .replace_models(
            "dev.refresh",
            "fake",
            &[StoredModel::from_definition(
                &serde_json::json!({"id":"model","displayName":"test"}),
            )
            .unwrap()],
        )
        .await
        .unwrap();
    let recorder = CallRecorder::start(
        registry.inner.store.clone(),
        NewLlmCall {
            call_id: "call".into(),
            run_id: "run".into(),
            conversation_id: "conversation".into(),
            provider_call_index: 0,
            model_hash: "test".into(),
            provider_type: ProviderType::OpenAiChat,
            provider_url: "plugin://dev.refresh/fake".into(),
            request_type: ProviderType::OpenAiChat,
            request_url: "plugin://dev.refresh/fake".into(),
            model_id: "model".into(),
            display_name: "test".into(),
            reasoning_effort: None,
            fast: false,
            message_count: 0,
            tool_count: 0,
            detailed: false,
        },
    )
    .await
    .unwrap();
    registry.stream_model(
        ModelInvocation {
            call_id: "call".into(),
            run_id: "run".into(),
            conversation_id: "conversation".into(),
            prompt_cache_enabled: true,
            provider_call_index: 0,
            request: ModelRequest {
                prompt: PromptSpec {
                    instructions: scenario.into(),
                    tools: vec![],
                },
                model: ModelSpec::new("plugin:dev.refresh/fake/model"),
                history: vec![],
            },
        },
        cancellation,
        recorder,
    )
}

pub(super) async fn worker_observations(
    registry: &PluginRegistry,
    entry: &PluginEntry,
    executable: &Path,
) -> serde_json::Value {
    let value = registry
        .worker(entry, executable)
        .await
        .invoke(
            "resource.present",
            serde_json::json!({"resourceType":"account","resources":[{}]}),
            registry.cancellation(),
        )
        .await
        .unwrap();
    serde_json::from_str(value[0]["displayName"].as_str().unwrap()).unwrap()
}

async fn wait_for_exchange(
    registry: &PluginRegistry,
    entry: &PluginEntry,
    executable: &Path,
    count: u64,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if worker_observations(registry, entry, executable).await["exchanges"].as_u64()
                == Some(count)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("worker started the expected token exchange");
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn model_stream_retries_auth_once_preserving_request_and_committing_rotation() {
    let (_root, registry, entry, executable, _) = fixture().await;
    let events = model_stream(&registry, "retry", CancellationToken::new())
        .await
        .collect::<Vec<_>>()
        .await;
    assert!(events.iter().all(Result::is_ok), "{events:?}");
    let texts: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            Ok(ModelEvent::TextDelta(text)) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["fresh-2"]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Ok(ModelEvent::Start { .. })))
            .count(),
        1
    );
    let observed = worker_observations(&registry, &entry, &executable).await;
    assert_eq!(observed["exchanges"], 2);
    let calls = observed["invocations"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["request"], calls[1]["request"]);
    assert_eq!(calls[0]["resource"]["privateData"]["token"], "fresh-1");
    assert_eq!(calls[1]["resource"]["privateData"]["token"], "fresh-2");
    let recorded = registry.inner.store.llm_calls(10).await.unwrap();
    assert_eq!(recorded.len(), 2);
    assert_eq!(
        recorded
            .iter()
            .find(|call| call.call_id == "call")
            .unwrap()
            .status,
        "error"
    );
    assert!(recorded
        .iter()
        .any(|call| call.call_id == "call:auth-retry"));
    let saved = registry
        .inner
        .state
        .resources("dev.refresh", "account")
        .await
        .unwrap();
    assert_eq!(saved[0].private_data["refreshToken"], "rotated-2");
    stop_workers(&registry).await;
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn model_stream_stops_after_second_auth_failure_or_any_output() {
    for (scenario, calls, texts) in [("always-reject", 2, 0), ("after-output", 1, 1)] {
        let (_root, registry, entry, executable, _) = fixture().await;
        let events = model_stream(&registry, scenario, CancellationToken::new())
            .await
            .collect::<Vec<_>>()
            .await;
        assert!(
            matches!(events.last(), Some(Err(Error::Provider(message))) if message == "synthetic 401")
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Ok(ModelEvent::TextDelta(_))))
                .count(),
            texts
        );
        let observed = worker_observations(&registry, &entry, &executable).await;
        assert_eq!(observed["invocations"].as_array().unwrap().len(), calls);
        assert_eq!(observed["exchanges"], calls);
        stop_workers(&registry).await;
    }
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn model_cancellation_returns_promptly_during_initial_or_retry_preparation() {
    for (scenario, delay_key, exchanges) in
        [("complete", "delayMs", 1), ("retry", "rejectedDelayMs", 2)]
    {
        let (_root, registry, entry, executable, _) = fixture().await;
        registry
            .inner
            .state
            .upsert_resources(
                "dev.refresh",
                "account",
                vec![ResourceDraft {
                    key: "one".into(),
                    private_data: serde_json::json!({"token":"expired", delay_key: 2000}),
                    state: None,
                }],
            )
            .await
            .unwrap();
        let cancellation = CancellationToken::new();
        let stream = model_stream(&registry, scenario, cancellation.clone()).await;
        let mut task = tokio::spawn(stream.collect::<Vec<_>>());
        wait_for_exchange(&registry, &entry, &executable, exchanges).await;
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_millis(500), &mut task).await;
        let prompt = result.is_ok();
        let events = match result {
            Ok(result) => result.unwrap(),
            Err(_) => task.await.unwrap(),
        };
        // Cancelling the consumer must not lose a token already rotated upstream.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let records = registry
                    .inner
                    .state
                    .resources("dev.refresh", "account")
                    .await
                    .unwrap();
                if records[0].private_data["refreshToken"] == format!("rotated-{exchanges}") {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let observed = worker_observations(&registry, &entry, &executable).await;
        stop_workers(&registry).await;
        assert!(
            prompt,
            "model cancellation waited for {scenario} preparation"
        );
        assert!(matches!(events.last(), Some(Err(Error::Cancelled))));
        assert_eq!(
            observed["invocations"].as_array().unwrap().len(),
            exchanges as usize - 1
        );
    }
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn reset_cancels_active_preparation_without_recreating_deleted_plugin_state() {
    let (root, registry, entry, executable, _) = fixture().await;
    registry
        .inner
        .state
        .upsert_resources(
            "dev.refresh",
            "account",
            vec![ResourceDraft {
                key: "one".into(),
                private_data: serde_json::json!({"token":"expired", "delayMs": 2000}),
                state: None,
            }],
        )
        .await
        .unwrap();
    let stream = model_stream(&registry, "complete", CancellationToken::new()).await;
    let task = tokio::spawn(stream.collect::<Vec<_>>());
    wait_for_exchange(&registry, &entry, &executable, 1).await;
    tokio::time::timeout(Duration::from_secs(1), registry.reset())
        .await
        .unwrap()
        .unwrap();
    let events = task.await.unwrap();
    assert!(matches!(events.last(), Some(Err(Error::Cancelled))));
    assert!(!registry.inner.root.exists());
    assert!(root.path().join("test.db").exists());
    assert!(registry.inner.pending_preparations.lock().await.is_empty());
    assert!(registry.inner.preparation_locks.lock().await.is_empty());
    assert!(registry.inner.workers.lock().await.is_empty());
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn concurrent_requests_refresh_once_and_observe_committed_tokens() {
    let (_root, registry, entry, executable, record) = fixture().await;
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let (registry, entry, executable, record) = (
            registry.clone(),
            entry.clone(),
            executable.clone(),
            record.clone(),
        );
        tasks.push(tokio::spawn(async move {
            registry
                .prepare_resource(&entry, &executable, "account", record, None)
                .await
                .unwrap()
        }));
    }
    for task in tasks {
        assert_eq!(
            task.await.unwrap().private_data["refreshToken"],
            "rotated-1"
        );
    }
    let stored = registry
        .inner
        .state
        .resources("dev.refresh", "account")
        .await
        .unwrap();
    assert_eq!(stored[0].private_data["refreshToken"], "rotated-1");
    stop_workers(&registry).await;
}

#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn cancelled_model_request_still_commits_rotated_credentials() {
    let (_root, registry, entry, executable, record) = fixture().await;
    let key = format!("dev.refresh/account/{}", record.id);
    let copy = registry.clone();
    let task = tokio::spawn(async move {
        copy.prepare_resource(&entry, &executable, "account", record, None)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if registry
                .inner
                .preparation_locks
                .lock()
                .await
                .contains_key(&key)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let stored = registry
                .inner
                .state
                .resources("dev.refresh", "account")
                .await
                .unwrap();
            if stored[0].private_data["refreshToken"] == "rotated-1" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    stop_workers(&registry).await;
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "requires DENO_TEST_EXECUTABLE"]
async fn failed_disk_write_retries_saved_rotation_without_reusing_old_token() {
    let (root, registry, entry, executable, record) = fixture().await;
    let path = root
        .path()
        .join("plugins/data/dev.refresh/resources-account.json");
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let result = registry
        .prepare_resource(&entry, &executable, "account", record.clone(), None)
        .await;
    std::fs::set_permissions(&path, original).unwrap();
    assert!(result.is_err());
    assert_eq!(registry.inner.pending_preparations.lock().await.len(), 1);
    let prepared = registry
        .prepare_resource(&entry, &executable, "account", record, None)
        .await
        .unwrap();
    assert_eq!(prepared.private_data["refreshToken"], "rotated-1");
    assert!(registry.inner.pending_preparations.lock().await.is_empty());
    stop_workers(&registry).await;
}

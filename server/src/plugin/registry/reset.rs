//! Serializes plugin reset against initialization, requests, and persistence.
use super::*;
use crate::plugin::PluginRuntimeStatus;

impl PluginRegistry {
    pub(super) fn begin_operation(&self) -> Result<tokio::sync::RwLockReadGuard<'_, ()>> {
        let guard = self
            .inner
            .operations
            .try_read()
            .map_err(|_| Error::Cancelled)?;
        if self.cancellation().is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(guard)
    }

    pub(super) fn cancellation(&self) -> CancellationToken {
        self.inner.cancellation.read().clone()
    }

    pub async fn initialize_runtime(&self) -> Result<PluginRuntimeStatus> {
        let _operation = self.begin_operation()?;
        self.inner.catalog.prepare()?;
        Ok(self.inner.runtime.initialize(self.inner.store.clone()))
    }

    /// Keep cleanup running even if the HTTP caller disconnects.
    pub async fn reset(&self) -> Result<PluginRuntimeStatus> {
        let registry = self.clone();
        tokio::spawn(async move { registry.reset_inner().await })
            .await
            .map_err(|error| Error::Config(format!("plugin reset task failed: {error}")))?
    }

    async fn reset_inner(&self) -> Result<PluginRuntimeStatus> {
        let _reset = self.inner.reset_lock.lock().await;
        self.cancellation().cancel();
        self.inner.runtime.cancel_initialization();
        // Requests cancel their network calls and finish any in-flight writes first.
        let _operations = self.inner.operations.write().await;
        let result = async {
            self.inner.oauth_sessions.lock().await.clear();
            self.inner.pending_preparations.lock().await.clear();
            self.inner.preparation_locks.lock().await.clear();
            for (_, worker) in self.inner.workers.lock().await.drain() {
                worker.stop().await;
            }
            *self.inner.entries.write().await = None;
            let status = self.inner.runtime.uninstall().await?;
            // Only plugin-owned files live here; ordinary models and history stay in SQLite.
            match tokio::fs::remove_dir_all(&self.inner.root).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            Ok(status)
        }
        .await;
        *self.inner.cancellation.write() = CancellationToken::new();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{asset::RuntimeAsset, installation, PluginRuntimeState};

    async fn setup() -> (tempfile::TempDir, PluginRegistry) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("plugins");
        let store = Store::connect(&format!(
            "sqlite://{}",
            directory.path().join("test.db").display()
        ))
        .await
        .unwrap();
        let catalog = PluginCatalog::in_directory(&root, "99.0.0".into()).unwrap();
        catalog.prepare().unwrap();
        let data = PluginDataStore::for_test(root.join("data")).unwrap();
        data.update(
            "test-plugin",
            "accounts",
            &serde_json::json!([{"token":"test-only"}]),
        )
        .await
        .unwrap();
        let executable = installation::runtime_executable(
            &root.join("runtime"),
            RuntimeAsset::current().unwrap(),
        );
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, "test runtime").unwrap();
        std::fs::write(executable.parent().unwrap().join(".ready"), "test marker").unwrap();
        let runtime = PluginRuntime::new(root.join("runtime")).unwrap();
        assert_eq!(runtime.status().state, PluginRuntimeState::Ready);
        let registry = PluginRegistry {
            inner: Arc::new(RegistryInner {
                root,
                operations: RwLock::new(()),
                cancellation: parking_lot::RwLock::new(CancellationToken::new()),
                reset_lock: Mutex::new(()),
                store,
                runtime,
                catalog,
                state: PluginStateStore::new(data),
                entries: RwLock::new(Some(Vec::new())),
                workers: Mutex::new(HashMap::new()),
                oauth_sessions: Mutex::new(HashMap::new()),
                preparation_locks: Mutex::new(HashMap::new()),
                pending_preparations: Mutex::new(HashMap::new()),
            }),
        };
        (directory, registry)
    }

    #[tokio::test]
    async fn reset_removes_only_plugins_and_allows_fresh_setup() {
        let (directory, registry) = setup().await;
        let ca = directory.path().join("ca");
        std::fs::create_dir(&ca).unwrap();
        std::fs::write(ca.join("certificate"), "keep").unwrap();
        registry
            .inner
            .store
            .set_detailed_logging(true)
            .await
            .unwrap();
        registry.inner.oauth_sessions.lock().await.insert(
            "pending-login".into(),
            OAuthSession {
                plugin_id: "test-plugin".into(),
                resource_type: "account".into(),
                method_id: "oauth".into(),
                session: serde_json::json!({}),
                expires_at_ms: i64::MAX,
                poll_interval_ms: 1000,
                next_poll_at_ms: 0,
                flow: OAuthFlow::DeviceCode,
            },
        );
        registry
            .preparation_lock("test-plugin", "account", "one")
            .await;
        registry.inner.pending_preparations.lock().await.insert(
            "test-plugin/account/one".into(),
            (
                ResourceRecord {
                    id: "one".into(),
                    key: "one".into(),
                    private_data: serde_json::json!({"token": "expired-test-token"}),
                    state: crate::plugin::state::ResourceState::Ready,
                    created_at_ms: 1,
                    updated_at_ms: 1,
                },
                serde_json::json!({"privateData": {"refreshToken": "rotated-test-token"}}),
            ),
        );
        let old_lifecycle = registry.cancellation();
        assert_eq!(
            registry.reset().await.unwrap().state,
            PluginRuntimeState::Uninitialized
        );
        assert!(old_lifecycle.is_cancelled());
        assert!(!registry.inner.root.exists());
        assert!(registry.inner.entries.read().await.is_none());
        assert!(registry.inner.oauth_sessions.lock().await.is_empty());
        assert!(registry.inner.pending_preparations.lock().await.is_empty());
        assert!(registry.inner.preparation_locks.lock().await.is_empty());
        assert!(directory.path().join("test.db").exists());
        assert_eq!(
            std::fs::read_to_string(ca.join("certificate")).unwrap(),
            "keep"
        );
        assert!(registry.inner.store.detailed_logging().await.unwrap());
        assert!(registry.configured_models().await.is_empty());
        // A repeated reset is harmless, and a restart does not download or restore files.
        registry.reset().await.unwrap();
        let restarted = PluginRuntime::new(registry.inner.root.join("runtime")).unwrap();
        assert_eq!(restarted.status().state, PluginRuntimeState::Uninitialized);
        let catalog = PluginCatalog::in_directory(&registry.inner.root, "99.0.0".into()).unwrap();
        assert!(!registry.inner.root.join("installed/codex-auth").exists());
        // The explicit initialization path restores the bundled code and SDK, without accounts.
        catalog.prepare().unwrap();
        assert!(registry
            .inner
            .root
            .join("installed/codex-auth/plugin.json")
            .exists());
        assert!(registry
            .inner
            .root
            .join("runtime/sdk/v1/worker.ts")
            .exists());
        assert!(!registry.inner.root.join("data").exists());
        assert!(registry.begin_operation().is_ok());
    }

    #[tokio::test]
    async fn reset_cancels_calls_and_waits_for_writes_before_deleting() {
        let (_directory, registry) = setup().await;
        let active = registry.begin_operation().unwrap();
        let cancellation = registry.cancellation();
        let resetting = registry.clone();
        let reset = tokio::spawn(async move { resetting.reset().await });
        tokio::time::timeout(Duration::from_secs(2), cancellation.cancelled())
            .await
            .unwrap();
        assert!(registry.begin_operation().is_err());
        assert!(!reset.is_finished());
        // Simulate an already-started persistence operation finishing during cancellation.
        std::fs::write(registry.inner.root.join("data/late-write"), "old result").unwrap();
        drop(active);
        tokio::time::timeout(Duration::from_secs(2), reset)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!registry.inner.root.exists());
        assert!(registry.begin_operation().is_ok());
    }

    #[tokio::test]
    async fn concurrent_resets_complete_without_restoring_data() {
        let (_directory, registry) = setup().await;
        let (first, second) = tokio::join!(registry.reset(), registry.reset());
        assert_eq!(first.unwrap().state, PluginRuntimeState::Uninitialized);
        assert_eq!(second.unwrap().state, PluginRuntimeState::Uninitialized);
        assert!(!registry.inner.root.exists());
    }
}

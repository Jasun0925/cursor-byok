//! Exercises cleanup against a real WAL database, including its physical files.

use std::{path::Path, time::Duration};

use sqlx::{Connection, SqliteConnection};

use super::*;
use crate::{
    model::{
        CanonicalMessage, ConversationId, NewLlmCall, Origin, ProviderType, Role,
        OPENAI_CHAT_ENDPOINT,
    },
    store::{BlobEdge, BlobId},
};

const LARGE_BYTES: usize = 4 * 1024 * 1024;

async fn fixture(path: &Path) -> Store {
    let store = Store::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    store.set_detailed_logging(true).await.unwrap();
    store
        .create_model(
            &serde_json::from_value(serde_json::json!({
                "display_name": "Preserved model", "type": "openai",
                "base_url": "https://example.invalid/v1", "api_key": "synthetic-test-key",
                "tooltip_data": "test", "model_id": "synthetic-model",
            "openai_endpoint": OPENAI_CHAT_ENDPOINT
            }))
            .unwrap(),
        )
        .await
        .unwrap();
    store
        .start_llm_call(&NewLlmCall {
            call_id: "call".into(),
            run_id: "run".into(),
            conversation_id: "conversation".into(),
            provider_call_index: 0,
            model_hash: "synthetic-model".into(),
            provider_type: ProviderType::OpenAiChat,
            provider_url: "https://example.invalid".into(),
            request_type: ProviderType::OpenAiChat,
            request_url: "https://example.invalid/chat/completions".into(),
            model_id: "synthetic-model".into(),
            display_name: "Synthetic model".into(),
            reasoning_effort: None,
            fast: false,
            message_count: 1,
            tool_count: 0,
            detailed: true,
        })
        .await
        .unwrap();
    store
        .record_llm_request(
            "call",
            &serde_json::json!({}),
            &serde_json::json!({"prompt": "q".repeat(LARGE_BYTES)}),
            true,
        )
        .await
        .unwrap();
    store
        .record_llm_chunk("call", 0, 10, &vec![b'r'; LARGE_BYTES], true)
        .await
        .unwrap();
    store
        .finish_llm_call("call", "completed", Some("stop"), 20, None, None)
        .await
        .unwrap();
    store
        .start_cursor_trace_if_detailed("trace", Some("conversation"), "local_byok", None)
        .await
        .unwrap();
    store
        .append_cursor_trace_artifact(
            "trace",
            "run_sse_chunk",
            "byok_server",
            &vec![b't'; LARGE_BYTES],
            &serde_json::json!({}),
        )
        .await
        .unwrap();
    store.finish_cursor_trace("trace", None).await.unwrap();
    // Put all fixture pages in the main file before measuring. The cleanup itself
    // must shrink both files while the application's connection pool stays open.
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(store.pool())
        .await
        .unwrap();
    store
}

fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

async fn assert_reclaimed(store: &Store, path: &Path, before: u64) {
    let main = file_bytes(path);
    let wal = file_bytes(&path.with_extension("db-wal"));
    let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(store.pool())
        .await
        .unwrap();
    println!("storage bytes: before={before}, main={main}, wal={wal}, free_pages={free}");
    assert!(main < before / 4, "main database did not shrink");
    assert_eq!(wal, 0, "cleanup left the released data in the WAL file");
    assert_eq!(free, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(store.pool())
            .await
            .unwrap(),
        "ok"
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(store.pool())
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn clear_details_reclaims_files_and_preserves_history_and_shared_blobs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("storage.db");
    let store = fixture(&path).await;
    let conversation = ConversationId::new("conversation");
    let history = vec![CanonicalMessage::text(
        "user-1",
        Role::User,
        Origin::User,
        "Keep this conversation.",
    )];
    let checkpoint = store
        .import_checkpoint(&conversation, &history)
        .await
        .unwrap();
    store
        .save_cursor_checkpoint(checkpoint, b"cursor presentation")
        .await
        .unwrap();
    let shared = store.put_blob(b"shared child", &[]).await.unwrap();
    let parent = store
        .put_blob(
            b"parent",
            &[BlobEdge {
                child: shared.clone(),
                field_name: "child".into(),
            }],
        )
        .await
        .unwrap();
    store
        .link_cursor_trace_artifact(
            "trace",
            "shared",
            "byok_server",
            &shared,
            &serde_json::json!({}),
        )
        .await
        .unwrap();
    let call_before = serde_json::to_value(store.llm_call("call").await.unwrap()).unwrap();
    let trace_before = serde_json::to_value(store.cursor_trace("trace").await.unwrap()).unwrap();
    let models_before = serde_json::to_value(store.models().await.unwrap()).unwrap();
    let before = file_bytes(&path);
    assert!(before >= (3 * LARGE_BYTES) as u64);

    let result = store.clear_statistics_storage().await.unwrap();
    assert_eq!((result.call_count, result.trace_count), (1, 1));
    assert!(store.llm_call_request("call").await.unwrap().is_none());
    assert!(store.llm_call_chunks("call").await.unwrap().is_empty());
    assert!(store
        .cursor_trace_artifacts("trace")
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        serde_json::to_value(store.llm_call("call").await.unwrap()).unwrap(),
        call_before
    );
    assert_eq!(
        serde_json::to_value(store.cursor_trace("trace").await.unwrap()).unwrap(),
        trace_before
    );
    assert_eq!(
        store.load_checkpoint_messages(checkpoint).await.unwrap(),
        history
    );
    assert_eq!(
        store.cursor_checkpoint(checkpoint).await.unwrap().unwrap(),
        b"cursor presentation"
    );
    assert_eq!(
        store.get_blob(&shared).await.unwrap().unwrap(),
        b"shared child"
    );
    assert_eq!(store.blob_closure(&[parent]).await.unwrap().len(), 2);
    assert!(store
        .get_blob(&BlobId::digest(&vec![b't'; LARGE_BYTES]))
        .await
        .unwrap()
        .is_none());
    assert!(store.detailed_logging().await.unwrap());
    assert_eq!(
        serde_json::to_value(store.models().await.unwrap()).unwrap(),
        models_before
    );
    assert_reclaimed(&store, &path, before).await;

    store.pool.close().await;
    let reopened = Store::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    assert_eq!(
        reopened.load_checkpoint_messages(checkpoint).await.unwrap(),
        history
    );
    assert_eq!(
        reopened
            .cursor_checkpoint(checkpoint)
            .await
            .unwrap()
            .unwrap(),
        b"cursor presentation"
    );
    assert!(reopened.detailed_logging().await.unwrap());
    assert_eq!(
        serde_json::to_value(reopened.models().await.unwrap()).unwrap(),
        models_before
    );
    let mut continued = history.clone();
    continued.push(CanonicalMessage::text(
        "user-2",
        Role::User,
        Origin::User,
        "Continue after cleanup.",
    ));
    let next = reopened
        .import_checkpoint(&conversation, &continued)
        .await
        .unwrap();
    assert_eq!(
        reopened.load_checkpoint_messages(checkpoint).await.unwrap(),
        history
    );
    assert_eq!(
        reopened.load_checkpoint_messages(next).await.unwrap(),
        continued
    );
    reopened.pool.close().await;
}

#[tokio::test]
async fn clear_all_statistics_reclaims_files_and_can_be_repeated() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("storage.db");
    let store = fixture(&path).await;
    let before = file_bytes(&path);
    let conversation = ConversationId::new("preserved");
    let models_before = serde_json::to_value(store.models().await.unwrap()).unwrap();
    let history = vec![CanonicalMessage::text(
        "user-1",
        Role::User,
        Origin::User,
        "Keep me.",
    )];
    let checkpoint = store
        .import_checkpoint(&conversation, &history)
        .await
        .unwrap();

    for _ in 0..2 {
        let result = store.clear_all_statistics_storage().await.unwrap();
        assert_eq!((result.call_count, result.trace_count), (0, 0));
        assert_eq!(
            store.load_checkpoint_messages(checkpoint).await.unwrap(),
            history
        );
        assert!(store.detailed_logging().await.unwrap());
        assert_eq!(
            serde_json::to_value(store.models().await.unwrap()).unwrap(),
            models_before
        );
        assert_reclaimed(&store, &path, before).await;
    }
    store.pool.close().await;
}

async fn pinned_reader(path: &Path) -> SqliteConnection {
    let mut reader = SqliteConnection::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("BEGIN").execute(&mut reader).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_calls")
        .fetch_one(&mut reader)
        .await
        .unwrap();
    assert_eq!(count, 1);
    reader
}

async fn wait_for_deletion(store: &Store) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if store.statistics_storage().await.unwrap().call_count == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cleanup must commit its deletion before waiting for the reader");
}

#[tokio::test]
async fn active_reader_survives_and_queued_conversation_write_resumes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("storage.db");
    let store = fixture(&path).await;
    let before = file_bytes(&path);
    let mut reader = pinned_reader(&path).await;
    let cleanup = tokio::spawn({
        let store = store.clone();
        async move { store.clear_all_statistics_storage().await }
    });
    wait_for_deletion(&store).await;
    let mut writer = tokio::spawn({
        let store = store.clone();
        async move {
            store
                .ensure_conversation(&ConversationId::new("queued-conversation"))
                .await
        }
    });
    assert!(tokio::time::timeout(Duration::from_millis(50), &mut writer)
        .await
        .is_err());
    assert!(!cleanup.is_finished());
    // The existing reader retains its original snapshot. Maintenance must not
    // close its connection or interrupt its statement to force a checkpoint.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM llm_calls")
            .fetch_one(&mut reader)
            .await
            .unwrap(),
        1
    );
    sqlx::query("COMMIT").execute(&mut reader).await.unwrap();
    cleanup.await.unwrap().unwrap();
    let checkpoint = tokio::time::timeout(Duration::from_secs(3), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(store
        .load_checkpoint_messages(checkpoint)
        .await
        .unwrap()
        .is_empty());
    assert!(file_bytes(&path) < before / 4);
    reader.close().await.unwrap();
    store.pool.close().await;
}

#[tokio::test]
async fn busy_reader_reports_partial_cleanup_and_retry_reclaims_space() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("storage.db");
    let store = fixture(&path).await;
    let before = file_bytes(&path);
    let reader = pinned_reader(&path).await;

    let error = store.clear_all_statistics_storage().await.unwrap_err();
    assert!(
        error.to_string().contains("records were cleared"),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("readers prevented WAL truncation"),
        "{error}"
    );
    assert_eq!(store.statistics_storage().await.unwrap().call_count, 0);
    // A busy checkpoint must release the application write coordinator and must
    // not poison the connection pool, even while that external reader remains.
    tokio::time::timeout(
        Duration::from_secs(3),
        store.ensure_conversation(&ConversationId::new("after-busy")),
    )
    .await
    .unwrap()
    .unwrap();
    reader.close().await.unwrap();
    store.clear_all_statistics_storage().await.unwrap();
    assert_reclaimed(&store, &path, before).await;
    store.pool.close().await;
}

#[tokio::test]
async fn disconnect_after_deletion_does_not_abandon_wal_reclamation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("storage.db");
    let store = fixture(&path).await;
    let before = file_bytes(&path);
    let reader = pinned_reader(&path).await;
    let caller = tokio::spawn({
        let store = store.clone();
        async move { store.clear_all_statistics_storage().await }
    });
    wait_for_deletion(&store).await;
    assert!(!caller.is_finished());
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    // A later application write waits behind the still-running cleanup. Its
    // acquisition proves that the detached cleanup has released the coordinator.
    reader.close().await.unwrap();
    let _write = tokio::time::timeout(Duration::from_secs(3), store.writes.lock())
        .await
        .unwrap();
    assert_reclaimed(&store, &path, before).await;
    drop(_write);
    store.set_detailed_logging(false).await.unwrap();
    store.pool.close().await;
}

#[tokio::test]
async fn failed_deletion_rolls_back_both_payload_types_and_keeps_store_usable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("storage.db");
    let store = fixture(&path).await;
    sqlx::query(
        "CREATE TRIGGER reject_trace_cleanup BEFORE DELETE ON cursor_run_trace_artifacts
                 BEGIN SELECT RAISE(ABORT, 'injected cleanup failure'); END",
    )
    .execute(store.pool())
    .await
    .unwrap();
    let error = store.clear_statistics_storage().await.unwrap_err();
    assert!(error.to_string().contains("injected cleanup failure"));
    assert!(!error.to_string().contains("records were cleared"));
    assert!(store.llm_call_request("call").await.unwrap().is_some());
    assert_eq!(store.llm_call_chunks("call").await.unwrap().len(), 1);
    assert_eq!(
        store.cursor_trace_artifacts("trace").await.unwrap().len(),
        1
    );
    store
        .ensure_conversation(&ConversationId::new("after-rollback"))
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER reject_trace_cleanup")
        .execute(store.pool())
        .await
        .unwrap();
    let before = file_bytes(&path);
    store.clear_statistics_storage().await.unwrap();
    assert_reclaimed(&store, &path, before).await;
    store.pool.close().await;
}

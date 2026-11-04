//! Verifies that unused-blob cleanup keeps everything a conversation can still reach.

use prost::Message;

use super::*;
use crate::{
    cursor::protocol::proto::agent::v1 as pb,
    model::{
        CanonicalMessage, ConversationId, MessageContent, Origin, Role, ToolImageReference,
        ToolResultContent,
    },
    store::{now_ms, BlobEdge, BlobId},
};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

async fn store(directory: &tempfile::TempDir) -> Store {
    let path = directory.path().join("blobs.db");
    Store::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap()
}

async fn put(store: &Store, data: &[u8], edges: &[BlobEdge]) -> BlobId {
    store.put_blob(data, edges).await.unwrap()
}

/// Moves every existing blob outside the recent-write window.
async fn age_all_blobs(store: &Store) {
    sqlx::query("UPDATE blobs SET created_at_ms = ?")
        .bind(now_ms() - 3 * DAY_MS)
        .execute(store.pool())
        .await
        .unwrap();
}

async fn exists(store: &Store, id: &BlobId) -> bool {
    store.get_blob(id).await.unwrap().is_some()
}

fn edge(child: &BlobId, field: &str) -> BlobEdge {
    BlobEdge {
        child: child.clone(),
        field_name: field.into(),
    }
}

struct Fixture {
    checkpoint: crate::model::CheckpointId,
    state: Vec<u8>,
    kept: Vec<(&'static str, BlobId)>,
    orphan_parent: BlobId,
    orphan_child: BlobId,
}

async fn build(store: &Store) -> Fixture {
    let root = put(store, b"root message", &[]).await;
    let step = put(store, b"turn step", &[]).await;
    let user = put(store, b"turn user", &[]).await;
    let turn = put(
        store,
        b"turn wrapper",
        &[
            edge(&user, "agent_conversation_turn.user_message"),
            edge(&step, "agent_conversation_turn.steps[0]"),
        ],
    )
    .await;
    let todo = put(store, b"todo", &[]).await;
    let summary = put(store, b"summary", &[]).await;
    let plan = put(store, b"plan", &[]).await;
    let summarized = put(store, b"summarized root", &[]).await;
    let archive = put(
        store,
        b"summary archive",
        &[edge(&summarized, "summarized_messages[0]")],
    )
    .await;
    let old_archive = put(store, b"older archive", &[]).await;
    let before = put(store, b"file before", &[]).await;
    let after = put(store, b"file after", &[]).await;
    let subagent = put(store, b"subagent state", &[]).await;
    let image = put(store, b"tool image bytes", &[]).await;
    let traced = put(store, b"trace artifact", &[]).await;

    let orphan_child = put(store, b"orphan child", &[]).await;
    let orphan_parent = put(
        store,
        b"orphan parent",
        &[edge(&orphan_child, "agent_conversation_turn.steps[0]")],
    )
    .await;

    let mut file_states_v2 = std::collections::HashMap::new();
    file_states_v2.insert(
        "src/main.rs".into(),
        pb::FileStateStructure {
            content: Some(after.as_bytes().to_vec()),
            initial_content: Some(before.as_bytes().to_vec()),
        },
    );
    let mut subagent_state_refs = std::collections::HashMap::new();
    subagent_state_refs.insert("subagent-1".into(), subagent.as_bytes().to_vec());
    let state = pb::ConversationStateStructure {
        root_prompt_messages_json: vec![root.as_bytes().to_vec()],
        turns: vec![turn.as_bytes().to_vec()],
        todos: vec![todo.as_bytes().to_vec()],
        summary: Some(summary.as_bytes().to_vec()),
        plan: Some(plan.as_bytes().to_vec()),
        summary_archive: Some(archive.as_bytes().to_vec()),
        summary_archives: vec![old_archive.as_bytes().to_vec()],
        file_states_v2,
        subagent_state_refs,
        ..Default::default()
    }
    .encode_to_vec();

    let conversation = ConversationId::new("conversation");
    let history = vec![
        CanonicalMessage::text("user-1", Role::User, Origin::User, "Read the image."),
        CanonicalMessage {
            message_id: "tool-result".into(),
            role: Role::Tool,
            origin: Origin::Tool,
            content: MessageContent::ToolResult(ToolResultContent {
                call_id: "call-1".into(),
                name: "Read".into(),
                content: "Read image file: a.png".into(),
                is_error: false,
                image: Some(ToolImageReference {
                    blob_id: image.to_base64(),
                    mime_type: "image/png".into(),
                    path: "a.png".into(),
                }),
                provider_parts: Vec::new(),
            }),
            runtime_event_id: None,
        },
    ];
    let checkpoint = store
        .import_checkpoint(&conversation, &history)
        .await
        .unwrap();
    store
        .save_cursor_checkpoint(checkpoint, &state)
        .await
        .unwrap();
    store.set_detailed_logging(true).await.unwrap();
    assert!(store
        .start_cursor_trace_if_detailed("trace-kept", Some("conversation"), "local_byok", None)
        .await
        .unwrap());
    store
        .link_cursor_trace_artifact(
            "trace-kept",
            "kept",
            "byok_server",
            &traced,
            &serde_json::json!({}),
        )
        .await
        .unwrap();

    Fixture {
        checkpoint,
        state,
        kept: vec![
            ("root message", root),
            ("turn", turn),
            ("turn user via edge", user),
            ("turn step via edge", step),
            ("todo", todo),
            ("summary", summary),
            ("plan", plan),
            ("summary archive", archive),
            ("summarized root via edge", summarized),
            ("older summary archive", old_archive),
            ("file initial content", before),
            ("file content", after),
            ("subagent state", subagent),
            ("tool image", image),
            ("trace artifact", traced),
        ],
        orphan_parent,
        orphan_child,
    }
}

#[tokio::test]
async fn unused_blob_cleanup_keeps_every_reachable_blob_and_removes_orphans() {
    let directory = tempfile::tempdir().unwrap();
    let store = store(&directory).await;
    let fixture = build(&store).await;
    age_all_blobs(&store).await;

    let preview = store.unreferenced_blobs().await.unwrap();
    assert_eq!(
        preview.blob_count, 2,
        "only the two orphan blobs are unused"
    );
    assert_eq!(
        preview.byte_count,
        (b"orphan parent".len() + b"orphan child".len()) as i64
    );

    let result = store.clear_unreferenced_blobs().await.unwrap();
    assert_eq!(result.removed_blobs, Some(preview));

    for (name, id) in &fixture.kept {
        assert!(exists(&store, id).await, "{name} blob was removed");
    }
    assert!(!exists(&store, &fixture.orphan_parent).await);
    assert!(!exists(&store, &fixture.orphan_child).await);
    let dangling: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM blob_edges e
         WHERE NOT EXISTS (SELECT 1 FROM blobs b WHERE b.blob_id = e.parent_blob_id)
            OR NOT EXISTS (SELECT 1 FROM blobs b WHERE b.blob_id = e.child_blob_id)",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(dangling, 0, "edges must not point at removed blobs");

    // The conversation still loads exactly as before.
    assert_eq!(
        store
            .cursor_checkpoint(fixture.checkpoint)
            .await
            .unwrap()
            .unwrap(),
        fixture.state
    );
    let state = pb::ConversationStateStructure::decode(fixture.state.as_slice()).unwrap();
    let turn = BlobId::from_bytes(&state.turns[0]).unwrap();
    assert_eq!(store.blob_closure(&[turn]).await.unwrap().len(), 3);
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

    // A second run finds nothing left to remove.
    let again = store.clear_unreferenced_blobs().await.unwrap();
    assert_eq!(again.removed_blobs, Some(UnreferencedBlobs::default()));
    store.pool.close().await;
}

#[tokio::test]
async fn unused_blob_cleanup_keeps_recent_blobs() {
    let directory = tempfile::tempdir().unwrap();
    let store = store(&directory).await;
    let fixture = build(&store).await;
    // Only the orphan parent is old. Its child was written recently, so it is
    // kept, and the old parent is the only blob removed.
    sqlx::query("UPDATE blobs SET created_at_ms = ? WHERE blob_id = ?")
        .bind(now_ms() - 3 * DAY_MS)
        .bind(fixture.orphan_parent.as_bytes().as_slice())
        .execute(store.pool())
        .await
        .unwrap();

    let result = store.clear_unreferenced_blobs().await.unwrap();
    assert_eq!(result.removed_blobs.unwrap().blob_count, 1);
    assert!(!exists(&store, &fixture.orphan_parent).await);
    assert!(exists(&store, &fixture.orphan_child).await);
    store.pool.close().await;
}

#[tokio::test]
async fn unused_blob_cleanup_removes_nothing_when_a_checkpoint_cannot_be_decoded() {
    let directory = tempfile::tempdir().unwrap();
    let store = store(&directory).await;
    let fixture = build(&store).await;
    age_all_blobs(&store).await;
    let count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blobs")
        .fetch_one(store.pool())
        .await
        .unwrap();
    // Field 1 must be length-delimited; a 32-bit value cannot be decoded.
    store
        .save_cursor_checkpoint(fixture.checkpoint, &[0x0d, 1, 2, 3, 4])
        .await
        .unwrap();

    let error = store.clear_unreferenced_blobs().await.unwrap_err();
    assert!(error.to_string().contains("no blob was removed"), "{error}");
    let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blobs")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count_after, count_before);
    assert!(exists(&store, &fixture.orphan_parent).await);
    assert!(store.unreferenced_blobs().await.is_err());
    store.pool.close().await;
}

#[tokio::test]
async fn unused_blob_cleanup_removes_nothing_when_a_blob_reference_is_invalid() {
    let directory = tempfile::tempdir().unwrap();
    let store = store(&directory).await;
    let fixture = build(&store).await;
    age_all_blobs(&store).await;
    let state = pb::ConversationStateStructure {
        turns: vec![vec![1, 2, 3]],
        ..Default::default()
    }
    .encode_to_vec();
    store
        .save_cursor_checkpoint(fixture.checkpoint, &state)
        .await
        .unwrap();

    let error = store.clear_unreferenced_blobs().await.unwrap_err();
    assert!(error.to_string().contains("no blob was removed"), "{error}");
    assert!(exists(&store, &fixture.orphan_parent).await);
    for (name, id) in &fixture.kept {
        assert!(exists(&store, id).await, "{name} blob was removed");
    }
    store.pool.close().await;
}

#[tokio::test]
async fn unused_blob_cleanup_keeps_blobs_written_during_a_running_turn() {
    let directory = tempfile::tempdir().unwrap();
    let store = store(&directory).await;
    let fixture = build(&store).await;
    let started = now_ms() - 5 * DAY_MS;
    // The orphan parent predates the running turn; the child was written while it ran.
    sqlx::query("UPDATE blobs SET created_at_ms = ?")
        .bind(started - DAY_MS)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE blobs SET created_at_ms = ? WHERE blob_id = ?")
        .bind(started + DAY_MS)
        .bind(fixture.orphan_child.as_bytes().as_slice())
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO runs(run_id, conversation_id, base_checkpoint_id, head_checkpoint_id,
             run_kind, status, created_at_ms, updated_at_ms)
         VALUES ('running-run', 'conversation', ?, ?, 'root', 'running', ?, ?)",
    )
    .bind(fixture.checkpoint.0)
    .bind(fixture.checkpoint.0)
    .bind(started)
    .bind(started)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE conversations SET active_run_id = 'running-run'")
        .execute(store.pool())
        .await
        .unwrap();

    let result = store.clear_unreferenced_blobs().await.unwrap();
    assert_eq!(result.removed_blobs.unwrap().blob_count, 1);
    assert!(!exists(&store, &fixture.orphan_parent).await);
    assert!(exists(&store, &fixture.orphan_child).await);
    for (name, id) in &fixture.kept {
        assert!(exists(&store, id).await, "{name} blob was removed");
    }
    store.pool.close().await;
}

/// Runs only on request against a throwaway copy of a real database:
/// `BYOK_BLOB_GC_COPY=/tmp/copy.db cargo test ... -- --ignored`.
#[tokio::test]
#[ignore]
async fn unused_blob_cleanup_on_a_real_database_copy() {
    let Some(path) = std::env::var_os("BYOK_BLOB_GC_COPY") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let store = Store::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    let blobs_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blobs")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let size_before = std::fs::metadata(&path).unwrap().len();
    let preview = store.unreferenced_blobs().await.unwrap();
    println!(
        "real copy: {blobs_before} blobs, file {size_before} bytes; would remove {} blobs, {} bytes",
        preview.blob_count, preview.byte_count
    );
    let checkpoints: Vec<(i64,)> = sqlx::query_as("SELECT checkpoint_id FROM cursor_checkpoints")
        .fetch_all(store.pool())
        .await
        .unwrap();
    // Some referenced blobs live only in Cursor and are fetched on demand, so
    // only blobs present before cleanup are required to remain.
    let mut present_before = std::collections::HashSet::new();
    for row in sqlx::query("SELECT blob_id FROM blobs")
        .fetch_all(store.pool())
        .await
        .unwrap()
    {
        use sqlx::Row;
        present_before.insert(BlobId::from_bytes(row.get::<Vec<u8>, _>(0).as_slice()).unwrap());
    }
    let result = store.clear_unreferenced_blobs().await.unwrap();
    assert_eq!(result.removed_blobs, Some(preview));
    let mut checked = 0usize;
    for (checkpoint_id,) in checkpoints {
        let state = store
            .cursor_checkpoint(crate::model::CheckpointId(checkpoint_id))
            .await
            .unwrap()
            .unwrap();
        let state = pb::ConversationStateStructure::decode(state.as_slice()).unwrap();
        let mut roots = std::collections::HashSet::new();
        super::blobs::collect_state_roots_for_test(&state, &mut roots);
        let closure = store
            .blob_closure(&roots.into_iter().collect::<Vec<_>>())
            .await
            .unwrap();
        for id in closure.iter().filter(|id| present_before.contains(id)) {
            checked += 1;
            assert!(
                store.get_blob(id).await.unwrap().is_some(),
                "checkpoint {checkpoint_id} lost blob {}",
                id.to_base64()
            );
        }
    }
    println!("real copy: verified {checked} checkpoint blob references survived");
    let size_after = std::fs::metadata(&path).unwrap().len();
    println!("real copy after cleanup: file {size_after} bytes");
    store.pool.close().await;
}

#[tokio::test]
async fn unused_blob_cleanup_reclaims_file_space() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("blobs.db");
    let store = Store::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    let large = vec![b'x'; 4 * 1024 * 1024];
    put(&store, &large, &[]).await;
    age_all_blobs(&store).await;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(store.pool())
        .await
        .unwrap();
    let before = std::fs::metadata(&path).unwrap().len();

    let result = store.clear_unreferenced_blobs().await.unwrap();
    assert_eq!(result.removed_blobs.unwrap().byte_count, large.len() as i64);
    let after = std::fs::metadata(&path).unwrap().len();
    assert!(
        after < before / 4,
        "file did not shrink: {before} -> {after}"
    );
    let free: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(free, 0);
    store.pool.close().await;
}

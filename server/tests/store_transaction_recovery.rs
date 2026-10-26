//! Exercises failed and cancelled SQLite writes through the real Store pool.
#[path = "support/fixtures.rs"]
mod fixtures;

use std::time::Duration;

use cursor_server::{model::ConversationId, store::Store};
use sqlx::{pool::PoolConnection, Connection, Sqlite};

// Keep only one pool slot available: another healthy connection must not hide
// whether the failed transaction's connection is safely returned or discarded.
async fn reserve_other_connections(store: &Store) -> Vec<PoolConnection<Sqlite>> {
    let mut held = Vec::new();
    for _ in 1..store.pool().options().get_max_connections() {
        held.push(store.pool().acquire().await.unwrap());
    }
    held
}

async fn mark_connection(store: &Store) {
    sqlx::query("CREATE TEMP TABLE connection_marker(value INTEGER)")
        .execute(store.pool())
        .await
        .unwrap();
}

async fn assert_healthy_connection_reused(store: &Store) {
    let mut conn = store.pool().acquire().await.unwrap();
    assert!(!conn.is_in_transaction());
    let marker: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_temp_master WHERE name = 'connection_marker'",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(
        marker, 1,
        "a clean rollback must keep the healthy connection"
    );
}

async fn assert_store_writes_recover(store: &Store) {
    tokio::time::timeout(Duration::from_secs(5), async {
        for index in 0..4 {
            let conversation = ConversationId::new(format!("after-failure-{index}"));
            let checkpoint = store.ensure_conversation(&conversation).await.unwrap();
            assert_eq!(
                store
                    .conversation(&conversation)
                    .await
                    .unwrap()
                    .unwrap()
                    .current_checkpoint_id,
                checkpoint
            );
            assert!(store
                .load_checkpoint_messages(checkpoint)
                .await
                .unwrap()
                .is_empty());
        }
    })
    .await
    .expect("the same Store must recover without restarting");
}

async fn install_rollback_trigger(store: &Store) {
    sqlx::query(
        "CREATE TRIGGER rollback_checkpoint BEFORE INSERT ON conversation_checkpoints
         WHEN NEW.conversation_id = 'doomed'
         BEGIN SELECT RAISE(ROLLBACK, 'injected transaction rollback'); END",
    )
    .execute(store.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn automatic_rollback_does_not_poison_later_store_writes() {
    let (_directory, store) = fixtures::temp_store().await;
    let _held = reserve_other_connections(&store).await;
    install_rollback_trigger(&store).await;

    let doomed = ConversationId::new("doomed");
    let error = store.ensure_conversation(&doomed).await.unwrap_err();
    assert!(error.to_string().contains("injected transaction rollback"));
    assert!(store.conversation(&doomed).await.unwrap().is_none());
    assert_store_writes_recover(&store).await;
}

#[tokio::test]
async fn failed_explicit_rollback_does_not_poison_later_store_writes() {
    failed_transaction_completion_releases_connection(false).await;
}

#[tokio::test]
async fn failed_commit_after_automatic_rollback_does_not_poison_later_store_writes() {
    failed_transaction_completion_releases_connection(true).await;
}

async fn failed_transaction_completion_releases_connection(commit: bool) {
    let (_directory, store) = fixtures::temp_store().await;
    let _held = reserve_other_connections(&store).await;
    sqlx::query("CREATE TABLE rollback_probe(value INTEGER)")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query(
        "CREATE TRIGGER rollback_probe_insert BEFORE INSERT ON rollback_probe
         BEGIN SELECT RAISE(ROLLBACK, 'injected transaction rollback'); END",
    )
    .execute(store.pool())
    .await
    .unwrap();

    let mut tx = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    let error = sqlx::query("INSERT INTO rollback_probe(value) VALUES (1)")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("injected transaction rollback"));
    let result = if commit {
        tx.commit().await
    } else {
        tx.rollback().await
    };
    let error = result.unwrap_err();
    assert!(error.to_string().contains("no transaction is active"));
    assert_store_writes_recover(&store).await;
}

#[tokio::test]
async fn sqlite_full_rolls_back_atomically_and_later_writes_recover() {
    let (_directory, store) = fixtures::temp_store().await;
    let _held = reserve_other_connections(&store).await;
    sqlx::query("CREATE TABLE space_probe(data BLOB)")
        .execute(store.pool())
        .await
        .unwrap();
    let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(store.pool())
        .await
        .unwrap();
    // Limit this temporary database, not the filesystem or the user's data.
    sqlx::query(&format!("PRAGMA max_page_count = {}", pages + 2))
        .execute(store.pool())
        .await
        .unwrap();
    let mut tx = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query(
        "INSERT INTO conversations(conversation_id, updated_at_ms) VALUES ('out-of-space', 0)",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    let error = sqlx::query("INSERT INTO space_probe(data) VALUES (zeroblob(1048576))")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("13"),
        "expected SQLITE_FULL, got {error}"
    );
    drop(tx);

    sqlx::query("PRAGMA max_page_count = 1073741823")
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .conversation(&ConversationId::new("out-of-space"))
        .await
        .unwrap()
        .is_none());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM space_probe")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
    assert_store_writes_recover(&store).await;
}

#[tokio::test]
async fn deferred_constraint_commit_failure_rolls_back_and_reuses_connection() {
    let (_directory, store) = fixtures::temp_store().await;
    let _held = reserve_other_connections(&store).await;
    mark_connection(&store).await;

    let mut tx = store.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO checkpoint_messages(checkpoint_id, ordinal, conversation_id, message_id)
         VALUES (999999, 0, 'missing-conversation', 'missing-message')",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    let error = tx.commit().await.unwrap_err();
    assert!(error.to_string().contains("FOREIGN KEY constraint failed"));

    assert_healthy_connection_reused(&store).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM checkpoint_messages")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_store_writes_recover(&store).await;
}

#[tokio::test]
async fn cancelling_a_write_rolls_back_before_connection_reuse() {
    let (_directory, store) = fixtures::temp_store().await;
    let _held = reserve_other_connections(&store).await;
    mark_connection(&store).await;
    let pool = store.pool().clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "INSERT INTO conversations(conversation_id, updated_at_ms) VALUES ('cancelled', 0)",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        started.send(()).unwrap();
        std::future::pending::<()>().await;
        tx.commit().await.unwrap();
    });
    ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());

    assert_healthy_connection_reused(&store).await;
    assert!(store
        .conversation(&ConversationId::new("cancelled"))
        .await
        .unwrap()
        .is_none());
    assert_store_writes_recover(&store).await;
}

#[tokio::test]
async fn successful_writes_reuse_the_connection_and_preserve_data() {
    let (_directory, store) = fixtures::temp_store().await;
    let _held = reserve_other_connections(&store).await;
    mark_connection(&store).await;
    assert_store_writes_recover(&store).await;
    assert_healthy_connection_reused(&store).await;
}

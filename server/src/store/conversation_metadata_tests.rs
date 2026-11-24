use super::*;
use crate::model::{
    CanonicalMessage, ModelSpec, Origin, PreparedRun, PromptSpec, Role, RunAction, RunKind,
};

#[tokio::test]
async fn metadata_survives_upgrade_and_restart_without_mutating_active_history() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("upgrade.db");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            all.iter().filter(|m| m.version <= 12).cloned().collect(),
        ),
        ..sqlx::migrate::Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .unwrap();
    let old = Store {
        pool,
        writes: Default::default(),
    };
    let id = ConversationId::new("existing");
    let root = old.ensure_conversation(&id).await.unwrap();
    let messages = vec![CanonicalMessage::text(
        "old-user",
        Role::User,
        Origin::User,
        "old conversation",
    )];
    let head = old.import_checkpoint(&id, &messages).await.unwrap();
    let prepared = PreparedRun {
        run_id: "active".into(),
        cursor_request_id: None,
        conversation_id: id.clone(),
        kind: RunKind::Root,
        model: ModelSpec::new("model"),
        prompt: PromptSpec {
            instructions: "unchanged".into(),
            tools: Vec::new(),
        },
        initial_messages: Vec::new(),
        action: RunAction::Start,
        base_checkpoint_id: head,
    };
    old.claim_run(&prepared).await.unwrap();
    let before = old.conversation(&id).await.unwrap().unwrap();
    old.pool.close().await;
    let url = format!("sqlite://{}", path.display());
    let store = Store::connect(&url).await.unwrap();
    assert!(store.conversation_metadata(&id).await.unwrap().is_none());
    store
        .update_conversation_metadata(
            &id,
            Some("  本地标题  "),
            true,
            &[" /workspace ".into(), "/workspace".into(), String::new()],
        )
        .await
        .unwrap();
    store
        .update_conversation_metadata(&id, Some("late generated name"), true, &[])
        .await
        .unwrap();
    let saved = store.conversation_metadata(&id).await.unwrap().unwrap();
    assert_eq!(saved.name.as_deref(), Some("本地标题"));
    assert_eq!(saved.workspace_paths, vec!["/workspace"]);
    assert_eq!(store.conversation(&id).await.unwrap(), Some(before.clone()));
    assert_eq!(
        store.load_checkpoint_messages(head).await.unwrap(),
        messages
    );
    assert!(store
        .load_checkpoint_messages(root)
        .await
        .unwrap()
        .is_empty());
    store.pool.close().await;
    let reopened = Store::connect(&url).await.unwrap();
    assert_eq!(
        reopened.conversation_metadata(&id).await.unwrap(),
        Some(saved)
    );
    assert_eq!(reopened.conversation(&id).await.unwrap(), Some(before));
    reopened
        .update_conversation_metadata(&id, Some("manual rename"), false, &[])
        .await
        .unwrap();
    reopened
        .update_conversation_metadata(&id, None, false, &["/next".into()])
        .await
        .unwrap();
    assert_eq!(
        reopened.conversation_metadata(&id).await.unwrap(),
        Some(ConversationMetadata {
            name: Some("manual rename".into()),
            workspace_paths: vec!["/next".into()],
        })
    );
    assert_eq!(
        reopened.load_checkpoint_messages(head).await.unwrap(),
        messages
    );
}

#[tokio::test]
async fn metadata_compare_and_set_is_atomic_across_independent_stores() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}",
        directory.path().join("concurrent.db").display()
    );
    let first = Store::connect(&url).await.unwrap();
    let second = Store::connect(&url).await.unwrap();
    let id = ConversationId::new("early");
    let (a, b) = tokio::join!(
        first.update_conversation_metadata(&id, Some("A"), true, &[]),
        second.update_conversation_metadata(&id, Some("B"), true, &[]),
    );
    a.unwrap();
    b.unwrap();
    let saved = first.conversation_metadata(&id).await.unwrap().unwrap();
    assert!(matches!(saved.name.as_deref(), Some("A" | "B")));
    first
        .update_conversation_metadata(&id, Some("late"), true, &[])
        .await
        .unwrap();
    assert_eq!(first.conversation_metadata(&id).await.unwrap(), Some(saved));
    assert!(first.conversation(&id).await.unwrap().is_none());
    assert!(first
        .update_conversation_metadata(&" ".into(), Some("invalid"), false, &[])
        .await
        .is_err());
    assert!(
        sqlx::query("INSERT INTO conversation_metadata VALUES ('invalid', NULL, '{}')")
            .execute(first.pool())
            .await
            .is_err()
    );
}

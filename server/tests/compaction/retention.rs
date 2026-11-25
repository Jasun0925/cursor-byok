//! Automatic compaction must account for old tasks before replacing history.
use super::*;

fn registry(
    store: cursor_server::store::Store,
    provider: &fake_provider::FakeProvider,
) -> TransportRegistry {
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    )
}

#[tokio::test]
async fn automatic_summary_reads_earliest_tasks_and_carries_progress_between_batches() {
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("retention", Some(100_000)))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    provider.push(text_response(
        &format!("EARLY_TASK_COMPLETED {}", "a".repeat(200_000)),
        4_000,
        50_000,
    ));
    provider.push(text_response(
        &format!("LATER_TASK_COMPLETED {}", "b".repeat(200_000)),
        60_000,
        50_000,
    ));
    provider.push(text_response(
        "SUMMARY_1: ORIGINAL_GOAL; EARLY_TASK_COMPLETED; keep completed work",
        80_000,
        50,
    ));
    provider.push(text_response(
        "SUMMARY_2: ORIGINAL_GOAL; EARLY_TASK_COMPLETED; LATER_TASK_COMPLETED",
        30_000,
        50,
    ));
    provider.push(text_response("continue pending work only", 1_000, 20));
    let registry = registry(store, &provider);
    let first = run(
        &registry,
        "retention-1",
        user_request(
            "retention",
            "u1",
            "ORIGINAL_GOAL: finish all tasks without repeating completed work",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let second = run(
        &registry,
        "retention-2",
        user_request(
            "retention",
            "u2",
            "work on the next task",
            &model.model_hash,
            first.checkpoints.last().cloned(),
        ),
    )
    .await;
    let third = run(
        &registry,
        "retention-3",
        user_request(
            "retention",
            "u3",
            "CURRENT_PENDING_TASK",
            &model.model_hash,
            second.checkpoints.last().cloned(),
        ),
    )
    .await;
    assert!(
        matches!(third.end_stream, EndStream::Success {}),
        "{:?}",
        third.end_stream
    );
    let requests = provider.requests();
    let summaries = requests
        .iter()
        .filter(|request| request.prompt.tools.is_empty())
        .collect::<Vec<_>>();
    let source = serde_json::to_string(&summaries).unwrap();
    for fact in [
        "ORIGINAL_GOAL",
        "EARLY_TASK_COMPLETED",
        "LATER_TASK_COMPLETED",
    ] {
        assert!(
            source.contains(fact),
            "summary input silently discarded {fact}"
        );
    }
    assert_eq!(
        summaries.len(),
        2,
        "all source content must fit in bounded summary calls"
    );
    assert!(serde_json::to_string(&summaries[1].history)
        .unwrap()
        .contains("SUMMARY_1"));
    assert_eq!(third.summary_started, 1);
    assert_eq!(third.summary_completed, 1);
    let continued = serde_json::to_string(&requests.last().unwrap().history).unwrap();
    for fact in [
        "SUMMARY_2",
        "ORIGINAL_GOAL",
        "EARLY_TASK_COMPLETED",
        "LATER_TASK_COMPLETED",
        "CURRENT_PENDING_TASK",
    ] {
        assert!(continued.contains(fact), "continuation missing {fact}");
    }
}

#[tokio::test]
async fn failed_large_automatic_summary_does_not_replace_history_with_its_tail() {
    check_failed_summary("error").await;
}

#[tokio::test]
async fn provider_overflow_without_configured_window_retries_summary_and_continues() {
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("unknown-window", None))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    provider.push(text_response(
        &format!("COMPLETED_TASK {}", "x".repeat(30_000)),
        4_000,
        8_000,
    ));
    provider.push_error(cursor_server::Error::Provider(
        "context_length_exceeded".into(),
    ));
    provider.push_error(cursor_server::Error::Provider(
        "context_length_exceeded".into(),
    ));
    provider.push(text_response(
        "ORIGINAL_GOAL; COMPLETED_TASK; next pending work",
        8_000,
        30,
    ));
    provider.push(text_response("continue only pending work", 500, 10));
    let registry = registry(store, &provider);
    let first = run(
        &registry,
        "unknown-first",
        user_request(
            "unknown-window",
            "u1",
            "ORIGINAL_GOAL",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let second = run(
        &registry,
        "unknown-second",
        user_request(
            "unknown-window",
            "u2",
            "continue",
            &model.model_hash,
            first.checkpoints.last().cloned(),
        ),
    )
    .await;
    assert!(
        matches!(second.end_stream, EndStream::Success {}),
        "{:?}",
        second.end_stream
    );
    assert_eq!(second.summary_completed, 1);
    let requests = provider.requests();
    assert_eq!(requests.len(), 5);
    for index in [2, 3] {
        assert!(requests[index].prompt.tools.is_empty());
        assert!(serde_json::to_string(&requests[index].history)
            .unwrap()
            .contains("ORIGINAL_GOAL"));
    }
    assert!(serde_json::to_string(&requests[4].history)
        .unwrap()
        .contains("COMPLETED_TASK"));
}

#[tokio::test]
async fn truncated_automatic_summary_preserves_history() {
    check_failed_summary("length").await;
}

#[tokio::test]
async fn empty_automatic_summary_preserves_history() {
    check_failed_summary("empty").await;
}

#[tokio::test]
async fn later_batch_failure_does_not_commit_the_first_summary() {
    check_failed_summary("later").await;
}

#[tokio::test]
async fn stalled_large_summary_times_out_without_replacing_history() {
    check_failed_summary("timeout").await;
}

async fn check_failed_summary(kind: &str) {
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("retention-failure", Some(100_000)))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    let original_answer = format!("COMPLETED_DO_NOT_REPEAT {}", "x".repeat(400_000));
    provider.push(text_response(&original_answer, 150_000, 100_000));
    match kind {
        "timeout" => provider.push_pending(),
        "empty" => provider.push(text_response(" ", 100, 1)),
        "length" => {
            let mut partial = text_response("UNCOMMITTED_PARTIAL_SUMMARY", 100, 4096);
            *partial.last_mut().unwrap() = ModelEvent::Done(FinishReason::Length);
            provider.push(partial);
        }
        "later" => {
            provider.push(text_response("UNCOMMITTED_FIRST_BATCH", 80_000, 20));
            provider.push_error(cursor_server::Error::Provider(
                "401 second batch failed".into(),
            ));
        }
        _ => provider.push_error(cursor_server::Error::Provider(
            "401 synthetic summary failure".into(),
        )),
    }
    provider.push(text_response(
        "must not continue from a lossy summary",
        500,
        10,
    ));
    let registry = registry(store.clone(), &provider);
    let first = run(
        &registry,
        "retention-failed-1",
        user_request(
            "retention-failed",
            "u1",
            "ORIGINAL_GOAL",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let second = run_with_timeout(
        &registry,
        "retention-failed-2",
        user_request(
            "retention-failed",
            "u2",
            "CURRENT_REQUEST",
            &model.model_hash,
            first.checkpoints.last().cloned(),
        ),
        // A stalled summary fails after the 60 second idle limit.
        Duration::from_secs(70),
    )
    .await;
    assert!(
        matches!(second.end_stream, EndStream::Failure { .. }),
        "{:?}",
        second.end_stream
    );
    assert_eq!(second.summary_completed, 0);
    assert_eq!(
        provider.requests().len(),
        if kind == "later" { 3 } else { 2 }
    );
    let retained = store
        .load_current_messages(&ConversationId::new("retention-failed"))
        .await
        .unwrap();
    let retained = serde_json::to_string(&retained).unwrap();
    assert!(retained.contains("ORIGINAL_GOAL"));
    assert!(retained.contains(&original_answer));
    assert!(retained.contains("CURRENT_REQUEST"));
    assert!(!retained.contains("<conversation_summary>"));
    assert!(!retained.contains("UNCOMMITTED_"));
}

#[tokio::test]
async fn cancellation_during_second_batch_keeps_original_history() {
    use cursor_server::{
        model::{CanonicalMessage, ModelSpec, PreparedRun, PromptSpec, RunAction, RunId, RunKind},
        run::{self, RunEngine, RunEvent, RunOutcome},
    };
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("cancel-batches", Some(100_000)))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    provider.push(text_response(&"x".repeat(400_000), 150_000, 100_000));
    provider.push(text_response("UNCOMMITTED_FIRST_BATCH", 80_000, 20));
    provider.push_pending();
    let registry = registry(store.clone(), &provider);
    run(
        &registry,
        "cancel-seed",
        user_request(
            "cancel-batches",
            "goal",
            "ORIGINAL_GOAL",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let conversation_id = ConversationId::new("cancel-batches");
    let original = store.load_current_messages(&conversation_id).await.unwrap();
    let mut spec = ModelSpec::new(&model.model_hash);
    spec.context_window_tokens = Some(100_000);
    let prepared = PreparedRun {
        run_id: RunId::new("cancel-second-batch"),
        cursor_request_id: None,
        base_checkpoint_id: store.ensure_conversation(&conversation_id).await.unwrap(),
        conversation_id: conversation_id.clone(),
        kind: RunKind::Root,
        model: spec,
        prompt: PromptSpec {
            instructions: "continue".into(),
            tools: Vec::new(),
        },
        initial_messages: vec![CanonicalMessage::text(
            "current",
            Role::User,
            Origin::User,
            "CURRENT_REQUEST",
        )],
        action: RunAction::Start,
    };
    let (port, mut session, handle) = run::channel(prepared.run_id.clone(), 32);
    let cancellation = handle.cancellation();
    let engine = tokio::spawn({
        let store = store.clone();
        let provider = provider.clone();
        async move {
            RunEngine::new(store, Arc::new(provider))
                .run(prepared, port, cancellation)
                .await
        }
    });
    let events = tokio::spawn(async move {
        let mut completed = 0;
        while let Some(event) = session.events.recv().await {
            match event {
                RunEvent::MessagesCommitted(committed) => committed.barrier.complete(Ok(())),
                RunEvent::AutoCompactionCompleted => completed += 1,
                RunEvent::Ended(_) => break,
                _ => {}
            }
        }
        completed
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while provider.requests().len() < 3 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    handle.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), engine)
            .await
            .unwrap()
            .unwrap(),
        RunOutcome::Cancelled
    );
    assert_eq!(events.await.unwrap(), 0);
    let retained = store.load_current_messages(&conversation_id).await.unwrap();
    assert!(retained.starts_with(&original));
    assert!(!serde_json::to_string(&retained)
        .unwrap()
        .contains("UNCOMMITTED_FIRST_BATCH"));
}

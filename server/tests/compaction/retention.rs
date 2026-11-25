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
    provider.push(text_response("ANSWERED_FROM_FULL_HISTORY", 150_000, 10));
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
    // A failed summary leaves the history as it was, and the turn is answered
    // from that full history instead of failing.
    assert!(
        matches!(second.end_stream, EndStream::Success {}),
        "{:?}",
        second.end_stream
    );
    assert_eq!(second.summary_started, 1);
    assert_eq!(second.summary_completed, 1);
    assert!(second.summary.is_empty());
    let requests = provider.requests();
    let summary_calls = if kind == "later" { 2 } else { 1 };
    assert_eq!(requests.len(), 2 + summary_calls);
    let answer = requests.last().unwrap();
    assert!(!answer.prompt.tools.is_empty());
    let answered_from = serde_json::to_string(&answer.history).unwrap();
    assert!(answered_from.contains("ORIGINAL_GOAL"));
    assert!(answered_from.contains(&original_answer));
    assert!(answered_from.contains("CURRENT_REQUEST"));
    assert!(!answered_from.contains("<conversation_summary>"));
    assert!(!answered_from.contains("UNCOMMITTED_"));
    let retained = store
        .load_current_messages(&ConversationId::new("retention-failed"))
        .await
        .unwrap();
    let retained = serde_json::to_string(&retained).unwrap();
    assert!(retained.contains("ORIGINAL_GOAL"));
    assert!(retained.contains(&original_answer));
    assert!(retained.contains("CURRENT_REQUEST"));
    assert!(retained.contains("ANSWERED_FROM_FULL_HISTORY"));
    assert!(!retained.contains("<conversation_summary>"));
    assert!(!retained.contains("UNCOMMITTED_"));
}

#[tokio::test]
async fn provider_refusal_after_failed_summary_compacts_once_more_then_fails() {
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("refusal-after-failure", Some(100_000)))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    let original_answer = format!("COMPLETED_DO_NOT_REPEAT {}", "x".repeat(400_000));
    provider.push(text_response(&original_answer, 150_000, 100_000));
    // Preflight summary fails, so the model is called with the full history.
    provider.push_error(cursor_server::Error::Provider(
        "401 first summary failure".into(),
    ));
    // The provider refuses that history as too long; the overflow path gets
    // one more summary attempt, and its failure ends the turn.
    provider.push_error(cursor_server::Error::Provider(
        "context_length_exceeded".into(),
    ));
    provider.push_error(cursor_server::Error::Provider(
        "401 second summary failure".into(),
    ));
    provider.push(text_response("MUST_NOT_BE_REQUESTED", 500, 10));
    let registry = registry(store.clone(), &provider);
    let first = run(
        &registry,
        "refusal-after-failure-1",
        user_request(
            "refusal-after-failure",
            "u1",
            "ORIGINAL_GOAL",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let second = run(
        &registry,
        "refusal-after-failure-2",
        user_request(
            "refusal-after-failure",
            "u2",
            "CURRENT_REQUEST",
            &model.model_hash,
            first.checkpoints.last().cloned(),
        ),
    )
    .await;
    assert!(
        matches!(&second.end_stream, EndStream::Failure { error }
            if error.message.contains("second summary failure")),
        "{:?}",
        second.end_stream
    );
    assert_eq!(second.summary_started, 2);
    // Only the preflight failure closes the indicator; the failing overflow
    // summary ends the turn instead.
    assert_eq!(second.summary_completed, 1);
    let requests = provider.requests();
    assert_eq!(requests.len(), 4);
    assert!(requests[1].prompt.tools.is_empty());
    assert!(!requests[2].prompt.tools.is_empty());
    assert!(requests[3].prompt.tools.is_empty());
    let retained = store
        .load_current_messages(&ConversationId::new("refusal-after-failure"))
        .await
        .unwrap();
    let retained = serde_json::to_string(&retained).unwrap();
    assert!(retained.contains(&original_answer));
    assert!(retained.contains("CURRENT_REQUEST"));
    assert!(!retained.contains("<conversation_summary>"));
}

#[tokio::test]
async fn failed_preflight_summary_is_not_retried_after_a_tool_round() {
    use cursor_server::{
        model::{
            CanonicalMessage, ModelSpec, PreparedRun, PromptSpec, RunAction, RunId, RunKind,
            ToolResult,
        },
        run::{self, RunEngine, RunEvent, RunOutcome},
    };
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("no-summary-retry", Some(100_000)))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    let original_answer = format!("COMPLETED_DO_NOT_REPEAT {}", "x".repeat(400_000));
    provider.push(text_response(&original_answer, 150_000, 100_000));
    provider.push_error(cursor_server::Error::Provider(
        "401 synthetic summary failure".into(),
    ));
    provider.push(vec![
        ModelEvent::Start {
            model_call_id: "tool-call".into(),
        },
        ModelEvent::ToolCallStart {
            index: 0,
            call_id: "call-1".into(),
            name: "Read".into(),
        },
        ModelEvent::ToolCallArgumentsDelta {
            index: 0,
            delta: "{\"path\":\"/tmp/a\"}".into(),
        },
        ModelEvent::ToolCallEnd { index: 0 },
        ModelEvent::Done(FinishReason::ToolUse),
    ]);
    // A second summary attempt would take this response as its summary.
    provider.push(text_response("FINAL_ANSWER", 150_000, 10));
    let registry = registry(store.clone(), &provider);
    run(
        &registry,
        "no-summary-retry-seed",
        user_request(
            "no-summary-retry",
            "goal",
            "ORIGINAL_GOAL",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let conversation_id = ConversationId::new("no-summary-retry");
    let mut spec = ModelSpec::new(&model.model_hash);
    spec.context_window_tokens = Some(100_000);
    let prepared = PreparedRun {
        run_id: RunId::new("no-summary-retry-run"),
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
        let (mut started, mut failed, mut completed) = (0, 0, 0);
        while let Some(event) = session.events.recv().await {
            match event {
                RunEvent::MessagesCommitted(committed) => committed.barrier.complete(Ok(())),
                RunEvent::ExecuteToolRound { calls, .. } => {
                    for call in calls {
                        handle
                            .tool_result(ToolResult {
                                call_id: call.call_id,
                                content: "TOOL_OUTPUT".into(),
                                is_error: false,
                                image: None,
                            })
                            .await;
                    }
                }
                RunEvent::AutoCompactionStarted => started += 1,
                RunEvent::AutoCompactionFailed => failed += 1,
                RunEvent::AutoCompactionCompleted => completed += 1,
                RunEvent::Ended(_) => break,
                _ => {}
            }
        }
        (started, failed, completed)
    });
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(10), engine)
            .await
            .unwrap()
            .unwrap(),
        RunOutcome::Completed
    );
    assert_eq!(events.await.unwrap(), (1, 1, 0));
    let requests = provider.requests();
    assert_eq!(requests.len(), 4);
    let last = serde_json::to_string(&requests[3].history).unwrap();
    assert!(last.contains(&original_answer));
    assert!(last.contains("CURRENT_REQUEST"));
    assert!(last.contains("TOOL_OUTPUT"));
    assert!(!last.contains("<conversation_summary>"));
    let retained =
        serde_json::to_string(&store.load_current_messages(&conversation_id).await.unwrap())
            .unwrap();
    assert!(retained.contains("FINAL_ANSWER"));
    assert!(!retained.contains("<conversation_summary>"));
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

//! #373: a failed summary or post-summary call must not wedge later runs.
use super::*;

#[test]
fn end_stream_without_error_text_is_still_a_failure() {
    let success = connect::encode_end_stream();
    let (_, payload) = connect::decode_frames(&success).unwrap().pop().unwrap();
    assert!(matches!(
        serde_json::from_slice::<EndStream>(&payload).unwrap(),
        EndStream::Success {}
    ));

    let failure = connect::encode_error_end_stream(&connect::ConnectStreamError {
        code: connect::ConnectCode::Internal,
        message: String::new(),
        details: vec![],
    })
    .unwrap();
    let (_, payload) = connect::decode_frames(&failure).unwrap().pop().unwrap();
    assert!(matches!(
        serde_json::from_slice::<EndStream>(&payload).unwrap(),
        EndStream::Failure { error } if error.code == "internal" && error.message.is_empty()
    ));
    for invalid in [
        "null",
        "[]",
        r#"{"error":null}"#,
        r#"{"error":{}}"#,
        r#"{"error":{"message":"failed"}}"#,
        r#"{"error":{"code":42}}"#,
        r#"{"error":{"code":"internal","message":42}}"#,
        r#"{"unexpected":true}"#,
    ] {
        assert!(
            serde_json::from_str::<EndStream>(invalid).is_err(),
            "{invalid}"
        );
    }
}

fn new_registry(
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

fn rejected_after_text(text: &str) -> Vec<Result<ModelEvent, cursor_server::Error>> {
    vec![
        Ok(ModelEvent::Start {
            model_call_id: "synthetic-failed-call".into(),
        }),
        Ok(ModelEvent::TextStart),
        Ok(ModelEvent::TextDelta(text.into())),
        Err(cursor_server::Error::Provider(
            "OpenAI Responses 401 Unauthorized: synthetic failure".into(),
        )),
    ]
}

#[tokio::test]
async fn failed_manual_summary_preserves_history_and_can_be_retried() {
    let (_directory, store) = fixtures::temp_store().await;
    let model = store
        .create_model(&windowed_model("summary-retry", None))
        .await
        .unwrap();
    let provider = fake_provider::FakeProvider::default();
    provider.push(text_response("remembered answer", 4000, 20));
    provider.push_results(rejected_after_text("UNCOMMITTED_SUMMARY"));
    provider.push(text_response("complete durable summary", 3000, 20));
    provider.push(text_response("continued normally", 600, 10));
    let registry = new_registry(store.clone(), &provider);
    let first = run(
        &registry,
        "summary-initial",
        user_request(
            "summary-retry",
            "first-user",
            "remember alpha",
            &model.model_hash,
            None,
        ),
    )
    .await;
    let before = store
        .load_current_messages(&ConversationId::new("summary-retry"))
        .await
        .unwrap();
    let state = first.checkpoints.last().unwrap().clone();
    let failed = run(
        &registry,
        "summary-failure",
        summary_request("summary-retry", &model.model_hash, state.clone()),
    )
    .await;
    assert!(
        matches!(&failed.end_stream, EndStream::Failure { error }
            if !error.code.is_empty() && error.message.contains("401")),
        "{:?}",
        failed.end_stream
    );
    assert_eq!(failed.summary_started, 1);
    assert_eq!(failed.summary_completed, 0);
    let retained = store
        .load_current_messages(&ConversationId::new("summary-retry"))
        .await
        .unwrap();
    // Cursor checkpoint hydration assigns root IDs to assistant messages. Compare
    // persisted content to the original, then compare exact projected identities
    // against the request that actually entered the failed summary lifecycle.
    assert_eq!(
        retained
            .iter()
            .map(|message| (
                &message.role,
                &message.origin,
                &message.content,
                &message.runtime_event_id
            ))
            .collect::<Vec<_>>(),
        before
            .iter()
            .map(|message| (
                &message.role,
                &message.origin,
                &message.content,
                &message.runtime_event_id
            ))
            .collect::<Vec<_>>()
    );
    let projected = cursor_server::model::project_messages(&retained).unwrap();
    assert_eq!(provider.requests()[1].history[..projected.len()], projected);

    let compacted = run(
        &registry,
        "summary-retry-run",
        summary_request("summary-retry", &model.model_hash, state),
    )
    .await;
    assert!(
        matches!(compacted.end_stream, EndStream::Success {}),
        "{:?}",
        compacted.end_stream
    );
    assert_eq!(compacted.summary_completed, 1);
    assert_eq!(compacted.summary, "complete durable summary");
    assert_eq!(compacted.checkpoints.last().unwrap().self_summary_count, 1);
    let after = run(
        &registry,
        "summary-followup",
        user_request(
            "summary-retry",
            "next-user",
            "continue",
            &model.model_hash,
            compacted.checkpoints.last().cloned(),
        ),
    )
    .await;
    assert!(
        matches!(after.end_stream, EndStream::Success {}),
        "{:?}",
        after.end_stream
    );
    assert_eq!(after.turn_ended, 1);
    let requests = provider.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[1], requests[2],
        "retry sees the exact same history and instructions"
    );
    assert!(!serde_json::to_string(&requests[3].history)
        .unwrap()
        .contains("UNCOMMITTED_SUMMARY"));
    assert!(serde_json::to_string(&requests[3].history)
        .unwrap()
        .contains("complete durable summary"));
}

#[tokio::test]
async fn post_auto_compaction_failure_resumes_from_persisted_checkpoint() {
    for summary_fails in [false, true] {
        let (_directory, store) = fixtures::temp_store().await;
        let model = store
            .create_model(&windowed_model("auto-summary-resume", Some(100_000)))
            .await
            .unwrap();
        let provider = fake_provider::FakeProvider::default();
        provider.push(text_response(&"h".repeat(400_000), 100_000, 20));
        if summary_fails {
            provider.push_results(rejected_after_text("UNCOMMITTED_AUTO_SUMMARY"));
        } else {
            provider.push(text_response("durable automatic summary", 90_000, 20));
        }
        provider.push_results(rejected_after_text("saved post-summary progress"));
        provider.push(text_response("continued after recovery", 500, 10));
        let registry = new_registry(store.clone(), &provider);
        let first = run(
            &registry,
            "auto-initial",
            user_request(
                "auto-recovery",
                "first-user",
                "start",
                &model.model_hash,
                None,
            ),
        )
        .await;
        let failed = run(
            &registry,
            "auto-failure",
            user_request(
                "auto-recovery",
                "current-user",
                "continue",
                &model.model_hash,
                first.checkpoints.last().cloned(),
            ),
        )
        .await;
        assert_eq!(failed.summary_started, 1);
        assert_eq!(failed.summary_completed, 1);
        assert!(
            matches!(&failed.end_stream, EndStream::Failure { error }
                if !error.code.is_empty() && error.message.contains("401")),
            "{:?}",
            failed.end_stream
        );
        let state = failed.checkpoints.last().unwrap().clone();
        assert_eq!(state.self_summary_count, 1);
        let stored = store
            .load_current_messages(&ConversationId::new("auto-recovery"))
            .await
            .unwrap();
        assert!(serde_json::to_string(&stored)
            .unwrap()
            .contains("saved post-summary progress"));
        assert!(!serde_json::to_string(&stored)
            .unwrap()
            .contains("UNCOMMITTED_AUTO_SUMMARY"));
        // New transport/conversation registries must recover from SQLite and the
        // emitted Cursor checkpoint, without relying on the old runtime owners.
        let restarted = new_registry(store, &provider);
        let resumed = run(
            &restarted,
            "auto-resume",
            request(
                "auto-recovery",
                &model.model_hash,
                Some(state),
                pb::conversation_action::Action::ResumeAction(pb::ResumeAction::default()),
            ),
        )
        .await;
        assert!(
            matches!(resumed.end_stream, EndStream::Success {}),
            "{:?}",
            resumed.end_stream
        );
        assert_eq!(resumed.summary_started, 0);
        assert_eq!(resumed.turn_ended, 1);
        assert_eq!(resumed.checkpoints.last().unwrap().self_summary_count, 1);
        let requests = provider.requests();
        assert_eq!(
            requests.len(),
            4,
            "recovery must not summarize again or replay the failed request"
        );
        assert!(requests[3].history.starts_with(&requests[2].history));
        assert_eq!(requests[3].prompt, requests[2].prompt);
        assert_eq!(
            requests[3]
                .history
                .iter()
                .filter(|message| message.message_id.starts_with("request-context:"))
                .count(),
            1
        );
        assert!(serde_json::to_string(&requests[3].history)
            .unwrap()
            .contains("saved post-summary progress"));
    }
}

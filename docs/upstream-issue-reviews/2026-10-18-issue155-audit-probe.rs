// Append to a temporary copy of server/tests/conversation_delivery.rs; uses its protocol driver and FakeProvider.

#[tokio::test]
async fn audit_delayed_completion_after_compaction_keeps_current_summary() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("before-summary", "WAITING_FOR_BACKGROUND"));
    provider.push(stop_response("summary", "LATEST_COMPACTED_SUMMARY"));
    provider.push(stop_response("after-summary", "COMPLETED"));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let mut initial = shell_completion_run(Default::default());
    if let Some(pb::agent_client_message::Message::RunRequest(request)) = initial.message.as_mut() {
        request.action = Some(pb::ConversationAction {
            action: Some(pb::conversation_action::Action::UserMessageAction(
                pb::UserMessageAction {
                    user_message: Some(pb::UserMessage {
                        text: "Start a background task and continue after it finishes.".into(),
                        message_id: "audit-initial-user".into(),
                        mode: pb::AgentMode::Agent as i32,
                        ..Default::default()
                    }),
                    request_context: Some(pb::RequestContext::default()),
                    ..Default::default()
                },
            )),
            ..Default::default()
        });
    }
    let handle = registry.get_or_create("audit-initial").await.unwrap();
    let (before_compaction, _) = drive_completion(&handle, initial).await;
    let mut summarize = shell_completion_run(before_compaction.clone());
    if let Some(pb::agent_client_message::Message::RunRequest(request)) = summarize.message.as_mut()
    {
        request.action = Some(pb::ConversationAction {
            action: Some(pb::conversation_action::Action::UserMessageAction(
                pb::UserMessageAction {
                    user_message: Some(pb::UserMessage {
                        text: "/summarize".into(),
                        message_id: "audit-summary-user".into(),
                        mode: pb::AgentMode::Agent as i32,
                        ..Default::default()
                    }),
                    request_context: Some(pb::RequestContext::default()),
                    ..Default::default()
                },
            )),
            ..Default::default()
        });
    }
    let handle = registry.get_or_create("audit-summary").await.unwrap();
    let (compacted, _) = drive_completion(&handle, summarize).await;
    assert_eq!(compacted.self_summary_count, 1);
    let handle = registry
        .get_or_create("audit-delayed-completion")
        .await
        .unwrap();
    let (final_state, _) = drive_completion(&handle, shell_completion_run(before_compaction)).await;
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    let resumed = serde_json::to_string(&requests[2].history).unwrap();
    println!(
        "audit summary_retained={} summary_count={} old_waiting_retained={}",
        resumed.contains("LATEST_COMPACTED_SUMMARY"),
        final_state.self_summary_count,
        resumed.contains("WAITING_FOR_BACKGROUND")
    );
    assert!(
        resumed.contains("LATEST_COMPACTED_SUMMARY"),
        "late notification must not undo committed compaction"
    );
    assert_eq!(final_state.self_summary_count, 1);
    registry.shutdown().await;
}

#[tokio::test]
async fn audit_rebatched_completion_does_not_wake_model_twice() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("batch", "Processed A and B."));
    provider.push(stop_response("unexpected-retry", "Processed A again."));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let mut batch = completion_run("task-A", "batch-run", Default::default());
    let other = completion_run("task-B", "unused", Default::default());
    let Some(pb::agent_client_message::Message::RunRequest(other)) = other.message else {
        unreachable!()
    };
    let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(other)) =
        other.action.unwrap().action
    else {
        unreachable!()
    };
    if let Some(pb::agent_client_message::Message::RunRequest(request)) = batch.message.as_mut() {
        if let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(action)) =
            request.action.as_mut().unwrap().action.as_mut()
        {
            action.completions.extend(other.completions);
        }
    }
    let handle = registry.get_or_create("audit-batch").await.unwrap();
    let (checkpoint, _) = drive_completion(&handle, batch).await;
    let handle = registry
        .get_or_create("audit-batch-partial-retry")
        .await
        .unwrap();
    // Capture both a correct acknowledgement and the erroneous extra model
    // turn without assuming which one the implementation will produce.
    drive_completion_end(
        &handle,
        completion_run("task-A", "retry-A", checkpoint),
        false,
    )
    .await;
    println!(
        "audit model_calls_after_batch_and_partial_retry={}",
        provider.requests().len()
    );
    assert_eq!(
        provider.requests().len(),
        1,
        "a previously completed task remains consumed when the retry batch changes"
    );
    registry.shutdown().await;
}

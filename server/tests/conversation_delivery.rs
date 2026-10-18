//! Verifies message delivery before, during, and after a Run.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{collections::HashMap, sync::Arc};

use cursor_server::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::connect,
        protocol::proto::agent::v1 as pb,
        TransportCommand, TransportHandle, TransportRegistry,
    },
    model::{ContentPart, MessageContent, ProjectedContent, Role},
    provider::{FinishReason, ModelEvent},
};
use prost::Message;

const FOLLOW_UP: &str = "Perform any necessary follow-up actions in response to the subagent completion above. If no follow-up work is needed, no further action is required. If you mention an agent or subagent in your response, link it with the `[Name](id)` Don't use generic label such as `[agent]`, `[worker]`, or `[subagent]`.";
const SHELL_FOLLOW_UP: &str = "Briefly inform the user about the task result and perform any follow-up actions (if needed). If there's no follow-ups needed, don't explicitly say that.";

#[tokio::test]
async fn first_background_completion_preserves_imported_history() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("imported-wakeup", "COMPLETED"));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    // The command ran upstream. BYOK first sees the conversation when Cursor
    // sends its completed-task notification with the original checkpoint.
    let roots = [
        serde_json::json!({"role":"system","content":"upstream instructions"}),
        serde_json::json!({"role":"user","content":"Run once, then say COMPLETED."}),
        serde_json::json!({"role":"assistant","content":[{"type":"text","text":"STARTED"}]}),
    ];
    let mut state = pb::ConversationStateStructure::default();
    for root in roots {
        let id = store
            .put_blob(&serde_json::to_vec(&root).unwrap(), &[])
            .await
            .unwrap();
        state.root_prompt_messages_json.push(id.as_bytes().to_vec());
    }
    let original_roots = state.root_prompt_messages_json.clone();
    let handle = registry.get_or_create("imported-completion").await.unwrap();
    let (checkpoint, _) = drive_completion(&handle, shell_completion_run(state)).await;
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let history = serde_json::to_string(&requests[0].history).unwrap();
    assert!(history.contains("Run once, then say COMPLETED."));
    assert!(history.contains("STARTED"));
    assert!(history.contains("<system_notification>"));
    assert_eq!(
        &checkpoint.root_prompt_messages_json[1..3],
        &original_roots[1..]
    );
    let handle = registry
        .get_or_create("imported-completion-retry")
        .await
        .unwrap();
    drive_forwarded_completion(&handle, shell_completion_run(checkpoint)).await;
    assert_eq!(provider.requests().len(), 1);
    registry.shutdown().await;
}

#[tokio::test]
async fn background_completion_preserves_history_newer_than_the_local_head() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("old-local-turn", "Earlier local response."));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry
        .get_or_create("earlier-local-completion")
        .await
        .unwrap();
    let (mut state, _) = drive_completion(
        &handle,
        completion_run("earlier-task", "earlier-run", Default::default()),
    )
    .await;
    // Cursor can add another turn upstream after the most recent local Run.
    for root in [
        serde_json::json!({"role":"user","content":"New upstream command."}),
        serde_json::json!({"role":"assistant","content":[{"type":"text","text":"NEW_STARTED"}]}),
    ] {
        let id = store
            .put_blob(&serde_json::to_vec(&root).unwrap(), &[])
            .await
            .unwrap();
        state.root_prompt_messages_json.push(id.as_bytes().to_vec());
    }
    let original_roots = state.root_prompt_messages_json.clone();
    provider.push(stop_response("new-wakeup", "NEW_COMPLETED"));
    let handle = registry
        .get_or_create("newer-imported-completion")
        .await
        .unwrap();
    let (checkpoint, _) = drive_completion(&handle, shell_completion_run(state)).await;
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].history.starts_with(&requests[0].history));
    let history = serde_json::to_string(&requests[1].history).unwrap();
    assert!(history.contains("Earlier local response."));
    assert!(history.contains("New upstream command."));
    assert!(history.contains("NEW_STARTED"));
    assert!(checkpoint
        .root_prompt_messages_json
        .starts_with(&original_roots));
    registry.shutdown().await;
}

#[tokio::test]
async fn background_subagent_completion_starts_a_simulated_parent_turn() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("model-call", "followed up"));
    let assets = PromptAssets::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("prompt/cursor")
            .as_path(),
    )
    .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let handle = registry.get_or_create("completion-request").await.unwrap();
    let (checkpoint, blobs) = drive_completion(
        &handle,
        completion_run(
            "child-id",
            "reusable-parent-run",
            pb::ConversationStateStructure {
                mode: Some(pb::AgentMode::Multitask as i32),
                ..Default::default()
            },
        ),
    )
    .await;

    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let [runtime] = requests[0].history.as_slice() else {
        panic!("completion Run must add exactly one runtime message")
    };
    assert_eq!(runtime.role, Role::User);
    let ProjectedContent::Parts(parts) = &runtime.content else {
        panic!("completion context must be text")
    };
    let [ContentPart::Text { text }] = parts.as_slice() else {
        panic!("completion context must have one text part")
    };
    assert!(text.contains("kind: subagent"));
    assert!(text.contains("agent_id: child-id"));
    assert!(text.contains("child result"));
    assert!(text.contains(FOLLOW_UP));

    let messages = store
        .load_current_messages(&cursor_server::model::ConversationId::new(
            "parent-conversation",
        ))
        .await
        .unwrap();
    assert!(messages.iter().any(|message| {
        message.runtime_event_id.as_deref()
            == Some(r#"background-completed:["BACKGROUND_TASK_KIND_SUBAGENT","child-id","child-id","task-call"]"#)
            && matches!(&message.content, MessageContent::Parts { parts } if !parts.is_empty())
    }));

    let turn = pb::ConversationTurnStructure::decode(
        blobs
            .get(checkpoint.turns.last().expect("completion Turn"))
            .expect("completion Turn Blob")
            .as_slice(),
    )
    .unwrap();
    let pb::conversation_turn_structure::Turn::AgentConversationTurn(turn) = turn.turn.unwrap()
    else {
        panic!("expected agent conversation Turn")
    };
    let user = pb::UserMessage::decode(
        blobs
            .get(&turn.user_message)
            .expect("simulated UserMessage Blob")
            .as_slice(),
    )
    .unwrap();
    assert!(user.text.contains(FOLLOW_UP));
    assert_eq!(user.is_simulated_msg, Some(true));
    assert_eq!(
        user.simulated_msg_reason,
        Some(pb::SimulatedMsgReason::BackgroundTaskCompletion as i32)
    );
    assert_eq!(
        user.simulated_message_metadata.unwrap().task_id.as_deref(),
        Some("child-id")
    );

    provider.push(stop_response("model-call-2", "followed up again"));
    let second = registry
        .get_or_create("completion-request-2")
        .await
        .unwrap();
    drive_completion(
        &second,
        completion_run("child-id-2", "reusable-parent-run-2", checkpoint),
    )
    .await;

    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    let runtime_ids = requests[1]
        .history
        .iter()
        .map(|message| message.message_id.as_str())
        .filter(|id| id.starts_with("runtime:"))
        .collect::<Vec<_>>();
    assert_eq!(
        runtime_ids,
        [
            r#"runtime:background-completed:["BACKGROUND_TASK_KIND_SUBAGENT","child-id","child-id","task-call"]"#,
            r#"runtime:background-completed:["BACKGROUND_TASK_KIND_SUBAGENT","child-id-2","child-id-2","task-call"]"#
        ]
    );
}

#[tokio::test]
async fn background_completion_joins_the_active_run_instead_of_replacing_it() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let first_ready = provider.push_gated(stop_response("model-call-1", "first response"));
    provider.push(stop_response("model-call-2", "processed both completions"));
    let assets = PromptAssets::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("prompt/cursor")
            .as_path(),
    )
    .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let first = registry.get_or_create("active-completion-1").await.unwrap();
    let first_run = tokio::spawn(async move {
        drive_completion(
            &first,
            completion_run(
                "child-1",
                "parent-run-1",
                pb::ConversationStateStructure::default(),
            ),
        )
        .await
    });
    while provider.requests().is_empty() {
        tokio::task::yield_now().await;
    }

    let second = registry.get_or_create("active-completion-2").await.unwrap();
    let second_run = tokio::spawn(async move {
        drive_forwarded_completion(
            &second,
            completion_run(
                "child-2",
                "parent-run-2",
                pb::ConversationStateStructure::default(),
            ),
        )
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    first_ready.notify_one();

    second_run.await.unwrap();
    first_run.await.unwrap();
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    let history = serde_json::to_string(&requests[1].history).unwrap();
    assert!(history.contains("child-1"));
    assert!(history.contains("first response"));
    assert!(history.contains("child-2"));
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT status FROM runs WHERE conversation_id = 'parent-conversation' ORDER BY created_at_ms",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(statuses, ["completed"]);
}

#[tokio::test]
async fn retrying_one_background_completion_does_not_call_the_model_again() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("model-call", "followed up"));
    let assets = PromptAssets::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("prompt/cursor")
            .as_path(),
    )
    .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let first = registry.get_or_create("completion-retry-1").await.unwrap();
    let (checkpoint, _) = drive_completion(
        &first,
        completion_run(
            "retry-child",
            "completion-retry-run-1",
            pb::ConversationStateStructure {
                mode: Some(pb::AgentMode::Multitask as i32),
                ..Default::default()
            },
        ),
    )
    .await;

    provider.push(stop_response("model-call-2", "followed up again"));
    let second = registry.get_or_create("completion-retry-2").await.unwrap();
    drive_forwarded_completion(
        &second,
        completion_run("retry-child", "completion-retry-run-2", checkpoint.clone()),
    )
    .await;
    assert_eq!(
        provider.requests().len(),
        1,
        "duplicate completion must not wake the model"
    );

    // Recreate all in-memory coordination while retaining the same durable store.
    registry.shutdown().await;
    let restarted = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let third = restarted.get_or_create("completion-retry-3").await.unwrap();
    drive_forwarded_completion(
        &third,
        completion_run(
            "retry-child",
            "completion-retry-run-3",
            pb::ConversationStateStructure::default(),
        ),
    )
    .await;
    assert_eq!(
        provider.requests().len(),
        1,
        "restart must not forget consumed completions"
    );

    // Explicit Resume is not a completion redelivery and still invokes the model.
    let mut resume = completion_run("retry-child", "explicit-resume", checkpoint);
    let Some(pb::agent_client_message::Message::RunRequest(request)) = resume.message.as_mut()
    else {
        unreachable!()
    };
    request.action = Some(pb::ConversationAction {
        action: Some(pb::conversation_action::Action::ResumeAction(
            pb::ResumeAction::default(),
        )),
        ..Default::default()
    });
    let handle = restarted.get_or_create("explicit-resume").await.unwrap();
    drive_completion(&handle, resume).await;
    assert_eq!(
        provider.requests().len(),
        2,
        "explicit Resume must remain available"
    );

    let messages = store
        .load_current_messages(&cursor_server::model::ConversationId::new(
            "parent-conversation",
        ))
        .await
        .unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| {
                message.runtime_event_id.as_deref()
                    == Some(
                        r#"background-completed:["BACKGROUND_TASK_KIND_SUBAGENT","retry-child","retry-child","task-call"]"#,
                    )
            })
            .count(),
        1
    );
}

#[tokio::test]
async fn issue300_successful_shell_notification_finishes_without_polling_delay() {
    for with_tool_id in [true, false] {
        let (_directory, store) = fixtures::temp_store().await;
        let provider = fake_provider::FakeProvider::default();
        provider.push(stop_response("issue300-response", "ISSUE300_CONTINUED"));
        let assets = PromptAssets::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"),
        )
        .unwrap();
        let registry = TransportRegistry::new(
            store,
            Arc::new(provider.clone()),
            PromptCompiler::new(assets),
        );
        let handle = registry
            .get_or_create("issue300-shell-finished")
            .await
            .unwrap();
        let mut message = shell_completion_run(pb::ConversationStateStructure {
            mode: Some(pb::AgentMode::Agent as i32),
            ..Default::default()
        });
        let Some(pb::agent_client_message::Message::RunRequest(request)) = message.message.as_mut()
        else {
            unreachable!()
        };
        let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(action)) =
            request.action.as_mut().unwrap().action.as_mut()
        else {
            unreachable!()
        };
        action.completions[0].status = pb::BackgroundTaskStatus::Success as i32;
        action.completions[0].title = "Issue 300 timed shell".into();
        action.completions[0].detail = Some("ISSUE300_SHELL_DONE exit_code=0".into());
        if !with_tool_id {
            action.completions[0].tool_call_id = None;
        }
        let started_at = std::time::Instant::now();
        let delivery = tokio::spawn(async move { drive_completion(&handle, message).await });
        let resumed_after = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while provider.requests().is_empty() {
                tokio::task::yield_now().await;
            }
            started_at.elapsed()
        })
        .await
        .expect("Shell completion must resume the model without polling delay");
        // Transport completion includes a separate 2s continuation idle window.
        let (checkpoint, _) = tokio::time::timeout(std::time::Duration::from_secs(4), delivery)
            .await
            .expect("transport must finish after its continuation idle window")
            .unwrap();
        let elapsed = started_at.elapsed();
        let requests = provider.requests();
        assert_eq!(requests.len(), 1);
        let ProjectedContent::Parts(parts) = &requests[0].history[0].content else {
            panic!("expected completion notification")
        };
        assert!(parts
            .iter()
            .any(|part| matches!(part, ContentPart::Text { text }
            if text.contains("status: success") && text.contains("ISSUE300_SHELL_DONE"))));
        assert!(!checkpoint.turns.is_empty());
        println!(
            "issue300 successful Shell completion with_tool_id={with_tool_id} model_resumed_ms={} transport_ended_ms={}",
            resumed_after.as_millis(),
            elapsed.as_millis()
        );
    }
}

#[tokio::test]
async fn background_progress_notification_is_ignored_without_model_resume() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let mut message = shell_completion_run(pb::ConversationStateStructure {
        mode: Some(pb::AgentMode::Agent as i32),
        ..Default::default()
    });
    let Some(pb::agent_client_message::Message::RunRequest(request)) = message.message.as_mut()
    else {
        unreachable!()
    };
    let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(action)) =
        request.action.as_mut().unwrap().action.as_mut()
    else {
        unreachable!()
    };
    action.completions[0].reason = pb::BackgroundTaskCompletionReason::TaskProgress as i32;
    action.completions[0].detail = Some("output matched PROBE_DONE".into());
    let handle = registry.get_or_create("progress-only").await.unwrap();

    let acknowledgement = drive_completion_end(&handle, message, false).await;
    assert_eq!(
        acknowledgement.len(),
        1,
        "pure progress must only send TurnEnded, without a new checkpoint or user message"
    );

    assert!(provider.requests().is_empty());
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM runs ORDER BY created_at_ms")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert!(
        statuses.is_empty(),
        "ignored progress must not create a run"
    );
    let runtime_messages: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE message_id LIKE 'runtime:background-completed:%'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(runtime_messages, 0);

    // Acknowledging progress must not consume the later finish for the same task.
    provider.push(stop_response("after-progress", "Task has finished."));
    let finished = registry
        .get_or_create("finished-after-progress")
        .await
        .unwrap();
    drive_completion(&finished, shell_completion_run(Default::default())).await;
    assert_eq!(provider.requests().len(), 1);
    registry.shutdown().await;
}

#[tokio::test]
async fn background_shell_completion_wakes_the_parent_with_the_captured_notification() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response(
        "shell-wakeup",
        "The background server was stopped.",
    ));
    let assets = PromptAssets::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("prompt/cursor")
            .as_path(),
    )
    .unwrap();
    let registry = TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let handle = registry
        .get_or_create("shell-completion-request")
        .await
        .unwrap();
    let (checkpoint, blobs) = drive_completion(
        &handle,
        shell_completion_run(pb::ConversationStateStructure {
            mode: Some(pb::AgentMode::Agent as i32),
            ..Default::default()
        }),
    )
    .await;

    let requests = provider.requests();
    let [runtime] = requests[0].history.as_slice() else {
        panic!("Shell completion Run must add exactly one runtime message")
    };
    let ProjectedContent::Parts(parts) = &runtime.content else {
        panic!("Shell completion context must be text")
    };
    let [ContentPart::Text { text }] = parts.as_slice() else {
        panic!("Shell completion context must have one text part")
    };
    assert!(text.contains("<system_notification>"));
    assert!(text.contains("kind: shell"));
    assert!(text.contains("status: aborted"));
    assert!(text.contains("task_id: 977679"));
    assert!(text.contains("detail: terminated_by_user"));
    assert!(text.contains("output_path: /tmp/977679.txt"));
    assert!(text.contains(SHELL_FOLLOW_UP));
    assert!(text.starts_with("<timestamp>"));
    assert!(!text.contains("You are still in **Agent Mode**"));
    assert!(text.find("<system_notification>").unwrap() < text.find("<user_query>").unwrap());

    let turn = pb::ConversationTurnStructure::decode(
        blobs
            .get(checkpoint.turns.last().expect("Shell completion Turn"))
            .expect("Shell completion Turn Blob")
            .as_slice(),
    )
    .unwrap();
    let pb::conversation_turn_structure::Turn::AgentConversationTurn(turn) = turn.turn.unwrap()
    else {
        panic!("expected agent conversation Turn")
    };
    let user = pb::UserMessage::decode(
        blobs
            .get(&turn.user_message)
            .expect("simulated Shell UserMessage Blob")
            .as_slice(),
    )
    .unwrap();
    assert_eq!(user.text, *text);
    assert_eq!(user.is_simulated_msg, Some(true));
    assert_eq!(
        user.simulated_msg_reason,
        Some(pb::SimulatedMsgReason::BackgroundTaskCompletion as i32)
    );
    let metadata = user.simulated_message_metadata.unwrap();
    assert_eq!(
        metadata.title.as_deref(),
        Some("Start Python HTTP server on 9000")
    );
    assert_eq!(metadata.task_id.as_deref(), Some("977679"));
}

async fn drive_completion(
    handle: &TransportHandle,
    message: pb::AgentClientMessage,
) -> (pb::ConversationStateStructure, HashMap<Vec<u8>, Vec<u8>>) {
    let mut output = handle.subscribe();
    handle
        .command(TransportCommand::Append {
            seqno: 0,
            message: Box::new(message),
        })
        .await
        .unwrap();

    let mut append_seqno = 1;
    let mut blobs = HashMap::new();
    let mut final_checkpoint = None;
    loop {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), output.recv())
            .await
            .unwrap()
            .unwrap();
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            let terminal: serde_json::Value = serde_json::from_slice(&payload).unwrap();
            assert!(terminal.get("error").is_none(), "{terminal}");
            break;
        }
        let server = pb::AgentServerMessage::decode(payload).unwrap();
        match server.message {
            Some(pb::agent_server_message::Message::ExecServerMessage(exec)) => {
                assert_eq!(exec.id, 0);
                assert!(matches!(
                    exec.message,
                    Some(pb::exec_server_message::Message::RequestContextArgs(_))
                ));
                handle
                    .command(TransportCommand::Append {
                        seqno: append_seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(
                                pb::agent_client_message::Message::ExecClientControlMessage(
                                    pb::ExecClientControlMessage {
                                        message: Some(
                                            pb::exec_client_control_message::Message::StreamClose(
                                                pb::ExecClientStreamClose { id: 0 },
                                            ),
                                        ),
                                    },
                                ),
                            ),
                        }),
                    })
                    .await
                    .unwrap();
                append_seqno += 1;
                handle
                    .command(TransportCommand::Append {
                        seqno: append_seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(pb::agent_client_message::Message::ExecClientMessage(
                                pb::ExecClientMessage {
                                    id: 0,
                                    message: Some(
                                        pb::exec_client_message::Message::RequestContextResult(
                                            pb::RequestContextResult {
                                                result: Some(
                                                    pb::request_context_result::Result::Success(
                                                        pb::RequestContextSuccess {
                                                            request_context: Some(
                                                                pb::RequestContext::default(),
                                                            ),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                ),
                                            },
                                        ),
                                    ),
                                    ..Default::default()
                                },
                            )),
                        }),
                    })
                    .await
                    .unwrap();
                append_seqno += 1;
            }
            Some(pb::agent_server_message::Message::KvServerMessage(kv)) => {
                if let Some(pb::kv_server_message::Message::SetBlobArgs(set)) = &kv.message {
                    blobs.insert(set.blob_id.clone(), set.blob_data.clone());
                }
                handle
                    .command(TransportCommand::Append {
                        seqno: append_seqno,
                        message: Box::new(kv_ack(kv.id)),
                    })
                    .await
                    .unwrap();
                append_seqno += 1;
            }
            Some(pb::agent_server_message::Message::ConversationCheckpointUpdate(state))
                if state.pending_tool_calls.is_empty() =>
            {
                final_checkpoint = Some(state);
            }
            _ => {}
        }
    }
    (
        final_checkpoint.expect("settled completion checkpoint"),
        blobs,
    )
}

async fn drive_forwarded_completion(handle: &TransportHandle, message: pb::AgentClientMessage) {
    drive_completion_end(handle, message, false).await;
}

async fn drive_completion_end(
    handle: &TransportHandle,
    message: pb::AgentClientMessage,
    cancelled: bool,
) -> Vec<pb::AgentServerMessage> {
    let mut output = handle.subscribe();
    handle
        .command(TransportCommand::Append {
            seqno: 0,
            message: Box::new(message),
        })
        .await
        .unwrap();
    let mut append_seqno = 1;
    let mut turn_ended_count = 0;
    let mut messages = Vec::new();
    loop {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), output.recv())
            .await
            .unwrap()
            .unwrap();
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            if cancelled {
                let error: serde_json::Value = serde_json::from_slice(&payload).unwrap();
                assert_eq!(error["error"]["code"], "canceled");
            } else {
                assert_eq!(payload.as_ref(), b"{}");
                assert_eq!(
                    turn_ended_count, 1,
                    "Cursor treats a successful stream without TurnEnded as a dropped connection"
                );
            }
            return messages;
        }
        let server = pb::AgentServerMessage::decode(payload).unwrap();
        messages.push(server.clone());
        match server.message {
            Some(pb::agent_server_message::Message::InteractionUpdate(update))
                if matches!(
                    update.message,
                    Some(pb::interaction_update::Message::TurnEnded(_))
                ) =>
            {
                turn_ended_count += 1;
            }
            Some(pb::agent_server_message::Message::ExecServerMessage(exec)) => {
                assert_eq!(exec.id, 0);
                handle
                    .command(TransportCommand::Append {
                        seqno: append_seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(
                                pb::agent_client_message::Message::ExecClientControlMessage(
                                    pb::ExecClientControlMessage {
                                        message: Some(
                                            pb::exec_client_control_message::Message::StreamClose(
                                                pb::ExecClientStreamClose { id: 0 },
                                            ),
                                        ),
                                    },
                                ),
                            ),
                        }),
                    })
                    .await
                    .unwrap();
                append_seqno += 1;
                handle
                    .command(TransportCommand::Append {
                        seqno: append_seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(pb::agent_client_message::Message::ExecClientMessage(
                                pb::ExecClientMessage {
                                    id: 0,
                                    message: Some(
                                        pb::exec_client_message::Message::RequestContextResult(
                                            pb::RequestContextResult {
                                                result: Some(
                                                    pb::request_context_result::Result::Success(
                                                        pb::RequestContextSuccess {
                                                            request_context: Some(
                                                                pb::RequestContext::default(),
                                                            ),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                ),
                                            },
                                        ),
                                    ),
                                    ..Default::default()
                                },
                            )),
                        }),
                    })
                    .await
                    .unwrap();
                append_seqno += 1;
            }
            Some(pb::agent_server_message::Message::KvServerMessage(kv)) => {
                handle
                    .command(TransportCommand::Append {
                        seqno: append_seqno,
                        message: Box::new(kv_ack(kv.id)),
                    })
                    .await
                    .unwrap();
                append_seqno += 1;
            }
            _ => {}
        }
    }
}

fn completion_run(
    child_id: &str,
    run_id: &str,
    conversation_state: pb::ConversationStateStructure,
) -> pb::AgentClientMessage {
    completion_run_with_detail(child_id, run_id, conversation_state, "child result")
}

fn completion_run_with_detail(
    child_id: &str,
    run_id: &str,
    conversation_state: pb::ConversationStateStructure,
    detail: &str,
) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::RunRequest(
            pb::AgentRunRequest {
                action: Some(pb::ConversationAction {
                    action: Some(
                        pb::conversation_action::Action::BackgroundTaskCompletionAction(
                            pb::BackgroundTaskCompletionAction {
                                completions: vec![pb::BackgroundTaskCompletion {
                                    task_id: child_id.into(),
                                    kind: pb::BackgroundTaskKind::Subagent as i32,
                                    status: pb::BackgroundTaskStatus::Success as i32,
                                    title: "Inspect protocol".into(),
                                    detail: Some(detail.into()),
                                    output_path: Some("/tmp/child.jsonl".into()),
                                    reason: pb::BackgroundTaskCompletionReason::TaskFinished as i32,
                                    subagent_id: Some(child_id.into()),
                                    tool_call_id: Some("task-call".into()),
                                    ..Default::default()
                                }],
                            },
                        ),
                    ),
                    ..Default::default()
                }),
                conversation_id: Some("parent-conversation".into()),
                requested_model: Some(pb::RequestedModel {
                    model_id: "test-model".into(),
                    ..Default::default()
                }),
                conversation_state: Some(conversation_state),
                run_id: Some(run_id.into()),
                ..Default::default()
            },
        )),
    }
}

fn shell_completion_run(
    conversation_state: pb::ConversationStateStructure,
) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::RunRequest(
            pb::AgentRunRequest {
                action: Some(pb::ConversationAction {
                    action: Some(
                        pb::conversation_action::Action::BackgroundTaskCompletionAction(
                            pb::BackgroundTaskCompletionAction {
                                completions: vec![pb::BackgroundTaskCompletion {
                                    task_id: "977679".into(),
                                    kind: pb::BackgroundTaskKind::Shell as i32,
                                    status: pb::BackgroundTaskStatus::Aborted as i32,
                                    title: "Start Python HTTP server on 9000".into(),
                                    detail: Some("terminated_by_user".into()),
                                    output_path: Some("/tmp/977679.txt".into()),
                                    reason: pb::BackgroundTaskCompletionReason::TaskFinished as i32,
                                    tool_call_id: Some("shell-call".into()),
                                    ..Default::default()
                                }],
                            },
                        ),
                    ),
                    ..Default::default()
                }),
                conversation_id: Some("parent-conversation".into()),
                requested_model: Some(pb::RequestedModel {
                    model_id: "test-model".into(),
                    ..Default::default()
                }),
                conversation_state: Some(conversation_state),
                run_id: Some("shell-parent-run".into()),
                ..Default::default()
            },
        )),
    }
}

fn stop_response(model_call_id: &str, text: &str) -> Vec<ModelEvent> {
    vec![
        ModelEvent::Start {
            model_call_id: model_call_id.into(),
        },
        ModelEvent::TextStart,
        ModelEvent::TextDelta(text.into()),
        ModelEvent::TextEnd,
        ModelEvent::Done(FinishReason::Stop),
    ]
}

fn kv_ack(id: u32) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::KvClientMessage(
            pb::KvClientMessage {
                id,
                message: Some(pb::kv_client_message::Message::SetBlobResult(
                    pb::SetBlobResult { error: None },
                )),
            },
        )),
    }
}

#[tokio::test]
async fn background_completion_without_tool_id_wakes_finished_parent() {
    assert_background_completion_wakes_finished_parent(false, None).await;
}

#[tokio::test]
async fn fast_background_completion_without_tool_id_wakes_finished_parent() {
    assert_background_completion_wakes_finished_parent(true, None).await;
}

#[tokio::test]
async fn distinct_background_child_turns_with_same_tool_id_wake_finished_parent() {
    assert_background_completion_wakes_finished_parent(false, Some("task-call")).await;
}

async fn assert_background_completion_wakes_finished_parent(
    fast: bool,
    tool_call_id: Option<&str>,
) {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let configure = |message: &mut pb::AgentClientMessage| {
        let Some(pb::agent_client_message::Message::RunRequest(request)) = message.message.as_mut()
        else {
            unreachable!()
        };
        request.requested_model.as_mut().unwrap().parameters =
            vec![pb::requested_model::ModelParameterValue {
                id: "fast".into(),
                value: fast.to_string(),
            }];
    };
    // The parent has already ended its turn before Cursor sends the completion.
    let mut start = completion_run(
        "unused",
        "parent-start",
        pb::ConversationStateStructure {
            mode: Some(pb::AgentMode::Multitask as i32),
            ..Default::default()
        },
    );
    let Some(pb::agent_client_message::Message::RunRequest(request)) = start.message.as_mut()
    else {
        unreachable!()
    };
    request.action = Some(pb::ConversationAction {
        action: Some(pb::conversation_action::Action::UserMessageAction(
            pb::UserMessageAction {
                user_message: Some(pb::UserMessage {
                    text: "Report the background child's result when it finishes.".into(),
                    message_id: "parent-user".into(),
                    mode: pb::AgentMode::Multitask as i32,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
        ..Default::default()
    });
    configure(&mut start);
    provider.push(stop_response(
        "parent-start-call",
        "Waiting for background child.",
    ));
    let handle = registry.get_or_create("parent-start").await.unwrap();
    let (mut checkpoint, _) = drive_completion(&handle, start).await;

    // A later completion from the same child is a new event, whether the
    // originating tool call is omitted or shared across child turns.
    for (index, task_id) in ["child-turn-1", "child-turn-2", "child-turn-2"]
        .iter()
        .enumerate()
    {
        let mut message = completion_run_with_detail(
            "same-child",
            &format!("wakeup-{index}"),
            checkpoint.clone(),
            &format!("result for {task_id}"),
        );
        let Some(pb::agent_client_message::Message::RunRequest(request)) = message.message.as_mut()
        else {
            unreachable!()
        };
        let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(action)) = request
            .action
            .as_mut()
            .and_then(|action| action.action.as_mut())
        else {
            unreachable!()
        };
        action.completions[0].task_id = (*task_id).into();
        action.completions[0].tool_call_id = tool_call_id.map(str::to_owned);
        configure(&mut message);
        provider.push(stop_response(
            &format!("wakeup-call-{index}"),
            &format!("Received {task_id}"),
        ));
        let handle = registry
            .get_or_create(&format!("wakeup-{index}"))
            .await
            .unwrap();
        if index == 2 {
            drive_forwarded_completion(&handle, message).await;
        } else {
            (checkpoint, _) = drive_completion(&handle, message).await;
        }
    }
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|request| (request.model.latency
        == cursor_server::model::ModelLatency::Fast)
        == fast));
    for pair in requests.windows(2) {
        assert!(pair[1].history.starts_with(&pair[0].history));
    }
    let second_history = &requests[2].history;
    let history = serde_json::to_string(second_history).unwrap();
    assert!(history.contains("Waiting for background child."));
    assert!(history.contains("result for child-turn-1"));
    assert!(history.contains("result for child-turn-2"));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE message_id LIKE 'runtime:background-completed:%'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        count, 2,
        "distinct child turns stay distinct, one retry is idempotent"
    );
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT status FROM runs ORDER BY created_at_ms")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(statuses, vec!["completed"; 3]);
    registry.shutdown().await;
}

#[tokio::test]
async fn simultaneous_shell_completion_requests_start_only_one_run() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(stop_response("first", "shell completed"));
    provider.push(stop_response("unexpected", "duplicate"));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let first = registry.get_or_create("shell-duplicate-1").await.unwrap();
    let second = registry.get_or_create("shell-duplicate-2").await.unwrap();
    tokio::join!(
        drive_forwarded_completion(&first, shell_completion_run(Default::default())),
        drive_forwarded_completion(&second, shell_completion_run(Default::default())),
    );
    assert_eq!(provider.requests().len(), 1);
    let before = store
        .load_current_messages(&cursor_server::model::ConversationId::new(
            "parent-conversation",
        ))
        .await
        .unwrap();
    let third = registry.get_or_create("shell-duplicate-3").await.unwrap();
    drive_forwarded_completion(&third, shell_completion_run(Default::default())).await;
    assert_eq!(provider.requests().len(), 1);
    let after = store
        .load_current_messages(&cursor_server::model::ConversationId::new(
            "parent-conversation",
        ))
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "stale duplicate checkpoint must not replace current history"
    );
}

#[tokio::test]
async fn cancelled_completion_is_not_restarted_by_automatic_redelivery() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push_pending();
    provider.push(stop_response("unwanted-restart", "unexpected"));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let first = registry.get_or_create("cancel-completion-1").await.unwrap();
    let client = first.clone();
    let pending = tokio::spawn(async move {
        drive_completion_end(&client, shell_completion_run(Default::default()), true).await;
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while provider.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    first.command(TransportCommand::Disconnect).await.unwrap();
    pending.await.unwrap();
    registry.shutdown().await;
    let restarted = TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let retry = restarted
        .get_or_create("cancel-completion-2")
        .await
        .unwrap();
    drive_forwarded_completion(&retry, shell_completion_run(Default::default())).await;
    assert_eq!(
        provider.requests().len(),
        1,
        "automatic notification must not undo cancellation"
    );
}

#[tokio::test]
async fn delayed_completion_after_compaction_keeps_current_summary() {
    assert_delayed_completion_after_compaction(false).await;
}

#[tokio::test]
async fn delayed_completion_after_restart_keeps_current_summary() {
    assert_delayed_completion_after_compaction(true).await;
}

async fn assert_delayed_completion_after_compaction(restart: bool) {
    let (directory, store) = fixtures::temp_store().await;
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
    let registry = if restart {
        registry.shutdown().await;
        store.pool().close().await;
        let reopened = cursor_server::store::Store::connect(&format!(
            "sqlite://{}",
            directory.path().join("test.db").display()
        ))
        .await
        .unwrap();
        TransportRegistry::new(
            reopened,
            Arc::new(provider.clone()),
            PromptCompiler::new(PromptAssets::embedded().unwrap()),
        )
    } else {
        registry
    };
    let handle = registry
        .get_or_create("audit-delayed-completion")
        .await
        .unwrap();
    let (final_state, _) = drive_completion(&handle, shell_completion_run(before_compaction)).await;
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    let resumed = serde_json::to_string(&requests[2].history).unwrap();
    assert!(
        resumed.contains("LATEST_COMPACTED_SUMMARY"),
        "late notification must not undo committed compaction"
    );
    assert_eq!(final_state.self_summary_count, 1);
    assert!(!resumed.contains("WAITING_FOR_BACKGROUND"));
    assert_eq!(final_state.summary, compacted.summary);
    assert_eq!(final_state.summary_archives, compacted.summary_archives);
    assert!(final_state
        .root_prompt_messages_json
        .starts_with(&compacted.root_prompt_messages_json));
    assert!(final_state.turns.starts_with(&compacted.turns));
    registry.shutdown().await;
}

#[tokio::test]
async fn rebatched_completion_does_not_wake_model_twice() {
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
    assert_eq!(
        provider.requests().len(),
        1,
        "a previously completed task remains consumed when the retry batch changes"
    );
    registry.shutdown().await;
}

fn completion_batch(ids: &[&str], state: pb::ConversationStateStructure) -> pb::AgentClientMessage {
    let mut message = completion_run(ids[0], "batch", state);
    let Some(pb::agent_client_message::Message::RunRequest(request)) = message.message.as_mut()
    else {
        unreachable!()
    };
    let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(action)) =
        request.action.as_mut().unwrap().action.as_mut()
    else {
        unreachable!()
    };
    for id in &ids[1..] {
        let other = completion_run(id, "batch", Default::default());
        let Some(pb::agent_client_message::Message::RunRequest(other)) = other.message else {
            unreachable!()
        };
        let Some(pb::conversation_action::Action::BackgroundTaskCompletionAction(other)) =
            other.action.unwrap().action
        else {
            unreachable!()
        };
        action.completions.extend(other.completions);
    }
    message
}

#[tokio::test]
async fn overlapping_completion_batches_after_restart_only_deliver_new_tasks() {
    for (first, next, all) in [
        (
            vec!["task-A", "task-B"],
            vec!["task-B", "task-C"],
            vec!["task-C", "task-A", "task-B"],
        ),
        (
            vec!["task-A"],
            vec!["task-A", "task-B"],
            vec!["task-B", "task-A"],
        ),
    ] {
        let (directory, store) = fixtures::temp_store().await;
        let provider = fake_provider::FakeProvider::default();
        provider.push(stop_response("first", "First batch finished."));
        provider.push(stop_response("next", "New task finished."));
        provider.push(stop_response("unexpected", "Unexpected duplicate."));
        let registry = TransportRegistry::new(
            store.clone(),
            Arc::new(provider.clone()),
            PromptCompiler::new(PromptAssets::embedded().unwrap()),
        );
        let handle = registry.get_or_create("overlap-first").await.unwrap();
        let (state, _) =
            drive_completion(&handle, completion_batch(&first, Default::default())).await;
        registry.shutdown().await;
        store.pool().close().await;
        let store = cursor_server::store::Store::connect(&format!(
            "sqlite://{}",
            directory.path().join("test.db").display()
        ))
        .await
        .unwrap();
        let registry = TransportRegistry::new(
            store.clone(),
            Arc::new(provider.clone()),
            PromptCompiler::new(PromptAssets::embedded().unwrap()),
        );
        let handle = registry.get_or_create("overlap-next").await.unwrap();
        let (state, _) = drive_completion(&handle, completion_batch(&next, state)).await;
        let handle = registry.get_or_create("overlap-retry").await.unwrap();
        drive_completion_end(&handle, completion_batch(&all, state), false).await;
        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].history.starts_with(&requests[0].history));
        let history = serde_json::to_string(&requests[1].history).unwrap();
        for id in &all {
            assert_eq!(
                history.matches(&format!("task_id: {id}")).count(),
                1,
                "{id} delivered twice"
            );
        }
        let persisted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM messages WHERE message_id LIKE 'runtime:background-completed:%'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(persisted as usize, all.len());
        registry.shutdown().await;
    }
}

#[tokio::test]
async fn overlapping_batch_joins_active_run_without_repeating_the_shared_task() {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let ready = provider.push_gated(stop_response("first", "First batch handled."));
    provider.push(stop_response("next", "New task handled."));
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry.get_or_create("active-batch-first").await.unwrap();
    let running = tokio::spawn(async move {
        drive_completion(
            &handle,
            completion_batch(&["task-A", "task-B"], Default::default()),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while provider.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let handle = registry.get_or_create("active-batch-next").await.unwrap();
    let incoming = tokio::spawn(async move {
        drive_completion_end(
            &handle,
            completion_batch(&["task-B", "task-C"], Default::default()),
            false,
        )
        .await;
    });
    // InsertMessages is acknowledged at the next model boundary. Let the
    // notification arrive while that boundary is gated, then release it.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    ready.notify_one();
    incoming.await.unwrap();
    let (state, _) = running.await.unwrap();
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].history.starts_with(&requests[0].history));
    let history = serde_json::to_string(&requests[1].history).unwrap();
    for id in ["task-A", "task-B", "task-C"] {
        assert_eq!(history.matches(&format!("task_id: {id}")).count(), 1);
    }
    let handle = registry.get_or_create("active-batch-retry").await.unwrap();
    drive_completion_end(
        &handle,
        completion_batch(&["task-C", "task-B", "task-A"], state),
        false,
    )
    .await;
    assert_eq!(provider.requests().len(), 2);
    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        run_count, 1,
        "overlapping batch must join the active lifecycle"
    );
    registry.shutdown().await;
}

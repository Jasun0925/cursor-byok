//! Separates Issue #504 input redelivery, checkpoint history, and Write arguments.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use cursor_server::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
        TransportCommand, TransportHandle, TransportRegistry,
    },
    model::ProjectedContent,
    provider::{FinishReason, ModelEvent},
};
use prost::Message;
use tokio::sync::mpsc::UnboundedReceiver;

struct Client {
    handle: TransportHandle,
    output: UnboundedReceiver<Bytes>,
    seq: i64,
    ended: bool,
    checkpoint: Option<pb::ConversationStateStructure>,
}

impl Client {
    async fn send(&mut self, message: pb::AgentClientMessage) {
        self.handle
            .command(TransportCommand::Append {
                seqno: self.seq,
                message: Box::new(message),
            })
            .await
            .unwrap();
        self.seq += 1;
    }

    async fn next(&mut self, timeout: Duration) -> Option<pb::agent_server_message::Message> {
        let frame = tokio::time::timeout(timeout, self.output.recv())
            .await
            .ok()?
            .expect("transport closed without EndStream");
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            assert_eq!(
                payload.as_ref(),
                b"{}",
                "{}",
                String::from_utf8_lossy(&payload)
            );
            self.ended = true;
            return None;
        }
        let message = pb::AgentServerMessage::decode(payload).unwrap().message;
        if let Some(pb::agent_server_message::Message::KvServerMessage(kv)) = &message {
            self.send(pb::AgentClientMessage {
                message: Some(pb::agent_client_message::Message::KvClientMessage(
                    pb::KvClientMessage {
                        id: kv.id,
                        message: Some(pb::kv_client_message::Message::SetBlobResult(
                            pb::SetBlobResult::default(),
                        )),
                    },
                )),
            })
            .await;
        }
        if let Some(pb::agent_server_message::Message::ConversationCheckpointUpdate(state)) =
            &message
        {
            self.checkpoint = Some(state.clone());
        }
        message
    }

    async fn wait_for_call(&mut self, provider: &fake_provider::FakeProvider) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while provider.requests().is_empty() {
            assert!(!self.ended && tokio::time::Instant::now() < deadline);
            self.next(Duration::from_millis(20)).await;
        }
    }

    async fn wait_for_exec(&mut self) -> pb::ExecServerMessage {
        loop {
            if let pb::agent_server_message::Message::ExecServerMessage(exec) = self
                .next(Duration::from_secs(5))
                .await
                .expect("expected Exec")
            {
                return exec;
            }
        }
    }

    async fn wait_for_read(&mut self) -> u32 {
        let exec = self.wait_for_exec().await;
        assert!(matches!(
            exec.message,
            Some(pb::exec_server_message::Message::ReadArgs(_))
        ));
        exec.id
    }

    async fn finish(&mut self) {
        while !self.ended {
            let message = self.next(Duration::from_secs(5)).await;
            assert!(self.ended || message.is_some(), "run stalled");
            assert!(
                !matches!(
                    message,
                    Some(pb::agent_server_message::Message::ExecServerMessage(_))
                ),
                "unexpected tool rerun"
            );
        }
    }
}

fn action(id: &str, text: &str) -> pb::ConversationAction {
    pb::ConversationAction {
        action: Some(pb::conversation_action::Action::UserMessageAction(
            pb::UserMessageAction {
                user_message: Some(pb::UserMessage {
                    message_id: id.into(),
                    text: text.into(),
                    mode: pb::AgentMode::Agent as i32,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

fn runtime_user(id: &str, text: &str) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::ConversationAction(
            action(id, text),
        )),
    }
}

fn response(model_id: &str, tool: Option<(&str, &str)>) -> Vec<ModelEvent> {
    let mut events = vec![ModelEvent::Start {
        model_call_id: model_id.into(),
    }];
    if let Some((name, args)) = tool {
        events.extend([
            ModelEvent::ToolCallStart {
                index: 0,
                call_id: "read-once".into(),
                name: name.into(),
            },
            ModelEvent::ToolCallArgumentsDelta {
                index: 0,
                delta: args.into(),
            },
            ModelEvent::ToolCallEnd { index: 0 },
            ModelEvent::Done(FinishReason::ToolUse),
        ]);
    } else {
        events.extend([
            ModelEvent::TextStart,
            ModelEvent::TextDelta("DONE".into()),
            ModelEvent::TextEnd,
            ModelEvent::Done(FinishReason::Stop),
        ]);
    }
    events
}

async fn setup(
    provider: &fake_provider::FakeProvider,
) -> (tempfile::TempDir, TransportRegistry, Client) {
    let (directory, store) = fixtures::temp_store().await;
    let registry = TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry.get_or_create("delivery-504").await.unwrap();
    let output = handle.subscribe();
    let mut client = Client {
        handle,
        output,
        seq: 0,
        ended: false,
        checkpoint: None,
    };
    client
        .send(pb::AgentClientMessage {
            message: Some(pb::agent_client_message::Message::RunRequest(
                pb::AgentRunRequest {
                    action: Some(action("initial", "INITIAL_504")),
                    conversation_id: Some("conversation-504".into()),
                    run_id: Some("delivery-504".into()),
                    requested_model: Some(pb::RequestedModel {
                        model_id: "test-model".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        })
        .await;
    (directory, registry, client)
}

fn read_success(id: u32) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::ExecClientMessage(
            pb::ExecClientMessage {
                id,
                message: Some(pb::exec_client_message::Message::ReadResult(
                    pb::ReadResult {
                        result: Some(pb::read_result::Result::Success(pb::ReadSuccess {
                            path: "/synthetic/once.txt".into(),
                            output: Some(pb::read_success::Output::Content("RESULT_504".into())),
                            ..Default::default()
                        })),
                    },
                )),
                ..Default::default()
            },
        )),
    }
}

#[tokio::test]
async fn duplicate_runtime_user_message_does_not_abort_the_active_tool() {
    duplicate_input(false).await;
}

#[tokio::test]
async fn redelivery_of_the_initial_user_message_does_not_abort_the_active_tool() {
    duplicate_input(true).await;
}

#[tokio::test]
async fn identical_text_with_a_new_message_id_is_a_new_instruction() {
    let provider = fake_provider::FakeProvider::default();
    provider.push_pending();
    provider.push(response(
        "tool",
        Some(("Read", r#"{"path":"/synthetic/once.txt"}"#)),
    ));
    provider.push(response("final", None));
    let (_directory, registry, mut client) = setup(&provider).await;
    client.wait_for_call(&provider).await;
    client.send(runtime_user("first", "REPEAT_504")).await;
    let exec_id = client.wait_for_read().await;
    client.send(runtime_user("second", "REPEAT_504")).await;
    let mut aborted = false;
    while !client.ended {
        let message = client.next(Duration::from_secs(5)).await;
        assert!(
            client.ended || message.is_some(),
            "new input was not handled"
        );
        if let Some(pb::agent_server_message::Message::ExecServerControlMessage(control)) = message
        {
            aborted |= matches!(
                control.message,
                Some(pb::exec_server_control_message::Message::Abort(abort)) if abort.id == exec_id
            );
        }
    }
    assert!(
        aborted,
        "a new user instruction must interrupt the active Read"
    );
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    let final_history = serde_json::to_string(&requests.last().unwrap().history).unwrap();
    assert_eq!(final_history.matches("REPEAT_504").count(), 2);
    for pair in requests.windows(2) {
        assert!(pair[1].history.starts_with(&pair[0].history));
    }
    registry.shutdown().await;
}

async fn duplicate_input(initial: bool) {
    let provider = fake_provider::FakeProvider::default();
    if !initial {
        provider.push_pending();
    }
    provider.push(response(
        "tool",
        Some(("Read", r#"{"path":"/synthetic/once.txt"}"#)),
    ));
    provider.push(response("final", None));
    let (_directory, registry, mut client) = setup(&provider).await;
    let (id, text) = if initial {
        ("initial", "INITIAL_504")
    } else {
        client.wait_for_call(&provider).await;
        client
            .send(runtime_user("follow-up", "FOLLOW_UP_504"))
            .await;
        ("follow-up", "FOLLOW_UP_504")
    };
    let exec_id = client.wait_for_read().await;
    client.send(runtime_user(id, text)).await;
    // Leave the original tool pending while the redelivery is handled. It must
    // neither be aborted nor re-queued as another user instruction.
    let deadline = tokio::time::Instant::now() + Duration::from_millis(200);
    let mut queued_again = false;
    while let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) {
        let Some(message) = client.next(remaining).await else {
            break;
        };
        if let pb::agent_server_message::Message::ExecServerControlMessage(control) = &message {
            assert!(
                !matches!(&control.message, Some(pb::exec_server_control_message::Message::Abort(abort)) if abort.id == exec_id),
                "duplicate user input aborted the already-running Read (exec {exec_id})"
            );
        }
        if let pb::agent_server_message::Message::InteractionUpdate(update) = message {
            if let Some(pb::interaction_update::Message::ContextInjectionState(update)) =
                update.message
            {
                queued_again |= matches!(
                    update.state.and_then(|state| state.state),
                    Some(pb::context_injection_state::State::Queued(_))
                );
            }
        }
    }
    assert!(!queued_again, "duplicate user input was queued again");
    client.send(read_success(exec_id)).await;
    client.finish().await;
    let requests = provider.requests();
    assert_eq!(requests.len(), if initial { 2 } else { 3 });
    let final_history = serde_json::to_string(&requests.last().unwrap().history).unwrap();
    assert_eq!(final_history.matches(text).count(), 1);
    assert!(final_history.contains("RESULT_504"));
    let history = &requests.last().unwrap().history;
    assert_eq!(
        history
            .iter()
            .filter(|message| matches!(
                &message.content, ProjectedContent::Assistant { calls, .. } if !calls.is_empty()
            ))
            .count(),
        1
    );
    assert_eq!(history.iter().filter(|message| matches!(
        &message.content, ProjectedContent::ToolResult(result) if result.call_id == "read-once" && !result.is_error
    )).count(), 1);
    for pair in requests.windows(2) {
        assert!(pair[1].history.starts_with(&pair[0].history));
    }
    registry.shutdown().await;
}

#[tokio::test]
async fn checkpoint_resume_keeps_old_input_out_of_the_new_user_message() {
    let provider = fake_provider::FakeProvider::default();
    provider.push(response("first", None));
    provider.push(response("second", None));
    let (directory, registry, mut client) = setup(&provider).await;
    client.finish().await;
    let state = client.checkpoint.take().expect("first turn checkpoint");
    registry.shutdown().await;
    drop(client);
    drop(registry);
    let store = cursor_server::store::Store::connect(&format!(
        "sqlite://{}",
        directory.path().join("test.db").display()
    ))
    .await
    .unwrap();
    let registry = TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry.get_or_create("resume-504").await.unwrap();
    let output = handle.subscribe();
    let mut client = Client {
        handle,
        output,
        seq: 0,
        ended: false,
        checkpoint: None,
    };
    client
        .send(pb::AgentClientMessage {
            message: Some(pb::agent_client_message::Message::RunRequest(
                pb::AgentRunRequest {
                    action: Some(action("second", "NEW_INPUT_504")),
                    conversation_state: Some(state),
                    conversation_id: Some("conversation-504".into()),
                    run_id: Some("resume-504".into()),
                    requested_model: Some(pb::RequestedModel {
                        model_id: "test-model".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        })
        .await;
    client.finish().await;
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].history.starts_with(&requests[0].history));
    assert_eq!(
        requests[0].prompt.instructions,
        requests[1].prompt.instructions
    );
    let history = serde_json::to_string(&requests[1].history).unwrap();
    assert_eq!(history.matches("INITIAL_504").count(), 1);
    assert_eq!(history.matches("NEW_INPUT_504").count(), 1);
    let current = serde_json::to_string(requests[1].history.last().unwrap()).unwrap();
    assert!(current.contains("NEW_INPUT_504"));
    assert!(!current.contains("INITIAL_504"));
    assert!(requests[1].history.iter().all(|message| !matches!(
        &message.content, ProjectedContent::Assistant { calls, .. } if !calls.is_empty()
    )));
    registry.shutdown().await;
}

#[tokio::test]
async fn streamed_write_contents_reach_cursor_unchanged() {
    write_contents(Some("首行\nsecond \"quoted\" line 🦀\n")).await;
}

#[tokio::test]
async fn missing_write_contents_returns_an_error_without_writing() {
    write_contents(None).await;
}

async fn write_contents(contents: Option<&str>) {
    let provider = fake_provider::FakeProvider::default();
    let mut arguments = serde_json::json!({"path": "/synthetic/once.txt"});
    if let Some(contents) = contents {
        arguments["contents"] = contents.into();
    }
    let mut events = response("write", Some(("Write", "")));
    // Split inside JSON keys, escapes, and Unicode strings, as a stream can do.
    events.splice(
        2..3,
        arguments
            .to_string()
            .chars()
            .map(|character| ModelEvent::ToolCallArgumentsDelta {
                index: 0,
                delta: character.to_string(),
            }),
    );
    provider.push(events);
    provider.push(response("final", None));
    let (_directory, registry, mut client) = setup(&provider).await;
    let read_id = client.wait_for_read().await;
    client.send(read_success(read_id)).await;
    if let Some(contents) = contents {
        let write = client.wait_for_exec().await;
        let Some(pb::exec_server_message::Message::WriteArgs(args)) = write.message else {
            panic!("expected native Write");
        };
        assert_eq!(args.path, "/synthetic/once.txt");
        assert_eq!(args.file_text, contents);
        client
            .send(pb::AgentClientMessage {
                message: Some(pb::agent_client_message::Message::ExecClientMessage(
                    pb::ExecClientMessage {
                        id: write.id,
                        message: Some(pb::exec_client_message::Message::WriteResult(
                            pb::WriteResult {
                                result: Some(pb::write_result::Result::Success(pb::WriteSuccess {
                                    path: args.path,
                                    file_content_after_write: Some(contents.into()),
                                    ..Default::default()
                                })),
                            },
                        )),
                        ..Default::default()
                    },
                )),
            })
            .await;
    }
    client.finish().await;
    let requests = provider.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].history.starts_with(&requests[0].history));
    let calls = requests[1]
        .history
        .iter()
        .filter_map(|message| match &message.content {
            ProjectedContent::Assistant { calls, .. } => Some(calls.as_slice()),
            _ => None,
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments, arguments);
    let results = requests[1]
        .history
        .iter()
        .filter_map(|message| match &message.content {
            ProjectedContent::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].is_error, contents.is_none());
    if contents.is_none() {
        assert!(
            results[0].content.contains("missing contents"),
            "{}",
            results[0].content
        );
    }
    registry.shutdown().await;
}

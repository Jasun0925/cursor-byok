//! Synthetic client silence checks; not real Cursor approval or a 20-minute process.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;
use cursor_server::{
    cursor::{
        TransportCommand, TransportRegistry,
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
    },
    provider::{FinishReason, ModelEvent},
};
use prost::Message;
use std::{sync::Arc, time::Duration};
#[tokio::test]
async fn no_client_start_for_35_seconds_is_not_an_execution_timeout() {
    exercise(false).await;
}
#[tokio::test]
async fn started_shell_waits_for_actual_result_past_block_threshold() {
    exercise(true).await;
}
async fn exercise(send_start: bool) {
    let (_dir, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(vec![
        ModelEvent::Start {
            model_call_id: "model-0".into(),
        },
        ModelEvent::ToolCallStart {
            index: 0,
            call_id: "shell-probe".into(),
            name: "Shell".into(),
        },
        ModelEvent::ToolCallArgumentsDelta {
            index: 0,
            delta: r#"{"command":"printf probe","block_until_ms":3000}"#.into(),
        },
        ModelEvent::ToolCallEnd { index: 0 },
        ModelEvent::Done(FinishReason::ToolUse),
    ]);
    provider.push(vec![
        ModelEvent::Start {
            model_call_id: "model-1".into(),
        },
        ModelEvent::TextStart,
        ModelEvent::TextDelta("finished".into()),
        ModelEvent::TextEnd,
        ModelEvent::Done(FinishReason::Stop),
    ]);
    let registry = TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry.get_or_create("issue155-request").await.unwrap();
    let mut output = handle.subscribe();
    let mut seq = 0;
    handle
        .command(TransportCommand::Append {
            seqno: seq,
            message: Box::new(client_run()),
        })
        .await
        .unwrap();
    seq += 1;
    let mut exec_id = None;
    let mut deadline = None;
    let mut completed = false;
    let mut result_sent = false;
    let mut wait_ms = 0;
    loop {
        let frame = if let Some(until) = deadline {
            match tokio::time::timeout_at(until, output.recv()).await {
                Ok(f) => f.unwrap(),
                Err(_) => {
                    assert_eq!(
                        provider.requests().len(),
                        1,
                        "model resumed before tool result"
                    );
                    assert!(!completed);
                    wait_ms = 35_000;
                    deadline = None;
                    result_sent = true;
                    handle
                        .command(TransportCommand::Append {
                            seqno: seq,
                            message: Box::new(shell_event(
                                exec_id.unwrap(),
                                pb::shell_stream::Event::Exit(pb::ShellStreamExit {
                                    code: 0,
                                    ..Default::default()
                                }),
                            )),
                        })
                        .await
                        .unwrap();
                    seq += 1;
                    continue;
                }
            }
        } else {
            tokio::time::timeout(Duration::from_secs(10), output.recv())
                .await
                .unwrap()
                .unwrap()
        };
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            assert!(result_sent);
            assert_eq!(payload.as_ref(), b"{}");
            break;
        }
        let server = pb::AgentServerMessage::decode(payload).unwrap();
        let reply = match server.message {
            Some(pb::agent_server_message::Message::KvServerMessage(kv)) => {
                Some(pb::AgentClientMessage {
                    message: Some(pb::agent_client_message::Message::KvClientMessage(
                        pb::KvClientMessage {
                            id: kv.id,
                            message: Some(pb::kv_client_message::Message::SetBlobResult(
                                pb::SetBlobResult { error: None },
                            )),
                        },
                    )),
                })
            }
            Some(pb::agent_server_message::Message::ExecServerMessage(exec)) => {
                assert!(exec_id.is_none(), "duplicate execution request");
                let Some(pb::exec_server_message::Message::ShellStreamArgs(args)) = exec.message
                else {
                    panic!("expected Shell")
                };
                assert_eq!(args.timeout, 3000);
                assert_eq!(args.hard_timeout, Some(86_400_000));
                assert_eq!(
                    args.timeout_behavior,
                    pb::TimeoutBehavior::Background as i32
                );
                exec_id = Some(exec.id);
                deadline = Some(tokio::time::Instant::now() + Duration::from_secs(35));
                send_start.then(|| {
                    shell_event(exec.id, pb::shell_stream::Event::Start(Default::default()))
                })
            }
            Some(pb::agent_server_message::Message::InteractionUpdate(u)) => {
                if let Some(pb::interaction_update::Message::ToolCallCompleted(t)) = u.message {
                    assert!(result_sent, "premature tool completion");
                    assert!(!completed);
                    let Some(pb::tool_call::Tool::ShellToolCall(s)) = t.tool_call.unwrap().tool
                    else {
                        panic!("expected Shell result")
                    };
                    assert!(matches!(
                        s.result.unwrap().result,
                        Some(pb::shell_result::Result::Success(_))
                    ));
                    completed = true;
                }
                None
            }
            _ => None,
        };
        if let Some(message) = reply {
            handle
                .command(TransportCommand::Append {
                    seqno: seq,
                    message: Box::new(message),
                })
                .await
                .unwrap();
            seq += 1;
        }
    }
    assert!(completed);
    assert_eq!(provider.requests().len(), 2);
    println!(
        "ISSUE155 {}",
        serde_json::json!({"sent_start":send_start,"client_silence_ms":wait_ms,"block_until_ms":3000,"premature_completion":false,"execution_requests":1,"provider_calls":2,"late_result":"success"})
    );
}
fn shell_event(id: u32, event: pb::shell_stream::Event) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::ExecClientMessage(
            pb::ExecClientMessage {
                id,
                message: Some(pb::exec_client_message::Message::ShellStream(
                    pb::ShellStream { event: Some(event) },
                )),
                ..Default::default()
            },
        )),
    }
}
fn client_run() -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::RunRequest(
            pb::AgentRunRequest {
                conversation_id: Some("issue155-conversation".into()),
                run_id: Some("issue155-request".into()),
                requested_model: Some(pb::RequestedModel {
                    model_id: "test-model".into(),
                    ..Default::default()
                }),
                action: Some(pb::ConversationAction {
                    action: Some(pb::conversation_action::Action::UserMessageAction(
                        pb::UserMessageAction {
                            user_message: Some(pb::UserMessage {
                                text: "test tool presentation".into(),
                                message_id: "issue155-user".into(),
                                mode: pb::AgentMode::Agent as i32,
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    }
}

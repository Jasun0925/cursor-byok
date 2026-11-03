//! Controlled protocol probe, not a real provider or Cursor UI reproduction.
//! Copy to server/tests/issue214_probe.rs to run, then remove that temporary copy.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use cursor_server::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
        TransportCommand, TransportRegistry,
    },
    provider::{FinishReason, ModelEvent},
    store::BlobId,
};
use prost::Message;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test]
async fn pending_todos_do_not_force_another_provider_call_after_stop() {
    probe("pending-stop", false, Some(FinishReason::Stop)).await;
}

#[tokio::test]
async fn todo_updates_can_continue_until_completed() {
    probe("completed-stop", true, Some(FinishReason::Stop)).await;
}

#[tokio::test]
async fn pending_todos_with_length_termination_fail() {
    probe("pending-length", false, Some(FinishReason::Length)).await;
}

#[tokio::test]
async fn pending_todos_with_missing_done_fail() {
    probe("pending-eof", false, None).await;
}

async fn probe(case: &str, complete: bool, finish: Option<FinishReason>) {
    let start = Instant::now();
    let (directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(todo_update("todo-1", false));
    if complete {
        provider.push(todo_update("todo-2", true));
    }
    let successful = matches!(finish, Some(FinishReason::Stop));
    let eof = finish.is_none();
    let mut last = vec![
        ModelEvent::Start {
            model_call_id: "final".into(),
        },
        ModelEvent::TextStart,
        ModelEvent::TextDelta("Controlled final response.".into()),
        ModelEvent::TextEnd,
    ];
    if let Some(reason) = finish {
        last.push(ModelEvent::Done(reason));
    }
    for _ in 0..if eof { 9 } else { 1 } {
        provider.push(last.clone());
    }
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    let registry = TransportRegistry::new(
        store.clone(),
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    );
    let handle = registry.get_or_create(case).await.unwrap();
    let mut output = handle.subscribe();
    handle
        .command(TransportCommand::Append {
            seqno: 0,
            message: Box::new(run(case)),
        })
        .await
        .unwrap();
    let mut seqno = 1;
    let mut last_checkpoint = None;
    let mut todo_cards = 0;
    let mut saw_turn_ended = false;
    let end = loop {
        let frame = tokio::time::timeout(Duration::from_secs(60), output.recv())
            .await
            .expect("stream timeout")
            .expect("stream closed without end");
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            break serde_json::from_slice::<serde_json::Value>(&payload).unwrap();
        }
        match pb::AgentServerMessage::decode(payload).unwrap().message {
            Some(pb::agent_server_message::Message::KvServerMessage(kv)) => {
                assert!(
                    matches!(
                        kv.message,
                        Some(pb::kv_server_message::Message::SetBlobArgs(_))
                    ),
                    "unexpected KV operation"
                );
                handle
                    .command(TransportCommand::Append {
                        seqno,
                        message: Box::new(pb::AgentClientMessage {
                            message: Some(pb::agent_client_message::Message::KvClientMessage(
                                pb::KvClientMessage {
                                    id: kv.id,
                                    message: Some(pb::kv_client_message::Message::SetBlobResult(
                                        pb::SetBlobResult { error: None },
                                    )),
                                },
                            )),
                        }),
                    })
                    .await
                    .unwrap();
                seqno += 1;
            }
            Some(pb::agent_server_message::Message::ConversationCheckpointUpdate(state)) => {
                last_checkpoint = Some(state)
            }
            Some(pb::agent_server_message::Message::InteractionUpdate(update)) => {
                match update.message {
                    Some(pb::interaction_update::Message::TurnEnded(_)) => saw_turn_ended = true,
                    Some(pb::interaction_update::Message::ToolCallCompleted(done)) => {
                        if let Some(pb::tool_call::Tool::UpdateTodosToolCall(todo)) =
                            done.tool_call.and_then(|call| call.tool)
                        {
                            assert!(matches!(
                                todo.result.and_then(|r| r.result),
                                Some(pb::update_todos_result::Result::Success(_))
                            ));
                            todo_cards += 1;
                        }
                    }
                    _ => {}
                }
            }
            Some(pb::agent_server_message::Message::ExecServerMessage(_)) => {
                panic!("TodoWrite must be local")
            }
            _ => {}
        }
    };
    let requests = provider.requests();
    assert_eq!(requests.len(), if eof { 10 } else if complete { 3 } else { 2 });
    assert_eq!(todo_cards, if complete { 2 } else { 1 });
    let checkpoint = last_checkpoint.expect("checkpoint exists");
    let mut todos = Vec::new();
    for id in &checkpoint.todos {
        let bytes = store
            .get_blob(&BlobId::from_bytes(id).unwrap())
            .await
            .unwrap()
            .unwrap();
        let todo = pb::TodoItem::decode(bytes.as_slice()).unwrap();
        todos.push(json!({"id": todo.id, "status": pb::TodoStatus::try_from(todo.status).unwrap().as_str_name()}));
    }
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        directory.path().join("test.db").display()
    ))
    .await
    .unwrap();
    let (status, failure): (String, Option<String>) =
        sqlx::query_as("SELECT status, failure_summary FROM runs WHERE cursor_request_id = ?")
            .bind(case)
            .fetch_one(&pool)
            .await
            .unwrap();
    if successful {
        assert_eq!(end, json!({}));
        assert_eq!(status, "completed");
        assert!(saw_turn_ended);
        assert_eq!(todos.len(), 2);
        let wanted = if complete {
            pb::TodoStatus::Completed
        } else {
            pb::TodoStatus::Pending
        };
        assert_eq!(todos[1]["status"], wanted.as_str_name());
    } else {
        assert!(end.get("error").is_some());
        assert_ne!(status, "completed");
        assert!(!saw_turn_ended);
        assert!(failure.is_some());
    }
    println!(
        "ISSUE214_EVIDENCE {}",
        json!({"case":case,"provider_calls":requests.len(),"todo_cards":todo_cards,"run_status":status,"failure_summary":failure,"turn_ended":saw_turn_ended,"checkpoint_todos":todos,"end_stream_error":end.get("error").is_some(),"elapsed_ms":start.elapsed().as_millis()})
    );
    pool.close().await;
    registry.shutdown().await;
}

fn todo_update(id: &str, complete: bool) -> Vec<ModelEvent> {
    vec![
        ModelEvent::Start { model_call_id: id.into() },
        ModelEvent::ToolCallStart { index:0, call_id:id.into(), name:"TodoWrite".into() },
        ModelEvent::ToolCallArgumentsDelta { index:0, delta:json!({"merge":false,"todos":[
            {"id":"one","content":"First task","status":"completed"},
            {"id":"two","content":"Second task","status":if complete {"completed"} else {"pending"}},
        ]}).to_string() },
        ModelEvent::ToolCallEnd { index:0 },
        ModelEvent::Done(FinishReason::ToolUse),
    ]
}

fn run(id: &str) -> pb::AgentClientMessage {
    pb::AgentClientMessage {
        message: Some(pb::agent_client_message::Message::RunRequest(
            pb::AgentRunRequest {
                action: Some(pb::ConversationAction {
                    action: Some(pb::conversation_action::Action::UserMessageAction(
                        pb::UserMessageAction {
                            user_message: Some(pb::UserMessage {
                                text: "Complete both tasks and update their todos.".into(),
                                message_id: "user".into(),
                                mode: pb::AgentMode::Agent as i32,
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                }),
                conversation_id: Some(format!("conversation-{id}")),
                run_id: Some(id.into()),
                requested_model: Some(pb::RequestedModel {
                    model_id: "test-model".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    }
}

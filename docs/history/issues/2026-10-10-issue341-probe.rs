//! Diagnostic only: run with cargo test -p cursor-server --test issue341_probe -- --nocapture.
use cursor_server::{
    cursor::{
        checkpoint::StepBuffer,
        protocol::proto::agent::v1 as pb,
        tools::{
            codec,
            runtime::{CursorToolRuntime, ExecContext},
            ToolBatchState, ToolDispatcher,
        },
    },
    model::ToolCall,
};
use prost::Message;
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

async fn edit_snapshot(tool_name: &str) {
    let before = "const value = 1;\n";
    let after = "const value = 2;\n";
    let call = ToolCall {
        index: 0,
        call_id: "issue341-edit".into(),
        model_call_id: "model-0".into(),
        name: tool_name.into(),
        arguments_text: String::new(),
        argument_error: None,
        arguments: if tool_name == "Write" {
            json!({"path":"/isolated/example.ts","contents":after})
        } else {
            json!({"path":"/isolated/example.ts","old_string":"1","new_string":"2"})
        },
    };
    let context = ExecContext {
        conversation_id: "issue341".into(),
        root_conversation_id: "issue341".into(),
        default_subagent_model: "test-model".into(),
        subagent_model: None,
        terminals_folder: String::new(),
        admin_command_denylist: Vec::new(),
        allow_subagents: true,
        subagents_disabled: false,
        mcp_routes: Default::default(),
    };
    let runtime = CursorToolRuntime::default();
    let dispatcher = ToolDispatcher::new(runtime.clone());
    let started = dispatcher
        .start_batch(
            &[call],
            ToolBatchState {
                completed: &HashSet::new(),
                started: &HashSet::new(),
                response_text: "",
                response_thinking: "",
            },
            &[],
            &BTreeMap::new(),
            &context,
        )
        .await
        .unwrap();
    let read_id = started[0]
        .messages
        .iter()
        .find_map(|message| match &message.message {
            Some(pb::agent_server_message::Message::ExecServerMessage(exec)) => Some(exec.id),
            _ => None,
        })
        .unwrap();
    let write_event = codec::client_event(
        &pb::ExecClientMessage {
            id: read_id,
            message: Some(pb::exec_client_message::Message::ReadResult(
                pb::ReadResult {
                    result: Some(pb::read_result::Result::Success(pb::ReadSuccess {
                        path: "/isolated/example.ts".into(),
                        output: Some(pb::read_success::Output::Content(before.into())),
                        ..Default::default()
                    })),
                },
            )),
            ..Default::default()
        },
        &runtime,
    )
    .await
    .unwrap();
    let codec::ClientExecEvent::Message(write_message) = write_event else {
        panic!("write request");
    };
    let Some(pb::agent_server_message::Message::ExecServerMessage(write)) = write_message.message
    else {
        panic!("exec");
    };
    let Some(pb::exec_server_message::Message::WriteArgs(args)) = write.message else {
        panic!("write args");
    };
    assert_eq!(
        args.file_text, after,
        "requested file modification is correct"
    );
    let completed = codec::client_event(
        &pb::ExecClientMessage {
            id: write.id,
            message: Some(pb::exec_client_message::Message::WriteResult(
                pb::WriteResult {
                    result: Some(pb::write_result::Result::Success(pb::WriteSuccess {
                        path: args.path,
                        ..Default::default()
                    })),
                },
            )),
            ..Default::default()
        },
        &runtime,
    )
    .await
    .unwrap();
    let codec::ClientExecEvent::Completed(completed) = completed else {
        panic!("completion");
    };
    assert!(!completed.result().is_error);
    let mut steps = StepBuffer::default();
    steps.tool_completed(&completed);
    let pending = steps.take();
    let replay = pb::ConversationStep::decode(pending.steps[0].encode_to_vec().as_slice()).unwrap();
    let Some(pb::conversation_step::Message::ToolCall(tool)) = replay.message else {
        panic!("replay tool");
    };
    let Some(pb::tool_call::Tool::EditToolCall(edit)) = tool.tool else {
        panic!("edit");
    };
    let Some(pb::edit_result::Result::Success(success)) = edit.result.unwrap().result else {
        panic!("success");
    };
    println!(
        "{tool_name}: write_correct=true before_present={} after_bytes={} diff_present={}",
        success.before_full_file_content.is_some(),
        success.after_full_file_content.len(),
        success.diff_string.as_ref().is_some_and(|s| !s.is_empty())
    );
    assert_eq!(
        success.before_full_file_content.as_deref(),
        Some(before),
        "original file lost before Cursor receives/replays tool result"
    );
    assert_eq!(success.after_full_file_content, after);
}
#[tokio::test]
async fn write_retains_file_snapshots_for_cursor() {
    edit_snapshot("Write").await;
}
#[tokio::test]
async fn strreplace_retains_file_snapshots_for_cursor() {
    edit_snapshot("StrReplace").await;
}

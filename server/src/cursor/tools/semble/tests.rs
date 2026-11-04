use super::*;
use crate::cursor::tools::tool_call_result::tool_result_channel;
use serde_json::json;

fn call() -> ToolCall {
    ToolCall {
        index: 0,
        call_id: "search-call".into(),
        model_call_id: "model-call".into(),
        name: "SembleSearch".into(),
        arguments_text: String::new(),
        argument_error: None,
        arguments: json!({"repo":"/workspace", "query":"auth"}),
    }
}

fn context() -> ExecContext {
    ExecContext {
        conversation_id: "test-owner".into(),
        workspace_paths: vec!["/workspace".into()],
        ..Default::default()
    }
}

fn unwrap_exec(message: pb::AgentServerMessage) -> pb::ExecServerMessage {
    let Some(pb::agent_server_message::Message::ExecServerMessage(exec)) = message.message else {
        panic!("expected Exec");
    };
    exec
}

fn inventory(id: u32) -> pb::ExecClientMessage {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let payload = json!({"ok":true,"fingerprint":"a".repeat(64),"inventory":{
    "root":"/workspace","rules":[],"entries":[
        {"path":"auth.rs","is_dir":false,"size":12,"git_match":"none"}
    ]}})
    .to_string();
    pb::ExecClientMessage {
        id,
        message: Some(pb::exec_client_message::Message::ShellResult(
            pb::ShellResult {
                result: Some(pb::shell_result::Result::Success(pb::ShellSuccess {
                    exit_code: 0,
                    stdout: json!({"offset":0,"total":payload.len(),
                        "sha256":hex::encode(Sha256::digest(payload.as_bytes())),
                        "data":base64::engine::general_purpose::STANDARD.encode(payload.as_bytes())}).to_string(),
                    ..Default::default()
                })),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

#[tokio::test]
async fn filesystem_search_uses_cursor_policy_without_requesting_an_override() {
    let runtime = CursorToolRuntime::default();
    let (sender, _receiver) = tool_result_channel();
    let exec = unwrap_exec(
        start(&runtime, &sender, &call(), &context(), None)
            .await
            .unwrap()
            .unwrap(),
    );
    let Some(pb::exec_server_message::Message::ShellStreamArgs(args)) = exec.message else {
        panic!();
    };
    assert_eq!(args.working_directory, "/workspace");
    assert_eq!(args.timeout_behavior, pb::TimeoutBehavior::Cancel as i32);
    // Ordinary Shell succeeds on the tested remote host with its configured
    // policy; forcing WorkspaceReadonly fails before this collector starts.
    assert!(args.requested_sandbox_policy.is_none());
    assert!(!args.skip_approval);
    assert_eq!(runtime.running_exec_ids().await, vec![exec.id]);
    runtime.drain_running().await;
}

#[tokio::test]
async fn completed_inventory_advances_once_and_cancellation_ignores_late_results() {
    let runtime = CursorToolRuntime::default();
    let (sender, mut receiver) = tool_result_channel();
    let first = unwrap_exec(
        start(&runtime, &sender, &call(), &context(), None)
            .await
            .unwrap()
            .unwrap(),
    );
    let reply = inventory(first.id);
    let ClientExecEvent::Message(second) = codec::client_event(&reply, &runtime).await.unwrap()
    else {
        panic!("expected read batch");
    };
    let second = unwrap_exec(*second);
    assert_ne!(second.id, first.id);
    assert!(matches!(
        codec::client_event(&reply, &runtime).await.unwrap(),
        ClientExecEvent::Pending
    ));
    assert_eq!(runtime.running_exec_ids().await, vec![second.id]);
    assert_eq!(runtime.interrupt_for_message().await, vec![second.id]);
    assert!(matches!(
        codec::client_event(&inventory(second.id), &runtime)
            .await
            .unwrap(),
        ClientExecEvent::Pending
    ));
    assert!(receiver.try_recv().is_none());
}

#[tokio::test]
async fn permission_denial_and_stream_close_complete_only_the_original_tool() {
    for closed in [false, true] {
        let runtime = CursorToolRuntime::default();
        let (sender, mut receiver) = tool_result_channel();
        let exec = unwrap_exec(
            start(&runtime, &sender, &call(), &context(), None)
                .await
                .unwrap()
                .unwrap(),
        );
        if closed {
            assert!(codec::stream_closed(exec.id, &runtime)
                .await
                .unwrap()
                .is_none());
        } else {
            let denied = pb::ExecClientMessage {
                id: exec.id,
                message: Some(pb::exec_client_message::Message::ShellStream(
                    pb::ShellStream {
                        event: Some(pb::shell_stream::Event::PermissionDenied(Default::default())),
                    },
                )),
                ..Default::default()
            };
            codec::client_event(&denied, &runtime).await.unwrap();
        }
        let completion = receiver.recv().await.unwrap().unwrap();
        assert!(completion.result().is_error);
        assert_eq!(completion.result().call_id, "search-call");
        assert!(runtime.running_exec_ids().await.is_empty());
        assert!(receiver.try_recv().is_none());
    }
}

#[tokio::test]
async fn interrupted_search_worker_cannot_publish_after_owner_is_replaced() {
    let runtime = CursorToolRuntime::default();
    let (sender, mut receiver) = tool_result_channel();
    let owner = runtime.register_search("search-call").await;
    runtime.interrupt_for_run_replacement().await;
    assert!(owner.load(std::sync::atomic::Ordering::Acquire));
    runtime
        .finish_search(
            "search-call",
            &owner,
            &sender,
            result::semble(&call(), 0, Ok(json!({"results":[]}))),
        )
        .await;
    assert!(receiver.try_recv().is_none());
}

#[tokio::test]
async fn cancellation_aborts_waiting_workers_and_workers_attached_after_interruption() {
    let runtime = CursorToolRuntime::default();
    let owner = runtime.register_search("search-call").await;
    let worker = tokio::spawn(std::future::pending::<()>());
    runtime
        .attach_search_worker("search-call", &owner, worker.abort_handle())
        .await;
    runtime.interrupt_for_message().await;
    assert!(worker.await.unwrap_err().is_cancelled());
    let late = tokio::spawn(std::future::pending::<()>());
    runtime
        .attach_search_worker("search-call", &owner, late.abort_handle())
        .await;
    assert!(late.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn missing_or_unrelated_workspace_never_falls_back_to_server_files() {
    let runtime = CursorToolRuntime::default();
    let (sender, _receiver) = tool_result_channel();
    for context in [
        ExecContext::default(),
        ExecContext {
            workspace_paths: vec!["/workspace-other".into()],
            ..Default::default()
        },
    ] {
        assert!(start(&runtime, &sender, &call(), &context, None)
            .await
            .is_err());
        assert!(runtime.running_exec_ids().await.is_empty());
    }
}

#[tokio::test]
async fn overflowing_collection_aborts_exec_and_reports_one_original_tool_error() {
    let runtime = CursorToolRuntime::default();
    let (sender, mut receiver) = tool_result_channel();
    let exec = unwrap_exec(
        start(&runtime, &sender, &call(), &context(), None)
            .await
            .unwrap()
            .unwrap(),
    );
    let output = pb::ExecClientMessage {
        id: exec.id,
        message: Some(pb::exec_client_message::Message::ShellStream(
            pb::ShellStream {
                event: Some(pb::shell_stream::Event::Stdout(pb::ShellStreamStdout {
                    data: "x".repeat(search::snapshot::MAX_OUTPUT_BYTES + 1),
                })),
            },
        )),
        ..Default::default()
    };
    assert!(matches!(
        codec::client_event(&output, &runtime).await.unwrap(),
        ClientExecEvent::Message(_)
    ));
    let completion = receiver.recv().await.unwrap().unwrap();
    assert_eq!(completion.result().call_id, "search-call");
    assert!(completion.result().is_error);
    codec::client_event(&inventory(exec.id), &runtime)
        .await
        .unwrap();
    assert!(receiver.try_recv().is_none());
    assert!(runtime.running_exec_ids().await.is_empty());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires the cached real Semble model; runs actual host shell commands, not Cursor UI"]
async fn real_model_host_exec_round_trip_search_and_related() {
    use std::{fs, process::Command};
    let cache = semble_core::SembleConfig::default().cache_dir;
    let model = cache.join("models/potion-code-16M-v2/model.safetensors");
    assert!(
        model.is_file() && model.with_file_name("tokenizer.json").is_file(),
        "cache the pinned model first"
    );
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    fs::write(
        root.join("auth.rs"),
        "pub fn validate_login(password: &str) -> bool { !password.is_empty() }\n",
    )
    .unwrap();
    fs::write(
        root.join("config.rs"),
        "pub fn configured_timeout() -> u64 { 30 }\n",
    )
    .unwrap();
    fs::write(
        root.join("private.rs"),
        "pub fn PRIVATE_BODY_MUST_NOT_TRANSFER() {}\n",
    )
    .unwrap();
    fs::write(root.join(".cursorignore"), "private.rs\n").unwrap();
    let context = ExecContext {
        workspace_paths: vec![root.to_string_lossy().into_owned()],
        ..context()
    };
    for related in [false, true] {
        let mut call = call();
        call.name = if related {
            "SembleFindRelated"
        } else {
            "SembleSearch"
        }
        .into();
        call.arguments = if related {
            json!({"repo":root,"file_path":"auth.rs","line":1,"top_k":2})
        } else {
            json!({"repo":root,"query":"Check a user password during login","top_k":2})
        };
        call.arguments_text = call.arguments.to_string();
        let runtime = CursorToolRuntime::default();
        let (sender, mut receiver) = tool_result_channel();
        let mut exec = unwrap_exec(
            start(&runtime, &sender, &call, &context, None)
                .await
                .unwrap()
                .unwrap(),
        );
        let mut rounds = 0;
        loop {
            let Some(pb::exec_server_message::Message::ShellStreamArgs(args)) = exec.message else {
                panic!("expected shell collection");
            };
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(&args.command)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            let stdout = String::from_utf8(output.stdout).unwrap();
            let wire: serde_json::Value = serde_json::from_str(&stdout).unwrap();
            // This small fixture fits one page per phase. Inspect the decoded
            // payload, not the base64 envelope, for excluded source bodies.
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(wire["data"].as_str().unwrap())
                .unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("PRIVATE_BODY_MUST_NOT_TRANSFER"));
            let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if let Some(bodies) = payload.get("files").and_then(serde_json::Value::as_array) {
                assert!(bodies.iter().all(|body| body["path"] != "private.rs"));
            }
            let reply = pb::ExecClientMessage {
                id: exec.id,
                message: Some(pb::exec_client_message::Message::ShellResult(
                    pb::ShellResult {
                        result: Some(pb::shell_result::Result::Success(pb::ShellSuccess {
                            exit_code: 0,
                            stdout,
                            ..Default::default()
                        })),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            };
            rounds += 1;
            match codec::client_event(&reply, &runtime).await.unwrap() {
                ClientExecEvent::Message(next) => exec = unwrap_exec(*next),
                ClientExecEvent::Pending => break,
                _ => panic!("raw collection must not be a provider-visible tool completion"),
            }
            assert!(rounds <= 2);
        }
        assert_eq!(rounds, 2);
        let completion = tokio::time::timeout(std::time::Duration::from_secs(30), receiver.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let result = completion.result();
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.call_id, "search-call");
        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(
            value["results"][0]["file_path"],
            if related { "config.rs" } else { "auth.rs" }
        );
        assert!(!result.content.contains("PRIVATE_BODY_MUST_NOT_TRANSFER"));
        assert!(receiver.try_recv().is_none());
        assert!(runtime.running_exec_ids().await.is_empty());
    }
}

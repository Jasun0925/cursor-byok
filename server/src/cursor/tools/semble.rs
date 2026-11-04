//! One Semble call owns collection Execs, immutable bodies and its search worker.

use std::sync::{atomic::AtomicBool, Arc};

use crate::{
    cursor::protocol::proto::agent::v1 as pb,
    model::ToolCall,
    search::{
        self,
        snapshot::{normalize_root, valid_root, windows_root, Collector},
        SembleOperation,
    },
    store::Store,
    Error, Result,
};

use super::{
    codec::{self, ClientExecEvent},
    runtime::{now_ms, CursorToolRuntime, ExecContext, ExecStage, PendingExec},
    tool_call_result::{self as result, ToolResultSender},
};

pub(crate) struct SembleExecution {
    operation: SembleOperation,
    collector: Collector,
    pub(super) cancelled: Arc<AtomicBool>,
    results: ToolResultSender,
    store: Option<Store>,
}

pub(crate) async fn start(
    runtime: &CursorToolRuntime,
    results: &ToolResultSender,
    call: &ToolCall,
    context: &ExecContext,
    store: Option<Store>,
) -> Result<Option<pb::AgentServerMessage>> {
    let tool_name = call.name.to_ascii_lowercase();
    let operation =
        SembleOperation::parse(&tool_name, call.arguments.clone()).map_err(Error::Protocol)?;
    let started = now_ms();
    if operation.repo().starts_with("https://") || operation.repo().starts_with("http://") {
        let cancelled = runtime.register_search(&call.call_id).await;
        let (owner, results, owned_call) = (runtime.clone(), results.clone(), call.clone());
        let worker_cancelled = cancelled.clone();
        let worker = tokio::spawn(async move {
            let output =
                search::execute_semble(&tool_name, owned_call.arguments.clone(), store).await;
            let completion = result::semble(&owned_call, started, output);
            owner
                .finish_search(&owned_call.call_id, &worker_cancelled, &results, completion)
                .await;
        });
        runtime
            .attach_search_worker(&call.call_id, &cancelled, worker.abort_handle())
            .await;
        return Ok(None);
    }
    let root = normalize_root(operation.repo());
    if !valid_root(&root)
        || !context.workspace_paths.iter().any(|workspace| {
            let workspace = normalize_root(workspace);
            let (root, workspace) = if windows_root(&root) {
                (root.to_lowercase(), workspace.to_lowercase())
            } else {
                (root.clone(), workspace)
            };
            valid_root(&workspace)
                && (root == workspace
                    || root
                        .strip_prefix(workspace.trim_end_matches('/'))
                        .is_some_and(|tail| tail.starts_with('/')))
        })
    {
        return Err(Error::Protocol(
            "Semble repo must be an absolute directory inside the current Cursor workspace".into(),
        ));
    }
    let collector = Collector::new(root, operation.content());
    let command = collector.command().map_err(Error::Protocol)?;
    let cancelled = runtime.register_search(&call.call_id).await;
    let state = Box::new(SembleExecution {
        operation,
        collector,
        cancelled,
        results: results.clone(),
        store,
    });
    let id = runtime
        .reserve_semble(call, context, state, started)
        .await?;
    Ok(Some(request(id, call, context, command)))
}

fn request(
    id: u32,
    call: &ToolCall,
    context: &ExecContext,
    command: String,
) -> pb::AgentServerMessage {
    let args = pb::ShellArgs {
        command: command.clone(),
        working_directory: call.arguments["repo"].as_str().unwrap_or_default().into(),
        tool_call_id: call.call_id.clone(),
        simple_commands: vec![command],
        timeout: 30_000,
        hard_timeout: Some(30_000),
        timeout_behavior: pb::TimeoutBehavior::Cancel as i32,
        file_output_threshold_bytes: Some((search::snapshot::MAX_OUTPUT_BYTES + 1024) as u64),
        // This collector is read-only. Let Cursor apply its configured Shell
        // policy and approval flow, just as for ordinary Shell requests. Do not
        // force an unavailable sandbox or request an unsandboxed escalation.
        close_stdin: true,
        conversation_id: Some(context.conversation_id.clone()),
        admin_command_denylist: context.admin_command_denylist.clone(),
        description: Some("Collect allowed workspace files for semantic search".into()),
        ..Default::default()
    };
    pb::AgentServerMessage {
        message: Some(pb::agent_server_message::Message::ExecServerMessage(
            pb::ExecServerMessage {
                id,
                exec_id: uuid::Uuid::new_v4().to_string(),
                message: Some(pb::exec_server_message::Message::ShellStreamArgs(args)),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

pub(crate) async fn client_event(
    message: &pb::ExecClientMessage,
    runtime: &CursorToolRuntime,
) -> Result<ClientExecEvent> {
    use pb::{exec_client_message::Message, shell_stream::Event};
    let terminal = match message.message.as_ref() {
        Some(Message::ShellStream(stream)) => match stream.event.as_ref() {
            Some(Event::Stdout(value)) => {
                return append(runtime, message.id, &value.data, true).await;
            }
            Some(Event::Stderr(value)) => {
                return append(runtime, message.id, &value.data, false).await;
            }
            Some(Event::Start(_)) | Some(Event::HookContext(_)) | None => {
                return Ok(ClientExecEvent::Pending)
            }
            Some(Event::Exit(exit)) if exit.code == 0 && !exit.aborted => Ok(None),
            Some(Event::Exit(exit)) => Err(format!(
                "Workspace collection exited with code {} (aborted={})",
                exit.code, exit.aborted
            )),
            Some(Event::Rejected(value)) => {
                Err(format!("Workspace collection rejected: {}", value.reason))
            }
            Some(Event::PermissionDenied(_)) => {
                Err("Workspace collection permission denied".into())
            }
            Some(Event::SandboxUnsupported(value)) => Err(format!(
                "Workspace read-only sandbox unavailable: {}",
                value.reason
            )),
            Some(Event::Backgrounded(_)) => {
                if let Some(entry) = runtime.take_exec(message.id).await {
                    fail(
                        entry,
                        runtime,
                        "Workspace collection must remain in the foreground".into(),
                    )
                    .await?;
                }
                return Ok(ClientExecEvent::Message(Box::new(codec::abort(message.id))));
            }
        },
        Some(Message::ShellResult(shell)) if shell.is_background != Some(true) => {
            match shell.result.as_ref() {
                Some(pb::shell_result::Result::Success(value)) if value.exit_code == 0 => {
                    Ok(Some(value.stdout.clone()))
                }
                Some(pb::shell_result::Result::Rejected(value)) => {
                    Err(format!("Workspace collection rejected: {}", value.reason))
                }
                Some(pb::shell_result::Result::PermissionDenied(_)) => {
                    Err("Workspace collection permission denied".into())
                }
                _ => Err("Workspace collection did not complete successfully".into()),
            }
        }
        None => return Ok(ClientExecEvent::Pending),
        _ => Err("Unexpected workspace collection response".into()),
    };
    let Some(mut entry) = runtime.take_exec(message.id).await else {
        return Ok(ClientExecEvent::Pending);
    };
    match terminal {
        Ok(output) => {
            if let Some(output) = output {
                entry.stdout = output;
            }
            advance(entry, runtime).await
        }
        Err(error) => {
            let detail = entry.stderr.trim();
            let error = if detail.is_empty() {
                error
            } else {
                format!("{error}: {detail}")
            };
            fail(entry, runtime, error).await?;
            Ok(ClientExecEvent::Pending)
        }
    }
}

async fn append(
    runtime: &CursorToolRuntime,
    id: u32,
    text: &str,
    stdout: bool,
) -> Result<ClientExecEvent> {
    if let Err(error) = runtime.append_semble_output(id, text, stdout).await {
        if let Some(entry) = runtime.take_exec(id).await {
            fail(entry, runtime, error).await?;
        }
        Ok(ClientExecEvent::Message(Box::new(codec::abort(id))))
    } else {
        Ok(ClientExecEvent::Pending)
    }
}

async fn advance(entry: PendingExec, runtime: &CursorToolRuntime) -> Result<ClientExecEvent> {
    let PendingExec {
        call,
        context,
        started_at_ms,
        stdout,
        stage,
        ..
    } = entry;
    let ExecStage::Semble(mut state) = stage else {
        return Err(Error::Protocol("Invalid Semble execution stage".into()));
    };
    match state.collector.accept(&stdout) {
        Ok(Some(files)) => {
            let owner = runtime.clone();
            let call_id = call.call_id.clone();
            let cancelled = state.cancelled.clone();
            let worker = tokio::spawn(async move {
                let output = state
                    .operation
                    .execute_snapshot(files, state.cancelled.clone(), state.store)
                    .await;
                let completion = result::semble(&call, started_at_ms, output);
                owner
                    .finish_search(&call.call_id, &state.cancelled, &state.results, completion)
                    .await;
            });
            runtime
                .attach_search_worker(&call_id, &cancelled, worker.abort_handle())
                .await;
            Ok(ClientExecEvent::Pending)
        }
        Ok(None) => match state.collector.command() {
            Ok(command) => {
                let id = runtime
                    .reserve_semble(&call, &context, state, started_at_ms)
                    .await?;
                Ok(ClientExecEvent::Message(Box::new(request(
                    id, &call, &context, command,
                ))))
            }
            Err(error) => {
                complete_error(&call, started_at_ms, state, runtime, error).await;
                Ok(ClientExecEvent::Pending)
            }
        },
        Err(error) => {
            complete_error(&call, started_at_ms, state, runtime, error).await;
            Ok(ClientExecEvent::Pending)
        }
    }
}

pub(crate) async fn fail(
    entry: PendingExec,
    runtime: &CursorToolRuntime,
    error: String,
) -> Result<()> {
    let ExecStage::Semble(state) = entry.stage else {
        return Err(Error::Protocol("Invalid Semble execution stage".into()));
    };
    complete_error(&entry.call, entry.started_at_ms, state, runtime, error).await;
    Ok(())
}

async fn complete_error(
    call: &ToolCall,
    started: u64,
    state: Box<SembleExecution>,
    runtime: &CursorToolRuntime,
    error: String,
) {
    let completion = result::semble(call, started, Err(error));
    runtime
        .finish_search(&call.call_id, &state.cancelled, &state.results, completion)
        .await;
}

#[cfg(test)]
mod tests;

//! Terminal presentation for one workspace collection Exec, never a model tool call.
use crate::cursor::protocol::{events::server_interaction, proto::agent::v1 as pb};

use super::{now_ms, ExecContext, ToolCall};

pub(super) struct Collection {
    args: pb::ShellArgs,
    started_at_ms: u64,
}

impl Collection {
    pub(super) fn new(call: &ToolCall, context: &ExecContext, command: String) -> Self {
        Self {
            args: pb::ShellArgs {
                command: command.clone(),
                working_directory: call.arguments["repo"].as_str().unwrap_or_default().into(),
                // Cursor resolves terminal review by tool_call_id. The parent
                // Semble card is an MCP card and cannot own a terminal review.
                tool_call_id: uuid::Uuid::new_v4().to_string(),
                simple_commands: vec![command],
                timeout: 30_000,
                hard_timeout: Some(30_000),
                timeout_behavior: pb::TimeoutBehavior::Cancel as i32,
                file_output_threshold_bytes: Some(
                    (crate::search::snapshot::MAX_OUTPUT_BYTES + 1024) as u64,
                ),
                close_stdin: true,
                conversation_id: Some(context.conversation_id.clone()),
                admin_command_denylist: context.admin_command_denylist.clone(),
                description: Some("Collect allowed workspace files for semantic search".into()),
                // Keep Cursor's configured policy, including approval.
                ..Default::default()
            },
            started_at_ms: now_ms(),
        }
    }

    fn card(&self, result: Option<pb::ShellResult>) -> pb::ToolCall {
        pb::ToolCall {
            tool_call_id: Some(self.args.tool_call_id.clone()),
            started_at_ms: Some(self.started_at_ms),
            completed_at_ms: result.as_ref().map(|_| now_ms()),
            tool: Some(pb::tool_call::Tool::ShellToolCall(pb::ShellToolCall {
                args: Some(self.args.clone()),
                result,
                description: self.args.description.clone(),
            })),
            ..Default::default()
        }
    }

    pub(super) fn request(&self, id: u32, model_call_id: &str) -> Vec<pb::AgentServerMessage> {
        vec![
            server_interaction(pb::interaction_update::Message::ToolCallStarted(
                pb::ToolCallStartedUpdate {
                    call_id: self.args.tool_call_id.clone(),
                    tool_call: Some(self.card(None)),
                    model_call_id: model_call_id.into(),
                },
            )),
            pb::AgentServerMessage {
                message: Some(pb::agent_server_message::Message::ExecServerMessage(
                    pb::ExecServerMessage {
                        id,
                        exec_id: uuid::Uuid::new_v4().to_string(),
                        message: Some(pb::exec_server_message::Message::ShellStreamArgs(
                            self.args.clone(),
                        )),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
        ]
    }

    pub(super) fn completed(
        &self,
        model_call_id: &str,
        error: Option<&str>,
    ) -> pb::AgentServerMessage {
        // Collector envelopes contain source bodies. Only the search result is
        // model-visible; the auxiliary card reports status, not those bodies.
        let result = match error {
            Some(error) => pb::shell_result::Result::SpawnError(pb::ShellSpawnError {
                command: self.args.command.clone(),
                working_directory: self.args.working_directory.clone(),
                error: error.into(),
            }),
            None => pb::shell_result::Result::Success(pb::ShellSuccess {
                command: self.args.command.clone(),
                working_directory: self.args.working_directory.clone(),
                exit_code: 0,
                stdout: "Workspace collection completed.".into(),
                ..Default::default()
            }),
        };
        server_interaction(pb::interaction_update::Message::ToolCallCompleted(
            pb::ToolCallCompletedUpdate {
                call_id: self.args.tool_call_id.clone(),
                tool_call: Some(self.card(Some(pb::ShellResult {
                    result: Some(result),
                    ..Default::default()
                }))),
                model_call_id: model_call_id.into(),
            },
        ))
    }
}

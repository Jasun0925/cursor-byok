//! Persists complete file snapshots for Cursor's change review.
use crate::{cursor::protocol::proto::agent::v1 as pb, Result};

use super::{steps::PendingSteps, CheckpointBuilder};

impl CheckpointBuilder {
    pub(super) async fn project_files(&mut self, presentation: &PendingSteps) -> Result<()> {
        for snapshot in &presentation.edit_snapshots {
            if snapshot.path.is_empty() {
                continue;
            }
            let initial_content = match self.base.file_states_v2.get(&snapshot.path) {
                Some(state) => state.initial_content.clone(),
                None => Some(
                    self.sync
                        .persist(snapshot.before.as_bytes(), &[])
                        .await?
                        .as_bytes()
                        .to_vec(),
                ),
            };
            let content = self.sync.persist(snapshot.after.as_bytes(), &[]).await?;
            self.base.file_states_v2.insert(
                snapshot.path.clone(),
                pb::FileStateStructure {
                    content: Some(content.as_bytes().to_vec()),
                    initial_content,
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tokio::sync::mpsc;

    use crate::{
        cursor::{
            protocol::{connect, proto::agent::v1 as pb},
            services::{blob_sync::BlobSynchronizer, observability::CursorTraceService},
            tools::tool_call_result::ToolCompletion,
            transport::{OutputHub, TransportHandle},
        },
        model::{ToolCall, ToolResult},
        store::{BlobId, Store},
    };

    fn edit_tool(path: &str, before: &str, after: &str, success: bool) -> pb::tool_call::Tool {
        let result = if success {
            pb::edit_result::Result::Success(pb::EditSuccess {
                path: path.into(),
                lines_added: Some(1),
                lines_removed: Some(1),
                diff_string: Some(format!("--- {path}\n")),
                before_full_file_content: Some(before.into()),
                after_full_file_content: after.into(),
                message: None,
            })
        } else {
            pb::edit_result::Result::Error(pb::EditError {
                path: path.into(),
                error: "rejected".into(),
                model_visible_error: Some("rejected".into()),
            })
        };
        pb::tool_call::Tool::EditToolCall(pb::EditToolCall {
            args: None,
            result: Some(pb::EditResult {
                result: Some(result),
            }),
        })
    }

    fn completion(name: &str, tool: pb::tool_call::Tool) -> ToolCompletion {
        ToolCompletion::new(
            &ToolCall {
                index: 0,
                call_id: "call".into(),
                model_call_id: "model".into(),
                name: name.into(),
                arguments_text: "{}".into(),
                arguments: serde_json::json!({}),
                argument_error: None,
            },
            0,
            ToolResult {
                call_id: "call".into(),
                content: "edited".into(),
                is_error: !matches!(
                    &tool,
                    pb::tool_call::Tool::EditToolCall(edit)
                        if matches!(
                            edit.result.as_ref().and_then(|result| result.result.as_ref()),
                            Some(pb::edit_result::Result::Success(_))
                        )
                ),
                image: None,
            },
            tool,
        )
    }

    #[test]
    fn successful_edit_keeps_full_file_text_after_the_model_gate() {
        let completion = completion("StrReplace", edit_tool("a.txt", "before", "after", true));
        let snapshot = completion.edit_snapshot.clone().expect("snapshot");
        assert_eq!(snapshot.before, "before");
        assert_eq!(snapshot.after, "after");
        let pb::tool_call::Tool::EditToolCall(edit) = completion.tool_call().tool.as_ref().unwrap()
        else {
            panic!("edit tool");
        };
        let pb::edit_result::Result::Success(success) =
            edit.result.as_ref().unwrap().result.as_ref().unwrap()
        else {
            panic!("success");
        };
        assert!(success.before_full_file_content.is_none());
        assert!(success.after_full_file_content.is_empty());
    }

    #[test]
    fn failed_edit_is_not_a_review_snapshot() {
        let completion = completion("Write", edit_tool("a.txt", "before", "after", false));
        assert!(completion.edit_snapshot.is_none());
    }

    #[tokio::test]
    async fn repeated_successful_edits_keep_the_first_baseline() {
        let store = Store::connect("sqlite::memory:").await.unwrap();
        let traces = CursorTraceService::new(store.clone());
        let (commands, _receiver) = mpsc::channel(8);
        let output = Arc::new(OutputHub::default());
        let handle = TransportHandle::new(
            "file-review".into(),
            commands,
            output,
            traces.recorder("file-review"),
        );
        let sync = BlobSynchronizer::new("file-review".into(), store.clone(), handle.clone());
        let mut frames = handle.subscribe();
        let acknowledgements = sync.clone();
        let ack = tokio::spawn(async move {
            while let Some(frame) = frames.recv().await {
                let message = connect::decode_unary::<pb::AgentServerMessage>(&frame).unwrap();
                let Some(pb::agent_server_message::Message::KvServerMessage(kv)) = message.message
                else {
                    continue;
                };
                if !matches!(
                    kv.message,
                    Some(pb::kv_server_message::Message::SetBlobArgs(_))
                ) {
                    continue;
                }
                acknowledgements
                    .handle_client(pb::KvClientMessage {
                        id: kv.id,
                        message: Some(pb::kv_client_message::Message::SetBlobResult(
                            pb::SetBlobResult { error: None },
                        )),
                    })
                    .await
                    .unwrap();
            }
        });

        let mut builder = CheckpointBuilder::new(store.clone(), sync, None, None);
        let mut steps = crate::cursor::checkpoint::StepBuffer::default();
        steps.tool_completed(&completion(
            "Write",
            edit_tool("a.txt", "one\n", "two\n", true),
        ));
        steps.tool_completed(&completion(
            "StrReplace",
            edit_tool("a.txt", "two\n", "three\n", true),
        ));
        steps.tool_completed(&completion(
            "Write",
            edit_tool("b.txt", "keep", "gone", false),
        ));
        builder.project_files(&steps.take()).await.unwrap();

        assert!(!builder.base.file_states_v2.contains_key("b.txt"));
        let state = builder.base.file_states_v2.get("a.txt").unwrap();
        let initial = store
            .get_blob(&BlobId::from_bytes(state.initial_content.as_ref().unwrap()).unwrap())
            .await
            .unwrap()
            .unwrap();
        let current = store
            .get_blob(&BlobId::from_bytes(state.content.as_ref().unwrap()).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(initial, b"one\n");
        assert_eq!(current, b"three\n");
        drop(builder);
        ack.abort();
    }
}

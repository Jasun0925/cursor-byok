//! Diagnostic for issue #288. Real BYOK transport/store with a simulated model and MCP client.
//! Copy to server/tests/issue288_mcp_switch_probe.rs before running.
//! Filter model_switch_: four MCP scenarios should pass on the recorded baseline.
//! Filter checkpoint_roundtrip_preserves_ids_without_model_switch: independent known failure.
//! Not part of the default test suite; no production repair was made.
#[path = "support/fake_provider.rs"]
mod fake_provider;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::{sync::Arc, time::Duration};

use cursor_server::{
    cursor::{
        prompting::{PromptAssets, PromptCompiler},
        protocol::{connect, proto::agent::v1 as pb},
        TransportCommand, TransportHandle, TransportRegistry,
    },
    model::ProjectedContent,
    provider::{FinishReason, ModelEvent},
    store::{BlobId, Store},
};
use prost::Message;
use serde_json::json;

#[derive(Clone, Copy, Debug)]
enum Source {
    RunTools,
    InlineContext,
    CachedContext,
    MetaCachedContext,
}

#[tokio::test]
async fn model_switch_preserves_direct_run_mcp() {
    verify(Source::RunTools, true, false).await;
}

#[tokio::test]
async fn model_switch_preserves_inline_context_mcp() {
    verify(Source::InlineContext, true, false).await;
}

#[tokio::test]
async fn model_switch_preserves_cached_context_mcp() {
    verify(Source::CachedContext, true, false).await;
}

#[tokio::test]
async fn model_switch_preserves_cached_call_mcp_tool_routes() {
    verify(Source::MetaCachedContext, true, false).await;
}

fn registry(store: Store, provider: &fake_provider::FakeProvider) -> TransportRegistry {
    let assets =
        PromptAssets::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prompt/cursor"))
            .unwrap();
    TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(assets),
    )
}

fn definition() -> pb::McpToolDefinition {
    pb::McpToolDefinition {
        name: "mcp_issue288_lookup".into(),
        provider_identifier: "issue288-provider".into(),
        tool_name: "lookup".into(),
        description: "Read an isolated test marker".into(),
        input_schema_json: Some(
            json!({"type":"object","properties":{"key":{"type":"string"}},"required":["key"]})
                .to_string(),
        ),
        ..Default::default()
    }
}

fn context(source: Source) -> pb::RequestContext {
    if matches!(source, Source::MetaCachedContext) {
        pb::RequestContext {
            mcp_meta_tool_options: Some(pb::McpMetaToolOptions {
                enabled: true,
                mcp_descriptors: vec![pb::McpDescriptor {
                    server_name: "issue288-provider".into(),
                    server_identifier: "issue288-server".into(),
                    tools: vec![pb::McpToolDescriptor {
                        tool_name: "lookup".into(),
                        description: Some("Read an isolated test marker".into()),
                        input_schema_json: definition().input_schema_json,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
            }),
            ..Default::default()
        }
    } else {
        pb::RequestContext {
            tools: vec![definition()],
            ..Default::default()
        }
    }
}

#[tokio::test]
async fn checkpoint_roundtrip_preserves_ids_without_model_switch() {
    verify(Source::RunTools, false, true).await;
}

async fn verify(source: Source, switch_models: bool, strict_ids: bool) {
    let (_dir, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    let mut registry = registry(store.clone(), &provider);
    let context = context(source);
    let cached = matches!(source, Source::CachedContext | Source::MetaCachedContext);
    let meta = matches!(source, Source::MetaCachedContext);
    let part = pb::RequestContextMcpsPart {
        tools: context.tools.clone(),
        mcp_meta_tool_options: context.mcp_meta_tool_options.clone(),
        ..Default::default()
    }
    .encode_to_vec();
    let part_id = BlobId::digest(&part);
    let models = if switch_models {
        ["claude-opus-4.8", "claude-opus-4.6", "claude-opus-4.8"]
    } else {
        ["claude-opus-4.8"; 3]
    };
    let mut identity_changes = Vec::new();
    let mut checkpoint = None;
    let mut context_loads = Vec::new();
    for (turn, model) in models.into_iter().enumerate() {
        // The final A turn uses a new registry and the same persisted Store.
        if turn == 2 {
            registry.shutdown().await;
            registry = self::registry(store.clone(), &provider);
        }
        let marker = format!("ISSUE288_OK_{turn}");
        let args = if meta {
            json!({"server":"issue288-server","toolName":"lookup","arguments":{"key":marker}})
        } else {
            json!({"key":marker})
        };
        provider.push(vec![
            ModelEvent::Start {
                model_call_id: format!("model-{turn}-tool"),
            },
            ModelEvent::ToolCallStart {
                index: 0,
                call_id: format!("issue288-call-{turn}"),
                name: if meta {
                    "CallMcpTool".into()
                } else {
                    definition().name
                },
            },
            ModelEvent::ToolCallArgumentsDelta {
                index: 0,
                delta: args.to_string(),
            },
            ModelEvent::ToolCallEnd { index: 0 },
            ModelEvent::Done(FinishReason::ToolUse),
        ]);
        provider.push(vec![
            ModelEvent::Start {
                model_call_id: format!("model-{turn}-answer"),
            },
            ModelEvent::TextStart,
            ModelEvent::TextDelta(format!("completed {turn}")),
            ModelEvent::TextEnd,
            ModelEvent::Done(FinishReason::Stop),
        ]);
        let request = pb::AgentRunRequest {
            conversation_id: Some("issue288-conversation".into()),
            conversation_state: checkpoint,
            requested_model: Some(pb::RequestedModel {
                model_id: model.into(),
                ..Default::default()
            }),
            mcp_tools: matches!(source, Source::RunTools).then(|| pb::McpTools {
                mcp_tools: vec![definition()],
            }),
            action: Some(pb::ConversationAction {
                request_context_parts: cached.then(|| pb::RequestContextPartReferences {
                    mcps_blob_id: part_id.as_bytes().to_vec(),
                    mcps_byte_length: part.len() as u32,
                    ..Default::default()
                }),
                action: Some(pb::conversation_action::Action::UserMessageAction(
                    pb::UserMessageAction {
                        user_message: Some(pb::UserMessage {
                            message_id: format!("issue288-user-{turn}"),
                            text: "Read the marker using the MCP tool".into(),
                            mode: pb::AgentMode::Agent as i32,
                            ..Default::default()
                        }),
                        request_context: matches!(source, Source::InlineContext)
                            .then(|| context.clone()),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }),
            ..Default::default()
        };
        let handle = registry
            .get_or_create(&format!("issue288-request-{turn}"))
            .await
            .unwrap();
        let (state, loads) = drive(&handle, request, &context, &marker, meta).await;
        checkpoint = Some(state);
        context_loads.push(loads);
        let requests = provider.requests();
        assert_eq!(requests.len(), (turn + 1) * 2);
        let first = &requests[turn * 2];
        let followup = &requests[turn * 2 + 1];
        assert_eq!(first.model.model_id, model);
        assert_eq!(followup.model.model_id, model);
        let tool_name = if meta {
            "CallMcpTool"
        } else {
            "mcp_issue288_lookup"
        };
        assert!(first.prompt.tools.iter().any(|tool| tool.name == tool_name));
        assert!(followup.history.iter().any(|message| matches!(&message.content,
            ProjectedContent::ToolResult(result) if result.call_id == format!("issue288-call-{turn}") && !result.is_error && result.content.contains(&marker)
        )), "MCP success must reach the model after switching");
        if turn > 0 {
            let previous = &requests[turn * 2 - 1];
            let current_prefix = &first.history[..previous.history.len()];
            assert_eq!(
                previous
                    .history
                    .iter()
                    .map(|m| (&m.role, &m.content))
                    .collect::<Vec<_>>(),
                current_prefix
                    .iter()
                    .map(|m| (&m.role, &m.content))
                    .collect::<Vec<_>>(),
                "roles, text, tool arguments, call IDs and results must retain their structure"
            );
            let changed = previous
                .history
                .iter()
                .zip(current_prefix)
                .filter(|(before, after)| before.message_id != after.message_id)
                .count();
            identity_changes.push(changed);
            for (before, after) in previous.history.iter().zip(current_prefix) {
                if before.message_id.starts_with("request-context:")
                    || before.message_id.starts_with("runtime:")
                {
                    assert_eq!(before.message_id, after.message_id);
                }
            }
            if strict_ids {
                assert_eq!(
                    changed, 0,
                    "checkpoint changes message IDs even without a model switch"
                );
            }
            assert_eq!(previous.prompt.tools, first.prompt.tools);
        }
    }
    assert_eq!(
        context_loads,
        if cached { vec![1, 0, 0] } else { vec![0, 0, 0] }
    );
    registry.shutdown().await;
    println!("{source:?}: A -> B -> A passed; 3 MCP execs, 6 model requests; context loads {context_loads:?}; registry restarted before final A; internal ID changes {identity_changes:?}");
}

async fn drive(
    handle: &TransportHandle,
    request: pb::AgentRunRequest,
    context: &pb::RequestContext,
    marker: &str,
    meta: bool,
) -> (pb::ConversationStateStructure, usize) {
    let mut output = handle.subscribe();
    append(
        handle,
        0,
        pb::agent_client_message::Message::RunRequest(request),
    )
    .await;
    let mut seq = 1;
    let mut checkpoint = None;
    let mut context_loads = 0;
    let mut execs = 0;
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), output.recv())
            .await
            .unwrap()
            .unwrap();
        for (flags, payload) in connect::decode_frames(&frame).unwrap() {
            if flags & connect::END_STREAM_FLAG != 0 {
                let terminal: serde_json::Value = serde_json::from_slice(&payload).unwrap();
                assert!(terminal.get("error").is_none(), "{terminal}");
                assert_eq!(execs, 1, "MCP must reach client exactly once per turn");
                return (checkpoint.expect("final checkpoint"), context_loads);
            }
            let response = match pb::AgentServerMessage::decode(payload).unwrap().message {
                Some(pb::agent_server_message::Message::KvServerMessage(kv)) => {
                    assert!(matches!(
                        kv.message,
                        Some(pb::kv_server_message::Message::SetBlobArgs(_))
                    ));
                    Some(pb::agent_client_message::Message::KvClientMessage(
                        pb::KvClientMessage {
                            id: kv.id,
                            message: Some(pb::kv_client_message::Message::SetBlobResult(
                                pb::SetBlobResult { error: None },
                            )),
                        },
                    ))
                }
                Some(pb::agent_server_message::Message::ConversationCheckpointUpdate(state)) => {
                    checkpoint = Some(state);
                    None
                }
                Some(pb::agent_server_message::Message::ExecServerMessage(exec)) => {
                    let message = match exec.message.unwrap() {
                        pb::exec_server_message::Message::RequestContextArgs(_) => {
                            context_loads += 1;
                            pb::exec_client_message::Message::RequestContextResult(
                                pb::RequestContextResult {
                                    result: Some(pb::request_context_result::Result::Success(
                                        pb::RequestContextSuccess {
                                            request_context: Some(context.clone()),
                                            ..Default::default()
                                        },
                                    )),
                                },
                            )
                        }
                        pb::exec_server_message::Message::McpArgs(args) => {
                            execs += 1;
                            assert_eq!(args.provider_identifier, "issue288-provider");
                            assert_eq!(args.tool_name, "lookup");
                            if meta {
                                assert_eq!(args.server_identifier, "issue288-server");
                            }
                            assert_eq!(
                                args.args["key"].kind,
                                Some(prost_types::value::Kind::StringValue(marker.into()))
                            );
                            pb::exec_client_message::Message::McpResult(pb::McpResult {
                                result: Some(pb::mcp_result::Result::Success(pb::McpSuccess {
                                    content: vec![pb::McpToolResultContentItem {
                                        content: Some(
                                            pb::mcp_tool_result_content_item::Content::Text(
                                                pb::McpTextContent {
                                                    text: marker.into(),
                                                    output_location: None,
                                                },
                                            ),
                                        ),
                                    }],
                                    is_error: false,
                                    structured_content: None,
                                })),
                            })
                        }
                        other => panic!("unexpected exec {other:?}"),
                    };
                    Some(pb::agent_client_message::Message::ExecClientMessage(
                        pb::ExecClientMessage {
                            id: exec.id,
                            message: Some(message),
                            ..Default::default()
                        },
                    ))
                }
                _ => None,
            };
            if let Some(response) = response {
                append(handle, seq, response).await;
                seq += 1;
            }
        }
    }
}

async fn append(handle: &TransportHandle, seqno: i64, message: pb::agent_client_message::Message) {
    handle
        .command(TransportCommand::Append {
            seqno,
            message: Box::new(pb::AgentClientMessage {
                message: Some(message),
            }),
        })
        .await
        .unwrap();
}

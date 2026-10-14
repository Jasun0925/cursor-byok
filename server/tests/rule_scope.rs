//! Scope-bearing rules must remain distinguishable in provider-visible request context.
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
    model::{ContentPart, ProjectedContent},
    provider::{FinishReason, ModelEvent},
};
use prost::Message;
use serde_json::json;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn nested_agents_rule_keeps_its_source_path() {
    let rule = pb::CursorRule {
        full_path: "/probe/frontend/AGENTS.md".into(),
        content: "Use FRONTEND_STYLE for files governed by this rule.".into(),
        ..Default::default()
    };
    let projected = project(rule.clone()).await;
    assert!(
        projected.contains(&rule.content),
        "fixture rule must reach provider"
    );
    assert!(
        projected.contains(&rule.full_path),
        "nested AGENTS.md source path was discarded"
    );
}

#[tokio::test]
async fn different_file_globs_produce_distinct_provider_context() {
    let frontend = glob_rule("frontend/**/*.tsx");
    let backend = glob_rule("backend/**/*.py");
    let first = project(frontend).await;
    let second = project(backend).await;
    assert_ne!(
        first, second,
        "changing the only applicability condition must change provider-visible context"
    );
}

fn glob_rule(glob: &str) -> pb::CursorRule {
    pb::CursorRule {
        full_path: "/probe/.cursor/rules/module.mdc".into(),
        content: "Apply MODULE_STYLE to matching files.".into(),
        r#type: Some(pb::CursorRuleType {
            r#type: Some(pb::cursor_rule_type::Type::FileGlobbed(
                pb::CursorRuleTypeFileGlobs {
                    globs: vec![glob.into()],
                },
            )),
        }),
        ..Default::default()
    }
}

async fn project(rule: pb::CursorRule) -> String {
    let (_directory, store) = fixtures::temp_store().await;
    let provider = fake_provider::FakeProvider::default();
    provider.push(vec![
        ModelEvent::Start {
            model_call_id: "scope-probe".into(),
        },
        ModelEvent::TextStart,
        ModelEvent::TextDelta("ok".into()),
        ModelEvent::TextEnd,
        ModelEvent::Done(FinishReason::Stop),
    ]);
    let registry = TransportRegistry::new(
        store,
        Arc::new(provider.clone()),
        PromptCompiler::new(PromptAssets::embedded().unwrap()),
    );
    let handle = registry.get_or_create("scope-request").await.unwrap();
    let mut output = handle.subscribe();
    handle
        .command(TransportCommand::Append {
            seqno: 0,
            message: Box::new(pb::AgentClientMessage {
                message: Some(pb::agent_client_message::Message::RunRequest(
                    pb::AgentRunRequest {
                        conversation_id: Some("scope-conversation".into()),
                        run_id: Some("scope-run".into()),
                        requested_model: Some(pb::RequestedModel {
                            model_id: "test-model".into(),
                            ..Default::default()
                        }),
                        action: Some(pb::ConversationAction {
                            action: Some(pb::conversation_action::Action::UserMessageAction(
                                pb::UserMessageAction {
                                    user_message: Some(pb::UserMessage {
                                        text: "Edit frontend/component.tsx only.".into(),
                                        message_id: "scope-user".into(),
                                        mode: pb::AgentMode::Agent as i32,
                                        ..Default::default()
                                    }),
                                    request_context: Some(pb::RequestContext {
                                        rules: vec![rule],
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
            }),
        })
        .await
        .unwrap();
    let mut seqno = 1;
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), output.recv())
            .await
            .unwrap()
            .unwrap();
        let (flags, payload) = connect::decode_frames(&frame).unwrap().pop().unwrap();
        if flags & connect::END_STREAM_FLAG != 0 {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&payload).unwrap(),
                json!({})
            );
            break;
        }
        if let Some(pb::agent_server_message::Message::KvServerMessage(kv)) =
            pb::AgentServerMessage::decode(payload).unwrap().message
        {
            assert!(matches!(
                kv.message,
                Some(pb::kv_server_message::Message::SetBlobArgs(_))
            ));
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
    }
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    let contexts: Vec<_> = requests[0]
        .history
        .iter()
        .filter(|m| m.message_id.starts_with("request-context:"))
        .collect();
    assert_eq!(contexts.len(), 1);
    let ProjectedContent::Parts(parts) = &contexts[0].content else {
        panic!("context must contain text")
    };
    let [ContentPart::Text { text }] = parts.as_slice() else {
        panic!("context must contain one text part")
    };
    let result = text.clone();
    registry.shutdown().await;
    result
}

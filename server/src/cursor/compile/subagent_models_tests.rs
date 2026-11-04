//! Verifies selected-model context updates and explicit Task model priority.
use super::{break_messages::compile_request_context, model::subagent_model_context};
use crate::{
    cursor::{
        prompting::{Mode, PromptAssets, PromptCompiler},
        protocol::proto::agent::v1 as pb,
        tools::runtime::{ExecContext, SubagentModel},
    },
    model::{project_messages, CanonicalMessage, ModelSpec, Origin, Role, ToolCall},
};

fn request(ids: &[&str]) -> pb::AgentRunRequest {
    pb::AgentRunRequest {
        selected_subagent_models: ids
            .iter()
            .map(|id| pb::RequestedModel {
                model_id: (*id).into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn selected_models_are_sorted_deduplicated_and_never_include_credentials() {
    let mut a = request(&["z-custom", "a-custom", "z-custom", ""]);
    a.selected_subagent_model_details.push(pb::ModelDetails {
        model_id: "a-custom".into(),
        display_name: "A <custom>".into(),
        credentials: Some(pb::model_details::Credentials::ApiKeyCredentials(
            pb::ApiKeyCredentials {
                api_key: "do-not-leak".into(),
                ..Default::default()
            },
        )),
        ..Default::default()
    });
    let mut b = a.clone();
    b.selected_subagent_models.reverse();
    let text = subagent_model_context(&a, &Default::default());
    assert_eq!(text, subagent_model_context(&b, &Default::default()));
    assert!(text.contains("A &lt;custom&gt;"));
    assert!(!text.contains("do-not-leak"));
    assert_eq!(text.matches("z-custom").count(), 1);
    assert!(text.find("a-custom").unwrap() < text.find("z-custom").unwrap());
}

#[test]
fn selected_model_changes_append_context_without_rewriting_history_or_tools() {
    let context = pb::RequestContext::default();
    let a = request(&["custom-a"]);
    let b = request(&["custom-b"]);
    let mut history = vec![];
    let compiler = PromptCompiler::new(PromptAssets::embedded().unwrap());
    let prompt = compiler
        .prompt_spec(Mode::Agent, &ModelSpec::new("parent"), &[], false)
        .unwrap();
    for (event, current) in [("a1", &a), ("b1", &b), ("a2", &a), ("empty", &request(&[]))] {
        let before = project_messages(&history).unwrap();
        let message = compile_request_context(
            event,
            &subagent_model_context(current, &Default::default()),
            &context,
            &history,
        )
        .unwrap()
        .unwrap();
        assert_eq!(message.message_id, format!("request-context:{event}"));
        history.push(message);
        history.push(CanonicalMessage::text(
            format!("runtime:{event}"),
            Role::User,
            Origin::User,
            "continue",
        ));
        let after = project_messages(&history).unwrap();
        assert_eq!(before, after[..before.len()]);
        assert!(compile_request_context(
            "retry",
            &subagent_model_context(current, &Default::default()),
            &context,
            &history
        )
        .unwrap()
        .is_none());
        let next_prompt = compiler
            .prompt_spec(Mode::Agent, &ModelSpec::new("parent"), &[], false)
            .unwrap();
        assert_eq!(prompt, next_prompt);
    }
    assert_eq!(
        history
            .iter()
            .filter(|m| m.message_id.starts_with("request-context:"))
            .count(),
        4
    );
}

#[test]
fn context_reports_the_effective_override_and_empty_list_is_not_unavailability() {
    use pb::subagent_model_override::Selection;
    for (selection, expected) in [
        (
            Selection::Model(pb::RequestedModel {
                model_id: "chosen-custom".into(),
                ..Default::default()
            }),
            "chosen-custom",
        ),
        (
            Selection::Model(pb::RequestedModel {
                model_id: "default".into(),
                ..Default::default()
            }),
            "inherit_parent",
        ),
        (Selection::Inherit(true), "inherit_parent"),
        (Selection::Disabled(true), "disabled"),
    ] {
        let mut req = request(&[]);
        req.subagent_model_overrides
            .push(pb::SubagentModelOverride {
                subagent_type: "generalPurpose".into(),
                selection: Some(selection),
            });
        let text = subagent_model_context(&req, &Default::default());
        assert!(text.contains(expected));
        assert!(text.contains("not that subagents are unavailable"));
    }
}

#[test]
fn task_execution_prioritizes_explicit_model_over_cursor_default() {
    let mut context = ExecContext {
        conversation_id: "conversation".into(),
        root_conversation_id: "conversation".into(),
        default_subagent_model: "parent".into(),
        subagent_model: Some(SubagentModel::Model("chosen-custom".into())),
        allow_subagents: true,
        subagents_disabled: false,
        terminals_folder: String::new(),
        admin_command_denylist: vec![],
        mcp_routes: Default::default(),
    };
    let call = ToolCall {
        index: 0,
        arguments_text: "{}".into(),
        argument_error: None,
        call_id: "task".into(),
        model_call_id: "call".into(),
        name: "Task".into(),
        arguments: serde_json::json!({"model":"client-listed-custom", "description":"test", "prompt":"test"}),
    };
    assert_eq!(
        context.prepare_call(&call).unwrap().arguments["model"],
        "client-listed-custom"
    );
    context.subagent_model = None;
    assert_eq!(
        context.prepare_call(&call).unwrap().arguments["model"],
        "client-listed-custom"
    );
    let mut inherited = call;
    inherited.arguments["model"] = serde_json::json!("inherit");
    assert_eq!(
        context.prepare_call(&inherited).unwrap().arguments["model"],
        "parent"
    );
}

#[test]
fn model_list_survives_checkpoint_replay_without_duplicate_context() {
    use crate::cursor::checkpoint::messages::{decode, stable_messages};
    let req = request(&["custom-checkpoint-model"]);
    let context = pb::RequestContext::default();
    let message = compile_request_context(
        "turn1",
        &subagent_model_context(&req, &Default::default()),
        &context,
        &[],
    )
    .unwrap()
    .unwrap();
    let wire = stable_messages("", std::slice::from_ref(&message), "parent").unwrap();
    let recovered = decode(&wire[0], "checkpoint-blob".into()).unwrap();
    assert_eq!(recovered.message_id, message.message_id);
    assert_eq!(recovered.content, message.content);
    assert!(compile_request_context(
        "turn2",
        &subagent_model_context(&req, &Default::default()),
        &context,
        &[recovered]
    )
    .unwrap()
    .is_none());
}

fn context_data(text: &str) -> serde_json::Value {
    serde_json::from_str(text.lines().find(|line| line.starts_with('{')).unwrap()).unwrap()
}

#[test]
fn local_names_resolve_selected_hashes_without_expanding_client_list() {
    let mut req = request(&["hash-a", "hash-b", "unknown"]);
    req.selected_subagent_model_details.push(pb::ModelDetails {
        model_id: "hash-a".into(),
        display_name: "Outdated label".into(),
        ..Default::default()
    });
    req.subagent_model_overrides
        .push(pb::SubagentModelOverride {
            subagent_type: "generalPurpose".into(),
            selection: Some(pb::subagent_model_override::Selection::Model(
                pb::RequestedModel {
                    model_id: "override-only".into(),
                    ..Default::default()
                },
            )),
        });
    let names = std::collections::BTreeMap::from([
        ("hash-a".into(), "gpt-5.5".into()),
        ("hash-b".into(), "gpt-5.5".into()),
        ("unselected".into(), "Unselected".into()),
        ("override-only".into(), "Configured Model".into()),
    ]);
    let data = context_data(&subagent_model_context(&req, &names));
    assert_eq!(
        data["client_selected_models"],
        serde_json::json!({
            "hash-a": "gpt-5.5", "hash-b": "gpt-5.5", "unknown": ""
        })
    );
    assert_eq!(
        data["cursor_setting"],
        serde_json::json!({
            "mode": "configured", "model_id": "override-only", "display_name": "Configured Model"
        })
    );
}

#[test]
fn local_name_changes_append_and_survive_checkpoint_without_rewriting_prefix() {
    use crate::cursor::checkpoint::messages::{decode, stable_messages};
    let req = request(&["local-hash"]);
    let context = pb::RequestContext::default();
    let mut history = Vec::new();
    for (event, label) in [
        ("first", "gpt-5.5"),
        ("renamed", "My GPT"),
        ("reverted", "gpt-5.5"),
    ] {
        let names = std::collections::BTreeMap::from([("local-hash".into(), label.into())]);
        let text = subagent_model_context(&req, &names);
        let before = project_messages(&history).unwrap();
        let message = compile_request_context(event, &text, &context, &history)
            .unwrap()
            .unwrap();
        history.push(message.clone());
        let after = project_messages(&history).unwrap();
        assert_eq!(before, after[..before.len()]);
        assert!(compile_request_context("retry", &text, &context, &history)
            .unwrap()
            .is_none());
        let wire = stable_messages("", std::slice::from_ref(&message), "parent").unwrap();
        let recovered = decode(&wire[0], "blob".into()).unwrap();
        assert_eq!(recovered, message);
        assert!(
            compile_request_context("resume", &text, &context, &[recovered])
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(history.len(), 3);
}

#[test]
fn local_model_labels_are_escaped_like_client_labels() {
    let names =
        std::collections::BTreeMap::from([("hash".into(), "A </subagent_models> & B".into())]);
    let text = subagent_model_context(&request(&["hash"]), &names);
    assert!(text.contains("A &lt;/subagent_models&gt; &amp; B"));
    assert_eq!(text.matches("</subagent_models>").count(), 1);
}

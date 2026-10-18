//! Rule scope must survive compilation, context updates, and checkpoint hydration.
use super::{break_messages::compile_request_context, context};
use crate::{
    cursor::{checkpoint::messages, protocol::proto::agent::v1 as pb},
    model::{project_messages, CanonicalMessage, Origin, Role},
};

fn scoped_rule(glob: &str) -> pb::CursorRule {
    pb::CursorRule {
        full_path: "/repo/.cursor/rules/style.mdc".into(),
        content: "Use the module's conventions.".into(),
        r#type: Some(pb::CursorRuleType {
            r#type: Some(pb::cursor_rule_type::Type::FileGlobbed(
                pb::CursorRuleTypeFileGlobs {
                    globs: vec![glob.into()],
                },
            )),
        }),
        scoped_to: vec!["/repo".into()],
        frontmatter: "alwaysApply: false".into(),
        ..Default::default()
    }
}

fn request_context(rule: pb::CursorRule) -> pb::RequestContext {
    pb::RequestContext {
        rules: vec![rule],
        ..Default::default()
    }
}

#[test]
fn regular_non_file_and_selected_rules_keep_the_same_scope_metadata() {
    let mut rule = scoped_rule("frontend/**/*.{ts,tsx}");
    rule.full_path = r#"C:\repo\a&b\"style".mdc"#.into();
    rule.scoped_to = vec!["repo<one>".into(), "repo&two".into()];
    rule.frontmatter = "description: \"<style>\"".into();
    let regular = context::compile_context(&request_context(rule.clone()), "2026-10-14");
    let non_file = context::compile_context(
        &pb::RequestContext {
            non_file_rules: vec![rule.clone()],
            ..Default::default()
        },
        "2026-10-14",
    );
    let selected = context::selected_context(&pb::UserMessage {
        selected_context: Some(pb::SelectedContext {
            cursor_rules: vec![pb::SelectedCursorRule {
                rule: Some(rule.clone()),
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(regular, non_file);
    assert_eq!(regular, selected);
    for value in [
        r#"path="C:\repo\a&amp;b\&quot;style&quot;.mdc""#,
        r#"type="file_globbed""#,
        "<glob>frontend/**/*.{ts,tsx}</glob>",
        "<scope>repo&lt;one&gt;</scope>",
        "<scope>repo&amp;two</scope>",
        "<frontmatter>description: &quot;&lt;style&gt;&quot;</frontmatter>",
        &rule.content,
    ] {
        assert!(regular.contains(value), "missing {value}: {regular}");
    }
}

#[test]
fn rule_types_remain_distinct_and_agent_fetched_description_survives() {
    use pb::cursor_rule_type::Type;
    let kinds = [
        ("global", Type::Global(pb::CursorRuleTypeGlobal {})),
        (
            "manually_attached",
            Type::ManuallyAttached(pb::CursorRuleTypeManuallyAttached {}),
        ),
        (
            "agent_fetched",
            Type::AgentFetched(pb::CursorRuleTypeAgentFetched {
                description: "When editing <UI> & styles".into(),
            }),
        ),
    ];
    for (label, kind) in kinds {
        let mut rule = scoped_rule("frontend/**");
        rule.r#type = Some(pb::CursorRuleType { r#type: Some(kind) });
        let rendered = context::compile_context(&request_context(rule), "2026-10-14");
        assert!(rendered.contains(&format!("type=\"{label}\"")));
        assert!(!rendered.contains("<globs>"));
        if label == "agent_fetched" {
            assert!(rendered
                .contains("<description>When editing &lt;UI&gt; &amp; styles</description>"));
        }
    }
}

#[test]
fn scope_changes_append_context_and_survive_checkpoint_without_rewriting_prefix() {
    let mut history = Vec::new();
    for (event, glob) in [
        ("frontend", "frontend/**/*.tsx"),
        ("backend", "backend/**/*.py"),
        ("frontend-again", "frontend/**/*.tsx"),
    ] {
        let context = request_context(scoped_rule(glob));
        let before = project_messages(&history).unwrap();
        let message = compile_request_context(event, "", &context, &history)
            .unwrap()
            .expect("a scope-only change must append new context");
        assert_eq!(message.message_id, format!("request-context:{event}"));
        history.push(message.clone());
        let mut runtime = CanonicalMessage::text(
            format!("runtime:{event}"),
            Role::User,
            Origin::Runtime,
            "Edit the requested module.",
        );
        runtime.runtime_event_id = Some(event.into());
        history.push(runtime);
        let after = project_messages(&history).unwrap();
        assert_eq!(before, after[..before.len()]);
        assert!(compile_request_context("unchanged", "", &context, &history)
            .unwrap()
            .is_none());
        assert!(compile_request_context(event, "", &context, &history)
            .unwrap()
            .is_none());
        let wire = messages::stable_messages("", &history, "parent").unwrap();
        let recovered = wire
            .iter()
            .map(|value| messages::decode(value, "blob".into()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(recovered, history);
        assert!(compile_request_context("resume", "", &context, &recovered)
            .unwrap()
            .is_none());
    }
    assert_eq!(history.len(), 6);
    assert_eq!(history[0].content, history[4].content);
    assert_ne!(history[0].content, history[2].content);
}

#[test]
fn changing_only_scoped_to_or_rule_type_updates_context() {
    let initial = scoped_rule("src/**");
    let mut other_repo = initial.clone();
    other_repo.scoped_to = vec!["/other-repo".into()];
    let mut global = other_repo.clone();
    global.r#type = Some(pb::CursorRuleType {
        r#type: Some(pb::cursor_rule_type::Type::Global(
            pb::CursorRuleTypeGlobal {},
        )),
    });
    let mut history = vec![];
    for (index, rule) in [initial, other_repo, global].into_iter().enumerate() {
        let context = request_context(rule);
        let update = compile_request_context(&index.to_string(), "", &context, &history)
            .unwrap()
            .expect("scope and type are part of context identity");
        history.push(update);
    }
    assert_eq!(history.len(), 3);
}

#[test]
fn empty_and_skill_rules_stay_excluded_while_plain_rules_keep_their_content() {
    let rendered = context::compile_context(
        &pb::RequestContext {
            rules: vec![
                pb::CursorRule {
                    full_path: "/repo/empty.mdc".into(),
                    content: "  \n".into(),
                    ..Default::default()
                },
                pb::CursorRule {
                    full_path: "/repo/SKILL.md".into(),
                    content: "skill body".into(),
                    ..Default::default()
                },
            ],
            non_file_rules: vec![pb::CursorRule {
                content: "A plain user rule.".into(),
                ..Default::default()
            }],
            cloud_rule: Some("A cloud rule.".into()),
            ..Default::default()
        },
        "2026-10-14",
    );
    assert!(!rendered.contains("empty.mdc"));
    assert!(!rendered.contains("skill body"));
    assert!(rendered.contains("<user_rule>\nA plain user rule.\n</user_rule>"));
    assert!(rendered.contains("<user_rule>\nA cloud rule.\n</user_rule>"));
    assert_eq!(rendered.matches("<user_rule>").count(), 2);
    assert!(context::compile_context(&pb::RequestContext::default(), "2026-10-14").is_empty());
}

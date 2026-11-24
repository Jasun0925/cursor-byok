//! Grep wire results must preserve scope and pagination in canonical history.
use cursor_server::{
    cursor::{
        protocol::proto::agent::v1 as pb,
        tools::{
            codec,
            runtime::{CursorToolRuntime, ExecContext},
        },
    },
    model::{
        project_messages, CanonicalMessage, MessageContent, Origin, Role, ToolCall,
        ToolResultContent,
    },
    store::Store,
};
use serde_json::json;

async fn complete(success: pb::GrepSuccess) -> String {
    let runtime = CursorToolRuntime::default();
    let call = ToolCall {
        index: 0,
        call_id: "grep-call".into(),
        model_call_id: "model-call".into(),
        name: "Grep".into(),
        arguments_text: r#"{"pattern":"needle"}"#.into(),
        arguments: json!({"pattern":"needle"}),
        argument_error: None,
    };
    let id = runtime
        .reserve_exec(&call, &ExecContext::default())
        .await
        .unwrap();
    let event = codec::client_event(
        &pb::ExecClientMessage {
            id,
            message: Some(pb::exec_client_message::Message::GrepResult(
                pb::GrepResult {
                    result: Some(pb::grep_result::Result::Success(success)),
                },
            )),
            ..Default::default()
        },
        &runtime,
    )
    .await
    .unwrap();
    let codec::ClientExecEvent::Completed(completion) = event else {
        panic!("expected Grep completion")
    };
    assert!(!completion.result().is_error);
    assert!(runtime.exec_call(id).await.is_none());
    completion.result().content.clone()
}

fn result(value: pb::grep_union_result::Result) -> pb::GrepUnionResult {
    pb::GrepUnionResult {
        result: Some(value),
    }
}

fn editor(value: pb::grep_union_result::Result) -> pb::GrepSuccess {
    pb::GrepSuccess {
        pattern: "needle".into(),
        path: "/workspace".into(),
        active_editor_result: Some(result(value)),
        ..Default::default()
    }
}

#[tokio::test]
async fn file_pages_keep_limits_and_empty_later_pages_are_not_reported_as_no_matches() {
    for files in [vec!["src/found.rs".into()], Vec::new()] {
        let text = complete(editor(pb::grep_union_result::Result::Files(
            pb::GrepFilesResult {
                files: files.clone(),
                total_files: 9,
                head_limit_applied: Some(1),
                offset_applied: Some(8),
                ..Default::default()
            },
        )))
        .await;
        assert!(text.contains("head_limit=1"), "{text}");
        assert!(text.contains("offset=8"), "{text}");
        assert!(
            text.contains(&format!("shown {} of 9 files", files.len())),
            "{text}"
        );
        assert!(
            !text.contains("No matches"),
            "an empty page is not an empty search: {text}"
        );
        if !files.is_empty() {
            assert!(text.contains("src/found.rs"));
        }
    }
}

#[tokio::test]
async fn count_and_content_results_keep_totals_limits_and_line_metadata() {
    let count = complete(editor(pb::grep_union_result::Result::Count(
        pb::GrepCountResult {
            counts: vec![pb::GrepFileCount {
                file: "a.rs".into(),
                count: 3,
            }],
            total_files: 4,
            total_matches: 12,
            ripgrep_truncated: true,
            head_limit_applied: Some(1),
            offset_applied: Some(2),
            ..Default::default()
        },
    )))
    .await;
    for expected in [
        "a.rs:3",
        "12 matches in 4 files",
        "shown 1 of 4 files",
        "head_limit=1",
        "offset=2",
        "ripgrep limit reached",
    ] {
        assert!(count.contains(expected), "missing {expected}: {count}");
    }
    let content = complete(editor(pb::grep_union_result::Result::Content(
        pb::GrepContentResult {
            matches: vec![pb::GrepFileMatch {
                file: "a.rs".into(),
                matches: vec![
                    pb::GrepContentMatch {
                        line_number: 7,
                        content: "context".into(),
                        is_context_line: true,
                        ..Default::default()
                    },
                    pb::GrepContentMatch {
                        line_number: 8,
                        content: "needle".into(),
                        content_truncated: true,
                        ..Default::default()
                    },
                ],
            }],
            total_lines: 10,
            total_matched_lines: 5,
            client_truncated: true,
            head_limit_applied: Some(2),
            offset_applied: Some(4),
            ..Default::default()
        },
    )))
    .await;
    for expected in [
        "a.rs-7-context",
        "a.rs:8:needle [line truncated]",
        "5 matched lines",
        "10 output lines",
        "shown 2 of 10 output lines",
        "head_limit=2",
        "offset=4",
        "client limit reached",
    ] {
        assert!(content.contains(expected), "missing {expected}: {content}");
    }
}

#[tokio::test]
async fn workspaces_and_editor_are_named_in_deterministic_order() {
    let files = |file: &str| {
        result(pb::grep_union_result::Result::Files(pb::GrepFilesResult {
            files: vec![file.into()],
            total_files: 1,
            ..Default::default()
        }))
    };
    let mut success = pb::GrepSuccess {
        active_editor_result: Some(files("editor.rs")),
        ..Default::default()
    };
    success
        .workspace_results
        .insert("/zeta".into(), files("same.rs"));
    success
        .workspace_results
        .insert("/alpha".into(), files("same.rs"));
    let first = complete(success.clone()).await;
    let second = complete(success).await;
    assert_eq!(first, second);
    let editor = first.find("[active editor]").expect("editor scope missing");
    let alpha = first
        .find("[workspace: /alpha]")
        .expect("first workspace missing");
    let zeta = first
        .find("[workspace: /zeta]")
        .expect("second workspace missing");
    assert!(editor < alpha && alpha < zeta, "{first}");
    assert!(first[editor..alpha].contains("editor.rs"));
    assert!(first[alpha..zeta].contains("same.rs"));
    assert!(first[zeta..].contains("same.rs"));
}

#[tokio::test]
async fn pagination_survives_the_existing_size_gate_and_checkpoint_projection() {
    let text = complete(editor(pb::grep_union_result::Result::Content(
        pb::GrepContentResult {
            matches: vec![pb::GrepFileMatch {
                file: "large.rs".into(),
                matches: vec![pb::GrepContentMatch {
                    line_number: 1,
                    content: "中".repeat(40_000),
                    ..Default::default()
                }],
            }],
            total_lines: 7,
            total_matched_lines: 7,
            head_limit_applied: Some(1),
            offset_applied: Some(3),
            ..Default::default()
        },
    )))
    .await;
    assert!(text.len() <= 32 * 1024);
    assert!(text.contains("[truncated:"));
    assert!(text.contains("head_limit=1"));
    assert!(text.contains("offset=3"));
    let store = Store::connect("sqlite::memory:").await.unwrap();
    let message = CanonicalMessage {
        message_id: "grep-result".into(),
        role: Role::Tool,
        origin: Origin::Tool,
        content: MessageContent::ToolResult(ToolResultContent {
            call_id: "grep-call".into(),
            name: "Grep".into(),
            content: text,
            is_error: false,
            image: None,
            provider_parts: Vec::new(),
        }),
        runtime_event_id: None,
    };
    let checkpoint = store
        .import_checkpoint(&"grep-history".into(), std::slice::from_ref(&message))
        .await
        .unwrap();
    let mut restored = store.load_checkpoint_messages(checkpoint).await.unwrap();
    assert_eq!(restored, vec![message]);
    let first = project_messages(&restored).unwrap();
    restored.push(CanonicalMessage::text(
        "next",
        Role::User,
        Origin::User,
        "continue",
    ));
    let next = project_messages(&restored).unwrap();
    assert_eq!(first, next[..first.len()]);
}

#[tokio::test]
async fn empty_search_without_limits_remains_a_no_match_result() {
    let text = complete(editor(pb::grep_union_result::Result::Files(
        Default::default(),
    )))
    .await;
    assert!(text.contains("No matches found for pattern `needle` in /workspace"));
}

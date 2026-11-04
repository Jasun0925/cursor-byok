use semble_core::{source::SourceInventory, ContentType};
use serde_json::json;

fn select(
    entries: serde_json::Value,
    rules: serde_json::Value,
) -> semble_core::Result<Vec<String>> {
    let inventory: SourceInventory = serde_json::from_value(json!({
        "root":"/workspace", "entries":entries, "rules":rules,
    }))
    .unwrap();
    inventory.select(&[ContentType::Code], 1_000_000)
}

fn file(path: &str, git: &str) -> serde_json::Value {
    json!({"path":path,"is_dir":false,"size":20,"git_match":git})
}

fn dir(path: &str) -> serde_json::Value {
    json!({"path":path,"is_dir":true,"size":0,"git_match":"none"})
}

fn rule(directory: &str, name: &str, contents: &str) -> serde_json::Value {
    json!({"directory":directory,"name":name,"contents":contents})
}

#[test]
fn cursor_exclusions_are_independent_and_blocked_directories_stay_blocked() {
    let entries = json!([
        file("public.rs", "none"),
        file("cursor.rs", "none"),
        file("git.rs", "ignore"),
        dir("blocked"),
        file("blocked/private.rs", "none"),
        file("custom.rs", "none")
    ]);
    let rules = json!([
        rule(
            "/workspace",
            ".cursorignore",
            "cursor.rs\nblocked/\n!custom.rs\n"
        ),
        rule(
            "/workspace",
            ".sembleignore",
            "!cursor.rs\n!git.rs\ncustom.rs\n"
        ),
        rule("/workspace/blocked", ".cursorignore", "!private.rs\n"),
    ]);
    assert_eq!(select(entries, rules).unwrap(), vec!["git.rs", "public.rs"]);
}

#[test]
fn parent_nested_and_case_sensitive_rules_preserve_host_semantics() {
    let entries = json!([
        file("Case.rs", "none"),
        file("case.rs", "none"),
        file("parent.rs", "none"),
        dir("nested"),
        file("nested/private.rs", "none"),
        file("nested/public.rs", "none")
    ]);
    let rules = json!([
        rule("/", ".cursorignore", "workspace/parent.rs\n"),
        rule("/workspace", ".cursorignore", "Case.rs\nnested/*.rs\n"),
        rule("/workspace/nested", ".cursorignore", "!public.rs\n"),
    ]);
    assert_eq!(
        select(entries, rules).unwrap(),
        vec!["case.rs", "nested/public.rs"]
    );
}

#[test]
fn invalid_rules_and_incomplete_or_duplicate_inventories_fail_closed() {
    assert!(select(
        json!([file("public.rs", "none")]),
        json!([rule("/workspace", ".cursorignore", "[z-a]\n")])
    )
    .is_err());
    for entries in [
        json!([file("missing/private.rs", "none")]),
        json!([file("../private.rs", "none")]),
        json!([file("a.rs", "none"), file("a.rs", "none")]),
    ] {
        assert!(select(entries, json!([])).is_err());
    }
    assert!(select(
        json!([file("public.rs", "none")]),
        json!([rule("/unrelated", ".cursorignore", "*")])
    )
    .is_err());
}

#[test]
fn ignore_rules_override_git_matches_and_fixed_directories_cannot_be_reincluded() {
    let entries = json!([
        file("git.rs", "ignore"),
        file("other.rs", "none"),
        dir("target"),
        file("target/build.rs", "none")
    ]);
    let rules = json!([rule(
        "/workspace",
        ".ignore",
        "!git.rs\nother.rs\n!target/\n"
    )]);
    assert_eq!(select(entries, rules).unwrap(), vec!["git.rs"]);
}

#[test]
fn host_drive_and_unc_roots_use_parent_rules_without_server_filesystem_access() {
    for (root, parent) in [
        ("C:/workspace", "C:/"),
        ("//server/share/workspace", "//server/share"),
    ] {
        let inventory: SourceInventory = serde_json::from_value(json!({
            "root":root,
            "entries":[file("public.rs", "none"), file("private.rs", "none")],
            "rules":[rule(parent, ".cursorignore", "workspace/private.rs\n")],
        }))
        .unwrap();
        assert_eq!(
            inventory.select(&[ContentType::Code], 1_000_000).unwrap(),
            vec!["public.rs"]
        );
    }
}

#[test]
fn starting_inside_an_excluded_ancestor_cannot_reinclude_its_bodies() {
    for (root, parent, pattern) in [
        ("/work/private/nested", "/work", "private/\n"),
        ("C:/work/private/nested", "C:/work", "private/\n"),
    ] {
        let inventory: SourceInventory = serde_json::from_value(json!({
            "root":root, "entries":[file("private.rs", "none")],
            "rules":[rule(parent, ".cursorignore", pattern), rule(root, ".cursorignore", "!private.rs\n")],
        })).unwrap();
        assert!(inventory
            .select(&[ContentType::Code], 1_000_000)
            .unwrap()
            .is_empty());
    }
}

//! Preserves Cursor rule provenance and applicability in model-visible context.
use super::{pb, xml};

pub(super) fn section(rules: Vec<String>) -> Option<String> {
    if rules.is_empty() {
        return None;
    }
    Some(format!(
        "<rules>\nApply each rule only within its stated scope; inclusion here does not make a scoped rule global. AGENTS.md instructions apply to files in their containing directory and its descendants, with more specific AGENTS.md instructions taking precedence within that subtree. File-globbed rules apply only to matching files. Honor scoped_to restrictions and use agent-fetched rules when relevant to their description.\n{}\n</rules>",
        rules.join("\n")
    ))
}

pub(super) fn compile(rule: &pb::CursorRule) -> String {
    use pb::cursor_rule_type::Type;

    let mut opening = String::from("<user_rule");
    if !rule.full_path.is_empty() {
        opening.push_str(&format!(" path=\"{}\"", xml(&rule.full_path)));
    }
    let kind = rule.r#type.as_ref().and_then(|kind| kind.r#type.as_ref());
    if let Some(kind) = kind {
        opening.push_str(&format!(
            " type=\"{}\"",
            match kind {
                Type::Global(_) => "global",
                Type::FileGlobbed(_) => "file_globbed",
                Type::AgentFetched(_) => "agent_fetched",
                Type::ManuallyAttached(_) => "manually_attached",
            }
        ));
    }
    opening.push('>');
    let mut lines = vec![opening];
    match kind {
        Some(Type::FileGlobbed(globbed)) => {
            lines.push("<globs>".into());
            lines.extend(
                globbed
                    .globs
                    .iter()
                    .map(|glob| format!("<glob>{}</glob>", xml(glob))),
            );
            lines.push("</globs>".into());
        }
        Some(Type::AgentFetched(fetched)) => {
            lines.push(format!(
                "<description>{}</description>",
                xml(&fetched.description)
            ));
        }
        _ => {}
    }
    if !rule.scoped_to.is_empty() {
        lines.push("<scoped_to>".into());
        lines.extend(
            rule.scoped_to
                .iter()
                .map(|scope| format!("<scope>{}</scope>", xml(scope))),
        );
        lines.push("</scoped_to>".into());
    }
    if !rule.frontmatter.is_empty() {
        lines.push(format!(
            "<frontmatter>{}</frontmatter>",
            xml(&rule.frontmatter)
        ));
    }
    lines.push(rule.content.clone());
    lines.push("</user_rule>".into());
    lines.join("\n")
}

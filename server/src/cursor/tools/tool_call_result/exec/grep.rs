//! Preserves Grep result scopes, pagination, and matching data for replay.
use crate::{cursor::protocol::proto::agent::v1 as pb, Error, Result};

pub(super) fn output(value: &pb::GrepResult) -> Result<(String, bool)> {
    use pb::grep_result::Result as R;
    match value.result.as_ref() {
        Some(R::Success(value)) => Ok((success(value), false)),
        Some(R::Error(value)) => Ok((value.error.clone(), true)),
        None => Err(Error::Protocol("grep result is missing".into())),
    }
}

fn success(value: &pb::GrepSuccess) -> String {
    let mut lines = Vec::new();
    if let Some(result) = &value.active_editor_result {
        append_scope(result, "[active editor]".into(), &mut lines);
    }
    let mut workspaces = value.workspace_results.iter().collect::<Vec<_>>();
    workspaces.sort_unstable_by_key(|(name, _)| *name);
    for (workspace, result) in workspaces {
        let label = if workspace.is_empty() {
            String::new()
        } else {
            format!("[workspace: {workspace}]")
        };
        append_scope(result, label, &mut lines);
    }
    if lines.is_empty() {
        format!(
            "No matches found for pattern `{}` in {}",
            value.pattern, value.path
        )
    } else {
        lines.join("\n")
    }
}

fn append_scope(value: &pb::GrepUnionResult, label: String, lines: &mut Vec<String>) {
    let mut body = Vec::new();
    format_result(value, &mut body);
    if body.is_empty() {
        return;
    }
    if !label.is_empty() {
        lines.push(label);
    }
    lines.extend(body);
}

fn format_result(value: &pb::GrepUnionResult, lines: &mut Vec<String>) {
    use pb::grep_union_result::Result as R;
    match value.result.as_ref() {
        Some(R::Files(value)) => {
            lines.extend(limit_notice(
                value.files.len(),
                value.total_files,
                "files",
                value.head_limit_applied,
                value.offset_applied,
                value.client_truncated,
                value.ripgrep_truncated,
            ));
            lines.extend(value.files.iter().cloned());
        }
        Some(R::Count(value)) => {
            lines.extend(limit_notice(
                value.counts.len(),
                value.total_files,
                "files",
                value.head_limit_applied,
                value.offset_applied,
                value.client_truncated,
                value.ripgrep_truncated,
            ));
            if value.total_files > 0 || value.total_matches > 0 {
                lines.push(format!(
                    "[total: {} matches in {} files]",
                    value.total_matches, value.total_files
                ));
            }
            lines.extend(
                value
                    .counts
                    .iter()
                    .map(|count| format!("{}:{}", count.file, count.count)),
            );
        }
        Some(R::Content(value)) => {
            let shown = value.matches.iter().map(|file| file.matches.len()).sum();
            lines.extend(limit_notice(
                shown,
                value.total_lines,
                "output lines",
                value.head_limit_applied,
                value.offset_applied,
                value.client_truncated,
                value.ripgrep_truncated,
            ));
            if value.total_lines > 0 || value.total_matched_lines > 0 {
                lines.push(format!(
                    "[total: {} matched lines, {} output lines]",
                    value.total_matched_lines, value.total_lines
                ));
            }
            for file in &value.matches {
                lines.extend(file.matches.iter().map(|matched| {
                    let separator = if matched.is_context_line { '-' } else { ':' };
                    let truncated = if matched.content_truncated {
                        " [line truncated]"
                    } else {
                        ""
                    };
                    format!(
                        "{}{separator}{}{separator}{}{truncated}",
                        file.file,
                        matched.line_number,
                        matched.content.trim_end_matches(['\r', '\n'])
                    )
                }));
            }
        }
        None => {}
    }
}

// Put page information before the payload so the existing byte gate cannot
// turn a long partial result into an apparently complete result.
fn limit_notice(
    shown: usize,
    total: i32,
    unit: &str,
    head_limit: Option<i32>,
    offset: Option<i32>,
    client_truncated: bool,
    ripgrep_truncated: bool,
) -> Option<String> {
    let total = usize::try_from(total).unwrap_or_default().max(shown);
    let head_limit = head_limit.filter(|value| *value > 0);
    let offset = offset.filter(|value| *value > 0);
    if !client_truncated
        && !ripgrep_truncated
        && head_limit.is_none()
        && offset.is_none()
        && total == shown
    {
        return None;
    }
    let mut details = vec![format!("shown {shown} of {total} {unit}")];
    if let Some(limit) = head_limit {
        details.push(format!("head_limit={limit}"));
    }
    if let Some(offset) = offset {
        details.push(format!("offset={offset}"));
    }
    if client_truncated {
        details.push("client limit reached".into());
    }
    if ripgrep_truncated {
        details.push("ripgrep limit reached".into());
    }
    Some(format!("[Grep results limited: {}]", details.join("; ")))
}

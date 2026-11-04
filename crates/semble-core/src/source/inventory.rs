//! Select bodies from a physical host inventory before requesting any content.

use std::{collections::HashSet, path::Path};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};

use super::{local::IGNORED_DIRS, MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_FILES};
use crate::{language::content_type_for_path, ContentType, Error, Result};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitMatch {
    None,
    Allow,
    Ignore,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InventoryEntry {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    /// Evaluated by Git on the workspace host, including its global/exclude rules.
    pub git_match: GitMatch,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IgnoreRule {
    /// Absolute host directory, with forward slashes as separators.
    pub directory: String,
    pub name: String,
    pub contents: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SourceInventory {
    pub root: String,
    pub entries: Vec<InventoryEntry>,
    pub rules: Vec<IgnoreRule>,
}

impl SourceInventory {
    pub fn select(&self, content: &[ContentType], max_file_bytes: u64) -> Result<Vec<String>> {
        if !absolute_path(&self.root) || self.entries.len() > MAX_SNAPSHOT_FILES {
            return invalid("invalid source root or inventory size");
        }
        let mut seen = HashSet::new();
        let mut directories = HashSet::new();
        for entry in &self.entries {
            if !relative_path(&entry.path) || !seen.insert(&entry.path) {
                return invalid("invalid or duplicate inventory path");
            }
            if entry.is_dir {
                directories.insert(entry.path.as_str());
            }
        }
        for entry in &self.entries {
            if let Some((parent, _)) = entry.path.rsplit_once('/') {
                if !directories.contains(parent) {
                    return invalid("inventory is missing a parent directory");
                }
            }
        }
        let rules = compile_rules(self, &directories)?;
        // Starting inside an excluded directory must not bypass its parent's
        // rule, including when the selected root is several levels below it.
        let mut ancestor = Some(self.root.as_str());
        while let Some(path) = ancestor {
            if matched(&rules, ".cursorignore", path, true) == Some(true)
                || matched(&rules, ".sembleignore", path, true)
                    .or_else(|| matched(&rules, ".ignore", path, true))
                    == Some(true)
            {
                return Ok(Vec::new());
            }
            ancestor = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .filter(|p| !p.is_empty());
        }
        // Parents precede their descendants. Their exclusion cannot be undone
        // by a rule inside a directory that was never allowed to be traversed.
        let mut entries = self.entries.iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        let mut blocked = HashSet::new();
        let mut selected = Vec::new();
        let mut total_bytes = 0_u64;
        for entry in entries {
            let parent_blocked = entry
                .path
                .rsplit_once('/')
                .is_some_and(|(parent, _)| blocked.contains(parent));
            let path = join(&self.root, &entry.path);
            let cursor = matched(&rules, ".cursorignore", &path, entry.is_dir);
            let other = matched(&rules, ".sembleignore", &path, entry.is_dir)
                .or_else(|| matched(&rules, ".ignore", &path, entry.is_dir))
                .unwrap_or(matches!(entry.git_match, GitMatch::Ignore));
            let fixed =
                entry.is_dir && IGNORED_DIRS.contains(&entry.path.rsplit('/').next().unwrap());
            if parent_blocked || fixed || cursor == Some(true) || other {
                if entry.is_dir {
                    blocked.insert(entry.path.as_str());
                }
                continue;
            }
            if entry.is_dir || entry.size == 0 || entry.size > max_file_bytes {
                continue;
            }
            if !content_type_for_path(Path::new(&entry.path))
                .is_some_and(|kind| content.contains(&kind))
            {
                continue;
            }
            total_bytes = total_bytes.saturating_add(entry.size);
            if total_bytes > MAX_SNAPSHOT_BYTES as u64 {
                return invalid("selected source bodies exceed the snapshot size limit");
            }
            selected.push(entry.path.clone());
        }
        Ok(selected)
    }
}

struct CompiledRule<'a> {
    directory: &'a str,
    name: &'a str,
    matcher: Gitignore,
}

fn compile_rules<'a>(
    inventory: &'a SourceInventory,
    directories: &HashSet<&str>,
) -> Result<Vec<CompiledRule<'a>>> {
    let mut seen = HashSet::new();
    let mut bytes = 0_usize;
    let mut rules = Vec::new();
    for rule in &inventory.rules {
        let ancestor = within(&rule.directory, &inventory.root);
        let nested = rule
            .directory
            .strip_prefix(&format!("{}/", inventory.root.trim_end_matches('/')))
            .is_some_and(|relative| directories.contains(relative));
        bytes = bytes.saturating_add(rule.contents.len());
        if !absolute_path(&rule.directory)
            || (!ancestor && !nested)
            || !matches!(
                rule.name.as_str(),
                ".cursorignore" | ".sembleignore" | ".ignore"
            )
            || !seen.insert((&rule.directory, &rule.name))
            || rule.contents.len() > 32768
            || bytes > 262144
        {
            return invalid("invalid or oversized ignore rules");
        }
        let mut builder = GitignoreBuilder::new(&rule.directory);
        for line in rule.contents.lines() {
            builder
                .add_line(None, line)
                .map_err(|error| Error::InvalidRequest(error.to_string()))?;
        }
        rules.push(CompiledRule {
            directory: &rule.directory,
            name: &rule.name,
            matcher: builder
                .build()
                .map_err(|error| Error::InvalidRequest(error.to_string()))?,
        });
    }
    rules.sort_by_key(|rule| std::cmp::Reverse(rule.directory.len()));
    Ok(rules)
}

fn matched(rules: &[CompiledRule<'_>], name: &str, path: &str, is_dir: bool) -> Option<bool> {
    for rule in rules {
        if rule.name != name || rule.directory == path || !within(rule.directory, path) {
            continue;
        }
        let matched = rule.matcher.matched(path, is_dir);
        if !matched.is_none() {
            return Some(matched.is_ignore());
        }
    }
    None
}

fn join(root: &str, relative: &str) -> String {
    format!("{}/{relative}", root.trim_end_matches('/'))
}

fn within(base: &str, path: &str) -> bool {
    base == path
        || path
            .strip_prefix(base.trim_end_matches('/'))
            .is_some_and(|tail| tail.starts_with('/'))
}

fn absolute_path(path: &str) -> bool {
    if let Some(tail) = path.strip_prefix("//") {
        return relative_path(tail) && tail.split('/').count() >= 2;
    }
    if path.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && path.as_bytes().get(1..3) == Some(b":/")
    {
        return path.len() == 3 || relative_path(&path[3..]);
    }
    path == "/" || path.strip_prefix('/').is_some_and(relative_path)
}

fn relative_path(path: &str) -> bool {
    path.len() <= 4096
        && !path.contains(['\\', '\0'])
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn invalid<T>(message: &str) -> Result<T> {
    Err(Error::InvalidRequest(message.into()))
}

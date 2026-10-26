//! Applies Cursor exclusions independently of Semble/Git allow-list rules.

use std::{collections::HashSet, path::Path};

use ignore::WalkBuilder;

use super::SourceFile;
use crate::{Error, Result};

pub(super) fn retain_allowed(root: &Path, files: &mut Vec<SourceFile>) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    // Visit only branches containing candidate files. The walker still reads
    // each directory's ignore file before applying this traversal filter.
    let candidates: HashSet<_> = files
        .iter()
        .flat_map(|file| {
            file.absolute_path
                .ancestors()
                .take_while(|path| path.starts_with(root))
                .map(Path::to_path_buf)
        })
        .collect();
    let mut builder = WalkBuilder::new(root);
    builder
        .standard_filters(false)
        .parents(true)
        .follow_links(false)
        .add_custom_ignore_filename(".cursorignore")
        .filter_entry(move |entry| candidates.contains(entry.path()));
    let mut allowed = HashSet::new();
    for entry in builder.build() {
        let entry = entry.map_err(|error| Error::InvalidRequest(error.to_string()))?;
        if let Some(error) = entry.error() {
            return Err(Error::InvalidRequest(error.to_string()));
        }
        if entry.file_type().is_some_and(|kind| kind.is_file()) {
            allowed.insert(entry.into_path());
        }
    }
    files.retain(|file| allowed.contains(&file.absolute_path));
    Ok(())
}

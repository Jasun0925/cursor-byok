//! Immutable request-owned source bodies. No filesystem or shared cache access.

use std::collections::HashSet;

use crate::{Error, Result};

pub const MAX_SNAPSHOT_FILES: usize = 20_000;
pub const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct SnapshotFile {
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

impl SnapshotFile {
    pub(crate) fn validate_all(files: &[Self], max_file_bytes: u64) -> Result<()> {
        if files.len() > MAX_SNAPSHOT_FILES {
            return Err(Error::InvalidRequest("snapshot has too many files".into()));
        }
        let mut seen = HashSet::new();
        let mut bytes = 0_usize;
        for file in files {
            let path = &file.relative_path;
            if path.contains(['\\', '\0'])
                || path.len() > 4096
                || path.split('/').any(|part| matches!(part, "" | "." | ".."))
                || !seen.insert(path)
            {
                return Err(Error::InvalidRequest(format!(
                    "invalid or duplicate snapshot path: {path}"
                )));
            }
            bytes = bytes.saturating_add(file.bytes.len());
            if file.bytes.len() as u64 > max_file_bytes || bytes > MAX_SNAPSHOT_BYTES {
                return Err(Error::InvalidRequest(
                    "snapshot exceeds source size limits".into(),
                ));
            }
        }
        Ok(())
    }
}

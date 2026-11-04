//! Bounded, stateless host collection; bodies are requested only after filtering.

use std::collections::{HashMap, HashSet, VecDeque};

use base64::{engine::general_purpose::STANDARD, Engine};
use semble_core::{
    source::{SnapshotFile, SourceInventory, MAX_SNAPSHOT_BYTES},
    ContentType,
};
use serde::Deserialize;
use serde_json::{json, Value};

// Cursor has a separate 1 MiB inline stdout limit. Each response page, including
// base64 and its envelope, stays below half that limit.
pub(crate) const MAX_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_BATCH_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1_000_000;

mod command;
mod transfer;
pub(crate) use command::{normalize_root, valid_root, windows_root};

pub(crate) struct Collector {
    root: String,
    content: Vec<ContentType>,
    phase: Phase,
    transfer: transfer::Transfer,
}

enum Phase {
    Inventory,
    Reading {
        fingerprint: String,
        remaining: VecDeque<Vec<(String, u64)>>,
        requested: Vec<(String, u64)>,
        files: Vec<SnapshotFile>,
    },
}

#[derive(Deserialize)]
struct FileBody {
    path: String,
    data: String,
}

impl Collector {
    pub(crate) fn new(root: String, content: Vec<ContentType>) -> Self {
        Self {
            root: normalize_root(&root),
            content,
            phase: Phase::Inventory,
            transfer: transfer::Transfer::default(),
        }
    }

    pub(crate) fn command(&self) -> Result<String, String> {
        let mut request = match &self.phase {
            Phase::Inventory => json!({"operation":"inventory", "root":self.root}),
            Phase::Reading {
                fingerprint,
                requested,
                ..
            } => json!({
                "operation":"read", "root":self.root, "fingerprint":fingerprint,
                "paths":requested.iter().map(|(path,_)|path).collect::<Vec<_>>(),
            }),
        };
        self.transfer.request(&mut request);
        command::build(&self.root, &request)
    }

    pub(crate) fn accept(&mut self, output: &str) -> Result<Option<Vec<SnapshotFile>>, String> {
        if output.len() > MAX_OUTPUT_BYTES {
            return Err("Semble collection output exceeds the transport size limit".into());
        }
        let Some(payload) = self.transfer.accept(output)? else {
            return Ok(None);
        };
        let value: Value = serde_json::from_slice(&payload)
            .map_err(|error| format!("Invalid workspace collection result: {error}"))?;
        self.accept_payload(value)
    }

    fn accept_payload(&mut self, value: Value) -> Result<Option<Vec<SnapshotFile>>, String> {
        if value.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("Workspace collection failed")
                .into());
        }
        let fingerprint = value
            .get("fingerprint")
            .and_then(Value::as_str)
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("Workspace collection has no valid fingerprint")?;
        match &mut self.phase {
            Phase::Inventory => {
                let inventory: SourceInventory = serde_json::from_value(
                    value.get("inventory").cloned().ok_or("Missing inventory")?,
                )
                .map_err(|error| error.to_string())?;
                if inventory.root != self.root {
                    return Err("Workspace collection returned a different root".into());
                }
                let selected = inventory
                    .select(&self.content, MAX_FILE_BYTES)
                    .map_err(|error| error.to_string())?;
                if selected.is_empty() {
                    return Err("No allowed source files to index".into());
                }
                let sizes = inventory
                    .entries
                    .iter()
                    .map(|entry| (entry.path.as_str(), entry.size))
                    .collect::<HashMap<_, _>>();
                let mut remaining = VecDeque::new();
                let mut batch = Vec::new();
                let (mut bytes, mut names) = (0, 0);
                for path in selected {
                    let size = sizes[path.as_str()];
                    let name_bytes = serde_json::to_string(&path)
                        .map_err(|e| e.to_string())?
                        .len()
                        + 1;
                    if !batch.is_empty()
                        && (batch.len() == 128
                            || bytes + size > MAX_BATCH_BYTES
                            || names + name_bytes
                                > if windows_root(&self.root) {
                                    1500
                                } else {
                                    24000
                                })
                    {
                        remaining.push_back(std::mem::take(&mut batch));
                        bytes = 0;
                        names = 0;
                    }
                    batch.push((path, size));
                    bytes += size;
                    names += name_bytes;
                }
                if !batch.is_empty() {
                    remaining.push_back(batch);
                }
                self.phase = Phase::Reading {
                    fingerprint: fingerprint.into(),
                    requested: remaining.pop_front().unwrap(),
                    remaining,
                    files: Vec::new(),
                };
                Ok(None)
            }
            Phase::Reading {
                fingerprint: expected,
                remaining,
                requested,
                files,
            } => {
                if fingerprint != expected {
                    return Err("Workspace fingerprint changed".into());
                }
                let bodies: Vec<FileBody> = serde_json::from_value(
                    value.get("files").cloned().ok_or("Missing source bodies")?,
                )
                .map_err(|error| error.to_string())?;
                if bodies.len() != requested.len() {
                    return Err("Incomplete source body batch".into());
                }
                let wanted = requested
                    .iter()
                    .map(|(path, size)| (path.as_str(), *size))
                    .collect::<HashMap<_, _>>();
                let mut seen = HashSet::new();
                let mut bytes = files.iter().map(|file| file.bytes.len()).sum::<usize>();
                for body in bodies {
                    let size = wanted
                        .get(body.path.as_str())
                        .ok_or("Unexpected source file in batch")?;
                    if !seen.insert(body.path.clone()) {
                        return Err("Duplicate source file in batch".into());
                    }
                    let decoded = STANDARD
                        .decode(body.data)
                        .map_err(|error| error.to_string())?;
                    bytes = bytes.saturating_add(decoded.len());
                    if decoded.len() as u64 != *size || bytes > MAX_SNAPSHOT_BYTES {
                        return Err("Source file changed or snapshot exceeds size limit".into());
                    }
                    files.push(SnapshotFile {
                        relative_path: body.path,
                        bytes: decoded,
                    });
                }
                if let Some(next) = remaining.pop_front() {
                    *requested = next;
                    Ok(None)
                } else {
                    Ok(Some(std::mem::take(files)))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

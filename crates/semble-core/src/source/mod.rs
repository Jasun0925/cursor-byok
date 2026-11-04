//! Source discovery, ignore handling, path safety, and remote Git acquisition.

mod cursor;
mod inventory;
mod local;
mod remote_git;
mod security;
mod snapshot;

pub use inventory::{GitMatch, IgnoreRule, InventoryEntry, SourceInventory};
pub use local::{discover_files, FileStamp, SourceFile};
pub use remote_git::RemoteRepository;
pub use security::canonical_source_root;
pub use snapshot::{SnapshotFile, MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_FILES};

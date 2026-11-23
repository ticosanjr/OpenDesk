//! Full workspace backup and restore. Archives include workspace files and
//! native OpenCode configuration, with a manifest, SHA-256 and safe extraction.

#[path = "backup_create.rs"]
mod create;
#[path = "backup_manifest.rs"]
mod manifest;
#[path = "backup_restore.rs"]
mod restore;

pub use create::create;
pub use manifest::{WorkspaceBackup, delete, get, list};
pub use restore::{RestoreInput, restore_as_new};

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;

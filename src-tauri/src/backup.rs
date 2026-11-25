//! Full workspace backup and restore. Archives include workspace files and
//! native OpenCode configuration, with a manifest, SHA-256 and safe extraction.

#[path = "backup_create.rs"]
mod create;
#[path = "backup_extract.rs"]
mod extract;
#[path = "backup_import.rs"]
mod import;
#[path = "backup_manifest.rs"]
mod manifest;
#[path = "backup_secrets.rs"]
mod portable;
#[path = "backup_restore.rs"]
mod restore;
#[path = "backup_state.rs"]
mod state;

pub use create::{BackupOptions, create, create_with_options};
pub use import::import_archive;
pub use manifest::MAX_IMPORT_BYTES;
pub use manifest::{BackupIncludes, WorkspaceBackup, delete, get, list};
pub use restore::{RestoreInput, RestoreMode, RestoreReport, restore, restore_as_new};

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;

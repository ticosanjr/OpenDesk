//! Enterprise cluster: multi-node registry and workspace synchronization.

#[cfg(feature = "ent-cluster")]
pub mod nodes;
#[cfg(feature = "ent-cluster")]
pub mod sync;

#[cfg(feature = "ent-cluster")]
pub use nodes::{HeartbeatInput, NodeView, UpdateNodeInput, Worker};
#[cfg(feature = "ent-cluster")]
pub use sync::{
    ApplyReport, BundleReport, FileEntry, SyncDelta, apply, bundle, delta, manifest,
    validate_bundle_path,
};

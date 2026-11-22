//! OpenDesk enterprise features.
//!
//! The modules are independent: `cargo check -p opendesk-enterprise
//! --no-default-features --features ent-auth` builds only auth, and so on.
//! Cluster/eval modules still depend on `opendesk-core` repositories and land
//! after the storage extraction.

#[cfg(feature = "ent-auth")]
pub mod auth;
pub mod cluster;
#[cfg(feature = "ent-computer")]
pub mod computer;
pub mod error;
pub mod eval;

#[cfg(test)]
#[path = "test_support.rs"]
pub mod test_support;

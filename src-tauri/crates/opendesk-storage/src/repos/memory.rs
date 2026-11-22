//! Structured, agent-scoped memory.
//!
//! `MEMORY.md` remains the human-editable workspace file. This module adds
//! queryable notes with namespaces, trust levels, expiry and per-agent ACL, so
//! an agent only receives context it is allowed to read.

include!("memory_part1.rs");
include!("memory_part2.rs");

#[cfg(test)]
include!("memory_tests.rs");

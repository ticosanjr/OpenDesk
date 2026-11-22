//! Path-addressed DAG executor for recipes with branch, parallel and map
//! groups. Flat recipes keep using the original sequential driver in
//! `engine/mod.rs`.
//!
//! Every executable node has a stable path (`plan`, `lote/b0/revisar`,
//! `arquivos/0001/analisar`). Durability is keyed by that path, so a restart
//! resumes at the first node that is not DONE/SKIPPED even when the earlier
//! part of the flow contained concurrent groups.

mod executor;
mod flow;
mod groups;
mod hitl;
mod map;
mod nodes;
mod parallel;
mod paths;
mod recipe_call;
mod types;

pub use executor::execute;

#[cfg(test)]
mod tests;

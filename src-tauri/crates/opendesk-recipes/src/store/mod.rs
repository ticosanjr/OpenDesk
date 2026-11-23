//! Recipe index, versions, runs, durable DAG nodes and HITL persistence.

mod hitl;
mod nodes;
mod recipes;
mod runs;
mod types;
mod versions;

pub use hitl::{
    create_hitl, dispatch_channel, get_hitl, pending_hitl, pending_hitl_for_checkpoint,
    pending_hitl_for_run, resolve_hitl,
};
pub use nodes::{
    ensure_dag_node, fail_running_nodes, list_nodes_under, list_run_nodes, mark_node_done,
    mark_node_path, mark_node_running, mark_node_skipped, mark_node_waiting, mark_pending_skipped,
    node_by_path, reset_node, reset_node_path, reset_node_subtree, sync_run_nodes,
};
pub use recipes::{
    get, list, list_all, save_yaml, set_enabled, set_yaml_enabled, sync_workspace, yaml_for,
};
pub use runs::{
    create_run, get_run, is_cancel_requested, latest_run_for_task, list_runs, request_cancel,
    reset_for_retry, resumable_runs, update_run,
};
pub use types::{HitlRequest, RecipeIndex, RecipeRun, RecipeRunNode};
pub use versions::{
    RecipeVersion, RecipeVersionSummary, diff_version, get_version, list_versions, rollback_version,
};

use sha2::{Digest, Sha256};

pub(super) fn sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests;

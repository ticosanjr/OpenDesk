//! Curated starter packs: catalog, install plans and workspace templates.

mod bundle;
mod install;
mod plan;
mod registry;
mod types;
mod workspace;

use include_dir::{Dir, include_dir};

use bundle::{collect_dir, file_name, safe_relative, sha256};

#[cfg(test)]
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[cfg(test)]
use crate::{
    app_state::AppState,
    db::{schedules, workspaces},
};

#[cfg(test)]
use bundle::{catalog, load_manifest};
#[cfg(test)]
use plan::planned_for;

#[allow(unused_imports)]
pub use bundle::{studio_paths, validate_bundle};
pub use install::{install, install_item, preview};
pub use registry::{detail, items, list};
#[allow(unused_imports)]
pub use types::{
    CatalogFile, Category, InstallPlan, InstallResult, ItemInstallResult, PackContents, PackCounts,
    PackDetail, PackExample, PackHealth, PackItem, PackManifest, PackSummary, PlanFile,
    PlanRoutine, Requires, UninstallResult, WorkspaceTemplate,
};
#[allow(unused_imports)]
pub use workspace::{
    TemplateResult, create_from_template, ensure_default_factory_workspace, install_base_pack,
    templates, uninstall,
};

pub(super) static BUNDLE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../starter-packs");

/// Pack installed in every newly created workspace. It carries the skill and
/// commands that teach agents to use the `opendesk-control` MCP. MCP bridges
/// themselves are registered for all workspaces by `proposals::ensure_all`.
pub const BASE_PACK_ID: &str = "controle-opendesk";

#[cfg(test)]
#[path = "../starter_packs_tests.rs"]
mod tests;

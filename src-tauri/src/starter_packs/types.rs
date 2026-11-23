//! Serialized catalog, manifest and install data structures.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Requires {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub integrations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackContents {
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub recipes: Vec<String>,
    #[serde(default)]
    pub routines: Vec<String>,
    #[serde(default)]
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackExample {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub recipe: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub category: String,
    pub description: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub requires: Requires,
    #[serde(default)]
    pub contents: PackContents,
    #[serde(default)]
    pub examples: Vec<PackExample>,
}

fn default_icon() -> String {
    "package".into()
}
fn default_color() -> String {
    "#3b82f6".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogFile {
    pub schema: u32,
    #[serde(default)]
    pub categories: Vec<Category>,
    pub packs: Vec<String>,
    pub templates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackCounts {
    pub agents: usize,
    pub skills: usize,
    pub commands: usize,
    pub recipes: usize,
    pub routines: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PackHealth {
    /// installed | modified | partial
    pub status: String,
    pub missing_files: usize,
    pub modified_files: usize,
    pub missing_routines: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackSummary {
    pub id: String,
    pub version: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub recommended: bool,
    pub requires: Requires,
    pub counts: PackCounts,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub installed_at: Option<String>,
    pub update_available: bool,
    pub health: PackHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackDetail {
    pub manifest: PackManifest,
    pub installed: bool,
    pub installed_at: Option<String>,
    pub installed_version: Option<String>,
    pub update_available: bool,
    pub health: PackHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackItem {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub files: Vec<String>,
    /// True when every file of the item exists with the packaged content.
    pub installed: bool,
    /// installed | modified | missing
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemInstallResult {
    pub kind: String,
    pub id: String,
    pub written: usize,
    pub skipped_conflicts: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanFile {
    pub path: String,
    pub kind: String,
    /// create | identical | conflict
    pub status: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanRoutine {
    pub file: String,
    pub name: String,
    pub trigger_type: String,
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallPlan {
    pub pack_id: String,
    pub version: String,
    pub files: Vec<PlanFile>,
    pub routines: Vec<PlanRoutine>,
    pub folders: Vec<String>,
    pub requires: Requires,
    pub conflicts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct InstalledFile {
    pub(super) path: String,
    pub(super) sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct InstalledRoutine {
    pub(super) id: String,
    #[serde(default)]
    pub(super) file: String,
    #[serde(default)]
    pub(super) name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum StoredRoutine {
    Legacy(String),
    Full {
        id: String,
        #[serde(default)]
        file: String,
        #[serde(default)]
        name: String,
    },
}

pub(super) fn parse_stored_routines(json: &str) -> Vec<InstalledRoutine> {
    serde_json::from_str::<Vec<StoredRoutine>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|routine| match routine {
            StoredRoutine::Legacy(id) => InstalledRoutine {
                id,
                file: String::new(),
                name: String::new(),
            },
            StoredRoutine::Full { id, file, name } => InstalledRoutine { id, file, name },
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub pack_id: String,
    pub version: String,
    pub written: usize,
    pub skipped: usize,
    pub skipped_conflicts: usize,
    pub routines: Vec<String>,
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallResult {
    pub pack_id: String,
    pub removed: usize,
    pub kept: Vec<String>,
    pub routines_removed: usize,
    pub routines_kept: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTemplate {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_policy")]
    pub router_policy: String,
    pub packs: Vec<String>,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

fn default_policy() -> String {
    "COST_BENEFIT".into()
}

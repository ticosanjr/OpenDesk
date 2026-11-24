pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_BUNDLE_BYTES: usize = 10 * 1024 * 1024;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBundle {
    pub schema_version: u32,
    pub exported_at: String,
    pub app_version: String,
    pub source_workspace: Value,
    pub sections: BundleSections,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BundleSections {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_md: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_memory: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personal_context: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_templates: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub router: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation_routes: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<Value>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ImportMode {
    #[default]
    MergeSkipExisting,
    MergeOverwrite,
    ReplaceSection,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExportRequest {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub sections: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub bundle: ConfigBundle,
    #[serde(default)]
    pub sections: Option<Vec<String>>,
    #[serde(default)]
    pub mode: ImportMode,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SectionPreview {
    pub incoming: usize,
    pub existing: usize,
    pub create: usize,
    pub update: usize,
    pub skip: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PreviewReport {
    pub sections: BTreeMap<String, SectionPreview>,
}

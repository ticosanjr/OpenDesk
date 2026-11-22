use crate::{
    config_sync::{agents, commands, jsonc_editor, paths::StudioPaths, skills},
    error::AppResult,
};

use serde_json::Value;

use super::{StudioItem, StudioKind};

impl StudioItem<'_> {
    pub fn list(
        paths: &StudioPaths,
        scope: &str,
        kind: StudioKind,
    ) -> AppResult<Vec<ItemSnapshot>> {
        match kind {
            StudioKind::Agent => Ok(agents::list(paths, scope)?
                .into_iter()
                .map(|item| {
                    let value = serde_json::to_value(&item).unwrap_or(Value::Null);
                    ItemSnapshot::new(item.id, item.path, item.disabled, value)
                })
                .collect()),
            StudioKind::Skill => Ok(skills::list(paths, scope)?
                .into_iter()
                .map(|item| {
                    let value = serde_json::to_value(&item).unwrap_or(Value::Null);
                    ItemSnapshot::new(item.id, item.path, item.disabled, value)
                })
                .collect()),
            StudioKind::Command => Ok(commands::list(paths, scope)?
                .into_iter()
                .map(|item| {
                    let value = serde_json::to_value(&item).unwrap_or(Value::Null);
                    ItemSnapshot::new(item.id, item.path, item.disabled, value)
                })
                .collect()),
            StudioKind::Mcp => {
                let document = jsonc_editor::read_for_scope(paths, scope)?;
                Ok(jsonc_editor::list_mcp_servers(&document)
                    .into_iter()
                    .map(|server| {
                        let disabled = server
                            .server
                            .get("disabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        ItemSnapshot::new(
                            server.name,
                            document.path.clone(),
                            disabled,
                            server.server,
                        )
                    })
                    .collect())
            }
        }
    }
}

/// Normalized list entry for the effective-config merge.
#[derive(Debug, Clone)]
pub struct ItemSnapshot {
    pub id: String,
    pub path: String,
    pub disabled: bool,
    pub value: Value,
}

impl ItemSnapshot {
    fn new(id: String, path: String, disabled: bool, value: Value) -> Self {
        Self {
            id,
            path,
            disabled,
            value,
        }
    }
}

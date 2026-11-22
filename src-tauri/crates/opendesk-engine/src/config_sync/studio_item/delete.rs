use crate::{
    config_sync::{agents, commands, jsonc_editor, skills},
    error::AppResult,
};

use std::path::Path;

use super::{StudioItem, StudioKind};

impl<'a> StudioItem<'a> {
    pub fn delete(&self, backups_dir: &Path) -> AppResult<()> {
        match self.kind {
            StudioKind::Agent => agents::delete(self.paths, self.scope, self.id),
            StudioKind::Skill => skills::delete(self.paths, self.scope, self.id),
            StudioKind::Command => commands::delete(self.paths, self.scope, self.id),
            StudioKind::Mcp => {
                let path = self.paths.config_path(self.scope)?;
                jsonc_editor::delete_mcp_server(&path, self.id, None, backups_dir)?;
                Ok(())
            }
        }
    }
}

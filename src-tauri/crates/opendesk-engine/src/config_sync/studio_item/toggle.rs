use crate::{
    config_sync::{
        agents::{self, AgentWriteInput},
        commands, jsonc_editor, skills,
    },
    error::{AppError, AppResult},
};

use serde_json::Value;
use std::path::{Path, PathBuf};

use super::{StudioItem, StudioKind, move_dir, move_file};

impl<'a> StudioItem<'a> {
    pub fn set_enabled(&self, enabled: bool, backups_dir: &Path) -> AppResult<()> {
        match self.kind {
            StudioKind::Agent => {
                let mut agent = agents::get(self.paths, self.scope, self.id)?;
                agent.disabled = !enabled;
                agents::write(
                    self.paths,
                    self.scope,
                    self.id,
                    AgentWriteInput {
                        name: agent.name,
                        avatar: agent.avatar,
                        description: agent.description,
                        mode: agent.mode,
                        model: agent.model,
                        color: agent.color,
                        router_policy: agent.router_policy,
                        hidden: agent.hidden,
                        steps: agent.steps,
                        disabled: agent.disabled,
                        permissions: agent.permissions,
                        system: agent.system,
                        expected_sha256: None,
                    },
                    backups_dir,
                )?;
            }
            StudioKind::Skill => {
                let skill = skills::get(self.paths, self.scope, self.id)?;
                let skill_path = PathBuf::from(&skill.path);
                let is_folder =
                    skill_path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md");
                let disabled_root = self.paths.opencode_dir(self.scope)?.join("disabled/skills");
                let skills_root = self.paths.skills_dir(self.scope)?;
                if is_folder {
                    let source = skill_path
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or_else(|| AppError::Internal("skill path has no parent".into()))?;
                    let target = if enabled {
                        skills_root.join(self.id)
                    } else {
                        disabled_root.join(self.id)
                    };
                    move_dir(&source, &target)?;
                } else {
                    let source = skill_path;
                    let target = if enabled {
                        skills_root.join(format!("{}.md", self.id))
                    } else {
                        disabled_root.join(format!("{}.md", self.id))
                    };
                    move_file(&source, &target)?;
                }
            }
            StudioKind::Command => {
                let command = commands::get(self.paths, self.scope, self.id)?;
                let source = PathBuf::from(&command.path);
                let disabled_root = self
                    .paths
                    .opencode_dir(self.scope)?
                    .join("disabled/commands");
                let target = if enabled {
                    self.paths
                        .commands_dir(self.scope)?
                        .join(format!("{}.md", self.id))
                } else {
                    disabled_root.join(format!("{}.md", self.id))
                };
                move_file(&source, &target)?;
            }
            StudioKind::Mcp => {
                let document = jsonc_editor::read_for_scope(self.paths, self.scope)?;
                let mut server = jsonc_editor::list_mcp_servers(&document)
                    .into_iter()
                    .find(|server| server.name == self.id)
                    .map(|server| server.server)
                    .ok_or_else(|| AppError::NotFound(format!("mcp {}", self.id)))?;
                server["disabled"] = Value::Bool(!enabled);
                let path = self.paths.config_path(self.scope)?;
                jsonc_editor::upsert_mcp_server(&path, self.id, &server, None, backups_dir)?;
            }
        }
        Ok(())
    }
}

use crate::{
    config_sync::{
        agents::{self, AgentWriteInput, PermissionRule},
        commands::{self, CommandWriteInput},
        jsonc_editor,
        skills::{self, SkillExtraFile, SkillWriteInput},
    },
    error::{AppError, AppResult},
};

use std::path::Path;

use super::{StudioItem, StudioKind};

impl<'a> StudioItem<'a> {
    pub fn copy_to(
        &self,
        target: &StudioItem<'_>,
        overwrite: bool,
        backups_dir: &Path,
    ) -> AppResult<()> {
        if self.kind != target.kind {
            return Err(AppError::BadRequest(
                "source and target kinds differ".into(),
            ));
        }
        if !overwrite && target.exists()? {
            return Err(AppError::Conflict(format!(
                "{} '{}' already exists in {}",
                self.kind.as_str(),
                target.id,
                target.scope
            )));
        }
        match self.kind {
            StudioKind::Agent => {
                let agent = agents::get(self.paths, self.scope, self.id)?;
                agents::write(
                    target.paths,
                    target.scope,
                    target.id,
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
                        permissions: agent
                            .permissions
                            .into_iter()
                            .map(|permission| PermissionRule {
                                action: permission.action,
                                resource: permission.resource,
                                effect: permission.effect,
                            })
                            .collect(),
                        system: agent.system,
                        expected_sha256: None,
                    },
                    backups_dir,
                )?;
            }
            StudioKind::Skill => {
                let skill = skills::get(self.paths, self.scope, self.id)?;
                skills::write(
                    target.paths,
                    target.scope,
                    target.id,
                    SkillWriteInput {
                        name: skill.name.clone().into(),
                        description: skill.description,
                        body: skill.body,
                        files: skill
                            .files
                            .into_iter()
                            .map(|file| SkillExtraFile {
                                path: file.path,
                                content: file.content,
                            })
                            .collect(),
                        expected_sha256: None,
                    },
                    backups_dir,
                )?;
            }
            StudioKind::Command => {
                let command = commands::get(self.paths, self.scope, self.id)?;
                commands::write(
                    target.paths,
                    target.scope,
                    target.id,
                    CommandWriteInput {
                        description: command.description,
                        agent: command.agent,
                        model: command.model,
                        subagent: command.subagent,
                        template: command.template,
                        expected_sha256: None,
                    },
                    backups_dir,
                )?;
            }
            StudioKind::Mcp => {
                let document = jsonc_editor::read_for_scope(self.paths, self.scope)?;
                let server = jsonc_editor::list_mcp_servers(&document)
                    .into_iter()
                    .find(|server| server.name == self.id)
                    .map(|server| server.server)
                    .ok_or_else(|| AppError::NotFound(format!("mcp {}", self.id)))?;
                let target_path = target.paths.config_path(target.scope)?;
                jsonc_editor::upsert_mcp_server(
                    &target_path,
                    target.id,
                    &server,
                    None,
                    backups_dir,
                )?;
            }
        }
        Ok(())
    }
}

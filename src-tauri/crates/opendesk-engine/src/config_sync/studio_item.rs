use crate::{
    config_sync::{
        agents::{self, AgentWriteInput, PermissionRule},
        commands::{self, CommandWriteInput},
        jsonc_editor,
        paths::StudioPaths,
        skills::{self, SkillExtraFile, SkillWriteInput},
    },
    error::{AppError, AppResult},
};

use serde_json::Value;
use std::path::{Path, PathBuf};

use super::file_ops::validate_slug;

/// The four assets Studio manages. Each one keeps its own storage layout
/// behind this interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioKind {
    Agent,
    Skill,
    Command,
    Mcp,
}

impl StudioKind {
    pub const ALL: [Self; 4] = [Self::Agent, Self::Skill, Self::Command, Self::Mcp];

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "agent" => Ok(Self::Agent),
            "skill" => Ok(Self::Skill),
            "command" => Ok(Self::Command),
            "mcp" => Ok(Self::Mcp),
            other => Err(AppError::BadRequest(format!(
                "invalid studio kind '{other}'"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Skill => "skill",
            Self::Command => "command",
            Self::Mcp => "mcp",
        }
    }
}

/// One identified Studio asset in one scope.
#[derive(Debug, Clone, Copy)]
pub struct StudioItem<'a> {
    pub paths: &'a StudioPaths,
    pub scope: &'a str,
    pub kind: StudioKind,
    pub id: &'a str,
}

impl<'a> StudioItem<'a> {
    pub fn new(paths: &'a StudioPaths, scope: &'a str, kind: StudioKind, id: &'a str) -> Self {
        Self {
            paths,
            scope,
            kind,
            id,
        }
    }

    pub fn exists(&self) -> AppResult<bool> {
        match self.kind {
            StudioKind::Agent => Ok(agents::get(self.paths, self.scope, self.id).is_ok()),
            StudioKind::Skill => Ok(skills::get(self.paths, self.scope, self.id).is_ok()),
            StudioKind::Command => Ok(commands::get(self.paths, self.scope, self.id).is_ok()),
            StudioKind::Mcp => {
                let document = jsonc_editor::read_for_scope(self.paths, self.scope)?;
                Ok(jsonc_editor::list_mcp_servers(&document)
                    .iter()
                    .any(|server| server.name == self.id))
            }
        }
    }

    pub fn path(&self) -> AppResult<PathBuf> {
        match self.kind {
            StudioKind::Agent => {
                let id = validate_slug(self.id, true)?;
                Ok(self.paths.agents_dir(self.scope)?.join(format!("{id}.md")))
            }
            StudioKind::Skill => Ok(PathBuf::from(
                skills::get(self.paths, self.scope, self.id)?.path,
            )),
            StudioKind::Command => Ok(PathBuf::from(
                commands::get(self.paths, self.scope, self.id)?.path,
            )),
            StudioKind::Mcp => self.paths.config_path(self.scope),
        }
    }

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

fn move_dir(source: &Path, target: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::NotFound(format!("{}", source.display())));
    }
    if target.exists() {
        return Err(AppError::Conflict(format!(
            "{} already exists",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(source, target).is_err() {
        copy_dir(source, target)?;
        std::fs::remove_dir_all(source)?;
    }
    Ok(())
}

fn copy_dir(source: &Path, target: &Path) -> AppResult<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &destination)?;
        } else {
            std::fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

fn move_file(source: &Path, target: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::NotFound(format!("{}", source.display())));
    }
    if target.exists() {
        return Err(AppError::Conflict(format!(
            "{} already exists",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(source, target).is_err() {
        std::fs::copy(source, target)?;
        std::fs::remove_file(source)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_sync::{
        agents::AgentWriteInput, commands::CommandWriteInput, skills::SkillWriteInput,
    };
    use serde_json::json;
    use tempfile::TempDir;

    fn paths(temp: &TempDir) -> StudioPaths {
        StudioPaths {
            global_dir: temp.path().join("global"),
            workspace_root: Some(temp.path().join("ws")),
        }
    }

    fn seed(paths: &StudioPaths, backups: &Path) {
        agents::write(
            paths,
            "workspace",
            "auditor",
            AgentWriteInput {
                name: Some("Auditor".into()),
                avatar: None,
                description: None,
                mode: Some("subagent".into()),
                model: None,
                color: None,
                router_policy: None,
                hidden: false,
                steps: None,
                disabled: false,
                permissions: Vec::new(),
                system: "Você audita.".into(),
                expected_sha256: None,
            },
            backups,
        )
        .expect("agent");
        skills::write(
            paths,
            "workspace",
            "pesquisa",
            SkillWriteInput {
                name: Some("Pesquisa".into()),
                description: None,
                body: "Use fontes.".into(),
                files: Vec::new(),
                expected_sha256: None,
            },
            backups,
        )
        .expect("skill");
        commands::write(
            paths,
            "workspace",
            "revisar",
            CommandWriteInput {
                description: None,
                agent: None,
                model: None,
                subagent: None,
                template: "Revise $ARGUMENTS".into(),
                expected_sha256: None,
            },
            backups,
        )
        .expect("command");
        jsonc_editor::upsert_mcp_server(
            &paths.config_path("workspace").expect("config path"),
            "fetch",
            &json!({ "type": "local", "command": ["npx", "fetch"] }),
            None,
            backups,
        )
        .expect("mcp");
    }

    #[test]
    fn parses_kinds_and_rejects_unknown_ones() {
        assert_eq!(
            StudioKind::parse("agent").expect("agent"),
            StudioKind::Agent
        );
        assert_eq!(
            StudioKind::parse("skill").expect("skill"),
            StudioKind::Skill
        );
        assert_eq!(
            StudioKind::parse("command").expect("command"),
            StudioKind::Command
        );
        assert_eq!(StudioKind::parse("mcp").expect("mcp"), StudioKind::Mcp);
        assert!(StudioKind::parse("plugin").is_err());
    }

    #[test]
    fn resolves_paths_and_lists_all_four_kinds() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);
        let backups = temp.path().join("backups");
        seed(&paths, &backups);

        for (kind, id) in [
            (StudioKind::Agent, "auditor"),
            (StudioKind::Skill, "pesquisa"),
            (StudioKind::Command, "revisar"),
            (StudioKind::Mcp, "fetch"),
        ] {
            let item = StudioItem::new(&paths, "workspace", kind, id);
            assert!(item.exists().expect("exists"), "{}", kind.as_str());
            assert!(item.path().expect("path").exists(), "{}", kind.as_str());
        }

        for kind in StudioKind::ALL {
            let items = StudioItem::list(&paths, "workspace", kind).expect("list");
            assert_eq!(items.len(), 1, "{}", kind.as_str());
        }
    }

    #[test]
    fn toggles_and_deletes_every_kind() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);
        let backups = temp.path().join("backups");
        seed(&paths, &backups);

        for (kind, id) in [
            (StudioKind::Agent, "auditor"),
            (StudioKind::Skill, "pesquisa"),
            (StudioKind::Command, "revisar"),
            (StudioKind::Mcp, "fetch"),
        ] {
            let item = StudioItem::new(&paths, "workspace", kind, id);
            item.set_enabled(false, &backups).expect("disable");
            assert!(
                match kind {
                    StudioKind::Agent =>
                        agents::get(&paths, "workspace", id)
                            .expect("agent")
                            .disabled,
                    StudioKind::Skill =>
                        skills::get(&paths, "workspace", id)
                            .expect("skill")
                            .disabled,
                    StudioKind::Command =>
                        commands::get(&paths, "workspace", id)
                            .expect("command")
                            .disabled,
                    StudioKind::Mcp => {
                        let document =
                            jsonc_editor::read_for_scope(&paths, "workspace").expect("config");
                        jsonc_editor::list_mcp_servers(&document)
                            .into_iter()
                            .find(|server| server.name == id)
                            .and_then(|server| {
                                server.server.get("disabled").and_then(Value::as_bool)
                            })
                            .unwrap_or(false)
                    }
                },
                "{} disabled",
                kind.as_str()
            );
            item.set_enabled(true, &backups).expect("enable");
            item.delete(&backups).expect("delete");
            assert!(!item.exists().expect("exists"), "{}", kind.as_str());
        }
    }
}

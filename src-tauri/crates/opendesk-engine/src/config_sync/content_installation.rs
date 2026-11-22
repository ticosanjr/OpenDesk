use crate::{
    config_sync::{
        agents::{self, AgentWriteInput, PermissionRule},
        commands::{self, CommandWriteInput},
        jsonc_editor, markdown,
        paths::StudioPaths,
        skills::{self, SkillExtraFile, SkillWriteInput},
        studio_item::StudioKind,
    },
    error::{AppError, AppResult},
};

use serde_json::Value;
use std::path::Path;

/// A parsed unit of content waiting to be installed in one scope.
#[derive(Debug, Clone)]
pub struct ContentDraft {
    pub kind: StudioKind,
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub body: String,
    pub files: Vec<SkillExtraFile>,
    pub server: Option<Value>,
}

/// One installation target. Callers describe content; this module owns how
/// each Studio kind is parsed and written.
pub struct ContentInstallation<'a> {
    pub paths: &'a StudioPaths,
    pub scope: &'a str,
    pub backups_dir: &'a Path,
}

impl<'a> ContentInstallation<'a> {
    pub fn new(paths: &'a StudioPaths, scope: &'a str, backups_dir: &'a Path) -> Self {
        Self {
            paths,
            scope,
            backups_dir,
        }
    }

    pub fn install(&self, draft: &ContentDraft) -> AppResult<()> {
        match draft.kind {
            StudioKind::Mcp => {
                let server = draft.server.as_ref().ok_or_else(|| {
                    AppError::BadRequest(format!("mcp {} has no server object", draft.id))
                })?;
                let path = self.paths.config_path(self.scope)?;
                jsonc_editor::upsert_mcp_server(&path, &draft.id, server, None, self.backups_dir)?;
            }
            StudioKind::Agent => {
                let (frontmatter, body) = parse_markdown(&draft.body);
                agents::write(
                    self.paths,
                    self.scope,
                    &draft.id,
                    AgentWriteInput {
                        name: markdown::get_string(&frontmatter, "name").or(draft.name.clone()),
                        avatar: markdown::get_string(&frontmatter, "avatar"),
                        description: markdown::get_string(&frontmatter, "description")
                            .or(draft.description.clone()),
                        mode: markdown::get_string(&frontmatter, "mode"),
                        model: markdown::get_string(&frontmatter, "model"),
                        color: markdown::get_string(&frontmatter, "color"),
                        router_policy: markdown::get_string(&frontmatter, "router_policy"),
                        hidden: markdown::get_bool(&frontmatter, "hidden").unwrap_or(false),
                        steps: markdown::get_u32(&frontmatter, "steps"),
                        disabled: markdown::get_bool(&frontmatter, "disabled").unwrap_or(false),
                        permissions: parse_permissions(&frontmatter)?,
                        system: body,
                        expected_sha256: None,
                    },
                    self.backups_dir,
                )?;
            }
            StudioKind::Skill => {
                let (frontmatter, body) = parse_markdown(&draft.body);
                skills::write(
                    self.paths,
                    self.scope,
                    &draft.id,
                    SkillWriteInput {
                        name: markdown::get_string(&frontmatter, "name").or(draft.name.clone()),
                        description: markdown::get_string(&frontmatter, "description")
                            .or(draft.description.clone()),
                        body,
                        files: draft.files.clone(),
                        expected_sha256: None,
                    },
                    self.backups_dir,
                )?;
            }
            StudioKind::Command => {
                let (frontmatter, body) = parse_markdown(&draft.body);
                commands::write(
                    self.paths,
                    self.scope,
                    &draft.id,
                    CommandWriteInput {
                        description: markdown::get_string(&frontmatter, "description")
                            .or(draft.description.clone()),
                        agent: markdown::get_string(&frontmatter, "agent"),
                        model: markdown::get_string(&frontmatter, "model"),
                        subagent: markdown::get_bool(&frontmatter, "subagent"),
                        template: body,
                        expected_sha256: None,
                    },
                    self.backups_dir,
                )?;
            }
        }
        Ok(())
    }
}

pub fn parse_markdown(content: &str) -> (serde_yaml::Mapping, String) {
    match markdown::parse(content) {
        Ok(document) => (document.frontmatter, document.body),
        Err(_) => (serde_yaml::Mapping::new(), content.to_string()),
    }
}

pub fn parse_permissions(frontmatter: &serde_yaml::Mapping) -> AppResult<Vec<PermissionRule>> {
    let Some(value) = frontmatter.get(serde_yaml::Value::String("permissions".into())) else {
        return Ok(Vec::new());
    };
    serde_yaml::from_value::<Vec<PermissionRule>>(value.clone()).map_err(|error| {
        AppError::BadRequest(format!("invalid permissions in imported agent: {error}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn paths(temp: &TempDir) -> StudioPaths {
        StudioPaths {
            global_dir: temp.path().join("global"),
            workspace_root: Some(temp.path().join("ws")),
        }
    }

    fn draft(kind: StudioKind, id: &str) -> ContentDraft {
        ContentDraft {
            kind,
            id: id.into(),
            name: Some(format!("Nome {id}")),
            description: Some("descrição".into()),
            body: "---\nname: Nome\ndescription: descrição\n---\nConteúdo".into(),
            files: Vec::new(),
            server: None,
        }
    }

    #[test]
    fn installs_every_studio_kind() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);
        let backups = temp.path().join("backups");
        let installation = ContentInstallation::new(&paths, "workspace", &backups);

        installation
            .install(&draft(StudioKind::Agent, "content-agent"))
            .expect("agent");
        installation
            .install(&draft(StudioKind::Skill, "content-skill"))
            .expect("skill");
        installation
            .install(&draft(StudioKind::Command, "content-command"))
            .expect("command");
        let mut mcp = draft(StudioKind::Mcp, "content-mcp");
        mcp.server = Some(json!({ "type": "local", "command": ["echo"] }));
        installation.install(&mcp).expect("mcp");

        assert!(agents::get(&paths, "workspace", "content-agent").is_ok());
        assert!(skills::get(&paths, "workspace", "content-skill").is_ok());
        assert!(commands::get(&paths, "workspace", "content-command").is_ok());
        let document = jsonc_editor::read_for_scope(&paths, "workspace").expect("config");
        assert!(
            jsonc_editor::list_mcp_servers(&document)
                .iter()
                .any(|server| server.name == "content-mcp")
        );
    }

    #[test]
    fn rejects_mcp_without_a_server_object() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);
        let backups = temp.path().join("backups");
        let installation = ContentInstallation::new(&paths, "workspace", &backups);
        let error = installation
            .install(&draft(StudioKind::Mcp, "missing"))
            .expect_err("missing server");
        assert!(error.to_string().contains("has no server object"));
    }

    #[test]
    fn parses_permissions_from_frontmatter() {
        let content =
            "---\npermissions:\n  - action: edit\n    resource: '*'\n    effect: deny\n---\nbody";
        let (frontmatter, body) = parse_markdown(content);
        let permissions = parse_permissions(&frontmatter).expect("permissions");
        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].action, "edit");
        assert_eq!(permissions[0].effect, "deny");
        assert_eq!(body.trim(), "body");
    }
}

use super::paths::StudioPaths;
use super::{
    file_ops::{self, list_files_recursive, read_text, validate_slug, write_text_atomic},
    markdown::{self, MarkdownDocument},
};
use crate::{
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
};

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,
    #[serde(default = "default_resource")]
    pub resource: String,
    pub effect: String,
}

fn default_resource() -> String {
    "*".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFile {
    pub id: String,
    pub scope: String,
    pub path: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub description: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub color: Option<String>,
    pub router_policy: Option<String>,
    pub hidden: bool,
    pub steps: Option<u32>,
    pub disabled: bool,
    pub permissions: Vec<PermissionRule>,
    pub system: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentWriteInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub router_policy: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub steps: Option<u32>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub permissions: Vec<PermissionRule>,
    #[serde(default)]
    pub system: String,
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

pub fn list(paths: &StudioPaths, scope: &str) -> AppResult<Vec<AgentFile>> {
    let dir = paths.agents_dir(scope)?;
    let mut agents = Vec::new();

    for path in list_files_recursive(&dir, "md")? {
        let id = relative_id(&dir, &path)?;
        match parse_agent(scope, &id, &path) {
            Ok(agent) => agents.push(agent),
            Err(error) => {
                tracing::warn!(path = %path.display(), error = %error, "skipping invalid agent file")
            }
        }
    }

    Ok(agents)
}

pub fn get(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<AgentFile> {
    let id = validate_slug(id, true)?;
    let path = agent_path(paths, scope, &id)?;
    parse_agent(scope, &id, &path)
}

pub fn write(
    paths: &StudioPaths,
    scope: &str,
    id: &str,
    input: AgentWriteInput,
    backups_dir: &Path,
) -> AppResult<AgentFile> {
    let id = validate_slug(id, true)?;
    if let Some(model) = input
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && ModelRefDto::parse(model).is_none()
    {
        return Err(AppError::BadRequest(format!(
            "modelo inválido '{model}'; use provider/modelo ou provider/modelo#variant"
        )));
    }
    let path = agent_path(paths, scope, &id)?;

    let existing = read_text(&path)?;
    let mut document = match existing {
        Some(snapshot) => markdown::parse(&snapshot.content)?,
        None => MarkdownDocument {
            frontmatter: Mapping::new(),
            body: String::new(),
        },
    };

    let mapping = &mut document.frontmatter;
    markdown::set_string(mapping, "name", input.name.as_deref());
    markdown::set_string(mapping, "avatar", input.avatar.as_deref());
    markdown::set_string(mapping, "description", input.description.as_deref());
    markdown::set_string(mapping, "mode", input.mode.as_deref());
    markdown::set_string(mapping, "model", input.model.as_deref());
    markdown::set_string(mapping, "color", input.color.as_deref());
    markdown::set_string(mapping, "router_policy", input.router_policy.as_deref());
    markdown::set_bool(mapping, "hidden", input.hidden);
    markdown::set_u32(mapping, "steps", input.steps);
    markdown::set_bool(mapping, "disabled", input.disabled);

    if input.permissions.is_empty() {
        mapping.remove(Value::String("permissions".to_string()));
    } else {
        let value = serde_yaml::to_value(&input.permissions)
            .map_err(|error| AppError::Internal(format!("invalid permissions: {error}")))?;
        mapping.insert(Value::String("permissions".to_string()), value);
    }

    if !matches!(
        input.mode.as_deref(),
        Some("primary") | Some("subagent") | Some("all")
    ) && input.mode.is_some()
    {
        return Err(AppError::BadRequest(
            "mode must be primary, subagent or all".into(),
        ));
    }

    let content = markdown::render(mapping, &input.system)?;
    write_text_atomic(
        &path,
        &content,
        input.expected_sha256.as_deref(),
        backups_dir,
    )?;

    parse_agent(scope, &id, &path)
}

pub fn delete(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<()> {
    let id = validate_slug(id, true)?;
    let path = agent_path(paths, scope, &id)?;
    file_ops::delete_path(&path)
}

fn agent_path(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<PathBuf> {
    // `validate_slug` already rejects absolute paths, `..` and backslashes.
    Ok(paths.agents_dir(scope)?.join(format!("{id}.md")))
}

fn parse_agent(scope: &str, id: &str, path: &Path) -> AppResult<AgentFile> {
    let snapshot =
        read_text(path)?.ok_or_else(|| AppError::NotFound(format!("agent {}", path.display())))?;
    let document = markdown::parse(&snapshot.content)?;
    let mapping = &document.frontmatter;

    let permissions = match mapping.get(Value::String("permissions".to_string())) {
        Some(value) => {
            serde_yaml::from_value::<Vec<PermissionRule>>(value.clone()).map_err(|error| {
                AppError::BadRequest(format!("invalid permissions in {id}: {error}"))
            })?
        }
        None => Vec::new(),
    };

    Ok(AgentFile {
        id: id.to_string(),
        scope: scope.to_string(),
        path: path.to_string_lossy().to_string(),
        name: markdown::get_string(mapping, "name"),
        avatar: markdown::get_string(mapping, "avatar"),
        description: markdown::get_string(mapping, "description"),
        mode: markdown::get_string(mapping, "mode"),
        model: markdown::get_string(mapping, "model"),
        color: markdown::get_string(mapping, "color"),
        router_policy: markdown::get_string(mapping, "router_policy"),
        hidden: markdown::get_bool(mapping, "hidden").unwrap_or(false),
        steps: markdown::get_u32(mapping, "steps"),
        disabled: markdown::get_bool(mapping, "disabled").unwrap_or(false),
        permissions,
        system: document.body,
        sha256: snapshot.sha256,
    })
}

fn relative_id(base: &Path, path: &Path) -> AppResult<String> {
    let relative = path
        .strip_prefix(base)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut id = relative.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = id.strip_suffix(".md") {
        id = stripped.to_string();
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn paths(temp: &TempDir) -> StudioPaths {
        StudioPaths {
            global_dir: temp.path().join("global"),
            workspace_root: Some(temp.path().join("ws")),
        }
    }

    fn input() -> AgentWriteInput {
        AgentWriteInput {
            name: Some("Auditor de Qualidade".into()),
            avatar: Some("🛡️".into()),
            description: Some("Especialista em auditoria".into()),
            mode: Some("subagent".into()),
            model: Some("opencode-go/glm-5.3".into()),
            color: Some("#ff6b6b".into()),
            router_policy: Some("BEST_QUALITY".into()),
            hidden: false,
            steps: Some(8),
            disabled: false,
            permissions: vec![PermissionRule {
                action: "edit".into(),
                resource: "*".into(),
                effect: "deny".into(),
            }],
            system: "Você é o Auditor de Qualidade.".into(),
            expected_sha256: None,
        }
    }

    #[test]
    fn writes_reads_and_deletes_v2_agent() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);

        let agent = write(
            &paths,
            "workspace",
            "auditor",
            input(),
            &temp.path().join("backups"),
        )
        .expect("write");

        assert_eq!(agent.id, "auditor");
        assert_eq!(agent.scope, "workspace");
        assert!(agent.path.ends_with(".opencode/agents/auditor.md"));
        assert_eq!(agent.name.as_deref(), Some("Auditor de Qualidade"));
        assert_eq!(agent.avatar.as_deref(), Some("🛡️"));
        assert_eq!(agent.mode.as_deref(), Some("subagent"));
        assert_eq!(agent.color.as_deref(), Some("#ff6b6b"));
        assert_eq!(agent.permissions.len(), 1);
        assert!(agent.system.contains("Auditor de Qualidade"));

        let content = std::fs::read_to_string(&agent.path).expect("read");
        assert!(content.contains("mode: subagent"));
        assert!(content.contains("action: edit"));
        assert!(content.contains("effect: deny"));
        assert!(content.contains("steps: 8"));
        assert!(!content.contains("temperature"));

        let listed = list(&paths, "workspace").expect("list");
        assert_eq!(listed.len(), 1);

        let stale = AgentWriteInput {
            expected_sha256: Some("deadbeef".into()),
            ..input()
        };
        let error = write(
            &paths,
            "workspace",
            "auditor",
            stale,
            &temp.path().join("backups"),
        )
        .expect_err("stale hash");
        assert!(matches!(error, AppError::Conflict(_)));

        delete(&paths, "workspace", "auditor").expect("delete");
        assert!(matches!(
            get(&paths, "workspace", "auditor"),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn rejects_invalid_mode_and_slug() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);
        let mut invalid = input();
        invalid.mode = Some("invalid".into());
        let error = write(
            &paths,
            "workspace",
            "auditor",
            invalid,
            &temp.path().join("backups"),
        )
        .expect_err("mode");
        assert!(matches!(error, AppError::BadRequest(_)));

        let error = write(
            &paths,
            "workspace",
            "../escape",
            input(),
            &temp.path().join("backups"),
        )
        .expect_err("slug");
        assert!(matches!(error, AppError::BadRequest(_)));
    }
}

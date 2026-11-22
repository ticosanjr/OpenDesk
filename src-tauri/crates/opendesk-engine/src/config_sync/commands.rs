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
pub struct CommandFile {
    pub id: String,
    pub scope: String,
    pub path: String,
    pub description: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub subagent: Option<bool>,
    pub template: String,
    pub sha256: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandWriteInput {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub subagent: Option<bool>,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

pub fn list(paths: &StudioPaths, scope: &str) -> AppResult<Vec<CommandFile>> {
    let dir = paths.commands_dir(scope)?;
    let mut commands = Vec::new();

    for path in list_files_recursive(&dir, "md")? {
        let id = relative_id(&dir, &path)?;
        match parse_command(scope, &id, &path) {
            Ok(command) => commands.push(command),
            Err(error) => {
                tracing::warn!(path = %path.display(), error = %error, "skipping invalid command")
            }
        }
    }

    let disabled_root = paths.opencode_dir(scope)?.join("disabled/commands");
    if disabled_root.is_dir() {
        for path in list_files_recursive(&disabled_root, "md")? {
            let Ok(id) = relative_id(&disabled_root, &path) else {
                continue;
            };
            if let Ok(mut command) = parse_command(scope, &id, &path) {
                command.disabled = true;
                commands.push(command);
            }
        }
    }
    Ok(commands)
}

pub fn get(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<CommandFile> {
    let id = validate_slug(id, true)?;
    let path = command_path(paths, scope, &id)?;
    if path.exists() {
        return parse_command(scope, &id, &path);
    }
    let disabled = paths
        .opencode_dir(scope)?
        .join("disabled/commands")
        .join(format!("{id}.md"));
    if disabled.exists() {
        let mut command = parse_command(scope, &id, &disabled)?;
        command.disabled = true;
        return Ok(command);
    }
    parse_command(scope, &id, &path)
}

pub fn write(
    paths: &StudioPaths,
    scope: &str,
    id: &str,
    input: CommandWriteInput,
    backups_dir: &Path,
) -> AppResult<CommandFile> {
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
    let path = command_path(paths, scope, &id)?;

    let existing = read_text(&path)?;
    let mut document = match existing {
        Some(snapshot) => markdown::parse(&snapshot.content)?,
        None => MarkdownDocument {
            frontmatter: Mapping::new(),
            body: String::new(),
        },
    };

    let mapping = &mut document.frontmatter;
    markdown::set_string(mapping, "description", input.description.as_deref());
    markdown::set_string(mapping, "agent", input.agent.as_deref());
    markdown::set_string(mapping, "model", input.model.as_deref());

    match input.subagent {
        Some(value) => {
            mapping.insert(Value::String("subagent".into()), Value::Bool(value));
        }
        None => {
            mapping.remove(Value::String("subagent".into()));
        }
    }

    let content = markdown::render(mapping, &input.template)?;
    write_text_atomic(
        &path,
        &content,
        input.expected_sha256.as_deref(),
        backups_dir,
    )?;

    parse_command(scope, &id, &path)
}

pub fn delete(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<()> {
    let id = validate_slug(id, true)?;
    let path = command_path(paths, scope, &id)?;
    if path.exists() {
        return file_ops::delete_path(&path);
    }
    let disabled = paths
        .opencode_dir(scope)?
        .join("disabled/commands")
        .join(format!("{id}.md"));
    file_ops::delete_path(&disabled)
}

fn command_path(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<PathBuf> {
    Ok(paths.commands_dir(scope)?.join(format!("{id}.md")))
}

fn parse_command(scope: &str, id: &str, path: &Path) -> AppResult<CommandFile> {
    let snapshot = read_text(path)?
        .ok_or_else(|| AppError::NotFound(format!("command {}", path.display())))?;
    let document = markdown::parse(&snapshot.content)?;
    let mapping = &document.frontmatter;

    Ok(CommandFile {
        id: id.to_string(),
        scope: scope.to_string(),
        path: path.to_string_lossy().to_string(),
        description: markdown::get_string(mapping, "description"),
        agent: markdown::get_string(mapping, "agent"),
        model: markdown::get_string(mapping, "model"),
        subagent: markdown::get_bool(mapping, "subagent"),
        template: document.body,
        sha256: snapshot.sha256,
        disabled: false,
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

    #[test]
    fn writes_reads_and_deletes_command() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);

        let command = write(
            &paths,
            "workspace",
            "auditar",
            CommandWriteInput {
                description: Some("Auditoria técnica".into()),
                agent: Some("auditor".into()),
                model: Some("opencode-go/glm-5.3".into()),
                subagent: Some(true),
                template: "Analise $ARGUMENTS.".into(),
                expected_sha256: None,
            },
            &temp.path().join("backups"),
        )
        .expect("write");

        assert_eq!(command.id, "auditar");
        assert_eq!(command.agent.as_deref(), Some("auditor"));
        assert_eq!(command.subagent, Some(true));
        assert_eq!(command.template, "Analise $ARGUMENTS.");
        assert!(command.path.ends_with("commands/auditar.md"));

        let listed = list(&paths, "workspace").expect("list");
        assert_eq!(listed.len(), 1);

        delete(&paths, "workspace", "auditar").expect("delete");
        assert!(list(&paths, "workspace").expect("list").is_empty());
    }
}

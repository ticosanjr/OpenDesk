use super::{
    file_ops::{self, read_text, validate_relative_file, validate_skill_slug, write_text_atomic},
    markdown::{self, MarkdownDocument},
    paths::StudioPaths,
};
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExtraFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    pub id: String,
    pub scope: String,
    pub path: String,
    pub name: String,
    pub description: Option<String>,
    pub body: String,
    pub files: Vec<SkillExtraFile>,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillWriteInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub files: Vec<SkillExtraFile>,
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

pub fn list(paths: &StudioPaths, scope: &str) -> AppResult<Vec<SkillFile>> {
    let dir = paths.skills_dir(scope)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();

    for entry in WalkDir::new(&dir)
        .min_depth(1)
        .max_depth(6)
        .follow_links(false)
    {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            if let Some(parent) = path.parent() {
                if let Ok(id) = relative_id(&dir, parent) {
                    if !id.is_empty() {
                        found.insert(id, path.to_path_buf());
                    }
                }
            }
        } else if path.parent() == Some(dir.as_path())
            && path.extension().and_then(|value| value.to_str()) == Some("md")
        {
            if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                found
                    .entry(stem.to_string())
                    .or_insert_with(|| path.to_path_buf());
            }
        }
    }

    let mut skills = Vec::new();
    for (id, path) in found {
        match parse_skill(scope, &id, &path) {
            Ok(skill) => skills.push(skill),
            Err(error) => {
                tracing::warn!(path = %path.display(), error = %error, "skipping invalid skill")
            }
        }
    }
    Ok(skills)
}

pub fn get(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<SkillFile> {
    let id = validate_skill_slug(id)?;
    let path = skill_path(paths, scope, &id)?;
    parse_skill(scope, &id, &path)
}

pub fn write(
    paths: &StudioPaths,
    scope: &str,
    id: &str,
    input: SkillWriteInput,
    backups_dir: &Path,
) -> AppResult<SkillFile> {
    let id = validate_skill_slug(id)?;
    let dir = paths.skills_dir(scope)?.join(&id);
    let skill_md = dir.join("SKILL.md");

    let existing = read_text(&skill_md)?;
    let mut document = match existing {
        Some(snapshot) => markdown::parse(&snapshot.content)?,
        None => MarkdownDocument {
            frontmatter: Mapping::new(),
            body: String::new(),
        },
    };

    let display_name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&id)
        .to_string();

    let mapping = &mut document.frontmatter;
    mapping.insert(Value::String("name".into()), Value::String(display_name));
    markdown::set_string(mapping, "description", input.description.as_deref());

    let content = markdown::render(mapping, &input.body)?;
    write_text_atomic(
        &skill_md,
        &content,
        input.expected_sha256.as_deref(),
        backups_dir,
    )?;

    for file in input.files {
        let relative = validate_relative_file(&file.path)?;
        let target = dir.join(&relative);
        write_text_atomic(&target, &file.content, None, backups_dir)?;
    }

    parse_skill(scope, &id, &skill_md)
}

pub fn delete(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<()> {
    let id = validate_skill_slug(id)?;
    let dir = paths.skills_dir(scope)?;
    let skill_dir = dir.join(&id);
    if skill_dir.exists() {
        let canonical_dir = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
        let canonical_skill = std::fs::canonicalize(&skill_dir).unwrap_or(skill_dir.clone());
        if !canonical_skill.starts_with(canonical_dir) {
            return Err(AppError::BadRequest(
                "skill path escapes the skills root".into(),
            ));
        }
        std::fs::remove_dir_all(&canonical_skill)?;
        return Ok(());
    }

    let flat = dir.join(format!("{id}.md"));
    file_ops::delete_path(&flat)
}

fn skill_path(paths: &StudioPaths, scope: &str, id: &str) -> AppResult<PathBuf> {
    let dir = paths.skills_dir(scope)?;
    let nested = dir.join(id).join("SKILL.md");
    if nested.exists() {
        return Ok(nested);
    }
    Ok(dir.join(format!("{id}.md")))
}

fn parse_skill(scope: &str, id: &str, path: &Path) -> AppResult<SkillFile> {
    let snapshot =
        read_text(path)?.ok_or_else(|| AppError::NotFound(format!("skill {}", path.display())))?;
    let document = markdown::parse(&snapshot.content)?;
    let mapping = &document.frontmatter;

    let mut files = Vec::new();
    if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
        if let Some(parent) = path.parent() {
            for entry in WalkDir::new(parent).min_depth(1).follow_links(false) {
                let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
                if !entry.file_type().is_file() || entry.path() == path {
                    continue;
                }
                let relative = entry
                    .path()
                    .strip_prefix(parent)
                    .map_err(|error| AppError::Internal(error.to_string()))?
                    .to_string_lossy()
                    .replace('\\', "/");
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                files.push(SkillExtraFile {
                    path: relative,
                    content,
                });
            }
            files.sort_by(|a, b| a.path.cmp(&b.path));
        }
    }

    Ok(SkillFile {
        id: id.to_string(),
        scope: scope.to_string(),
        path: path.to_string_lossy().to_string(),
        name: markdown::get_string(mapping, "name").unwrap_or_else(|| id.to_string()),
        description: markdown::get_string(mapping, "description"),
        body: document.body,
        files,
        sha256: snapshot.sha256,
    })
}

fn relative_id(base: &Path, path: &Path) -> AppResult<String> {
    let relative = path
        .strip_prefix(base)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
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
    fn writes_reads_and_deletes_skill_with_extra_file() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);

        let skill = write(
            &paths,
            "workspace",
            "relatorio-executivo",
            SkillWriteInput {
                name: Some("Relatório Executivo".into()),
                description: Some("Padroniza relatórios".into()),
                body: "Sempre comece com TL;DR.".into(),
                files: vec![SkillExtraFile {
                    path: "references/template.md".into(),
                    content: "# Template".into(),
                }],
                expected_sha256: None,
            },
            &temp.path().join("backups"),
        )
        .expect("write");

        assert_eq!(skill.name, "Relatório Executivo");
        assert_eq!(skill.files.len(), 1);
        assert_eq!(skill.files[0].path, "references/template.md");
        assert!(skill.path.ends_with("skills/relatorio-executivo/SKILL.md"));

        let listed = list(&paths, "workspace").expect("list");
        assert_eq!(listed.len(), 1);

        delete(&paths, "workspace", "relatorio-executivo").expect("delete");
        assert!(list(&paths, "workspace").expect("list").is_empty());
    }

    #[test]
    fn rejects_path_traversal_in_extra_files() {
        let temp = TempDir::new().expect("temp");
        let paths = paths(&temp);
        let error = write(
            &paths,
            "workspace",
            "safe",
            SkillWriteInput {
                name: None,
                description: None,
                body: "body".into(),
                files: vec![SkillExtraFile {
                    path: "../escape.md".into(),
                    content: "bad".into(),
                }],
                expected_sha256: None,
            },
            &temp.path().join("backups"),
        )
        .expect_err("traversal");
        assert!(matches!(error, AppError::BadRequest(_)));
    }
}

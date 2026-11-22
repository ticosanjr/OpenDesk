use crate::{
    config_sync::{agents, commands, jsonc_editor, paths::StudioPaths, skills},
    error::{AppError, AppResult},
};

use std::path::{Path, PathBuf};

use super::file_ops::validate_slug;

mod copy;
mod delete;
mod list;
mod toggle;

#[cfg(test)]
mod tests;

pub use list::ItemSnapshot;

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

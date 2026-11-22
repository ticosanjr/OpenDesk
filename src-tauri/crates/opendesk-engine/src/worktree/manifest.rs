use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "output", ".cache"];

/// Baseline of an overlay worktree: sha256 per file at creation time. The merge
/// uses it to tell apart files the run changed from files the user changed
/// after the run started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BaseManifest {
    version: u32,
    pub(super) files: BTreeMap<String, String>,
}

impl Default for BaseManifest {
    fn default() -> Self {
        Self {
            version: 1,
            files: BTreeMap::new(),
        }
    }
}

pub fn manifest_path(worktree_path: &Path) -> PathBuf {
    let mut name = worktree_path.as_os_str().to_owned();
    name.push(".manifest.json");
    PathBuf::from(name)
}

pub(super) fn relative_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn hash_file(path: &Path) -> AppResult<String> {
    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn build_manifest(root: &Path) -> AppResult<BaseManifest> {
    let mut manifest = BaseManifest::default();
    for entry in WalkDir::new(root).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        manifest
            .files
            .insert(relative_key(relative), hash_file(entry.path())?);
    }
    Ok(manifest)
}

pub(super) fn write_manifest(worktree_path: &Path, manifest: &BaseManifest) -> AppResult<()> {
    let path = manifest_path(worktree_path);
    let body = serde_json::to_vec(manifest)
        .map_err(|error| AppError::Internal(format!("manifesto do worktree: {error}")))?;
    fs::write(&path, body)?;
    Ok(())
}

pub(super) fn load_manifest(worktree_path: &Path) -> AppResult<Option<BaseManifest>> {
    let path = manifest_path(worktree_path);
    let body = match fs::read(&path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let manifest = serde_json::from_slice(&body)
        .map_err(|error| AppError::Internal(format!("manifesto do worktree inválido: {error}")))?;
    Ok(Some(manifest))
}

pub(super) fn not_skipped(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !SKIP_DIRS.contains(&name.as_ref())
}

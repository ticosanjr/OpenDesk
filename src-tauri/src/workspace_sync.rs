//! Delta sync for remote nodes.
//!
//! A node sends its manifest; the controller builds a tar.zst bundle with only
//! added/changed files. Applying validates every path and never follows
//! symlinks outside the destination.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "output", ".cache"];
const MAX_HASH_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    /// Modification time in milliseconds since the Unix epoch.
    pub modified_ms: i64,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncDelta {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BundleReport {
    pub path: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyReport {
    pub files: usize,
    pub bytes: u64,
    pub dry_run: bool,
}

pub fn manifest(root: &Path) -> AppResult<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| AppError::Internal(error.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let sha256 = if metadata.len() <= MAX_HASH_BYTES {
            Some(hash_file(entry.path())?)
        } else {
            None
        };
        entries.push(FileEntry {
            path: relative,
            size: metadata.len(),
            modified_ms: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as i64)
                .unwrap_or(0),
            sha256,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

pub fn delta(base: &[FileEntry], target: &[FileEntry]) -> SyncDelta {
    use std::collections::HashMap;
    let base: HashMap<&str, &FileEntry> = base
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut delta = SyncDelta::default();
    for entry in target {
        match base.get(entry.path.as_str()) {
            None => {
                delta.added.push(entry.path.clone());
                delta.total_bytes += entry.size;
            }
            Some(previous) if changed(previous, entry) => {
                delta.changed.push(entry.path.clone());
                delta.total_bytes += entry.size;
            }
            Some(_) => {}
        }
    }
    let target_paths: std::collections::HashSet<&str> =
        target.iter().map(|entry| entry.path.as_str()).collect();
    for entry in base.values() {
        if !target_paths.contains(entry.path.as_str()) {
            delta.removed.push(entry.path.to_string());
        }
    }
    delta.added.sort();
    delta.changed.sort();
    delta.removed.sort();
    delta
}

fn changed(left: &FileEntry, right: &FileEntry) -> bool {
    if left.size != right.size {
        return true;
    }
    match (&left.sha256, &right.sha256) {
        (Some(left), Some(right)) => left != right,
        _ => left.modified_ms != right.modified_ms,
    }
}

pub fn bundle(root: &Path, files: &[String], destination: &Path) -> AppResult<BundleReport> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(destination)?;
    let encoder = zstd::Encoder::new(file, 3)
        .map_err(|error| AppError::Internal(format!("zstd: {error}")))?;
    let mut archive = tar::Builder::new(encoder);
    let mut bytes = 0u64;
    for relative in files {
        let safe = safe_relative(relative)?;
        let path = root.join(&safe);
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            continue;
        }
        bytes += metadata.len();
        archive
            .append_path_with_name(&path, &safe)
            .map_err(|error| AppError::Internal(format!("tar: {error}")))?;
    }
    let encoder = archive
        .into_inner()
        .map_err(|error| AppError::Internal(format!("tar: {error}")))?;
    encoder
        .finish()
        .map_err(|error| AppError::Internal(format!("zstd: {error}")))?;
    Ok(BundleReport {
        path: destination.to_string_lossy().to_string(),
        files: files.len(),
        bytes,
    })
}

pub fn apply(bundle_path: &Path, destination: &Path, dry_run: bool) -> AppResult<ApplyReport> {
    let file = fs::File::open(bundle_path)?;
    let decoder =
        zstd::Decoder::new(file).map_err(|error| AppError::Internal(format!("zstd: {error}")))?;
    let mut archive = tar::Archive::new(decoder);
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in archive
        .entries()
        .map_err(|error| AppError::Internal(format!("tar: {error}")))?
    {
        let mut entry = entry.map_err(|error| AppError::Internal(format!("tar: {error}")))?;
        let path = entry
            .path()
            .map_err(|error| AppError::Internal(format!("tar path: {error}")))?
            .to_path_buf();
        let safe = safe_relative(&path.to_string_lossy())?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let target = destination.join(&safe);
        files += 1;
        bytes += entry.size();
        if dry_run {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(ApplyReport {
        files,
        bytes,
        dry_run,
    })
}

fn safe_relative(raw: &str) -> AppResult<PathBuf> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(AppError::BadRequest(format!(
            "caminho absoluto no bundle: {raw}"
        )));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => {
                return Err(AppError::BadRequest(format!(
                    "caminho inseguro no bundle: {raw}"
                )));
            }
        }
    }
    if path.as_os_str().is_empty() {
        return Err(AppError::BadRequest("caminho vazio no bundle".into()));
    }
    Ok(path.to_path_buf())
}

fn hash_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
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

fn not_skipped(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !SKIP_DIRS.contains(&name.as_ref())
}

/// Validates a bundle path so only files created by OpenDesk sync can be read.
pub fn validate_bundle_path(path: &Path, data_dir: &Path) -> AppResult<PathBuf> {
    let canonical = fs::canonicalize(path)?;
    let root = fs::canonicalize(data_dir.join("sync"))?;
    if !canonical.starts_with(&root) {
        return Err(AppError::BadRequest(
            "bundle fora do diretório de sync do OpenDesk".into(),
        ));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_delta_bundle_and_applies_it() {
        let dir = tempfile::tempdir().expect("temp");
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        fs::create_dir_all(&source).expect("source");
        fs::create_dir_all(&target).expect("target");
        fs::write(source.join("a.txt"), "one").expect("a");
        fs::write(source.join("b.txt"), "two").expect("b");
        let base = manifest(&source).expect("base");

        fs::write(source.join("a.txt"), "one changed").expect("a2");
        fs::write(source.join("c.txt"), "three").expect("c");
        let current = manifest(&source).expect("current");
        let delta = delta(&base, &current);
        assert_eq!(delta.added, vec!["c.txt"]);
        assert_eq!(delta.changed, vec!["a.txt"]);
        assert!(delta.removed.is_empty());

        let bundle_path = dir.path().join("sync.pkg");
        let report = bundle(
            &source,
            &delta
                .added
                .iter()
                .chain(delta.changed.iter())
                .cloned()
                .collect::<Vec<_>>(),
            &bundle_path,
        )
        .expect("bundle");
        assert!(report.files >= 2);
        let dry = apply(&bundle_path, &target, true).expect("dry");
        assert!(dry.dry_run);
        assert!(!target.join("a.txt").exists());
        apply(&bundle_path, &target, false).expect("apply");
        assert_eq!(
            fs::read_to_string(target.join("a.txt")).expect("a"),
            "one changed"
        );
        assert_eq!(
            fs::read_to_string(target.join("c.txt")).expect("c"),
            "three"
        );
        assert!(
            !target.join("b.txt").exists(),
            "unchanged files are not in the delta"
        );
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_relative("../evil").is_err());
        assert!(safe_relative("/etc/passwd").is_err());
        assert!(safe_relative("ok/file.txt").is_ok());
    }
}

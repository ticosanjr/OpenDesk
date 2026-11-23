use super::ranking::{snippet, urlencode};
use super::*;

pub(super) async fn add_studio_hits(
    state: &AppState,
    workspace_id: &str,
    needle: &str,
    hits: &mut Vec<SearchHit>,
) {
    let Ok(workspace) = crate::db::workspaces::get(&state.db, workspace_id).await else {
        return;
    };
    let paths = StudioPaths::new(Some(PathBuf::from(workspace.root_path)));
    let needle_lower = needle.to_lowercase();
    for scope in ["workspace", "global"] {
        if let Ok(items) = agents::list(&paths, scope) {
            for item in items {
                let haystack = format!(
                    "{} {} {}",
                    item.id,
                    item.name.clone().unwrap_or_default(),
                    item.description.clone().unwrap_or_default()
                )
                .to_lowercase();
                if haystack.contains(&needle_lower) {
                    hits.push(SearchHit {
                        link: format!(
                            "/studio?kind=agent&id={}&scope={}&workspace_id={}",
                            urlencode(&item.id),
                            scope,
                            urlencode(workspace_id)
                        ),
                        id: item.id.clone(),
                        kind: "agent".into(),
                        title: item.name.unwrap_or_else(|| item.id.clone()),
                        snippet: format!("agente {} · {}", scope, item.id),
                        updated_at: None,
                    });
                }
            }
        }
        if let Ok(items) = skills::list(&paths, scope) {
            for item in items {
                let haystack = format!(
                    "{} {} {}",
                    item.id,
                    item.name.clone(),
                    item.description.clone().unwrap_or_default()
                )
                .to_lowercase();
                if haystack.contains(&needle_lower) {
                    hits.push(SearchHit {
                        link: format!(
                            "/studio?kind=skill&id={}&scope={}&workspace_id={}",
                            urlencode(&item.id),
                            scope,
                            urlencode(workspace_id)
                        ),
                        id: item.id.clone(),
                        kind: "agent".into(),
                        title: item.name,
                        snippet: format!("skill {} · {}", scope, item.id),
                        updated_at: None,
                    });
                }
            }
        }
        if let Ok(items) = commands::list(&paths, scope) {
            for item in items {
                let haystack = format!(
                    "{} {} {}",
                    item.id,
                    item.description.clone().unwrap_or_default(),
                    scope
                )
                .to_lowercase();
                if haystack.contains(&needle_lower) {
                    hits.push(SearchHit {
                        link: format!(
                            "/studio?kind=command&id={}&scope={}&workspace_id={}",
                            urlencode(&item.id),
                            scope,
                            urlencode(workspace_id)
                        ),
                        id: item.id.clone(),
                        kind: "agent".into(),
                        title: format!("/{}", item.id),
                        snippet: format!("comando {} · {}", scope, item.id),
                        updated_at: None,
                    });
                }
            }
        }
    }
}

pub(super) async fn add_file_hits(
    state: &AppState,
    workspace_id: &str,
    needle: &str,
    hits: &mut Vec<SearchHit>,
    limit: usize,
) -> AppResult<()> {
    let Ok(workspace) = crate::db::workspaces::get(&state.db, workspace_id).await else {
        return Ok(());
    };
    let root = PathBuf::from(&workspace.root_path);
    let needle = needle.to_lowercase();
    let mut found = 0usize;
    walk_files(&root, &root, &needle, hits, &mut found, limit.min(20), 0);
    Ok(())
}

fn walk_files(
    root: &PathBuf,
    directory: &PathBuf,
    needle: &str,
    hits: &mut Vec<SearchHit>,
    found: &mut usize,
    limit: usize,
    depth: usize,
) {
    if depth > 4 || *found >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if *found >= limit {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            ".git" | "node_modules" | "target" | "dist" | ".cache" | ".vite"
        ) {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            walk_files(root, &path, needle, hits, found, limit, depth + 1);
            continue;
        }
        if metadata.len() > 512 * 1024 {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.iter().take(4000).any(|byte| *byte == 0) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if !text.to_lowercase().contains(needle) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        hits.push(SearchHit {
            link: format!("/files?path={}", urlencode(&relative)),
            id: relative.clone(),
            kind: "file".into(),
            title: relative,
            snippet: snippet(&text, needle),
            updated_at: metadata
                .modified()
                .ok()
                .map(|value| chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()),
        });
        *found += 1;
    }
}

pub(super) fn recent_files(root: &std::path::Path, limit: usize) -> Vec<SearchHit> {
    let mut files = Vec::new();
    walk_recent(root, root, &mut files, limit, 0);
    files.sort_by(|left, right| {
        right
            .updated_at
            .as_deref()
            .unwrap_or_default()
            .cmp(left.updated_at.as_deref().unwrap_or_default())
    });
    files.truncate(limit);
    files
}

fn walk_recent(
    root: &std::path::Path,
    directory: &std::path::Path,
    files: &mut Vec<SearchHit>,
    limit: usize,
    depth: usize,
) {
    if depth > 4 || files.len() >= limit * 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            ".git" | "node_modules" | "target" | "dist" | ".cache" | ".vite"
        ) {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            walk_recent(root, &path, files, limit, depth + 1);
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        files.push(SearchHit {
            link: format!("/files?path={}", urlencode(&relative)),
            id: relative.clone(),
            kind: "file".into(),
            title: relative,
            snippet: format!("{} KB", metadata.len().div_ceil(1024)),
            updated_at: metadata
                .modified()
                .ok()
                .map(|value| chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()),
        });
    }
}

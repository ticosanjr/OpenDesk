use std::path::Path;
use walkdir::WalkDir;

pub(super) fn list_input_files(root: &Path, limit: usize) -> Vec<String> {
    let input = root.join("input");
    if !input.exists() {
        return Vec::new();
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(&input).max_depth(4).follow_links(false) {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if let Ok(relative) = entry.path().strip_prefix(root) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
            if files.len() >= limit {
                break;
            }
        }
    }
    files.sort();
    files
}

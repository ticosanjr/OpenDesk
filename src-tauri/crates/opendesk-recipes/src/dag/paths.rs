use serde_json::Value;

pub(super) fn child_path(parent: Option<&str>, id: &str) -> String {
    match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}/{id}"),
        _ => id.to_string(),
    }
}

pub(super) fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

pub(super) fn item_key(item: &Value, index: usize) -> String {
    if let Some(path) = item.as_str() {
        return format!("{index:04}-{}", file_stem(path));
    }
    if let Some(key) = item.get("key").and_then(Value::as_str)
        && !key.trim().is_empty()
    {
        let sanitized: String = key
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '-'
                }
            })
            .collect();
        return format!("{index:04}-{sanitized}");
    }
    if let Some(name) = item.get("path").and_then(Value::as_str) {
        return format!("{index:04}-{}", file_stem(name));
    }
    format!("{index:04}")
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "item".into())
}

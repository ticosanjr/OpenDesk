use std::path::Path;

pub fn mime_for_path(path: &Path) -> &'static str {
    mime_for_name(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default(),
    )
}

fn mime_for_name(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" | "cjs" => "text/javascript",
        "json" | "jsonc" => "application/json",
        "md" | "markdown" | "mdx" => "text/markdown",
        "txt" | "log" | "jsonl" | "csv" | "tsv" | "yaml" | "yml" | "toml" => "text/plain",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "m4a" => "audio/mp4",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Content-Type with charset for browser responses.
pub fn content_type_for_path(path: &Path) -> &'static str {
    match mime_for_path(path) {
        "text/html" => "text/html; charset=utf-8",
        "text/css" => "text/css; charset=utf-8",
        "text/javascript" => "text/javascript; charset=utf-8",
        "application/json" => "application/json; charset=utf-8",
        "text/markdown" => "text/markdown; charset=utf-8",
        "text/plain" => "text/plain; charset=utf-8",
        other => other,
    }
}

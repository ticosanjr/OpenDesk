use chrono::{Datelike, Utc};
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub fn slugify(value: &str) -> String {
    let normalized: String = value
        .nfd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .collect();
    let slug: String = normalized
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "task".into()
    } else {
        slug.chars().take(48).collect()
    }
}

pub fn output_directory(root: &Path, title: &str) -> PathBuf {
    let now = Utc::now();
    let month = format!("{:04}-{:02}", now.year(), now.month());
    let base = root.join("output").join(month).join(slugify(title));
    if !base.exists() {
        return base;
    }
    for index in 2..100 {
        let candidate = base.with_file_name(format!(
            "{}-{}",
            base.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("task"),
            index
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    base
}

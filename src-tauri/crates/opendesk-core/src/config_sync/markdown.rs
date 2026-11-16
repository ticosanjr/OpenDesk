use crate::error::{AppError, AppResult};
use serde_yaml::{Mapping, Value};

#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub frontmatter: Mapping,
    pub body: String,
}

pub fn parse(content: &str) -> AppResult<MarkdownDocument> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    if !content.starts_with("---") {
        return Ok(MarkdownDocument {
            frontmatter: Mapping::new(),
            body: content.trim().to_string(),
        });
    }

    let mut lines = content.lines();
    let first = lines.next().unwrap_or_default();
    if first.trim() != "---" {
        return Ok(MarkdownDocument {
            frontmatter: Mapping::new(),
            body: content.trim().to_string(),
        });
    }

    let mut frontmatter_text = String::new();
    let mut found_end = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            found_end = true;
            break;
        }
        frontmatter_text.push_str(line);
        frontmatter_text.push('\n');
    }

    if !found_end {
        return Err(AppError::BadRequest(
            "frontmatter opened with --- but never closed".into(),
        ));
    }

    let frontmatter = if frontmatter_text.trim().is_empty() {
        Mapping::new()
    } else {
        serde_yaml::from_str::<Value>(&frontmatter_text)
            .map_err(|error| AppError::BadRequest(format!("invalid YAML frontmatter: {error}")))?
            .as_mapping()
            .cloned()
            .unwrap_or_default()
    };

    let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();

    Ok(MarkdownDocument { frontmatter, body })
}

pub fn render(frontmatter: &Mapping, body: &str) -> AppResult<String> {
    let body = body.trim();

    if frontmatter.is_empty() {
        return Ok(format!("{body}\n"));
    }

    let yaml = serde_yaml::to_string(&Value::Mapping(frontmatter.clone()))
        .map_err(|error| AppError::Internal(format!("could not serialize frontmatter: {error}")))?;
    let yaml = yaml.trim_end();

    if body.is_empty() {
        Ok(format!("---\n{yaml}\n---\n"))
    } else {
        Ok(format!("---\n{yaml}\n---\n\n{body}\n"))
    }
}

pub fn get_string(mapping: &Mapping, key: &str) -> Option<String> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_str)
        .map(|value| value.to_string())
}

pub fn get_bool(mapping: &Mapping, key: &str) -> Option<bool> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_bool)
}

pub fn get_u32(mapping: &Mapping, key: &str) -> Option<u32> {
    mapping
        .get(Value::String(key.to_string()))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

pub fn set_string(mapping: &mut Mapping, key: &str, value: Option<&str>) {
    let key = Value::String(key.to_string());
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => {
            mapping.insert(key, Value::String(value.to_string()));
        }
        None => {
            mapping.remove(&key);
        }
    }
}

pub fn set_bool(mapping: &mut Mapping, key: &str, value: bool) {
    let key = Value::String(key.to_string());
    if value {
        mapping.insert(key, Value::Bool(true));
    } else {
        mapping.remove(&key);
    }
}

pub fn set_u32(mapping: &mut Mapping, key: &str, value: Option<u32>) {
    let key = Value::String(key.to_string());
    match value.filter(|value| *value > 0) {
        Some(value) => {
            mapping.insert(key, Value::Number(value.into()));
        }
        None => {
            mapping.remove(&key);
        }
    }
}

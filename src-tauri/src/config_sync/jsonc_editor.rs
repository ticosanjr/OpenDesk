use super::{
    file_ops::{read_text, sha256_hex, validate_slug, write_text_atomic},
    paths::StudioPaths,
};
use crate::error::{AppError, AppResult};
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::{ParseOptions, parse_to_serde_value};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

const DEFAULT_CONFIG: &str = "{\n  \"$schema\": \"https://opencode.ai/config.json\"\n}\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDocument {
    pub path: String,
    pub content: String,
    pub sha256: String,
    pub exists: bool,
    pub parsed: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub server: Value,
}

pub fn read_document(path: &Path) -> AppResult<ConfigDocument> {
    let snapshot = read_text(path)?;
    let exists = snapshot.is_some();
    let content = snapshot
        .as_ref()
        .map(|snapshot| snapshot.content.clone())
        .unwrap_or_else(|| DEFAULT_CONFIG.to_string());
    let sha256 = snapshot
        .as_ref()
        .map(|snapshot| snapshot.sha256.clone())
        .unwrap_or_default();
    let parsed = parse_jsonc(&content)?;

    Ok(ConfigDocument {
        path: path.to_string_lossy().to_string(),
        content,
        sha256,
        exists,
        parsed,
    })
}

pub fn read_for_scope(paths: &StudioPaths, scope: &str) -> AppResult<ConfigDocument> {
    read_document(&paths.config_path(scope)?)
}

pub fn write_document(
    path: &Path,
    content: &str,
    expected_sha256: Option<&str>,
    backups_dir: &Path,
) -> AppResult<ConfigDocument> {
    parse_jsonc(content)?;
    write_text_atomic(path, content, expected_sha256, backups_dir)?;
    read_document(path)
}

pub fn write_for_scope(
    paths: &StudioPaths,
    scope: &str,
    content: &str,
    expected_sha256: Option<&str>,
    backups_dir: &Path,
) -> AppResult<ConfigDocument> {
    let path = paths.config_path(scope)?;
    write_document(&path, content, expected_sha256, backups_dir)
}

pub fn list_mcp_servers(document: &ConfigDocument) -> Vec<McpServer> {
    let Some(mcp) = document.parsed.get("mcp") else {
        return Vec::new();
    };

    let servers = mcp.get("servers").or_else(|| {
        // Read-only compatibility with V1 files where servers lived directly
        // under `mcp`. A V2 config always has the `servers` object.
        mcp.as_object().and_then(|object| {
            let looks_like_servers = !object.is_empty()
                && object
                    .values()
                    .all(|value| value.get("type").is_some() || value.get("command").is_some());
            if looks_like_servers { Some(mcp) } else { None }
        })
    });

    let Some(servers) = servers.and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut result: Vec<McpServer> = servers
        .iter()
        .map(|(name, server)| McpServer {
            name: name.clone(),
            server: server.clone(),
        })
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

pub fn upsert_mcp_server(
    path: &Path,
    name: &str,
    server: &Value,
    expected_sha256: Option<&str>,
    backups_dir: &Path,
) -> AppResult<ConfigDocument> {
    let name = validate_slug(name, false)?;
    if !server.is_object() {
        return Err(AppError::BadRequest(
            "MCP server must be a JSON object".into(),
        ));
    }

    let current = read_text(path)?
        .map(|snapshot| snapshot.content)
        .unwrap_or_else(|| DEFAULT_CONFIG.to_string());

    let root = parse_cst(&current)?;
    let root_object = root.object_value_or_set();
    let mcp = root_object.object_value_or_set("mcp");
    let servers = mcp.object_value_or_set("servers");

    let value = json_to_cst(server);
    match servers.get(&name) {
        Some(property) => property.set_value(value),
        None => {
            servers.append(&name, value);
        }
    }

    let content = root.to_string();
    write_text_atomic(path, &content, expected_sha256, backups_dir)?;
    read_document(path)
}

pub fn delete_mcp_server(
    path: &Path,
    name: &str,
    expected_sha256: Option<&str>,
    backups_dir: &Path,
) -> AppResult<ConfigDocument> {
    let name = validate_slug(name, false)?;
    let snapshot =
        read_text(path)?.ok_or_else(|| AppError::NotFound(format!("config {}", path.display())))?;

    let root = parse_cst(&snapshot.content)?;
    let root_object = root.object_value_or_set();
    if let Some(mcp) = root_object.object_value("mcp")
        && let Some(servers) = mcp.object_value("servers")
        && let Some(property) = servers.get(&name)
    {
        property.remove();
    }

    let content = root.to_string();
    write_text_atomic(path, &content, expected_sha256, backups_dir)?;
    read_document(path)
}

/// Generic nested value update used by permissions and future Studio tabs.
#[allow(dead_code)]
pub fn set_config_value(
    path: &Path,
    keys: &[&str],
    value: Option<Value>,
    expected_sha256: Option<&str>,
    backups_dir: &Path,
) -> AppResult<ConfigDocument> {
    if keys.is_empty() {
        return Err(AppError::BadRequest("a config key is required".into()));
    }

    let current = read_text(path)?
        .map(|snapshot| snapshot.content)
        .unwrap_or_else(|| DEFAULT_CONFIG.to_string());
    let root = parse_cst(&current)?;
    let mut object: CstObject = root.object_value_or_set();

    for key in &keys[..keys.len() - 1] {
        object = object.object_value_or_set(key);
    }

    let last = keys[keys.len() - 1];
    match value {
        Some(value) => {
            let cst = json_to_cst(&value);
            match object.get(last) {
                Some(property) => property.set_value(cst),
                None => {
                    object.append(last, cst);
                }
            }
        }
        None => {
            if let Some(property) = object.get(last) {
                property.remove();
            }
        }
    }

    let content = root.to_string();
    write_text_atomic(path, &content, expected_sha256, backups_dir)?;
    read_document(path)
}

#[allow(dead_code)]
pub fn content_hash(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

fn parse_jsonc(content: &str) -> AppResult<Value> {
    parse_to_serde_value::<Value>(content, &ParseOptions::default())
        .map_err(|error| AppError::BadRequest(format!("invalid JSONC: {error}")))
}

fn parse_cst(content: &str) -> AppResult<CstRootNode> {
    CstRootNode::parse(content, &ParseOptions::default())
        .map_err(|error| AppError::BadRequest(format!("invalid JSONC: {error}")))
}

fn json_to_cst(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(value) => CstInputValue::Bool(*value),
        Value::Number(value) => CstInputValue::Number(value.to_string()),
        Value::String(value) => CstInputValue::String(value.clone()),
        Value::Array(values) => CstInputValue::Array(values.iter().map(json_to_cst).collect()),
        Value::Object(values) => CstInputValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), json_to_cst(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use tempfile::TempDir;

    fn write_config(temp: &TempDir, content: &str) -> std::path::PathBuf {
        let path = temp.path().join("opencode.jsonc");
        std::fs::write(&path, content).expect("write config");
        path
    }

    #[test]
    fn upsert_and_delete_mcp_preserve_comments() {
        let temp = TempDir::new().expect("temp");
        let path = write_config(
            &temp,
            r#"{
  // keep this comment
  "model": "opencode-go/deepseek-v4.1-flash",
  "mcp": {
    // servers live here
    "servers": {
      "existing": { "type": "local", "command": ["echo"] }
    }
  }
}"#,
        );

        let document = upsert_mcp_server(
            &path,
            "new-server",
            &json!({"type": "remote", "url": "https://example.com/mcp"}),
            None,
            &temp.path().join("backups"),
        )
        .expect("upsert");

        assert!(document.content.contains("// keep this comment"));
        assert!(document.content.contains("// servers live here"));
        assert_eq!(
            document
                .parsed
                .pointer("/mcp/servers/new-server/type")
                .and_then(Value::as_str),
            Some("remote")
        );
        assert_eq!(
            document
                .parsed
                .pointer("/mcp/servers/existing/type")
                .and_then(Value::as_str),
            Some("local")
        );

        let document = delete_mcp_server(
            &path,
            "existing",
            Some(&document.sha256),
            &temp.path().join("backups"),
        )
        .expect("delete");

        assert!(document.content.contains("// keep this comment"));
        assert!(document.parsed.pointer("/mcp/servers/existing").is_none());
        assert!(document.parsed.pointer("/mcp/servers/new-server").is_some());
    }

    #[test]
    fn rejects_invalid_jsonc_and_stale_hash() {
        let temp = TempDir::new().expect("temp");
        let path = write_config(&temp, "{ invalid");
        let error = read_document(&path).expect_err("invalid jsonc");
        assert!(matches!(error, AppError::BadRequest(_)));

        let path = write_config(&temp, "{\n  \"model\": \"a/b\"\n}\n");
        let document = read_document(&path).expect("read");
        std::fs::write(&path, "{\n  \"model\": \"changed\"\n}\n").expect("external edit");

        let error = write_document(
            &path,
            "{\n  \"model\": \"other\"\n}\n",
            Some(&document.sha256),
            &temp.path().join("backups"),
        )
        .expect_err("stale hash");
        assert!(matches!(error, AppError::Conflict(_)));
    }
}

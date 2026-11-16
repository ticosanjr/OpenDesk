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
    let Some(mcp) = document.parsed.get("mcp").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    // V1: servers live directly under `mcp`.
    for (name, server) in mcp {
        if name == "servers" || !is_mcp_server(server) {
            continue;
        }
        result.push(McpServer {
            name: name.clone(),
            server: normalize_mcp_server(server.clone()),
        });
    }
    // V2: servers live under `mcp.servers` and win over a same-name V1 entry.
    if let Some(servers) = mcp.get("servers").and_then(Value::as_object) {
        for (name, server) in servers {
            result.retain(|entry| &entry.name != name);
            result.push(McpServer {
                name: name.clone(),
                server: normalize_mcp_server(server.clone()),
            });
        }
    }
    result.sort_by(|left, right| left.name.cmp(&right.name));
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpShape {
    V1,
    V2,
}

fn is_mcp_server(value: &Value) -> bool {
    value.is_object()
        && (value.get("type").is_some()
            || value.get("command").is_some()
            || value.get("url").is_some())
}

fn mcp_shape(parsed: &Value) -> McpShape {
    let Some(mcp) = parsed.get("mcp").and_then(Value::as_object) else {
        return McpShape::V2;
    };
    if mcp.get("servers").and_then(Value::as_object).is_some() {
        return McpShape::V2;
    }
    if mcp.is_empty() || mcp.values().all(is_mcp_server) {
        return McpShape::V1;
    }
    McpShape::V2
}

fn normalize_mcp_server(mut server: Value) -> Value {
    if let Some(enabled) = server.get("enabled").and_then(Value::as_bool) {
        server["disabled"] = Value::Bool(!enabled);
    }
    if !server
        .get("disabled")
        .map(Value::is_boolean)
        .unwrap_or(false)
    {
        server["disabled"] = Value::Bool(false);
    }
    server
}

fn server_for_shape(server: &Value, shape: McpShape) -> Value {
    let mut value = server.clone();
    match shape {
        McpShape::V1 => {
            let disabled = value
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            value["enabled"] = Value::Bool(!disabled);
            if let Some(object) = value.as_object_mut() {
                object.remove("disabled");
            }
        }
        McpShape::V2 => {
            if let Some(enabled) = value.get("enabled").and_then(Value::as_bool) {
                value["disabled"] = Value::Bool(!enabled);
                if let Some(object) = value.as_object_mut() {
                    object.remove("enabled");
                }
            }
        }
    }
    value
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
    let shape = mcp_shape(&parse_jsonc(&current)?);
    let value = json_to_cst(&server_for_shape(server, shape));

    let root = parse_cst(&current)?;
    let root_object = root.object_value_or_set();
    let mcp = root_object.object_value_or_set("mcp");
    let container = match shape {
        McpShape::V2 => mcp.object_value_or_set("servers"),
        McpShape::V1 => mcp,
    };
    match container.get(&name) {
        Some(property) => property.set_value(value),
        None => {
            container.append(&name, value);
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
    let shape = mcp_shape(&parse_jsonc(&snapshot.content)?);

    let root = parse_cst(&snapshot.content)?;
    let root_object = root.object_value_or_set();
    if let Some(mcp) = root_object.object_value("mcp") {
        match shape {
            McpShape::V2 => {
                if let Some(servers) = mcp.object_value("servers")
                    && let Some(property) = servers.get(&name)
                {
                    property.remove();
                }
            }
            McpShape::V1 => {
                if let Some(property) = mcp.get(&name) {
                    property.remove();
                }
            }
        }
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

#[cfg(test)]
mod mcp_shape_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn preserves_v1_direct_mcp_shape_and_enabled_flag() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("opencode.jsonc");
        std::fs::write(
            &path,
            r#"{
  "mcp": {
    "rea": { "type": "local", "command": ["rea", "mcp"], "enabled": true }
  }
}"#,
        )
        .expect("config");
        let backups = temp.path().join("backups");

        let document = upsert_mcp_server(
            &path,
            "rea",
            &serde_json::json!({
                "type": "local",
                "command": ["rea", "mcp"],
                "disabled": true
            }),
            None,
            &backups,
        )
        .expect("upsert");
        assert!(document.parsed.pointer("/mcp/rea").is_some());
        assert!(document.parsed.pointer("/mcp/servers").is_none());
        assert_eq!(
            document
                .parsed
                .pointer("/mcp/rea/enabled")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(document.parsed.pointer("/mcp/rea/disabled").is_none());

        let servers = list_mcp_servers(&document);
        assert_eq!(servers.len(), 1);
        assert_eq!(
            servers[0].server.get("disabled").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn preserves_v2_servers_shape() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("opencode.jsonc");
        std::fs::write(&path, r#"{"mcp":{"servers":{}}}"#).expect("config");
        let backups = temp.path().join("backups");
        let document = upsert_mcp_server(
            &path,
            "fetch",
            &serde_json::json!({"type":"local","command":["npx","fetch"],"disabled":false}),
            None,
            &backups,
        )
        .expect("upsert");
        assert!(document.parsed.pointer("/mcp/servers/fetch").is_some());
        assert!(document.parsed.pointer("/mcp/fetch").is_none());
    }
}

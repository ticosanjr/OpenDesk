//! Safe execution of user-managed script tools.
//!
//! Scripts run in a 0700 temp directory, with `cwd` at the workspace root,
//! validated JSON input on stdin, resolved secret refs in the environment and a
//! hard timeout that kills the whole process group.

use crate::{
    db::{script_tools as repo, script_tools::ScriptTool, workspaces},
    error::{AppError, AppResult},
    paths::AppPaths,
    secrets::SecretStore,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub const RUNTIMES: [&str; 5] = ["bash", "python3", "node", "uv", "custom"];
const MAX_OUTPUT_CHARS: usize = 1024 * 1024;
const MAX_TIMEOUT: i64 = 300;

#[derive(Debug, Clone, Serialize)]
pub struct ScriptRunResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub fn valid_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.len() < 2 || bytes.len() > 49 {
        return false;
    }
    if !(bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit()) {
        return false;
    }
    bytes[1..].iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
    })
}

fn validate_schema(schema_json: &str) -> AppResult<Value> {
    let schema: Value = serde_json::from_str(schema_json)
        .map_err(|error| AppError::BadRequest(format!("input_schema_json inválido: {error}")))?;
    if !schema.is_object() {
        return Err(AppError::BadRequest(
            "input_schema_json deve ser um objeto JSON Schema".into(),
        ));
    }
    jsonschema::validator_for(&schema)
        .map_err(|error| AppError::BadRequest(format!("input_schema inválido: {error}")))?;
    Ok(schema)
}

fn env_refs(value: &str, paths: &AppPaths) -> AppResult<Vec<(String, String)>> {
    let parsed: std::collections::BTreeMap<String, String> = serde_json::from_str(value)
        .map_err(|error| AppError::BadRequest(format!("env_secret_refs_json inválido: {error}")))?;
    let store = SecretStore::new(paths);
    let mut output = Vec::new();
    let mut check = serde_json::Map::new();
    for (name, reference) in parsed {
        if name.is_empty()
            || !name.chars().all(|character| {
                character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
            })
            || name
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        {
            return Err(AppError::BadRequest(format!(
                "nome de variável inválido: {name}"
            )));
        }
        let reference = reference.trim();
        if reference.is_empty() {
            return Err(AppError::BadRequest(format!(
                "secret_ref vazio para {name}"
            )));
        }
        check.insert(format!("{name}_ref"), Value::String(reference.to_string()));
        let secret = store
            .get("script_tools", reference)?
            .or(store.get("integrations", reference)?)
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "secret_ref '{reference}' não encontrado para {name}"
                ))
            })?;
        output.push((name, secret));
    }
    crate::io_bus::secrets_guard::validate_no_plaintext_secrets(&Value::Object(check))?;
    Ok(output)
}

pub fn validate_fields(
    slug: &str,
    name: &str,
    runtime: &str,
    custom_command: Option<&str>,
    script_body: &str,
    input_schema_json: &str,
    timeout_secs: i64,
) -> AppResult<()> {
    if !valid_slug(slug) {
        return Err(AppError::BadRequest(format!(
            "slug inválido '{slug}' (use ^[a-z0-9][a-z0-9_-]{{1,48}}$)"
        )));
    }
    if name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "nome da ferramenta é obrigatório".into(),
        ));
    }
    let runtime = runtime.trim().to_lowercase();
    if !RUNTIMES.contains(&runtime.as_str()) {
        return Err(AppError::BadRequest(format!(
            "runtime inválido '{runtime}'; use {}",
            RUNTIMES.join(", ")
        )));
    }
    if script_body.trim().is_empty() {
        return Err(AppError::BadRequest(
            "script_body não pode ficar vazio".into(),
        ));
    }
    if runtime == "custom"
        && custom_command
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(AppError::BadRequest(
            "runtime custom exige custom_command".into(),
        ));
    }
    validate_schema(input_schema_json)?;
    if !(1..=MAX_TIMEOUT).contains(&timeout_secs) {
        return Err(AppError::BadRequest(format!(
            "timeout_secs deve estar entre 1 e {MAX_TIMEOUT}"
        )));
    }
    Ok(())
}

pub async fn prompt_summary(pool: &SqlitePool, workspace_id: &str) -> AppResult<String> {
    let tools = repo::list_enabled(pool, Some(workspace_id)).await?;
    let lines = tools
        .into_iter()
        .map(|tool| {
            let description = [tool.description.trim(), tool.agent_prompt.trim()]
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(" — ");
            format!("- `script_{}`: {description}", tool.slug)
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(String::new());
    }
    Ok(format!(
        "## Ferramentas (Scripts MCP)\n\nUse as ferramentas abaixo quando fizer sentido; os argumentos são validados pelo OpenDesk.\n\n{}",
        lines.join("\n")
    ))
}

pub fn validate_env_refs(value: &str, paths: &AppPaths) -> AppResult<()> {
    env_refs(value, paths).map(|_| ())
}

pub fn validate_tool(tool: &ScriptTool) -> AppResult<()> {
    validate_fields(
        &tool.slug,
        &tool.name,
        &tool.runtime,
        tool.custom_command.as_deref(),
        &tool.script_body,
        &tool.input_schema_json,
        tool.timeout_secs,
    )
}

fn truncate(text: Vec<u8>) -> String {
    let text = String::from_utf8_lossy(&text).to_string();
    if text.chars().count() <= MAX_OUTPUT_CHARS {
        text
    } else {
        text.chars().take(MAX_OUTPUT_CHARS).collect()
    }
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    // SAFETY: negative pid means "the process group led by pid" for kill(2).
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

fn script_path(dir: &Path, runtime: &str) -> PathBuf {
    match runtime {
        "python3" | "uv" => dir.join("script.py"),
        "node" => dir.join("script.js"),
        _ => dir.join("script.sh"),
    }
}

fn command_for(
    runtime: &str,
    custom: Option<&str>,
    path: &Path,
) -> AppResult<(String, Vec<String>)> {
    match runtime {
        "bash" => Ok(("bash".into(), vec![path.to_string_lossy().to_string()])),
        "python3" => Ok(("python3".into(), vec![path.to_string_lossy().to_string()])),
        "node" => Ok(("node".into(), vec![path.to_string_lossy().to_string()])),
        "uv" => Ok((
            "uv".into(),
            vec!["run".into(), path.to_string_lossy().to_string()],
        )),
        "custom" => {
            let command = custom
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::BadRequest("custom_command ausente".into()))?;
            Ok((
                command.to_string(),
                vec![path.to_string_lossy().to_string()],
            ))
        }
        other => Err(AppError::BadRequest(format!("runtime inválido: {other}"))),
    }
}

pub async fn execute(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    tool: &ScriptTool,
    input: &Value,
) -> AppResult<ScriptRunResult> {
    validate_tool(tool)?;
    let schema = validate_schema(&tool.input_schema_json)?;
    jsonschema::validator_for(&schema)
        .map_err(|error| AppError::BadRequest(format!("input_schema inválido: {error}")))?
        .validate(input)
        .map_err(|error| AppError::BadRequest(format!("input fora do schema: {error}")))?;
    let env = env_refs(&tool.env_secret_refs_json, paths)?;
    let workspace = workspaces::get(pool, workspace_id).await?;
    let root = PathBuf::from(&workspace.root_path);
    if !root.is_dir() {
        return Err(AppError::NotFound(
            "pasta do workspace não encontrada".into(),
        ));
    }
    let runtime = tool.runtime.trim().to_lowercase();
    let id = Uuid::now_v7().to_string();
    let dir = paths.data_dir.join("script-runs").join(&id);
    tokio::fs::create_dir_all(&dir).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = script_path(&dir, &runtime);
    tokio::fs::write(&path, tool.script_body.as_bytes()).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    }
    let input_json = serde_json::to_string(input)
        .map_err(|error| AppError::Internal(format!("input json: {error}")))?;
    let (program, args) = command_for(&runtime, tool.custom_command.as_deref(), &path)?;
    let started = Instant::now();
    let mut command = tokio::process::Command::new(&program);
    command
        .args(&args)
        .current_dir(&root)
        .env("OPENDESK_INPUT_JSON", &input_json)
        .env("OPENDESK_WORKSPACE_ID", workspace_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (name, secret) in env {
        command.env(name, secret);
    }
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(|error| {
        AppError::Internal(format!("não foi possível executar {program}: {error}"))
    })?;
    let pid = child.id();
    if let Some(mut stdin) = child.stdin.take() {
        // Scripts are allowed to ignore stdin and exit early.
        let _ = stdin.write_all(input_json.as_bytes()).await;
    }
    let timeout = Duration::from_secs(tool.timeout_secs.clamp(1, MAX_TIMEOUT) as u64);
    let waited = tokio::time::timeout(timeout, child.wait_with_output()).await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    match waited {
        Ok(output) => {
            let output = output?;
            Ok(ScriptRunResult {
                ok: output.status.success(),
                exit_code: output.status.code(),
                stdout: truncate(output.stdout),
                stderr: truncate(output.stderr),
                duration_ms: started.elapsed().as_millis() as u64,
                timed_out: false,
            })
        }
        Err(_) => {
            if let Some(pid) = pid {
                kill_group(pid);
            }
            Ok(ScriptRunResult {
                ok: false,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("script excedeu {}s e foi encerrado", tool.timeout_secs),
                duration_ms: started.elapsed().as_millis() as u64,
                timed_out: true,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_slugs_and_runtimes() {
        assert!(valid_slug("echo_json"));
        assert!(valid_slug("a1-b"));
        assert!(!valid_slug("A"));
        assert!(valid_slug("um"));
        assert!(!valid_slug("u"));
        assert!(!valid_slug("../escape"));
        assert!(RUNTIMES.contains(&"bash"));
    }
}

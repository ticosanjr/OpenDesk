use super::*;
use crate::config_sync::{
    agents::{self, AgentWriteInput},
    content_installation::{ContentDraft, ContentInstallation},
    skills::{self, SkillExtraFile, SkillWriteInput},
};
use std::path::Path as FsPath;

#[derive(Debug, Deserialize)]
pub struct InstallRequest {
    pub source_type: String,
    pub value: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallItem {
    pub kind: String,
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub body: String,
    #[serde(default)]
    pub files: Vec<SkillExtraFile>,
    #[serde(default)]
    pub server: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct InstallPreview {
    pub items: Vec<InstallItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InstallApply {
    pub items: Vec<InstallItem>,
    #[serde(default)]
    pub target_scope: Option<String>,
    #[serde(default)]
    pub target_workspace_id: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

pub async fn install_preview(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<InstallRequest>,
) -> AppResult<Json<InstallPreview>> {
    let (items, warnings) = collect_source(&request).await?;
    Ok(Json(InstallPreview { items, warnings }))
}

pub async fn install_apply(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InstallApply>,
) -> AppResult<Json<Value>> {
    let target = target(
        &state,
        &TargetBody {
            target_scope: body.target_scope,
            target_workspace_id: body.target_workspace_id,
            new_id: None,
            overwrite: body.overwrite,
        },
        "workspace",
    )
    .await?;
    let installation =
        ContentInstallation::new(&target.paths, &target.scope, &state.paths.backups_dir);
    let mut written = Vec::new();
    for item in body.items {
        let kind = StudioKind::parse(&item.kind)?;
        let draft = ContentDraft {
            kind,
            id: item.id,
            name: item.name,
            description: item.description,
            body: item.body,
            files: item.files,
            server: item.server,
        };
        installation.install(&draft)?;
        written.push(json!({ "kind": kind.as_str(), "id": draft.id }));
    }
    Ok(Json(
        json!({ "ok": true, "scope": target.scope, "items": written }),
    ))
}

pub(super) fn dedupe_by<T, F>(items: Vec<T>, key: F) -> Vec<T>
where
    F: Fn(&T) -> String,
{
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}

#[derive(Debug, Deserialize)]
pub struct RuntimeImportBody {
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub target_scope: Option<String>,
    #[serde(default)]
    pub target_workspace_id: Option<String>,
}

pub async fn import_runtime(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RuntimeImportBody>,
) -> AppResult<Json<Value>> {
    let target = target(
        &state,
        &TargetBody {
            target_scope: body.target_scope,
            target_workspace_id: body.target_workspace_id,
            new_id: None,
            overwrite: false,
        },
        "workspace",
    )
    .await?;
    let client = state.daemon.client().await.ok_or_else(|| {
        AppError::Daemon("OpenCode runtime is offline; start the engine first".into())
    })?;
    match body.kind.as_str() {
        "skill" => {
            let skills = client.skills(None).await.unwrap_or_default();
            let skill = skills
                .into_iter()
                .find(|skill| skill.get("id").and_then(Value::as_str) == Some(body.id.as_str()))
                .ok_or_else(|| AppError::NotFound(format!("runtime skill {}", body.id)))?;
            let name = skill
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string);
            let description = skill
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string);
            let content = skill
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            skills::write(
                &target.paths,
                &target.scope,
                &body.id,
                SkillWriteInput {
                    name,
                    description,
                    body: content,
                    files: Vec::new(),
                    expected_sha256: None,
                },
                &state.paths.backups_dir,
            )?;
        }
        "agent" => {
            let agents = client.agents(None).await.unwrap_or_default();
            let agent = agents
                .into_iter()
                .find(|agent| agent.id == body.id)
                .ok_or_else(|| AppError::NotFound(format!("runtime agent {}", body.id)))?;
            agents::write(
                &target.paths,
                &target.scope,
                &body.id,
                AgentWriteInput {
                    name: agent.name,
                    avatar: None,
                    description: agent.description,
                    mode: agent.mode,
                    model: agent.model.map(|model| {
                        let mut value = format!("{}/{}", model.provider_id, model.id);
                        if let Some(variant) = model.variant {
                            value.push('#');
                            value.push_str(&variant);
                        }
                        value
                    }),
                    color: None,
                    router_policy: None,
                    hidden: agent.hidden,
                    steps: None,
                    disabled: false,
                    permissions: serde_json::from_value(agent.permissions).unwrap_or_default(),
                    system: String::new(),
                    expected_sha256: None,
                },
                &state.paths.backups_dir,
            )?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "runtime import supports skill and agent, not '{other}'"
            )));
        }
    }
    Ok(Json(json!({
        "ok": true,
        "kind": body.kind,
        "id": body.id,
        "scope": target.scope,
    })))
}

pub(super) async fn collect_source(
    request: &InstallRequest,
) -> AppResult<(Vec<InstallItem>, Vec<String>)> {
    let source = request.value.trim();
    if source.is_empty() {
        return Err(AppError::BadRequest("source value is required".into()));
    }
    let mut warnings = Vec::new();
    let mut items = Vec::new();

    match request.source_type.as_str() {
        "mcp_json" => {
            let server: Value = serde_json::from_str(source)
                .map_err(|error| AppError::BadRequest(format!("invalid MCP JSON: {error}")))?;
            items.push(InstallItem {
                kind: "mcp".into(),
                id: request.name.clone().unwrap_or_else(|| "mcp-server".into()),
                name: request.name.clone(),
                description: None,
                body: String::new(),
                files: Vec::new(),
                server: Some(server),
            });
        }
        "npm" => {
            let package = source.trim_start_matches("npm:");
            let id = request
                .name
                .clone()
                .unwrap_or_else(|| package.rsplit('/').next().unwrap_or("mcp").to_string());
            items.push(InstallItem {
                kind: "mcp".into(),
                id,
                name: Some(package.to_string()),
                description: Some(format!("MCP instalado a partir do pacote npm {package}")),
                body: String::new(),
                files: Vec::new(),
                server: Some(json!({
                    "type": "local",
                    "command": ["npx", "-y", package],
                    "disabled": false
                })),
            });
        }
        "url" | "github" => {
            let bytes = if request.source_type == "github" {
                download_github(source).await?
            } else {
                download_url(source).await?
            };
            if source.to_lowercase().ends_with(".zip") || request.source_type == "github" {
                collect_zip(&bytes, &mut items, &mut warnings)?;
            } else {
                let text = String::from_utf8_lossy(&bytes).to_string();
                collect_text(source, &text, request, &mut items, &mut warnings)?;
            }
        }
        "local" => {
            let path = PathBuf::from(source);
            if path.is_dir() {
                collect_local_dir(&path, &mut items, &mut warnings)?;
            } else if path.is_file() {
                let text = std::fs::read_to_string(&path)?;
                collect_text(
                    &path.to_string_lossy(),
                    &text,
                    request,
                    &mut items,
                    &mut warnings,
                )?;
            } else {
                return Err(AppError::BadRequest(format!(
                    "local path not found: {}",
                    path.display()
                )));
            }
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "invalid source_type '{other}'; expected github, url, local, mcp_json or npm"
            )));
        }
    }

    if items.is_empty() {
        return Err(AppError::BadRequest(
            "no agent, skill, command or MCP was detected in this source".into(),
        ));
    }
    Ok((items, warnings))
}

pub(super) async fn download_github(source: &str) -> AppResult<Vec<u8>> {
    let source = source.trim_start_matches("github:");
    let (repo_path, requested_ref) = match source.split_once('@') {
        Some((repo, reference)) => (repo, Some(reference.to_string())),
        None => (source, None),
    };
    let mut parts = repo_path.splitn(3, '/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts.next().unwrap_or_default();
    if owner.is_empty() || repo.is_empty() {
        return Err(AppError::BadRequest(
            "GitHub source must be owner/repo, owner/repo@ref or github:owner/repo".into(),
        ));
    }
    let token = github_token().await;
    let reference = match requested_ref {
        Some(reference) => reference,
        None => github_default_branch(owner, repo, token.as_deref()).await?,
    };

    let (url, authenticated) = match &token {
        Some(_) => (
            format!("https://api.github.com/repos/{owner}/{repo}/zipball/{reference}"),
            true,
        ),
        None => (
            format!("https://codeload.github.com/{owner}/{repo}/zip/refs/heads/{reference}"),
            false,
        ),
    };
    let bytes = download_url_auth(&url, token.as_deref(), authenticated).await.map_err(|_| {
        AppError::BadRequest(format!(
            "could not download GitHub repository {owner}/{repo}@{reference}; check the ref, visibility and token"
        ))
    })?;
    Ok(bytes)
}

pub(super) async fn github_token() -> Option<String> {
    for key in ["GITHUB_TOKEN", "GH_TOKEN", "OPENDESK_GITHUB_TOKEN"] {
        if let Ok(value) = std::env::var(key)
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_string());
        }
    }
    let output = tokio::process::Command::new("gh")
        .args(["auth", "token"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub(super) async fn github_default_branch(
    owner: &str,
    repo: &str,
    token: Option<&str>,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("OpenDesk-Studio/1")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut request = client
        .get(format!("https://api.github.com/repos/{owner}/{repo}"))
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|error| AppError::BadRequest(format!("GitHub API failed: {error}")))?;
    if !response.status().is_success() {
        // Public repositories with a rate limit still work through codeload
        // with the conventional main branch.
        return Ok("main".into());
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| AppError::BadRequest(format!("invalid GitHub response: {error}")))?;
    Ok(value
        .get("default_branch")
        .and_then(Value::as_str)
        .unwrap_or("main")
        .to_string())
}

pub(super) async fn download_url(url: &str) -> AppResult<Vec<u8>> {
    download_url_auth(url, None, false).await
}

pub(super) async fn download_url_auth(
    url: &str,
    token: Option<&str>,
    authenticated: bool,
) -> AppResult<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .user_agent("OpenDesk-Studio/1")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut request = client.get(url);
    if authenticated {
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request = request.header("Accept", "application/vnd.github+json");
    }
    let response = request
        .send()
        .await
        .map_err(|error| AppError::BadRequest(format!("download failed: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::BadRequest(format!(
            "download failed with {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::BadRequest(format!("download failed: {error}")))?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err(AppError::BadRequest("download exceeds 20 MB".into()));
    }
    Ok(bytes.to_vec())
}

pub(super) fn collect_zip(
    bytes: &[u8],
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec()))
        .map_err(|error| AppError::BadRequest(format!("invalid zip: {error}")))?;
    if archive.len() > 500 {
        return Err(AppError::BadRequest("zip contains too many files".into()));
    }
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| AppError::BadRequest(format!("invalid zip entry: {error}")))?;
        if file.is_dir() || file.size() > 2 * 1024 * 1024 {
            continue;
        }
        let name = file
            .name()
            .map_err(|error| AppError::BadRequest(format!("invalid zip entry name: {error}")))?
            .replace('\\', "/");
        if name.contains("..") || name.starts_with('/') {
            warnings.push(format!("ignored unsafe path {name}"));
            continue;
        }
        let mut content = String::new();
        if file.read_to_string(&mut content).is_err() {
            continue;
        }
        entries.push((name, content));
    }
    collect_entries(entries, items, warnings, None);
    Ok(())
}

pub(super) fn collect_local_dir(
    path: &FsPath,
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    let mut entries = Vec::new();
    for entry in walkdir::WalkDir::new(path).max_depth(8) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(path) else {
            continue;
        };
        let name = relative.to_string_lossy().replace('\\', "/");
        if !name.to_lowercase().ends_with(".md") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        entries.push((name, content));
    }
    collect_entries(entries, items, warnings, Some(path));
    Ok(())
}

pub(super) fn collect_entries(
    entries: Vec<(String, String)>,
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
    _root: Option<&FsPath>,
) {
    use std::collections::HashSet;
    let mut consumed: HashSet<usize> = HashSet::new();
    for (index, (path, content)) in entries.iter().enumerate() {
        if path.rsplit('/').next() != Some("SKILL.md") {
            continue;
        }
        let Some(directory) = path.rsplit_once('/').map(|(directory, _)| directory) else {
            continue;
        };
        let id = directory.rsplit('/').next().unwrap_or("skill").to_string();
        let mut files = Vec::new();
        for (other_index, (other_path, other_content)) in entries.iter().enumerate() {
            if other_index == index || !other_path.starts_with(&format!("{directory}/")) {
                continue;
            }
            consumed.insert(other_index);
            if let Some(relative) = other_path.strip_prefix(&format!("{directory}/")) {
                files.push(SkillExtraFile {
                    path: relative.to_string(),
                    content: other_content.clone(),
                });
            }
        }
        consumed.insert(index);
        items.push(InstallItem {
            kind: "skill".into(),
            id,
            name: None,
            description: None,
            body: content.clone(),
            files,
            server: None,
        });
    }
    for (index, (path, content)) in entries.into_iter().enumerate() {
        if consumed.contains(&index) {
            continue;
        }
        let request = InstallRequest {
            source_type: "import".into(),
            value: path.clone(),
            kind: None,
            name: None,
        };
        if let Err(error) = collect_text(&path, &content, &request, items, warnings) {
            warnings.push(format!("skipped {path}: {error}"));
        }
    }
}

pub(super) fn collect_text(
    path: &str,
    content: &str,
    request: &InstallRequest,
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    let lower = path.to_lowercase();
    let file_name = lower.rsplit('/').next().unwrap_or(&lower);
    let id_from_path = |suffix: &str| {
        path.rsplit('/')
            .find(|segment| !segment.is_empty())
            .map(|segment| segment.trim_end_matches(suffix).to_string())
            .unwrap_or_else(|| "item".into())
    };

    if file_name == "skill.md"
        || ((lower.contains("/skills/") || lower.starts_with("skills/")) && lower.ends_with(".md"))
    {
        let id = request
            .name
            .clone()
            .or_else(|| path.split('/').rev().nth(1).map(str::to_string))
            .unwrap_or_else(|| "skill".into());
        items.push(InstallItem {
            kind: "skill".into(),
            id,
            name: None,
            description: None,
            body: content.to_string(),
            files: Vec::new(),
            server: None,
        });
    } else if lower.contains("/agents/")
        || lower.starts_with("agents/")
        || request.kind.as_deref() == Some("agent")
    {
        items.push(InstallItem {
            kind: "agent".into(),
            id: request.name.clone().unwrap_or_else(|| id_from_path(".md")),
            name: None,
            description: None,
            body: content.to_string(),
            files: Vec::new(),
            server: None,
        });
    } else if lower.contains("/commands/")
        || lower.starts_with("commands/")
        || request.kind.as_deref() == Some("command")
    {
        items.push(InstallItem {
            kind: "command".into(),
            id: request.name.clone().unwrap_or_else(|| id_from_path(".md")),
            name: None,
            description: None,
            body: content.to_string(),
            files: Vec::new(),
            server: None,
        });
    } else if request.kind.as_deref() == Some("skill") {
        items.push(InstallItem {
            kind: "skill".into(),
            id: request.name.clone().unwrap_or_else(|| "skill".into()),
            name: None,
            description: None,
            body: content.to_string(),
            files: Vec::new(),
            server: None,
        });
    } else {
        warnings.push(format!("skipped unrecognised file {path}"));
    }
    Ok(())
}

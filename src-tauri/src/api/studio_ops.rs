use crate::{
    app_state::AppState,
    config_sync::{
        agents::{self, AgentWriteInput, PermissionRule},
        commands::{self, CommandWriteInput},
        jsonc_editor, markdown,
        paths::StudioPaths,
        skills::{self, SkillExtraFile, SkillWriteInput},
    },
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read},
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

use super::studio::{StudioQuery, context, require_id};

#[derive(Debug, Deserialize)]
pub struct TargetBody {
    #[serde(default)]
    pub target_scope: Option<String>,
    #[serde(default)]
    pub target_workspace_id: Option<String>,
    #[serde(default)]
    pub new_id: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

struct Target {
    paths: StudioPaths,
    scope: String,
}

async fn target(state: &AppState, body: &TargetBody, source_scope: &str) -> AppResult<Target> {
    let query = StudioQuery {
        scope: Some(
            body.target_scope
                .clone()
                .unwrap_or_else(|| source_scope.to_string()),
        ),
        workspace_id: body.target_workspace_id.clone(),
        id: None,
        expected_sha256: None,
        location: None,
    };
    let (paths, scope, _) = context(state, &query).await?;
    Ok(Target { paths, scope })
}

fn exists(paths: &StudioPaths, scope: &str, kind: &str, id: &str) -> AppResult<bool> {
    match kind {
        "agent" => Ok(agents::get(paths, scope, id).is_ok()),
        "skill" => Ok(skills::get(paths, scope, id).is_ok()),
        "command" => Ok(commands::get(paths, scope, id).is_ok()),
        "mcp" => {
            let document = jsonc_editor::read_for_scope(paths, scope)?;
            Ok(jsonc_editor::list_mcp_servers(&document)
                .iter()
                .any(|server| server.name == id))
        }
        _ => Err(AppError::BadRequest(format!(
            "invalid studio kind '{kind}'"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn copy_item(
    source: &StudioPaths,
    source_scope: &str,
    target: &StudioPaths,
    target_scope: &str,
    kind: &str,
    id: &str,
    new_id: &str,
    overwrite: bool,
    backups_dir: &FsPath,
) -> AppResult<()> {
    if !overwrite && exists(target, target_scope, kind, new_id)? {
        return Err(AppError::Conflict(format!(
            "{kind} '{new_id}' already exists in {target_scope}"
        )));
    }
    match kind {
        "agent" => {
            let agent = agents::get(source, source_scope, id)?;
            agents::write(
                target,
                target_scope,
                new_id,
                AgentWriteInput {
                    name: agent.name,
                    avatar: agent.avatar,
                    description: agent.description,
                    mode: agent.mode,
                    model: agent.model,
                    color: agent.color,
                    router_policy: agent.router_policy,
                    hidden: agent.hidden,
                    steps: agent.steps,
                    disabled: agent.disabled,
                    permissions: agent
                        .permissions
                        .into_iter()
                        .map(|permission| PermissionRule {
                            action: permission.action,
                            resource: permission.resource,
                            effect: permission.effect,
                        })
                        .collect(),
                    system: agent.system,
                    expected_sha256: None,
                },
                backups_dir,
            )?;
        }
        "skill" => {
            let skill = skills::get(source, source_scope, id)?;
            skills::write(
                target,
                target_scope,
                new_id,
                SkillWriteInput {
                    name: skill.name.clone().into(),
                    description: skill.description,
                    body: skill.body,
                    files: skill
                        .files
                        .into_iter()
                        .map(|file| SkillExtraFile {
                            path: file.path,
                            content: file.content,
                        })
                        .collect(),
                    expected_sha256: None,
                },
                backups_dir,
            )?;
        }
        "command" => {
            let command = commands::get(source, source_scope, id)?;
            commands::write(
                target,
                target_scope,
                new_id,
                CommandWriteInput {
                    description: command.description,
                    agent: command.agent,
                    model: command.model,
                    subagent: command.subagent,
                    template: command.template,
                    expected_sha256: None,
                },
                backups_dir,
            )?;
        }
        "mcp" => {
            let document = jsonc_editor::read_for_scope(source, source_scope)?;
            let server = jsonc_editor::list_mcp_servers(&document)
                .into_iter()
                .find(|server| server.name == id)
                .map(|server| server.server)
                .ok_or_else(|| AppError::NotFound(format!("mcp {id}")))?;
            let target_path = target.config_path(target_scope)?;
            jsonc_editor::upsert_mcp_server(&target_path, new_id, &server, None, backups_dir)?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "invalid studio kind '{other}'"
            )));
        }
    }
    Ok(())
}

pub async fn duplicate(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let mut target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{id}-copy"));
    let mut counter = 2;
    while exists(&paths, &scope, &kind, &target_id)? {
        target_id = format!("{id}-copy-{counter}");
        counter += 1;
        if counter > 100 {
            return Err(AppError::Conflict("could not find a free copy id".into()));
        }
    }
    copy_item(
        &paths,
        &scope,
        &paths,
        &scope,
        &kind,
        &id,
        &target_id,
        false,
        &state.paths.backups_dir,
    )?;
    Ok(Json(
        json!({ "ok": true, "kind": kind, "id": target_id, "scope": scope }),
    ))
}

pub async fn copy(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let target = target(&state, &body, &scope).await?;
    let target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    copy_item(
        &paths,
        &scope,
        &target.paths,
        &target.scope,
        &kind,
        &id,
        &target_id,
        body.overwrite,
        &state.paths.backups_dir,
    )?;
    Ok(Json(
        json!({ "ok": true, "kind": kind, "id": target_id, "scope": target.scope }),
    ))
}

pub async fn move_item(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let target = target(&state, &body, &scope).await?;
    let target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    if target.scope == scope && target_id == id {
        return Err(AppError::BadRequest(
            "source and target are the same item".into(),
        ));
    }
    copy_item(
        &paths,
        &scope,
        &target.paths,
        &target.scope,
        &kind,
        &id,
        &target_id,
        body.overwrite,
        &state.paths.backups_dir,
    )?;
    if let Err(error) = delete_item(&paths, &scope, &kind, &id, &state.paths.backups_dir) {
        let _ = delete_item(
            &target.paths,
            &target.scope,
            &kind,
            &target_id,
            &state.paths.backups_dir,
        );
        return Err(error);
    }
    Ok(Json(json!({
        "ok": true,
        "kind": kind,
        "id": target_id,
        "scope": target.scope,
    })))
}

fn delete_item(
    paths: &StudioPaths,
    scope: &str,
    kind: &str,
    id: &str,
    backups_dir: &FsPath,
) -> AppResult<()> {
    match kind {
        "agent" => agents::delete(paths, scope, id),
        "skill" => skills::delete(paths, scope, id),
        "command" => commands::delete(paths, scope, id),
        "mcp" => {
            let path = paths.config_path(scope)?;
            jsonc_editor::delete_mcp_server(&path, id, None, backups_dir)?;
            Ok(())
        }
        other => Err(AppError::BadRequest(format!(
            "invalid studio kind '{other}'"
        ))),
    }
}

fn item_path(paths: &StudioPaths, scope: &str, kind: &str, id: &str) -> AppResult<PathBuf> {
    match kind {
        "agent" => Ok(paths.agents_dir(scope)?.join(format!("{id}.md"))),
        "skill" => Ok(PathBuf::from(skills::get(paths, scope, id)?.path)),
        "command" => Ok(PathBuf::from(commands::get(paths, scope, id)?.path)),
        "mcp" => paths.config_path(scope),
        other => Err(AppError::BadRequest(format!(
            "invalid studio kind '{other}'"
        ))),
    }
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let path = item_path(&paths, &scope, &kind, &id)?;
    let entries = crate::config_sync::file_ops::list_backups_for(&path, &state.paths.backups_dir)?;
    Ok(Json(json!({ "path": path, "backups": entries })))
}

#[derive(Debug, Deserialize)]
pub struct RestoreBody {
    pub backup_id: String,
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<RestoreBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let path = item_path(&paths, &scope, &kind, &id)?;
    crate::config_sync::file_ops::restore_backup(&body.backup_id, &path, &state.paths.backups_dir)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove_item(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    delete_item(&paths, &scope, &kind, &id, &state.paths.backups_dir)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn toggle(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<ToggleBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let id = require_id(&StudioQuery {
        id: Some(id),
        ..query.clone()
    })?;
    match kind.as_str() {
        "agent" => {
            let mut agent = agents::get(&paths, &scope, &id)?;
            agent.disabled = !body.enabled;
            agents::write(
                &paths,
                &scope,
                &id,
                AgentWriteInput {
                    name: agent.name,
                    avatar: agent.avatar,
                    description: agent.description,
                    mode: agent.mode,
                    model: agent.model,
                    color: agent.color,
                    router_policy: agent.router_policy,
                    hidden: agent.hidden,
                    steps: agent.steps,
                    disabled: agent.disabled,
                    permissions: agent.permissions,
                    system: agent.system,
                    expected_sha256: None,
                },
                &state.paths.backups_dir,
            )?;
        }
        "skill" => {
            let skill = skills::get(&paths, &scope, &id)?;
            let skill_path = PathBuf::from(&skill.path);
            let is_folder =
                skill_path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md");
            let disabled_root = paths.opencode_dir(&scope)?.join("disabled/skills");
            let skills_root = paths.skills_dir(&scope)?;
            if is_folder {
                let source = skill_path
                    .parent()
                    .map(FsPath::to_path_buf)
                    .ok_or_else(|| AppError::Internal("skill path has no parent".into()))?;
                let target = if body.enabled {
                    skills_root.join(&id)
                } else {
                    disabled_root.join(&id)
                };
                move_dir(&source, &target)?;
            } else {
                let source = skill_path;
                let target = if body.enabled {
                    skills_root.join(format!("{id}.md"))
                } else {
                    disabled_root.join(format!("{id}.md"))
                };
                move_file(&source, &target)?;
            }
        }
        "command" => {
            let command = commands::get(&paths, &scope, &id)?;
            let source = PathBuf::from(&command.path);
            let disabled_root = paths.opencode_dir(&scope)?.join("disabled/commands");
            let target = if body.enabled {
                paths.commands_dir(&scope)?.join(format!("{id}.md"))
            } else {
                disabled_root.join(format!("{id}.md"))
            };
            move_file(&source, &target)?;
        }
        "mcp" => {
            let document = jsonc_editor::read_for_scope(&paths, &scope)?;
            let mut server = jsonc_editor::list_mcp_servers(&document)
                .into_iter()
                .find(|server| server.name == id)
                .map(|server| server.server)
                .ok_or_else(|| AppError::NotFound(format!("mcp {id}")))?;
            server["disabled"] = Value::Bool(!body.enabled);
            let path = paths.config_path(&scope)?;
            jsonc_editor::upsert_mcp_server(&path, &id, &server, None, &state.paths.backups_dir)?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "invalid studio kind '{other}'"
            )));
        }
    }
    Ok(Json(
        json!({ "ok": true, "kind": kind, "id": id, "enabled": body.enabled }),
    ))
}

fn move_dir(source: &FsPath, target: &FsPath) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::NotFound(format!("{}", source.display())));
    }
    if target.exists() {
        return Err(AppError::Conflict(format!(
            "{} already exists",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(source, target).is_err() {
        copy_dir(source, target)?;
        std::fs::remove_dir_all(source)?;
    }
    Ok(())
}

fn copy_dir(source: &FsPath, target: &FsPath) -> AppResult<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &destination)?;
        } else {
            std::fs::copy(&path, &destination)?;
        }
    }
    Ok(())
}

fn move_file(source: &FsPath, target: &FsPath) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::NotFound(format!("{}", source.display())));
    }
    if target.exists() {
        return Err(AppError::Conflict(format!(
            "{} already exists",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(source, target).is_err() {
        std::fs::copy(source, target)?;
        std::fs::remove_file(source)?;
    }
    Ok(())
}

fn insert_effective(
    items: &mut std::collections::BTreeMap<(String, String), (Value, usize)>,
    kind: &str,
    id: String,
    source_scope: &str,
    source_path: &str,
    value: Value,
    disabled: bool,
) {
    let key = (kind.to_string(), id.clone());
    let count = items.get(&key).map(|(_, count)| *count).unwrap_or(0);
    items.insert(
        key,
        (
            json!({
                "kind": kind,
                "id": id,
                "source_scope": source_scope,
                "source_path": source_path,
                "overridden": count > 0,
                "disabled": disabled,
                "value": value,
            }),
            count + 1,
        ),
    );
}

pub async fn effective(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id = match &query.workspace_id {
        Some(id) => id.clone(),
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    let global = StudioPaths::new(None);
    let workspace_root = PathBuf::from(&workspace.root_path);

    // OpenCode searches the current directory and every ancestor through the
    // filesystem root, merging direct `opencode.json(c)` and `.opencode`.
    // Studio mirrors that so a skill/command in a parent project is visible.
    let mut roots = Vec::new();
    let mut current = Some(workspace_root.as_path());
    while let Some(root) = current {
        roots.push(root.to_path_buf());
        if roots.len() >= 12 {
            break;
        }
        current = root.parent();
    }
    roots.reverse();

    let mut items: std::collections::BTreeMap<(String, String), (Value, usize)> =
        std::collections::BTreeMap::new();

    for kind in ["agent", "skill", "command"] {
        for item in list_kind(&global, "global", kind)? {
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let path = item
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let disabled = item
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            insert_effective(&mut items, kind, id, "global", &path, item, disabled);
        }
    }
    let global_doc = jsonc_editor::read_for_scope(&global, "global")?;
    for server in jsonc_editor::list_mcp_servers(&global_doc) {
        let disabled = server
            .server
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        insert_effective(
            &mut items,
            "mcp",
            server.name,
            "global",
            &global_doc.path,
            server.server,
            disabled,
        );
    }

    for root in roots {
        let is_workspace = root == workspace_root;
        let paths = StudioPaths {
            global_dir: global.global_dir.clone(),
            workspace_root: Some(root.clone()),
        };
        let scope = if is_workspace {
            "workspace"
        } else {
            "ancestor"
        };
        for kind in ["agent", "skill", "command"] {
            for item in list_kind(&paths, "workspace", kind)? {
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let path = item
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let disabled = item
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                insert_effective(&mut items, kind, id, scope, &path, item, disabled);
            }
        }
        let document = jsonc_editor::read_for_scope(&paths, "workspace")?;
        for server in jsonc_editor::list_mcp_servers(&document) {
            let disabled = server
                .server
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            insert_effective(
                &mut items,
                "mcp",
                server.name,
                scope,
                &document.path,
                server.server,
                disabled,
            );
        }
    }

    if let Some(client) = state.daemon.client().await {
        let runtime_root = query
            .location
            .clone()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root.clone());
        let directory = runtime_root.to_str();
        if let Ok(skills) = client.skills(directory).await {
            for skill in skills {
                let id = skill
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if id.is_empty() || items.contains_key(&("skill".into(), id.clone())) {
                    continue;
                }
                insert_effective(&mut items, "skill", id, "runtime", "", skill, false);
            }
        }
        if let Ok(commands) = client.commands(directory).await {
            for command in commands {
                let id = command
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if id.is_empty() || items.contains_key(&("command".into(), id.clone())) {
                    continue;
                }
                insert_effective(&mut items, "command", id, "runtime", "", command, false);
            }
        }
        if let Ok(agents) = client.agents(directory).await {
            for agent in agents {
                let id = agent.id.clone();
                if items.contains_key(&("agent".into(), id.clone())) {
                    continue;
                }
                insert_effective(
                    &mut items,
                    "agent",
                    id,
                    "runtime",
                    "",
                    serde_json::to_value(agent).unwrap_or(Value::Null),
                    false,
                );
            }
        }
        if let Ok(mcps) = client.mcp_servers(directory).await {
            for mcp in mcps {
                let id = mcp
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                if let Some((item, _)) = items.get_mut(&("mcp".into(), id.clone())) {
                    if let Some(value) = item.get_mut("value") {
                        value["runtime_status"] = mcp.get("status").cloned().unwrap_or(Value::Null);
                    }
                    continue;
                }
                insert_effective(&mut items, "mcp", id, "runtime", "", mcp, false);
            }
        }
    }

    Ok(Json(json!({
        "items": items.into_values().map(|(mut item, count)| {
            item["overridden"] = json!(count > 1);
            item
        }).collect::<Vec<_>>()
    })))
}

fn list_kind(paths: &StudioPaths, scope: &str, kind: &str) -> AppResult<Vec<Value>> {
    let values = match kind {
        "agent" => agents::list(paths, scope)?
            .into_iter()
            .map(|item| serde_json::to_value(item).unwrap_or(Value::Null))
            .collect(),
        "skill" => skills::list(paths, scope)?
            .into_iter()
            .map(|item| serde_json::to_value(item).unwrap_or(Value::Null))
            .collect(),
        "command" => commands::list(paths, scope)?
            .into_iter()
            .map(|item| serde_json::to_value(item).unwrap_or(Value::Null))
            .collect(),
        _ => Vec::new(),
    };
    Ok(values)
}

// Installation ------------------------------------------------------------

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
    let mut written = Vec::new();
    for item in body.items {
        if item.kind == "mcp" {
            let server = item.server.clone().ok_or_else(|| {
                AppError::BadRequest(format!("mcp {} has no server object", item.id))
            })?;
            let path = target.paths.config_path(&target.scope)?;
            jsonc_editor::upsert_mcp_server(
                &path,
                &item.id,
                &server,
                None,
                &state.paths.backups_dir,
            )?;
        } else if item.kind == "agent" {
            let (frontmatter, body) = parse_markdown(&item.body);
            agents::write(
                &target.paths,
                &target.scope,
                &item.id,
                AgentWriteInput {
                    name: markdown::get_string(&frontmatter, "name").or(item.name.clone()),
                    avatar: markdown::get_string(&frontmatter, "avatar"),
                    description: markdown::get_string(&frontmatter, "description")
                        .or(item.description.clone()),
                    mode: markdown::get_string(&frontmatter, "mode"),
                    model: markdown::get_string(&frontmatter, "model"),
                    color: markdown::get_string(&frontmatter, "color"),
                    router_policy: markdown::get_string(&frontmatter, "router_policy"),
                    hidden: markdown::get_bool(&frontmatter, "hidden").unwrap_or(false),
                    steps: markdown::get_u32(&frontmatter, "steps"),
                    disabled: markdown::get_bool(&frontmatter, "disabled").unwrap_or(false),
                    permissions: parse_permissions(&frontmatter)?,
                    system: body,
                    expected_sha256: None,
                },
                &state.paths.backups_dir,
            )?;
        } else if item.kind == "skill" {
            let (frontmatter, body) = parse_markdown(&item.body);
            skills::write(
                &target.paths,
                &target.scope,
                &item.id,
                SkillWriteInput {
                    name: markdown::get_string(&frontmatter, "name").or(item.name.clone()),
                    description: markdown::get_string(&frontmatter, "description")
                        .or(item.description.clone()),
                    body,
                    files: item.files.clone(),
                    expected_sha256: None,
                },
                &state.paths.backups_dir,
            )?;
        } else if item.kind == "command" {
            let (frontmatter, body) = parse_markdown(&item.body);
            commands::write(
                &target.paths,
                &target.scope,
                &item.id,
                CommandWriteInput {
                    description: markdown::get_string(&frontmatter, "description")
                        .or(item.description.clone()),
                    agent: markdown::get_string(&frontmatter, "agent"),
                    model: markdown::get_string(&frontmatter, "model"),
                    subagent: markdown::get_bool(&frontmatter, "subagent"),
                    template: body,
                    expected_sha256: None,
                },
                &state.paths.backups_dir,
            )?;
        } else {
            return Err(AppError::BadRequest(format!(
                "invalid install kind '{}'",
                item.kind
            )));
        }
        written.push(json!({ "kind": item.kind, "id": item.id }));
    }
    Ok(Json(
        json!({ "ok": true, "scope": target.scope, "items": written }),
    ))
}

fn dedupe_by<T, F>(items: Vec<T>, key: F) -> Vec<T>
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

pub async fn runtime(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let workspace_id = match query.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    let runtime_root = query
        .location
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| workspace.root_path.clone());
    let directory = Some(runtime_root.as_str());
    let client = state.daemon.client().await;
    let (agents, models, skills, commands, mcps) = match client {
        Some(client) => {
            let mut agents = client.agents(None).await.unwrap_or_default();
            let local_agents = client.agents(directory).await.unwrap_or_default();
            agents.extend(local_agents);
            let mut agents = dedupe_by(agents, |agent| agent.id.clone());

            let models = client
                .models(directory)
                .await
                .unwrap_or_else(|_| Vec::new());
            let mut skills = client.skills(None).await.unwrap_or_default();
            skills.extend(client.skills(directory).await.unwrap_or_default());
            let mut skills = dedupe_by(skills, |skill| {
                skill
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            let mut commands = client.commands(None).await.unwrap_or_default();
            commands.extend(client.commands(directory).await.unwrap_or_default());
            let mut commands = dedupe_by(commands, |command| {
                command
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            let mut mcps = client.mcp_servers(None).await.unwrap_or_default();
            mcps.extend(client.mcp_servers(directory).await.unwrap_or_default());
            let mut mcps = dedupe_by(mcps, |mcp| {
                mcp.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            skills.sort_by_key(|value| {
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            commands.sort_by_key(|value| {
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            mcps.sort_by_key(|value| {
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            });
            agents.sort_by_key(|agent| agent.id.clone());
            (agents, models, skills, commands, mcps)
        }
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
    };
    let status = state.daemon.status().await;
    Ok(Json(json!({
        "engine": status,
        "agents": agents,
        "models": models,
        "skills": skills,
        "commands": commands,
        "mcps": mcps,
    })))
}

pub async fn catalog() -> Json<Value> {
    Json(json!({
        "mcps": [
            {
                "id": "filesystem",
                "name": "Filesystem",
                "description": "Leitura e escrita de arquivos em um diretório permitido.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "."],
                    "disabled": false
                }
            },
            {
                "id": "git",
                "name": "Git",
                "description": "Operações de git em repositórios locais.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-git"],
                    "disabled": false
                }
            },
            {
                "id": "fetch",
                "name": "Fetch",
                "description": "Busca páginas HTTP e converte para markdown.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-fetch"],
                    "disabled": false
                }
            },
            {
                "id": "memory",
                "name": "Memory",
                "description": "Memória persistente em grafo para agentes.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-memory"],
                    "disabled": false
                }
            },
            {
                "id": "sequential-thinking",
                "name": "Sequential Thinking",
                "description": "Raciocínio passo a passo estruturado.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-sequential-thinking"],
                    "disabled": false
                }
            },
            {
                "id": "playwright",
                "name": "Playwright",
                "description": "Automação de navegador via Playwright MCP.",
                "server": {
                    "type": "local",
                    "command": ["npx", "-y", "@playwright/mcp@latest"],
                    "disabled": false
                }
            }
        ]
    }))
}

fn parse_markdown(content: &str) -> (serde_yaml::Mapping, String) {
    match markdown::parse(content) {
        Ok(document) => (document.frontmatter, document.body),
        Err(_) => (serde_yaml::Mapping::new(), content.to_string()),
    }
}

fn parse_permissions(mapping: &serde_yaml::Mapping) -> AppResult<Vec<PermissionRule>> {
    let Some(value) = mapping.get(serde_yaml::Value::String("permissions".into())) else {
        return Ok(Vec::new());
    };
    serde_yaml::from_value::<Vec<PermissionRule>>(value.clone()).map_err(|error| {
        AppError::BadRequest(format!("invalid permissions in imported agent: {error}"))
    })
}

async fn collect_source(request: &InstallRequest) -> AppResult<(Vec<InstallItem>, Vec<String>)> {
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

async fn download_github(source: &str) -> AppResult<Vec<u8>> {
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

async fn github_token() -> Option<String> {
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

async fn github_default_branch(owner: &str, repo: &str, token: Option<&str>) -> AppResult<String> {
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

async fn download_url(url: &str) -> AppResult<Vec<u8>> {
    download_url_auth(url, None, false).await
}

async fn download_url_auth(
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

fn collect_zip(
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

fn collect_local_dir(
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

fn collect_entries(
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

fn collect_text(
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn studio_paths(temp: &TempDir) -> (StudioPaths, StudioPaths) {
        let global = StudioPaths {
            global_dir: temp.path().join("global"),
            workspace_root: None,
        };
        let workspace = StudioPaths {
            global_dir: temp.path().join("global"),
            workspace_root: Some(temp.path().join("ws")),
        };
        (global, workspace)
    }

    fn agent_input() -> AgentWriteInput {
        AgentWriteInput {
            name: Some("Auditor".into()),
            avatar: Some("🛡️".into()),
            description: Some("Auditor".into()),
            mode: Some("subagent".into()),
            model: None,
            color: Some("#60a5fa".into()),
            router_policy: None,
            hidden: false,
            steps: None,
            disabled: false,
            permissions: Vec::new(),
            system: "Você audita.".into(),
            expected_sha256: None,
        }
    }

    #[test]
    fn copies_agents_and_commands_between_global_and_workspace() {
        let temp = TempDir::new().expect("temp");
        let (global, workspace) = studio_paths(&temp);
        let backups = temp.path().join("backups");
        agents::write(&global, "global", "auditor", agent_input(), &backups).expect("agent");
        copy_item(
            &global,
            "global",
            &workspace,
            "workspace",
            "agent",
            "auditor",
            "auditor-copy",
            false,
            &backups,
        )
        .expect("copy agent");
        assert_eq!(
            agents::get(&workspace, "workspace", "auditor-copy")
                .expect("copied agent")
                .name
                .as_deref(),
            Some("Auditor")
        );

        commands::write(
            &global,
            "global",
            "revisar",
            CommandWriteInput {
                description: Some("Revisar".into()),
                agent: Some("auditor".into()),
                model: None,
                subagent: None,
                template: "Revise $ARGUMENTS".into(),
                expected_sha256: None,
            },
            &backups,
        )
        .expect("command");
        copy_item(
            &global,
            "global",
            &workspace,
            "workspace",
            "command",
            "revisar",
            "revisar",
            false,
            &backups,
        )
        .expect("copy command");
        assert_eq!(
            commands::get(&workspace, "workspace", "revisar")
                .expect("copied command")
                .template,
            "Revise $ARGUMENTS"
        );
    }

    #[test]
    fn copies_skills_and_mcp_servers() {
        let temp = TempDir::new().expect("temp");
        let (global, workspace) = studio_paths(&temp);
        let backups = temp.path().join("backups");
        skills::write(
            &global,
            "global",
            "pesquisa",
            SkillWriteInput {
                name: Some("Pesquisa".into()),
                description: Some("Pesquisa".into()),
                body: "Use fontes confiáveis.".into(),
                files: vec![],
                expected_sha256: None,
            },
            &backups,
        )
        .expect("skill");
        copy_item(
            &global,
            "global",
            &workspace,
            "workspace",
            "skill",
            "pesquisa",
            "pesquisa",
            false,
            &backups,
        )
        .expect("copy skill");
        assert_eq!(
            skills::get(&workspace, "workspace", "pesquisa")
                .expect("copied skill")
                .name,
            "Pesquisa"
        );

        let global_config = global.config_path("global").expect("global config");
        jsonc_editor::upsert_mcp_server(
            &global_config,
            "fetch",
            &json!({"type": "local", "command": ["npx", "fetch"]}),
            None,
            &backups,
        )
        .expect("mcp");
        copy_item(
            &global,
            "global",
            &workspace,
            "workspace",
            "mcp",
            "fetch",
            "fetch",
            false,
            &backups,
        )
        .expect("copy mcp");
        let workspace_doc = jsonc_editor::read_for_scope(&workspace, "workspace").expect("config");
        assert!(
            jsonc_editor::list_mcp_servers(&workspace_doc)
                .iter()
                .any(|server| server.name == "fetch")
        );
    }

    #[test]
    fn move_file_refuses_to_overwrite() {
        let temp = TempDir::new().expect("temp");
        let source = temp.path().join("a.md");
        let target = temp.path().join("b.md");
        std::fs::write(&source, "a").expect("source");
        std::fs::write(&target, "b").expect("target");
        assert!(move_file(&source, &target).is_err());
        assert_eq!(std::fs::read_to_string(&source).expect("source"), "a");
    }
}

#[cfg(test)]
mod install_tests {
    use super::*;

    #[test]
    fn imported_agent_frontmatter_is_parsed() {
        let content = r#"---
name: Auditor
description: Audita
mode: subagent
model: deepseek/deepseek-flash
avatar: "🛡️"
permissions:
  - action: edit
    resource: "*"
    effect: deny
---
Você audita."#;
        let (frontmatter, body) = parse_markdown(content);
        assert_eq!(
            markdown::get_string(&frontmatter, "name").as_deref(),
            Some("Auditor")
        );
        assert_eq!(body.trim(), "Você audita.");
        let permissions = parse_permissions(&frontmatter).expect("permissions");
        assert_eq!(permissions.len(), 1);
        assert_eq!(permissions[0].action, "edit");
        assert_eq!(permissions[0].effect, "deny");
    }

    #[test]
    fn imported_command_uses_body_as_template() {
        let content = r#"---
description: Revisar
agent: auditor
---
Revise $ARGUMENTS"#;
        let (frontmatter, body) = parse_markdown(content);
        assert_eq!(body.trim(), "Revise $ARGUMENTS");
        assert_eq!(
            markdown::get_string(&frontmatter, "agent").as_deref(),
            Some("auditor")
        );
    }
}

#[cfg(test)]
mod zip_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn zip_import_groups_skill_assets() {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            writer
                .start_file("repo-main/skills/revisor/SKILL.md", options)
                .expect("skill");
            writer
                .write_all(b"---\nname: Revisor\ndescription: Revisa\n---\nUse.\n")
                .expect("write");
            writer
                .start_file("repo-main/skills/revisor/reference.md", options)
                .expect("ref");
            writer.write_all(b"referencia").expect("write ref");
            writer.finish().expect("finish");
        }
        let mut items = Vec::new();
        let mut warnings = Vec::new();
        collect_zip(&buffer.into_inner(), &mut items, &mut warnings).expect("zip");
        let skill = items
            .iter()
            .find(|item| item.kind == "skill")
            .expect("skill item");
        assert_eq!(skill.id, "revisor");
        assert_eq!(skill.files.len(), 1);
        assert_eq!(skill.files[0].path, "reference.md");
        assert_eq!(skill.files[0].content, "referencia");
    }
}

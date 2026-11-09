use crate::{
    app_state::AppState,
    config_sync::{
        paths::StudioPaths,
        studio_item::{ItemSnapshot, StudioItem, StudioKind},
    },
    error::{AppError, AppResult},
};
#[path = "studio_ops_install.rs"]
mod install;
pub use install::*;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read},
    path::PathBuf,
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

pub async fn duplicate(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    let source = StudioItem::new(&paths, &scope, kind, &id);
    let mut target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{id}-copy"));
    let mut counter = 2;
    while StudioItem::new(&paths, &scope, kind, &target_id).exists()? {
        target_id = format!("{id}-copy-{counter}");
        counter += 1;
        if counter > 100 {
            return Err(AppError::Conflict("could not find a free copy id".into()));
        }
    }
    let target = StudioItem::new(&paths, &scope, kind, &target_id);
    source.copy_to(&target, false, &state.paths.backups_dir)?;
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": target_id,
        "scope": scope
    })))
}

pub async fn copy(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let target = target(&state, &body, &scope).await?;
    let kind = StudioKind::parse(&kind)?;
    let target_id = body
        .new_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let source = StudioItem::new(&paths, &scope, kind, &id);
    let destination = StudioItem::new(&target.paths, &target.scope, kind, &target_id);
    source.copy_to(&destination, body.overwrite, &state.paths.backups_dir)?;
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": target_id,
        "scope": target.scope
    })))
}

pub async fn move_item(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
    Json(body): Json<TargetBody>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let target = target(&state, &body, &scope).await?;
    let kind = StudioKind::parse(&kind)?;
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
    let source = StudioItem::new(&paths, &scope, kind, &id);
    let destination = StudioItem::new(&target.paths, &target.scope, kind, &target_id);
    source.copy_to(&destination, body.overwrite, &state.paths.backups_dir)?;
    if let Err(error) = source.delete(&state.paths.backups_dir) {
        let _ = destination.delete(&state.paths.backups_dir);
        return Err(error);
    }
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": target_id,
        "scope": target.scope,
    })))
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    let path = StudioItem::new(&paths, &scope, kind, &id).path()?;
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
    let kind = StudioKind::parse(&kind)?;
    let path = StudioItem::new(&paths, &scope, kind, &id).path()?;
    crate::config_sync::file_ops::restore_backup(&body.backup_id, &path, &state.paths.backups_dir)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove_item(
    State(state): State<Arc<AppState>>,
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<StudioQuery>,
) -> AppResult<Json<Value>> {
    let (paths, scope, _) = context(&state, &query).await?;
    let kind = StudioKind::parse(&kind)?;
    StudioItem::new(&paths, &scope, kind, &id).delete(&state.paths.backups_dir)?;
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
    let kind = StudioKind::parse(&kind)?;
    StudioItem::new(&paths, &scope, kind, &id)
        .set_enabled(body.enabled, &state.paths.backups_dir)?;
    Ok(Json(json!({
        "ok": true,
        "kind": kind.as_str(),
        "id": id,
        "enabled": body.enabled
    })))
}

fn insert_effective(
    items: &mut std::collections::BTreeMap<(String, String), (Value, usize)>,
    kind: StudioKind,
    source_scope: &str,
    item: ItemSnapshot,
) {
    let key = (kind.as_str().to_string(), item.id.clone());
    let count = items.get(&key).map(|(_, count)| *count).unwrap_or(0);
    items.insert(
        key,
        (
            json!({
                "kind": kind.as_str(),
                "id": item.id,
                "source_scope": source_scope,
                "source_path": item.path,
                "overridden": count > 0,
                "disabled": item.disabled,
                "value": item.value,
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

    for kind in StudioKind::ALL {
        for item in StudioItem::list(&global, "global", kind)? {
            insert_effective(&mut items, kind, "global", item);
        }
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
        if !paths.has_studio_files("workspace") {
            continue;
        }
        for kind in StudioKind::ALL {
            for item in StudioItem::list(&paths, "workspace", kind)? {
                insert_effective(&mut items, kind, scope, item);
            }
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
                insert_effective(
                    &mut items,
                    StudioKind::Skill,
                    "runtime",
                    ItemSnapshot {
                        id,
                        path: String::new(),
                        disabled: false,
                        value: skill,
                    },
                );
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
                insert_effective(
                    &mut items,
                    StudioKind::Command,
                    "runtime",
                    ItemSnapshot {
                        id,
                        path: String::new(),
                        disabled: false,
                        value: command,
                    },
                );
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
                    StudioKind::Agent,
                    "runtime",
                    ItemSnapshot {
                        id,
                        path: String::new(),
                        disabled: false,
                        value: serde_json::to_value(agent).unwrap_or(Value::Null),
                    },
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
                insert_effective(
                    &mut items,
                    StudioKind::Mcp,
                    "runtime",
                    ItemSnapshot {
                        id,
                        path: String::new(),
                        disabled: false,
                        value: mcp,
                    },
                );
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

// Installation ------------------------------------------------------------

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
        "repos": [
            {
                "id": "anthropic-skills",
                "name": "Anthropic Skills",
                "description": "Coleção oficial de skills de exemplo da Anthropic.",
                "source_type": "github",
                "source": "anthropics/skills",
                "kind": "skill"
            },
            {
                "id": "superpowers",
                "name": "Superpowers",
                "description": "Workflows e skills de engenharia para agentes.",
                "source_type": "github",
                "source": "obra/superpowers",
                "kind": "skill"
            },
            {
                "id": "playwright-mcp",
                "name": "Playwright MCP",
                "description": "Automação de navegador via MCP (Microsoft).",
                "source_type": "npm",
                "source": "@playwright/mcp",
                "kind": "mcp"
            },
            {
                "id": "context7",
                "name": "Context7 MCP",
                "description": "Documentação atualizada de bibliotecas via MCP.",
                "source_type": "npm",
                "source": "@upstash/context7-mcp",
                "kind": "mcp"
            },
            {
                "id": "mcp-servers",
                "name": "MCP Servers",
                "description": "Servidores MCP de referência da comunidade.",
                "source_type": "github",
                "source": "modelcontextprotocol/servers",
                "kind": "mcp"
            }
        ],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_sync::{
        agents::{self, AgentWriteInput},
        commands::{self, CommandWriteInput},
        jsonc_editor,
        skills::{self, SkillWriteInput},
    };
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
        let source = StudioItem::new(&global, "global", StudioKind::Agent, "auditor");
        let target = StudioItem::new(&workspace, "workspace", StudioKind::Agent, "auditor-copy");
        source
            .copy_to(&target, false, &backups)
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
        let source = StudioItem::new(&global, "global", StudioKind::Command, "revisar");
        let target = StudioItem::new(&workspace, "workspace", StudioKind::Command, "revisar");
        source
            .copy_to(&target, false, &backups)
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
        let source = StudioItem::new(&global, "global", StudioKind::Skill, "pesquisa");
        let target = StudioItem::new(&workspace, "workspace", StudioKind::Skill, "pesquisa");
        source
            .copy_to(&target, false, &backups)
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
        let source = StudioItem::new(&global, "global", StudioKind::Mcp, "fetch");
        let target = StudioItem::new(&workspace, "workspace", StudioKind::Mcp, "fetch");
        source.copy_to(&target, false, &backups).expect("copy mcp");
        let workspace_doc = jsonc_editor::read_for_scope(&workspace, "workspace").expect("config");
        assert!(
            jsonc_editor::list_mcp_servers(&workspace_doc)
                .iter()
                .any(|server| server.name == "fetch")
        );
    }

    #[test]
    fn refusing_to_overwrite_a_copy_keeps_the_target() {
        let temp = TempDir::new().expect("temp");
        let (global, workspace) = studio_paths(&temp);
        let backups = temp.path().join("backups");
        agents::write(&global, "global", "auditor", agent_input(), &backups).expect("agent");
        agents::write(&workspace, "workspace", "auditor", agent_input(), &backups).expect("target");
        let source = StudioItem::new(&global, "global", StudioKind::Agent, "auditor");
        let target = StudioItem::new(&workspace, "workspace", StudioKind::Agent, "auditor");
        assert!(source.copy_to(&target, false, &backups).is_err());
    }
}

#[cfg(test)]
mod install_tests {
    use crate::config_sync::{
        content_installation::{parse_markdown, parse_permissions},
        markdown,
    };

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

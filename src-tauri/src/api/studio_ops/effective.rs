use super::*;

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

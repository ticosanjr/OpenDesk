use super::*;

#[tokio::test]
async fn starter_pack_api_previews_installs_and_removes() {
    let ctx = test_support::context().await;
    let (status, catalog) = call(&ctx, Method::GET, "/api/starter-packs", None).await;
    assert_eq!(status, StatusCode::OK);
    let packs = catalog["packs"].as_array().expect("packs");
    assert!(packs.len() >= 20);
    assert!(packs.iter().all(|pack| pack["installed"] == false));

    let (status, preview) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/preview",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!preview["files"].as_array().unwrap().is_empty());

    let (status, installed) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/install",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(installed["written"].as_u64().unwrap() > 0);
    assert!(
        ctx.workspace_root
            .join(".opencode/agents/assistente.md")
            .is_file()
    );

    let (status, detail) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/starter-packs/essenciais?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["installed"], true);
    assert_eq!(detail["health"]["status"], "installed");

    let (status, removed) = call(
        &ctx,
        Method::DELETE,
        &format!(
            "/api/starter-packs/essenciais?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(removed["removed"].as_u64().unwrap() > 0);
    assert!(
        !ctx.workspace_root
            .join(".opencode/agents/assistente.md")
            .exists()
    );
}

#[tokio::test]
async fn starter_pack_conflict_requires_explicit_overwrite() {
    let ctx = test_support::context().await;
    let agent = ctx.workspace_root.join(".opencode/agents/assistente.md");
    std::fs::create_dir_all(agent.parent().unwrap()).expect("agents dir");
    std::fs::write(&agent, "conteúdo do usuário").expect("custom agent");

    let (_, preview) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/preview",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(preview["conflicts"], 1);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/install",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/install",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "overwrite": [".opencode/agents/assistente.md"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        std::fs::read_to_string(&agent)
            .expect("agent")
            .contains("Assistente")
    );
}

#[tokio::test]
async fn workspace_template_creates_workspace_and_installs_packs() {
    let ctx = test_support::context().await;
    let root = ctx.temp.path().join("template-workspace");
    let (status, result) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/templates/create",
        Some(json!({
            "template_id": "conteudo-midia",
            "name": "Conteúdo API",
            "root_path": root.to_string_lossy(),
            "color": "#8b5cf6"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["errors"].as_array().unwrap().len(), 0);
    assert_eq!(result["installed"].as_array().unwrap().len(), 6);
    assert!(
        root.join(".opencode/agents/roteirista-youtube.md")
            .is_file()
    );
    assert!(root.join("README.md").is_file());

    let (status, workspaces) = call(&ctx, Method::GET, "/api/workspaces", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        workspaces
            .as_array()
            .unwrap()
            .iter()
            .any(|workspace| workspace["name"] == "Conteúdo API")
    );
}

#[tokio::test]
async fn studio_creates_agent_skill_and_command() {
    let ctx = test_support::context().await;
    let auth_query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);

    let (status, agent) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/agent?id=auditor&{auth_query}"),
        Some(json!({
            "name": "Auditor",
            "description": "Revisa qualidade",
            "mode": "subagent",
            "system": "Você revisa."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(agent["name"], "Auditor");

    let (status, skill) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/skill?id=revisao&{auth_query}"),
        Some(json!({
            "name": "Revisão",
            "description": "Revisa entregáveis com checklist",
            "body": "## Processo\n1. Leia.\n2. Verifique.",
            "files": [{ "path": "references/checklist.md", "content": "- item" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(skill["files"].as_array().unwrap().len(), 1);

    let (status, command) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/command?id=revisar&{auth_query}"),
        Some(json!({
            "description": "Revisa um texto",
            "agent": "auditor",
            "template": "Revise: $ARGUMENTS"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(command["template"].as_str().unwrap().contains("$ARGUMENTS"));
}

#[tokio::test]
async fn studio_ops_manage_assets_duplicates_and_backups() {
    let ctx = test_support::context().await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);
    for (id, kind) in [
        ("qa-agent", "agent"),
        ("qa-skill", "skill"),
        ("qa-command", "command"),
    ] {
        let uri = match kind {
            "agent" => format!("/api/studio/agent?id={id}&{query}"),
            "skill" => format!("/api/studio/skill?id={id}&{query}"),
            _ => format!("/api/studio/command?id={id}&{query}"),
        };
        let body = match kind {
            "agent" => json!({
                "name": "QA Agent",
                "description": "Audita entregas",
                "mode": "subagent",
                "system": "Você audita."
            }),
            "skill" => json!({
                "name": "QA Skill",
                "description": "Checklist de QA",
                "body": "## Processo\n1. Verifique."
            }),
            _ => json!({
                "description": "Executa QA",
                "agent": "qa-agent",
                "template": "Revise $ARGUMENTS"
            }),
        };
        let (status, _) = call(&ctx, Method::PUT, &uri, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{kind} write");
    }

    for uri in [
        format!("/api/studio/effective?{query}"),
        "/api/studio/catalog".to_string(),
        "/api/studio/runtime".to_string(),
    ] {
        let (status, body) = call(&ctx, Method::GET, &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(!body.is_null(), "{uri}");
    }

    let (status, duplicated) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/agent/qa-agent/duplicate?{query}"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let copy_id = duplicated["id"].as_str().expect("copy id").to_string();

    let (status, _toggled) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/agent/{copy_id}/toggle?{query}"),
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, backups) = call(
        &ctx,
        Method::GET,
        &format!("/api/studio/agent/qa-agent/backups?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let backup_id = backups["backups"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("id").or_else(|| item.get("backup_id")))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(backup_id) = backup_id {
        let (status, _) = call(
            &ctx,
            Method::POST,
            &format!("/api/studio/agent/qa-agent/restore?{query}"),
            Some(json!({ "backup_id": backup_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, copied) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/skill/qa-skill/copy?{query}"),
        Some(json!({ "new_id": "qa-skill-copy" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{copied}");
    assert_eq!(copied["scope"], "workspace");
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/studio/skill/qa-skill-copy?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/studio/agent/{copy_id}?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn studio_crud_lists_gets_deletes_and_saves_config() {
    let ctx = test_support::context().await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);

    for (kind, body) in [
        (
            "agent",
            json!({ "name": "CRUD Agent", "mode": "subagent", "system": "Ok." }),
        ),
        (
            "skill",
            json!({ "name": "CRUD Skill", "body": "## Processo", "files": [] }),
        ),
        (
            "command",
            json!({ "description": "CRUD", "template": "Faça $ARGUMENTS" }),
        ),
    ] {
        let singular = kind;
        let (status, _) = call(
            &ctx,
            Method::PUT,
            &format!("/api/studio/{singular}?id=crud-{kind}&{query}"),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{kind} write");

        let (status, listed) = call(
            &ctx,
            Method::GET,
            &format!("/api/studio/{kind}s?{query}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{kind} list");
        assert!(listed.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["id"] == format!("crud-{kind}"))
        }));

        let (status, fetched) = call(
            &ctx,
            Method::GET,
            &format!("/api/studio/{singular}?id=crud-{kind}&{query}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{kind} get");
        assert_eq!(fetched["id"], format!("crud-{kind}"));
    }

    let (status, _) = call(
        &ctx,
        Method::GET,
        &format!("/api/studio/agent?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _config) = call(
        &ctx,
        Method::GET,
        &format!("/api/studio/config?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, saved) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/config?{query}"),
        Some(json!({ "content": "{\n  \"mcp\": {}\n}\n" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert!(
        saved["content"]
            .as_str()
            .is_some_and(|value| value.contains("mcp"))
    );

    for kind in ["agent", "skill", "command"] {
        let (status, _) = call(
            &ctx,
            Method::DELETE,
            &format!("/api/studio/{kind}?id=crud-{kind}&{query}"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{kind} delete");
    }
}

#[tokio::test]
async fn studio_moves_items_and_toggles_mcp_servers() {
    let ctx = test_support::context().await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);

    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/agent?id=move-me&{query}"),
        Some(json!({
            "name": "Move Me",
            "mode": "subagent",
            "system": "Você se move."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, moved) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/agent/move-me/move?{query}"),
        Some(json!({ "new_id": "moved" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["id"], "moved");

    let (_, effective) = call(
        &ctx,
        Method::GET,
        &format!("/api/studio/effective?{query}"),
        None,
    )
    .await;
    let agents = effective["items"].as_array().expect("items");
    assert!(agents.iter().any(|item| item["id"] == "moved"));
    assert!(!agents.iter().any(|item| item["id"] == "move-me"));

    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/mcp/fetch?{query}"),
        Some(json!({
            "server": { "type": "local", "command": ["npx", "fetch"] }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/mcp/fetch/toggle?{query}"),
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, mcp) = call(&ctx, Method::GET, &format!("/api/studio/mcp?{query}"), None).await;
    let servers = mcp["servers"].as_array().expect("servers");
    let fetch = servers
        .iter()
        .find(|server| server["name"] == "fetch")
        .expect("fetch server");
    assert_eq!(fetch["server"]["disabled"], true);
}

#[tokio::test]
async fn studio_install_preview_and_apply_write_all_asset_kinds() {
    let ctx = test_support::context().await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);
    let (status, preview) = call(
        &ctx,
        Method::POST,
        "/api/studio/install/preview",
        Some(json!({
            "source_type": "mcp_json",
            "value": "{\"type\":\"local\",\"command\":[\"echo\"]}",
            "name": "qa-mcp"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["items"][0]["kind"], "mcp");

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/studio/install/preview",
        Some(json!({ "source_type": "mcp_json", "value": "not-json" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, applied) = call(
        &ctx,
        Method::POST,
        "/api/studio/install/apply",
        Some(json!({
            "target_scope": "workspace",
            "target_workspace_id": ctx.workspace_id,
            "items": [
                {
                    "kind": "agent",
                    "id": "install-agent",
                    "body": "---\nname: Install Agent\ndescription: Instalado\nmode: subagent\n---\nVocê instala."
                },
                {
                    "kind": "skill",
                    "id": "install-skill",
                    "body": "---\nname: Install Skill\ndescription: Skill instalada\n---\n## Processo\n1. Faça."
                },
                {
                    "kind": "command",
                    "id": "install-command",
                    "body": "---\ndescription: Comando instalado\n---\nExecute $ARGUMENTS"
                },
                {
                    "kind": "mcp",
                    "id": "qa-mcp",
                    "body": "",
                    "server": { "type": "local", "command": ["echo"] }
                }
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert_eq!(applied["items"].as_array().expect("items").len(), 4);

    let (status, agents) = call(
        &ctx,
        Method::GET,
        &format!("/api/studio/agents?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        agents
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == "install-agent"))
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/studio/install/apply",
        Some(json!({ "items": [{ "kind": "unknown", "id": "x", "body": "" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn studio_mcp_config_can_be_written_listed_and_deleted() {
    let ctx = test_support::context().await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);
    let (status, document) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/mcp/qa-mcp?{query}"),
        Some(json!({
            "server": { "type": "local", "command": ["echo"], "disabled": false }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{document}");

    let (status, list) = call(&ctx, Method::GET, &format!("/api/studio/mcp?{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list["servers"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "qa-mcp"))
    );

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/studio/mcp/qa-mcp?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

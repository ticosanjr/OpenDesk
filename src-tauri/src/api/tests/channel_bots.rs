use super::*;

fn token(suffix: &str) -> String {
    format!("123456:abcdefghijklmnopqrstuvwxyz{suffix}")
}

async fn create_bot(ctx: &TestContext, workspace_id: &str, name: &str, token: &str) -> Value {
    let (status, value) = call(
        ctx,
        Method::POST,
        "/api/channels/bots",
        Some(json!({
            "workspace_id": workspace_id,
            "channel": "telegram",
            "name": name,
            "enabled": false,
            "token": token,
            "allowlist": ["123456", "@rafa"],
            "default_agent": "build"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}

#[tokio::test]
async fn workspaces_keep_independent_bots_and_never_expose_tokens() {
    let ctx = test_support::context().await;
    let second_root = ctx.temp.path().join("workspace-2");
    std::fs::create_dir_all(&second_root).expect("workspace");
    sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-2', 'Dois', ?)")
        .bind(second_root.to_string_lossy().to_string())
        .execute(&ctx.state.db)
        .await
        .expect("workspace");

    let first = create_bot(&ctx, &ctx.workspace_id, "Primeiro", &token("a")).await;
    let second = create_bot(&ctx, "ws-2", "Segundo", &token("b")).await;
    assert_eq!(first["workspace_id"], ctx.workspace_id);
    assert_eq!(second["workspace_id"], "ws-2");
    assert_eq!(first["token_configured"], true);
    assert!(first.get("token").is_none());
    assert!(first.get("secret_ref").is_none());

    let (status, value) = call(
        &ctx,
        Method::GET,
        &format!("/api/channels/bots?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let bots = value.as_array().expect("array");
    assert_eq!(bots.len(), 1);
    assert_eq!(bots[0]["name"], "Primeiro");

    let stored = ctx
        .state
        .secrets
        .get("channel_bot", first["id"].as_str().expect("id"))
        .expect("secret")
        .expect("stored");
    assert_eq!(stored, token("a"));

    let row: String = sqlx::query_scalar("SELECT printf('%s|%s|%s', name, config_json, allowlist_json) FROM channel_bots WHERE id = ?")
        .bind(first["id"].as_str().expect("id"))
        .fetch_one(&ctx.state.db)
        .await
        .expect("row");
    assert!(!row.contains(&token("a")));

    let (status, value) = call(
        &ctx,
        Method::POST,
        "/api/channels/bots",
        Some(json!({
            "workspace_id": "ws-2",
            "channel": "telegram",
            "name": "Duplicado",
            "enabled": false,
            "token": token("a"),
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
}

#[tokio::test]
async fn chat_contexts_are_isolated_by_external_chat() {
    let ctx = test_support::context().await;
    let bot = create_bot(&ctx, &ctx.workspace_id, "Grupos", &token("c")).await;
    let bot_id = bot["id"].as_str().expect("id");
    let base = format!("/api/channels/bots/{bot_id}/contexts");

    let (status, work) = call(
        &ctx,
        Method::POST,
        &base,
        Some(json!({
            "external_chat_id": "-1001",
            "chat_type": "group",
            "title": "Trabalho",
            "system_context": "Responda como o time de engenharia.",
            "memory_scope": "chat"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{work}");

    let (status, hobbies) = call(
        &ctx,
        Method::POST,
        &base,
        Some(json!({
            "external_chat_id": "-1002",
            "chat_type": "group",
            "title": "Hobbies",
            "system_context": "Responda de forma leve.",
            "memory_scope": "none"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{hobbies}");
    assert_ne!(work["id"], hobbies["id"]);

    let (status, _) = call(
        &ctx,
        Method::POST,
        &base,
        Some(json!({ "external_chat_id": "-1001", "title": "Repetido" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, value) = call(&ctx, Method::GET, &base, None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let contexts = value.as_array().expect("array");
    assert_eq!(contexts.len(), 2);

    let work_id = work["id"].as_str().expect("work id");
    let (status, updated) = call(
        &ctx,
        Method::PUT,
        &format!("{base}/{work_id}"),
        Some(json!({
            "title": "Trabalho (editado)",
            "system_context": "Contexto editado.",
            "agent_override": "review",
            "memory_scope": "both"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["agent_override"], "review");
    assert_eq!(updated["memory_scope"], "both");

    let (status, value) = call(&ctx, Method::GET, &base, None).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let hobbies_after = value
        .as_array()
        .expect("array")
        .iter()
        .find(|context| context["external_chat_id"] == "-1002")
        .expect("hobbies");
    assert_eq!(hobbies_after["system_context"], "Responda de forma leve.");
    assert!(hobbies_after["agent_override"].is_null());

    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!("{base}/{work_id}"),
        Some(json!({ "memory_scope": "invalido" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_failing_bot_only_changes_its_own_status() {
    let ctx = test_support::context().await;
    let first = create_bot(&ctx, &ctx.workspace_id, "Sem token", &token("d")).await;
    let second = create_bot(&ctx, &ctx.workspace_id, "Parado", &token("e")).await;
    let first_id = first["id"].as_str().expect("id");
    let second_id = second["id"].as_str().expect("id");

    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!("/api/channels/bots/{first_id}"),
        Some(json!({
            "name": "Sem token",
            "enabled": false,
            "clear_token": true,
            "allowlist": ["123456"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, value) = call(
        &ctx,
        Method::POST,
        &format!("/api/channels/bots/{first_id}/start"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");

    let (_, value) = call(
        &ctx,
        Method::GET,
        &format!("/api/channels/bots?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    let bots = value.as_array().expect("array");
    let first_after = bots
        .iter()
        .find(|bot| bot["id"] == first_id)
        .expect("first");
    let second_after = bots
        .iter()
        .find(|bot| bot["id"] == second_id)
        .expect("second");
    assert_eq!(first_after["status"], "error");
    assert!(first_after["last_error"].as_str().is_some());
    assert_eq!(second_after["status"], "stopped");
    assert_eq!(second_after["token_configured"], true);
}

#[tokio::test]
async fn two_groups_get_isolated_threads_and_sessions() {
    use crate::integrations::telegram::TelegramConfig;
    use opendesk_channels::telegram::{IncomingTelegram, TelegramUpdate};

    let ctx = test_support::context().await;
    let bot = create_bot(&ctx, &ctx.workspace_id, "Isolado", &token("f")).await;
    let bot_id = bot["id"].as_str().expect("id").to_string();
    let base = format!("/api/channels/bots/{bot_id}/contexts");
    let (status, _) = call(
        &ctx,
        Method::POST,
        &base,
        Some(json!({
            "external_chat_id": "-1001",
            "title": "Trabalho",
            "system_context": "Contexto de trabalho",
            "opencode_session_id": "ses-trabalho"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::POST,
        &base,
        Some(json!({
            "external_chat_id": "-1002",
            "title": "Hobbies",
            "system_context": "Contexto de hobbies",
            "opencode_session_id": "ses-hobbies"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let bot = crate::db::channel_bots::get_bot(&ctx.state.db, &bot_id)
        .await
        .expect("bot");
    let config = TelegramConfig {
        enabled: true,
        allowlist: vec!["-1001".into(), "-1002".into()],
        workspace_id: Some(ctx.workspace_id.clone()),
        agent: "build".into(),
    };
    for (chat_id, text) in [("-1001", "Oi trabalho"), ("-1002", "Oi hobbies")] {
        crate::integrations::telegram::handle_bot_update(
            &ctx.state,
            &bot,
            &config,
            "123456:abcdefghijklmnopqrstuvwxyzf",
            TelegramUpdate::Message(IncomingTelegram {
                update_id: 1,
                message_id: 1,
                chat_id: chat_id.into(),
                username: Some("rafa".into()),
                chat_title: Some(chat_id.into()),
                text: text.into(),
                file: None,
            }),
        )
        .await
        .expect("update");
    }

    let threads: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT external_id, opencode_session_id FROM conversations
         WHERE channel = 'chat' AND source = 'telegram'
           AND external_id IN (?, ?) ORDER BY external_id",
    )
    .bind(format!("telegram:{bot_id}:-1001"))
    .bind(format!("telegram:{bot_id}:-1002"))
    .fetch_all(&ctx.state.db)
    .await
    .expect("threads");
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0].0, format!("telegram:{bot_id}:-1001"));
    assert_eq!(threads[1].0, format!("telegram:{bot_id}:-1002"));
    assert_eq!(threads[0].1.as_deref(), Some("ses-trabalho"));
    assert_eq!(threads[1].1.as_deref(), Some("ses-hobbies"));
}

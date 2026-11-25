use super::*;

#[tokio::test]
async fn workspace_backups_create_list_restore_and_delete() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("brief");
    let workspace = format!("workspace_id={}", ctx.workspace_id);

    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create: {created}");
    let id = created["id"].as_str().expect("backup id");
    assert_eq!(created["files"], 1);
    assert!(created["sha256"].as_str().unwrap_or_default().len() == 64);

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["id"], id);

    let target = ctx.temp.path().join("restored-api");
    let (status, restored) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups/{id}/restore?{workspace}"),
        Some(json!({
            "target_path": target.to_string_lossy().to_string(),
            "name": "Restaurado via API"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "restore: {restored}");
    assert_eq!(restored["name"], "Restaurado via API");
    assert!(target.join("input/brief.md").is_file());

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/backups/{id}?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().expect("list").is_empty());
}

#[tokio::test]
async fn workspace_backup_accepts_log_and_secret_options() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups?{workspace}"),
        Some(json!({ "include_logs": false, "passphrase": "segredo-forte" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create: {created}");
    assert_eq!(created["version"], 2);
    assert_eq!(created["includes"]["encrypted_secrets"], true);
    assert_eq!(created["includes"]["run_logs"], false);
    assert!(created["counts"]["files"].is_number());
}

#[tokio::test]
async fn full_workspace_round_trip_restores_files_and_state() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "versão original").expect("brief");
    let workspace = format!("workspace_id={}", ctx.workspace_id);

    let (status, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?{workspace}"),
        Some(json!({
            "title": "Tarefa do round-trip",
            "description": "precisa voltar idêntica",
            "assigned_agent": "build"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{task}");

    let (status, schedule) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina do round-trip",
            "description": "teste",
            "cron_expression": "0 9 * * *",
            "timezone": "America/Sao_Paulo",
            "command_or_prompt": "teste",
            "trigger_type": "CRON",
            "trigger_config_json": "{}",
            "target_kind": "PROMPT",
            "router_policy": "COST_BENEFIT",
            "output_folder_template": "output/{YYYY}-{MM}/{schedule_slug}"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schedule}");

    let yaml = "id: backup-recipe\nname: Receita do round-trip\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
    crate::recipes::store::save_yaml(&ctx.state.db, &ctx.workspace_id, "backup-recipe", yaml)
        .await
        .expect("recipe");

    let (status, context) = call(
        &ctx,
        Method::POST,
        &format!("/api/personal-context?{workspace}"),
        Some(json!({
            "scope": "workspace",
            "workspace_id": ctx.workspace_id,
            "instructions": "Regra que precisa voltar"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{context}");

    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().expect("backup id").to_string();
    assert_eq!(created["includes"]["sqlite_state"], true);
    let counts = created["counts"].as_object().expect("counts");
    assert!(counts["files"].as_u64().unwrap_or_default() >= 2);
    assert!(counts["tasks"].as_u64().unwrap_or_default() >= 1);
    assert!(counts["schedules"].as_u64().unwrap_or_default() >= 1);
    assert!(
        counts["personal_context_groups"]
            .as_u64()
            .unwrap_or_default()
            >= 1
    );

    std::fs::write(ctx.workspace_root.join("input/brief.md"), "mutado").expect("mutate");
    sqlx::query("DELETE FROM tasks WHERE workspace_id = ?")
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("delete tasks");
    sqlx::query("DELETE FROM schedules WHERE workspace_id = ?")
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("delete schedules");
    sqlx::query("DELETE FROM recipes_index WHERE workspace_id = ?")
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("delete recipes");
    let _ = std::fs::remove_file(
        ctx.workspace_root
            .join(".opencode/recipes/backup-recipe.yaml"),
    );
    sqlx::query("DELETE FROM personal_context_groups WHERE workspace_id = ?")
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("delete context");

    let (status, restored) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups/{id}/restore?{workspace}"),
        Some(json!({ "mode": "replace_in_place" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(restored["id"], ctx.workspace_id);
    assert_eq!(restored["restore_report"]["mode"], "in_place");

    let content =
        std::fs::read_to_string(ctx.workspace_root.join("input/brief.md")).expect("restored file");
    assert_eq!(content, "versão original");
    let tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE workspace_id = ? AND title = 'Tarefa do round-trip'",
    )
    .bind(&ctx.workspace_id)
    .fetch_one(&ctx.state.db)
    .await
    .expect("tasks");
    assert_eq!(tasks, 1);
    let schedules: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM schedules WHERE workspace_id = ? AND name = 'Rotina do round-trip'",
    )
    .bind(&ctx.workspace_id)
    .fetch_one(&ctx.state.db)
    .await
    .expect("schedules");
    assert_eq!(schedules, 1);
    let recipes = crate::recipes::store::list(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("recipes");
    assert!(recipes.iter().any(|recipe| recipe.id == "backup-recipe"));
    let contexts = crate::personal_context::list(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("context");
    assert!(
        contexts
            .iter()
            .any(|group| group.instructions.contains("precisa voltar"))
    );
}

#[tokio::test]
async fn imports_a_backup_zip_and_rejects_tampered_archives() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("brief");
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().expect("id");
    let (path, _) =
        crate::backup::get(&ctx.state.paths, &ctx.workspace_id, id).expect("backup path");
    let bytes = std::fs::read(path).expect("zip");
    let encoded = STANDARD.encode(&bytes);

    let (status, imported) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups/import?{workspace}"),
        Some(json!({
            "file_name": "externo.zip",
            "content_base64": encoded,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert!(
        imported["file_name"]
            .as_str()
            .unwrap_or_default()
            .starts_with("imported-")
    );
    let (_, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(list.as_array().map(Vec::len), Some(2));

    let tampered = STANDARD.encode(&bytes[..bytes.len() / 2]);
    let (status, rejected) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups/import?{workspace}"),
        Some(json!({
            "file_name": "quebrado.zip",
            "content_base64": tampered,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
}

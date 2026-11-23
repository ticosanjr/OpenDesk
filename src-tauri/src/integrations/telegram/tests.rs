use super::format::{interaction_markup, parse_selected_options};
use super::http::{resolve_interaction_callback, resolve_permission_callback};
use super::*;
use crate::db::interactions;

#[test]
fn parses_interaction_replies_and_options() {
    assert_eq!(
        parse_interaction_reply("#1a2b3c4d 2"),
        Some(("1a2b3c4d", "2"))
    );
    assert_eq!(parse_interaction_reply("texto normal"), None);
    assert_eq!(parse_interaction_reply("#abc 1"), None);
    let options = vec!["Aprovar".to_string(), "Rejeitar".to_string()];
    assert_eq!(
        parse_selected_options("2", &options, false),
        vec!["Rejeitar".to_string()]
    );
    assert_eq!(
        parse_selected_options("1,2", &options, true),
        vec!["Aprovar".to_string(), "Rejeitar".to_string()]
    );
    assert!(parse_selected_options("9", &options, false).is_empty());
    assert_eq!(short_id("1234567890"), "12345678");
}

#[test]
fn builds_inline_keyboards_for_single_choice_interactions() {
    let interaction = interactions::TaskInteraction {
        id: "abcdef12-0000-0000-0000-000000000000".into(),
        workspace_id: "ws".into(),
        task_id: "task".into(),
        run_id: None,
        question: "Aprovar?".into(),
        context_markdown: None,
        options_json: r#"["Aprovar","Rejeitar"]"#.into(),
        kind: "SINGLE_CHOICE".into(),
        multiple: false,
        allow_custom: true,
        status: "PENDING".into(),
        answer_json: None,
        expires_at: "2999-01-01T00:00:00Z".into(),
        created_at: String::new(),
        resolved_at: None,
    };
    let markup = interaction_markup(&interaction).expect("markup");
    let rows = markup["inline_keyboard"].as_array().expect("rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0][0]["callback_data"],
        "ti:abcdef12-0000-0000-0000-000000000000:0"
    );
    assert_eq!(
        rows[2][0]["callback_data"],
        "ti:abcdef12-0000-0000-0000-000000000000:deny"
    );
    assert!(rows[0][0]["callback_data"].as_str().unwrap().len() <= 64);

    let approval = interactions::TaskInteraction {
        kind: "APPROVAL".into(),
        multiple: false,
        options_json: "[]".into(),
        ..interaction
    };
    let markup = interaction_markup(&approval).expect("approval markup");
    let rows = markup["inline_keyboard"].as_array().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0][1]["callback_data"],
        "ti:abcdef12-0000-0000-0000-000000000000:deny"
    );
}

#[tokio::test]
async fn resolves_task_interaction_callbacks() {
    let ctx = crate::test_support::context().await;
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description)
             VALUES ('task-cb', ?, 'Revisar', 'Descrição')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("task");
    let created = interactions::create(
        &ctx.state.db,
        interactions::CreateInteractionInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: "task-cb".into(),
            run_id: None,
            question: "Aprovar?".into(),
            context_markdown: None,
            options: vec!["Aprovar".into(), "Rejeitar".into()],
            kind: "SINGLE_CHOICE".into(),
            multiple: false,
            allow_custom: true,
            expires_at: "2999-01-01T00:00:00Z".into(),
        },
    )
    .await
    .expect("interaction");
    let callback = format!("{}:0", created.id);
    let message = resolve_interaction_callback(&ctx.state, &callback)
        .await
        .expect("resolve");
    assert!(message.contains("Resposta registrada"));
    let interaction = interactions::get(&ctx.state.db, &created.id)
        .await
        .expect("interaction");
    assert_eq!(interaction.status, "RESOLVED");
    assert!(
        interaction
            .answer_json
            .unwrap_or_default()
            .contains("Aprovar")
    );
}

#[tokio::test]
async fn resolves_permission_callbacks() {
    let ctx = crate::test_support::context().await;
    sqlx::query(
        "INSERT INTO permission_requests
                (id, workspace_id, session_id, request_id, permission_action)
             VALUES ('perm-row', ?, 'ses-1', 'req-1', 'bash')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("permission");

    let message = resolve_permission_callback(&ctx.state, "req-1:once")
        .await
        .expect("resolve");
    assert!(message.contains("aprovada"));
    let decision = crate::db::permissions::get_by_request(&ctx.state.db, "req-1")
        .await
        .expect("get")
        .decision;
    assert_eq!(decision.as_deref(), Some("once"));
}

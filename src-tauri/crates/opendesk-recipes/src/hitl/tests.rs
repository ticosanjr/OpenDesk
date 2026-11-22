use super::replies::parse_resolution;
use crate::parser::HitlCheckpoint;
use crate::store::RecipeRun;

use super::*;

#[test]
fn parses_approval_and_feedback() {
    assert_eq!(parse_reply("BINARY_GATE", &[], "APROVAR").0, "APPROVED");
    assert_eq!(parse_reply("BINARY_GATE", &[], "não").0, "REJECTED");
    assert_eq!(
        parse_reply("APPROVAL_WITH_FEEDBACK", &[], "Refaça a tabela 2").0,
        "FEEDBACK"
    );
    assert_eq!(extract_token("ok [OD-#8F2A]").as_deref(), Some("8F2A"));
    assert_eq!(extract_token("sem token").as_deref(), None);
    let options = vec!["A".to_string(), "B".to_string()];
    let (kind, payload) = parse_reply("MULTI_CHOICE", &options, "2");
    assert_eq!(kind, "CHOICE");
    assert!(payload.expect("payload").contains("B"));
}

use crate::{parser, store};

use crate::{context::RecipeContext, test_support};

use opendesk_storage::events::EventBus;
use opendesk_storage::secrets::SecretStore;

use serde_json::json;
use std::sync::Arc;

fn checkpoint(channels: &[&str]) -> HitlCheckpoint {
    HitlCheckpoint {
        id: "checkpoint-1".into(),
        after_step: String::new(),
        name: Some("Aprovação".into()),
        description: None,
        interaction_type: "BINARY_GATE".into(),
        channels: channels.iter().map(|value| value.to_string()).collect(),
        timeout_minutes: 30,
        on_timeout: "PAUSE_AND_ALERT".into(),
        prompt_template: "Aprovar?".into(),
        attach_files: Vec::new(),
        options: Vec::new(),
    }
}

async fn fixture() -> (test_support::TestContext, RecipeRun) {
    let ctx = test_support::context().await;
    let yaml = "id: hitl-recipe\nname: Receita HITL\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
    let recipe = parser::parse(yaml).expect("recipe");
    sqlx::query(
        "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('hitl-recipe', ?, 'Receita HITL', '/tmp/hitl.yaml', 'MANUAL')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.db)
    .await
    .expect("index");
    let run = store::create_run(
        &ctx.db,
        &recipe,
        &ctx.workspace_id,
        None,
        &json!({}),
        "/tmp/out",
        false,
    )
    .await
    .expect("run");
    (ctx, run)
}

#[test]
fn parses_every_reply_shape() {
    assert_eq!(parse_reply("BINARY_GATE", &[], " Aprovar ").0, "APPROVED");
    assert_eq!(parse_reply("BINARY_GATE", &[], "sim").0, "APPROVED");
    assert_eq!(parse_reply("BINARY_GATE", &[], "👍").0, "APPROVED");
    let (kind, payload) = parse_reply("BINARY_GATE", &[], "não");
    assert_eq!(kind, "REJECTED");
    assert_eq!(payload.as_deref(), Some("não"));
    assert_eq!(parse_reply("BINARY_GATE", &[], "> citado\n").0, "FEEDBACK");

    let (kind, _payload) = parse_reply("SINGLE_CHOICE", &["sim".into(), "não".into()], "2");
    assert_eq!(kind, "CHOICE");
    assert_eq!(parse_reply("TEXT", &[], "revisar o texto").0, "FEEDBACK");
    let (kind, payload) = parse_reply("MULTI_CHOICE", &["sim".into(), "não".into()], "2");
    assert_eq!(kind, "CHOICE");
    assert!(payload.expect("payload").contains("\"option_index\":1"));
    assert_eq!(
        parse_reply("MULTI_CHOICE", &["sim".into()], "9").0,
        "FEEDBACK"
    );

    let (kind, payload) = parse_reply("BINARY_GATE", &[], "revisar o texto");
    assert_eq!(kind, "FEEDBACK");
    assert_eq!(payload.as_deref(), Some("revisar o texto"));
}

#[test]
fn extracts_tokens_from_messages() {
    assert_eq!(
        extract_token("responda OD-#AB12CD34").as_deref(),
        Some("AB12CD34")
    );
    assert_eq!(extract_token("#ab12 ok").as_deref(), Some("AB12"));
    assert_eq!(extract_token("sem token"), None);
    assert_eq!(extract_token("OD-#ZZ"), None);
}

#[test]
fn parses_resolution_payloads() {
    assert!(matches!(
        parse_resolution("REJECTED"),
        HitlResolution::Rejected { .. }
    ));
    assert!(matches!(
        parse_resolution("desconhecido"),
        HitlResolution::Approved
    ));
    match parse_resolution(r#"{"path":"out/a.md"}"#) {
        HitlResolution::ArtifactEdited { path } => assert_eq!(path, "out/a.md"),
        other => panic!("unexpected {other:?}"),
    }
    match parse_resolution(r#"{"feedback_text":"ok","attached_files":["a.md"]}"#) {
        HitlResolution::FeedbackProvided {
            feedback_text,
            attached_files,
        } => {
            assert_eq!(feedback_text, "ok");
            assert_eq!(attached_files, vec!["a.md"]);
        }
        other => panic!("unexpected {other:?}"),
    }
    match parse_resolution(r#"{"option_index":1,"option_value":"sim"}"#) {
        HitlResolution::ChoiceSelected {
            option_index,
            option_value,
        } => {
            assert_eq!(option_index, 1);
            assert_eq!(option_value, "sim");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn request_creates_and_dispatches_channels() {
    let (ctx, run) = fixture().await;
    let recipe_context = RecipeContext::new(
        ctx.db.clone(),
        ctx.paths.clone(),
        EventBus::new(32),
        Arc::new(SecretStore::new(&ctx.paths)),
    );
    let token = request(
        &recipe_context,
        &run,
        &checkpoint(&["DESKTOP_UI", "UNKNOWN"]),
        "Aprovar envio",
        "Envie o relatório?",
        Some("Publicar"),
        &[],
    )
    .await
    .expect("request");
    assert_eq!(token.len(), 4);
    let stored = store::get_hitl(&ctx.db, &token).await.expect("hitl");
    assert_eq!(stored.status, "PENDING");

    resolve(
        &recipe_context,
        &token,
        "DESKTOP_UI",
        None,
        "APPROVED",
        None,
    )
    .await
    .expect("resolve");
    let outcome = wait(&recipe_context, &token, 1).await.expect("wait");
    assert!(matches!(outcome, HitlOutcome::Resolved(_)));

    assert!(
        resolve(
            &recipe_context,
            "missing",
            "DESKTOP_UI",
            None,
            "APPROVED",
            None
        )
        .await
        .expect("missing")
        .is_none()
    );

    sqlx::query("UPDATE hitl_requests SET status = 'EXPIRED' WHERE token = ?")
        .bind(&token)
        .execute(&ctx.db)
        .await
        .expect("expire");
    assert!(matches!(
        wait(&recipe_context, &token, 1).await.expect("expired"),
        HitlOutcome::Expired
    ));
}

use super::replace_attachment;
use super::start;
use super::steps::{ensure_step_finished, mode_for_step};
use crate::context::RecipeContext;
use crate::parser::Step;
use crate::store;

use std::time::Duration;

#[test]
fn aborta_receita_quando_step_falha() {
    assert!(ensure_step_finished("s1", "SUCCESS").is_ok());
    assert!(ensure_step_finished("s1", "FAILED").is_err());
    assert!(ensure_step_finished("s1", "CANCELLED").is_err());
    assert!(ensure_step_finished("s1", "RUNNING").is_err());
}

#[test]
fn maps_step_mode() {
    assert_eq!(
        mode_for_step(&Step {
            id: "s".into(),
            name: "S".into(),
            mode: "plan".into(),
            agent: None,
            model: None,
            effort: None,
            description: None,
            prompt: "x".into(),
            when: None,
            input_schema: None,
            output_schema: None,
        }),
        "PLAN_ONLY"
    );
}

#[tokio::test]
async fn dry_run_completes_without_dispatch() {
    let test = crate::test_support::context().await;
    let context = RecipeContext::new(
        test.db.clone(),
        test.paths.clone(),
        opendesk_storage::events::EventBus::new(32),
        std::sync::Arc::new(opendesk_storage::secrets::SecretStore::new(&test.paths)),
    );
    let yaml = "id: dry\nname: Dry\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
    store::save_yaml(&test.db, &test.workspace_id, "dry", yaml)
        .await
        .expect("save");
    // New recipes start disabled; this test exercises the run path, so it
    // performs the explicit activation a human would do after review.
    store::set_enabled(&test.db, &test.workspace_id, "dry", true)
        .await
        .expect("enable");
    let recipe = crate::parser::parse(yaml).expect("parse");
    let run = start(
        context.clone(),
        &test.workspace_id,
        recipe,
        serde_json::json!({}),
        true,
    )
    .await
    .expect("run");
    let mut status = run.status;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        status = store::get_run(&test.db, &run.id).await.expect("get").status;
        if status == "COMPLETED" {
            break;
        }
    }
    assert_eq!(status, "COMPLETED");
}

#[test]
fn replaces_attachment_safely_in_temp_dir() {
    let temp = tempfile::TempDir::new().expect("temp");
    let source = temp.path().join("new.md");
    let target = temp.path().join("out/final.md");
    std::fs::write(&source, "novo").expect("source");
    replace_attachment(
        &source.to_string_lossy(),
        &[target.to_string_lossy().to_string()],
    )
    .expect("replace");
    assert_eq!(std::fs::read_to_string(target).expect("target"), "novo");
}

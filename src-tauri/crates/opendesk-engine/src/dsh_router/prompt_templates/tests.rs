use super::*;

#[test]
fn renders_placeholders_and_keeps_unknown_ones() {
    let output = render("Olá {{name}}, {{missing}}", &[("name", "Rafael")]);
    assert_eq!(output, "Olá Rafael, {{missing}}");
}

#[test]
fn defaults_cover_the_documented_sections() {
    let templates = PromptTemplates::default();
    for field in FIELDS {
        let value = match field.id {
            "permanent" => &templates.permanent,
            "completion" => &templates.completion,
            "personal_block" => &templates.personal_block,
            "memory_block" => &templates.memory_block,
            "structured_memory_block" => &templates.structured_memory_block,
            "task_meta" => &templates.task_meta,
            "description" => &templates.description,
            "output" => &templates.output,
            "inputs" => &templates.inputs,
            "plan_phase" => &templates.plan_phase,
            "build_phase" => &templates.build_phase,
            "audit_phase" => &templates.audit_phase,
            "browse" => &templates.browse,
            "care_quick" => &templates.care_quick,
            "care_standard" => &templates.care_standard,
            "care_careful" => &templates.care_careful,
            "opendesk_context" => &templates.opendesk_context,
            "chat_channel" => &templates.chat_channel,
            "follow_up" => &templates.follow_up,
            "execution_instruction" => &templates.execution_instruction,
            "memory_policy" => &templates.memory_policy,
            "untrusted_wrapper" => &templates.untrusted_wrapper,
            "json_schema_contract" => &templates.json_schema_contract,
            "harness_overrides" => &templates.harness_overrides,
            "recipe_feedback" => &templates.recipe_feedback,
            "model_test" => &templates.model_test,
            _ => panic!("uncovered field {}", field.id),
        };
        assert!(!value.trim().is_empty(), "field {}", field.id);
    }
}

#[tokio::test]
async fn saves_and_resets_workspace_templates() {
    let ctx = crate::test_support::context().await;
    let templates = PromptTemplates {
        output: "saída {{output_directory}}".into(),
        ..Default::default()
    };
    save(&ctx.db, Some(&ctx.workspace_id), &templates)
        .await
        .expect("save");
    let loaded = load(&ctx.db, Some(&ctx.workspace_id)).await.expect("load");
    assert!(loaded.output.contains("saída"));
    reset(&ctx.db, Some(&ctx.workspace_id))
        .await
        .expect("reset");
    let loaded = load(&ctx.db, Some(&ctx.workspace_id)).await.expect("load");
    assert_eq!(loaded.output, PromptTemplates::default().output);
}

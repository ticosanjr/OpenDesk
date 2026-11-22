use super::model::with_variant;
use super::*;
use ::opendesk_storage::repos::router::RouterRule;

fn model(provider: &str, id: &str, input: f64, output: f64, variants: &[&str]) -> ModelDto {
    ModelDto {
        id: id.to_string(),
        model_id: Some(id.to_string()),
        provider_id: Some(provider.to_string()),
        name: Some(id.to_string()),
        variants: variants
            .iter()
            .map(|id| crate::opencode_client::rest_client::ModelVariantDto {
                id: (*id).to_string(),
            })
            .collect(),
        status: Some("active".into()),
        enabled: Some(true),
        cost: vec![serde_json::json!({
            "input": input,
            "output": output,
            "cache": { "read": 0, "write": 0 }
        })],
    }
}

fn available() -> Vec<ModelDto> {
    vec![
        model("opencode-go", "deepseek-v4.1-flash", 0.1, 0.2, &["low"]),
        model("opencode-go", "glm-5.3", 1.0, 2.0, &["medium", "high"]),
        model("opencode-go", "deepseek-v4-pro", 2.0, 4.0, &["high"]),
    ]
}

fn input<'a>(
    available: &'a [ModelDto],
    rules: &'a [RouterRule],
    policy: &'a str,
    execution_mode: &'a str,
    tags: &'a [String],
) -> RouterInput<'a> {
    RouterInput {
        workspace_id: "ws-1",
        workspace_root: Path::new("/tmp/opendesk-router-test"),
        title: "Relatório de vendas",
        prompt: "Analise os dados e produza um relatório executivo.",
        execution_mode,
        priority: 2,
        tags,
        manual_agent: None,
        manual_model: None,
        manual_effort: None,
        manual_harness: None,
        default_harness: None,
        policy,
        rules,
        available_models: available,
        default_model: None,
        manual_default_model: None,
        approved_plan: None,
        output_directory: None,
        budget_daily_pct: 10.0,
    }
}

#[test]
fn workspace_default_model_wins_over_hardcoded_provider() {
    let models = vec![
        model("deepseek", "deepseek-flash", 0.1, 0.2, &["low"]),
        model("opencode-go", "deepseek-v4.1-flash", 0.1, 0.2, &["low"]),
    ];
    let mut router_input = input(&models, &[], "COST_BENEFIT", "BUILD_DIRECT", &[]);
    router_input.default_model = Some("deepseek/deepseek-flash");
    let decision = classify(&router_input);
    assert_eq!(decision.selected_model, "deepseek/deepseek-flash#low");
    assert_eq!(decision.origin, "COST_BENEFIT");
}

#[test]
fn cost_benefit_plan_uses_plan_agent() {
    let models = available();
    let decision = classify(&input(&models, &[], "COST_BENEFIT", "PLAN_ONLY", &[]));
    assert_eq!(decision.phase, "plan");
    assert_eq!(decision.selected_agent, "plan");
    assert!(decision.selected_model.contains("glm-5.3"));
    assert!(decision.output_directory.contains("/output/"));
}

#[test]
fn cheapest_policy_chooses_lowest_price_and_low_effort() {
    let models = available();
    let decision = classify(&input(&models, &[], "CHEAPEST", "BUILD_DIRECT", &[]));
    assert_eq!(decision.effort, "low");
    assert!(decision.selected_model.contains("flash"));
    assert_eq!(decision.phase, "build");
}

#[test]
fn custom_rule_matches_tags_and_sets_target() {
    let models = available();
    let tags = vec!["roteiro".to_string()];
    let rules = vec![RouterRule {
        id: "rule-1".into(),
        workspace_id: Some("ws-1".into()),
        name: "Roteiros".into(),
        priority_order: 1,
        condition_json: r#"{"field":"tags","op":"contains","value":"roteiro"}"#.into(),
        target_mode: "build".into(),
        target_agent: Some("roteirista".into()),
        target_model: "opencode-go/glm-5.3".into(),
        target_effort: "high".into(),
        is_enabled: true,
    }];
    let decision = classify(&input(&models, &rules, "COST_BENEFIT", "AUTO_DSH", &tags));
    assert_eq!(decision.phase, "build");
    assert_eq!(decision.selected_agent, "roteirista");
    assert_eq!(decision.effort, "high");
    assert_eq!(decision.rule_name.as_deref(), Some("Roteiros"));
}

#[test]
fn high_complexity_auto_starts_with_plan() {
    let models = available();
    let tags = vec!["code".to_string()];
    let mut router_input = input(&models, &[], "COST_BENEFIT", "AUTO_DSH", &tags);
    router_input.priority = 4;
    router_input.prompt =
        "Refatore a arquitetura de integração com foco em segurança e performance.";
    let decision = classify(&router_input);
    assert_eq!(decision.phase, "plan");
}

#[test]
fn slug_removes_accents_and_output_is_monthly() {
    assert_eq!(slugify("Relatório de Vendas"), "relatorio-de-vendas");
    let path = output_directory(Path::new("/tmp/root"), "Relatório de Vendas");
    assert!(path.to_string_lossy().contains("/output/"));
    assert!(path.to_string_lossy().ends_with("relatorio-de-vendas"));
}
#[test]
fn never_builds_an_empty_variant_suffix() {
    assert_eq!(
        with_variant(&[], "provider/model#", "high"),
        "provider/model"
    );
    assert_eq!(with_variant(&[], "provider/model", ""), "provider/model");
    assert_eq!(
        with_variant(&[], "provider/model# high ", "low"),
        "provider/model#high"
    );
}

#[test]
fn manual_policy_uses_the_workspace_manual_model() {
    let models = available();
    let mut router_input = input(&models, &[], "MANUAL", "BUILD_DIRECT", &[]);
    router_input.manual_default_model = Some("opencode-go/glm-5.3");
    let decision = classify(&router_input);
    assert_eq!(decision.selected_model, "opencode-go/glm-5.3#medium");
    assert_eq!(decision.origin, "MANUAL");
}

#[test]
fn task_model_wins_over_the_workspace_manual_model() {
    let models = available();
    let mut router_input = input(&models, &[], "MANUAL", "BUILD_DIRECT", &[]);
    router_input.manual_default_model = Some("opencode-go/glm-5.3");
    router_input.manual_model = Some("opencode-go/deepseek-v4-pro");
    let decision = classify(&router_input);
    assert_eq!(decision.selected_model, "opencode-go/deepseek-v4-pro");
    assert_eq!(decision.origin, "MANUAL_OVERRIDE");
}

#[test]
fn manual_model_wins_over_cheapest_policy() {
    let models = available();
    let mut router_input = input(&models, &[], "CHEAPEST", "BUILD_DIRECT", &[]);
    router_input.manual_model = Some("opencode-go/glm-5.3");
    let decision = classify(&router_input);
    assert_eq!(decision.selected_model, "opencode-go/glm-5.3");
}

#[test]
fn unavailable_manual_model_is_kept_instead_of_falling_back() {
    let models = available();
    let mut router_input = input(&models, &[], "COST_BENEFIT", "BUILD_DIRECT", &[]);
    router_input.manual_model = Some("vendor/private-model");
    let decision = classify(&router_input);
    assert_eq!(decision.selected_model, "vendor/private-model");
    assert!(decision.rationale.contains("mantendo a escolha"));
}

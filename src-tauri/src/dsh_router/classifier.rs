use crate::{db::router::RouterRule, opencode_client::rest_client::ModelDto};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterDecision {
    pub phase: String,
    pub selected_agent: String,
    pub selected_model: String,
    pub effort: String,
    pub output_directory: String,
    pub rationale: String,
    pub requires_plan_approval: bool,
    pub audit_after_build: bool,
    pub policy_applied: String,
    pub rule_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RouterInput<'a> {
    pub workspace_id: &'a str,
    pub workspace_root: &'a Path,
    pub title: &'a str,
    pub prompt: &'a str,
    pub execution_mode: &'a str,
    pub priority: i64,
    pub tags: &'a [String],
    pub manual_agent: Option<&'a str>,
    pub manual_model: Option<&'a str>,
    pub manual_effort: Option<&'a str>,
    pub policy: &'a str,
    pub rules: &'a [RouterRule],
    pub available_models: &'a [ModelDto],
    pub approved_plan: Option<&'a str>,
    pub output_directory: Option<&'a Path>,
    pub budget_daily_pct: f64,
}

#[derive(Debug, Clone)]
struct Complexity {
    label: &'static str,
    score: u8,
}

#[allow(unused_assignments)]
pub fn classify(input: &RouterInput<'_>) -> RouterDecision {
    let complexity = estimate_complexity(input);
    let mode = input.execution_mode.to_uppercase();

    let mut phase = match mode.as_str() {
        "PLAN_ONLY" => "plan",
        "BUILD_DIRECT" => "build",
        "PLAN_THEN_BUILD" => {
            if input.approved_plan.is_some() {
                "build"
            } else {
                "plan"
            }
        }
        _ => {
            if input.approved_plan.is_some() {
                "build"
            } else if matches!(input.policy, "BEST_QUALITY") || complexity.score >= 3 {
                "plan"
            } else {
                "build"
            }
        }
    };

    let mut selected_agent = "build".to_string();
    let mut selected_model = "opencode-go/deepseek-v4.1-flash".to_string();
    let mut effort = "medium".to_string();
    let mut rationale = String::new();
    let mut requires_plan_approval = mode == "PLAN_THEN_BUILD";
    let mut audit_after_build = false;
    let mut policy_applied = input.policy.to_string();
    let mut rule_name = None;

    if let Some(rule) = input
        .rules
        .iter()
        .find(|rule| rule.is_enabled && matches_rule(rule, input, &complexity))
    {
        policy_applied = "CUSTOM_RULE".into();
        rule_name = Some(rule.name.clone());
        phase = match rule.target_mode.as_str() {
            "plan" => "plan",
            "build" => "build",
            "plan_then_build" => "plan",
            _ => phase,
        };
        requires_plan_approval = rule.target_mode == "plan_then_build";
        selected_agent = rule.target_agent.clone().unwrap_or_else(|| "build".into());
        selected_model = rule.target_model.clone();
        effort = rule.target_effort.clone();
        rationale = format!("regra '{}' aplicada", rule.name);
    } else {
        match input.policy {
            "MANUAL" => {
                selected_agent = input.manual_agent.unwrap_or("build").to_string();
                selected_model = input
                    .manual_model
                    .unwrap_or("opencode-go/deepseek-v4.1-flash")
                    .to_string();
                effort = input.manual_effort.unwrap_or("medium").to_string();
                rationale = "política manual".into();
            }
            "CHEAPEST" => {
                selected_model = cheapest_model(input.available_models)
                    .unwrap_or_else(|| "opencode-go/deepseek-v4.1-flash".into());
                effort = "low".into();
                selected_agent = if phase == "plan" { "plan" } else { "build" }.into();
                rationale = "política mais barata: modelo de menor custo, effort low".into();
            }
            "BEST_QUALITY" => {
                if phase == "plan" {
                    selected_model = prefer_model(
                        input.available_models,
                        &["opencode-go/deepseek-v4-pro", "opencode-go/glm-5.3"],
                        "high",
                    );
                    effort = "high".into();
                    selected_agent = "plan".into();
                    rationale = "máxima qualidade: Plan forte antes do Build".into();
                } else {
                    selected_model = prefer_model(
                        input.available_models,
                        &["opencode-go/glm-5.3", "opencode-go/deepseek-v4-pro"],
                        "high",
                    );
                    effort = "high".into();
                    selected_agent = "build".into();
                    audit_after_build = true;
                    rationale = "máxima qualidade: Build forte seguido de auditoria".into();
                }
                requires_plan_approval = true;
            }
            _ => {
                // COST_BENEFIT / unknown falls back to the default policy.
                policy_applied = "COST_BENEFIT".into();
                if phase == "plan" {
                    selected_model = prefer_model(
                        input.available_models,
                        &["opencode-go/glm-5.3", "opencode-go/deepseek-v4-pro"],
                        "medium",
                    );
                    effort = "medium".into();
                    selected_agent = "plan".into();
                    rationale = "custo-benefício: Plan em modelo forte com effort medium".into();
                } else {
                    selected_model = prefer_model(
                        input.available_models,
                        &["opencode-go/deepseek-v4.1-flash", "opencode-go/glm-5.3"],
                        "low",
                    );
                    effort = "low".into();
                    selected_agent = "build".into();
                    rationale = if complexity.score >= 3 {
                        "custo-benefício: Build complexo em modelo forte".into()
                    } else {
                        "custo-benefício: triagem/Build simples no modelo econômico".into()
                    };
                }
            }
        }
    }

    if mode == "PLAN_ONLY" {
        phase = "plan";
        requires_plan_approval = false;
    }
    if mode == "BUILD_DIRECT" {
        phase = "build";
        requires_plan_approval = false;
    }
    if phase == "build" {
        requires_plan_approval = false;
    }

    // Explicit user choices always win over the policy/rule, so editing the
    // agent or model on a task actually changes the next execution.
    if let Some(agent) = input.manual_agent.filter(|value| !value.trim().is_empty()) {
        selected_agent = agent.to_string();
    }
    if let Some(model) = input.manual_model.filter(|value| !value.trim().is_empty()) {
        selected_model = model.to_string();
    }

    // If the selected model is not exposed by this OpenCode instance, fall back
    // to the closest available chat model instead of failing the run.
    let (resolved_model, model_note) =
        resolve_model(input.available_models, &selected_model, &effort);
    if let Some(note) = model_note {
        rationale.push_str(&format!(" · {note}"));
    }

    let output = input
        .output_directory
        .map(Path::to_path_buf)
        .unwrap_or_else(|| output_directory(input.workspace_root, input.title));

    RouterDecision {
        phase: phase.into(),
        selected_agent,
        selected_model: resolved_model,
        effort,
        output_directory: output.to_string_lossy().to_string(),
        rationale: format!(
            "{rationale} · complexidade {}, score {}",
            complexity.label, complexity.score
        ),
        requires_plan_approval,
        audit_after_build,
        policy_applied,
        rule_name,
    }
}

fn estimate_complexity(input: &RouterInput<'_>) -> Complexity {
    let text = format!("{} {}", input.title, input.prompt).to_lowercase();
    let mut score = 0u8;

    if input.priority >= 3 {
        score += 1;
    }
    if input.prompt.len() > 800 {
        score += 1;
    }
    if [
        "arquitetura",
        "refatorar",
        "migrar",
        "migração",
        "implementar",
        "auditar",
        "multi",
        "integração",
        "performance",
        "segurança",
    ]
    .iter()
    .any(|keyword| text.contains(keyword))
    {
        score += 1;
    }
    if input.tags.iter().any(|tag| {
        matches!(
            tag.to_lowercase().as_str(),
            "code" | "architecture" | "refactor" | "complex"
        )
    }) {
        score += 1;
    }
    if input.prompt.len() < 220 && input.priority <= 2 {
        score = score.saturating_sub(1);
    }

    let score = score.min(4);
    let label = match score {
        0..=1 => "low",
        2..=2 => "medium",
        _ => "high",
    };
    Complexity { label, score }
}

fn matches_rule(rule: &RouterRule, input: &RouterInput<'_>, complexity: &Complexity) -> bool {
    let Ok(condition) = serde_json::from_str::<Value>(&rule.condition_json) else {
        return false;
    };
    evaluate_condition(&condition, input, complexity)
}

fn evaluate_condition(condition: &Value, input: &RouterInput<'_>, complexity: &Complexity) -> bool {
    if let Some(all) = condition.get("all").and_then(Value::as_array) {
        return all
            .iter()
            .all(|item| evaluate_condition(item, input, complexity));
    }
    if let Some(any) = condition.get("any").and_then(Value::as_array) {
        return any
            .iter()
            .any(|item| evaluate_condition(item, input, complexity));
    }
    if let Some(not) = condition.get("not") {
        return !evaluate_condition(not, input, complexity);
    }

    let field = condition.get("field").and_then(Value::as_str).unwrap_or("");
    let op = condition.get("op").and_then(Value::as_str).unwrap_or("eq");
    let expected = condition.get("value").cloned().unwrap_or(Value::Null);

    match field {
        "tags" => compare_tags(op, input.tags, &expected),
        "title" => compare_string(op, input.title, &expected),
        "prompt" => compare_string(op, input.prompt, &expected),
        "priority" => compare_number(op, input.priority as f64, &expected),
        "estimated_complexity" => compare_string(op, complexity.label, &expected),
        "budget_daily_pct" => compare_number(op, input.budget_daily_pct, &expected),
        "workspace_id" => compare_string(op, input.workspace_id, &expected),
        "execution_mode" => compare_string(op, input.execution_mode, &expected),
        _ => false,
    }
}

fn compare_tags(op: &str, tags: &[String], expected: &Value) -> bool {
    let expected = expected.as_str().unwrap_or_default().to_lowercase();
    match op {
        "contains" => tags.iter().any(|tag| tag.to_lowercase() == expected),
        "not_contains" => tags.iter().all(|tag| tag.to_lowercase() != expected),
        "in" => expected.is_empty(),
        _ => false,
    }
}

fn compare_string(op: &str, actual: &str, expected: &Value) -> bool {
    let actual = actual.to_lowercase();
    let expected = expected.as_str().unwrap_or_default().to_lowercase();
    match op {
        "eq" => actual == expected,
        "ne" => actual != expected,
        "contains" => actual.contains(&expected),
        "not_contains" => !actual.contains(&expected),
        "starts_with" => actual.starts_with(&expected),
        "ends_with" => actual.ends_with(&expected),
        "in" => expected.split(',').any(|value| value.trim() == actual),
        _ => false,
    }
}

fn compare_number(op: &str, actual: f64, expected: &Value) -> bool {
    let Some(expected) = expected.as_f64() else {
        return false;
    };
    match op {
        "eq" => (actual - expected).abs() < f64::EPSILON,
        "ne" => (actual - expected).abs() >= f64::EPSILON,
        "gt" => actual > expected,
        "gte" => actual >= expected,
        "lt" => actual < expected,
        "lte" => actual <= expected,
        _ => false,
    }
}

fn prefer_model(available: &[ModelDto], candidates: &[&str], effort: &str) -> String {
    for candidate in candidates {
        if model_exists(available, candidate) {
            return with_variant(available, candidate, effort);
        }
    }
    if let Some(model) = available.iter().find(|model| {
        model.enabled.unwrap_or(true)
            && model
                .status
                .as_deref()
                .map(|status| status != "deprecated")
                .unwrap_or(true)
    }) {
        let key = model_key(model);
        return with_variant(available, &key, effort);
    }
    candidates
        .first()
        .copied()
        .unwrap_or("opencode-go/deepseek-v4.1-flash")
        .to_string()
}

fn cheapest_model(available: &[ModelDto]) -> Option<String> {
    available
        .iter()
        .filter(|model| model.enabled.unwrap_or(true))
        .min_by(|a, b| {
            let a_price = model_price(a);
            let b_price = model_price(b);
            a_price
                .partial_cmp(&b_price)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|model| with_variant(available, &model_key(model), "low"))
}

fn model_price(model: &ModelDto) -> f64 {
    model
        .cost
        .iter()
        .map(|cost| {
            let input = cost
                .get("input")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX);
            let output = cost
                .get("output")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX);
            input + output
        })
        .fold(f64::MAX, f64::min)
}

fn model_key(model: &ModelDto) -> String {
    let provider = model.provider_id.clone().unwrap_or_default();
    let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
    format!("{provider}/{id}")
}

fn model_exists(available: &[ModelDto], candidate: &str) -> bool {
    let (provider, id) = split_model(candidate);
    available.iter().any(|model| {
        let model_provider = model.provider_id.as_deref().unwrap_or_default();
        let model_id = model.model_id.as_deref().unwrap_or(&model.id);
        (provider.is_empty() || provider == model_provider) && id == model_id
    })
}

fn with_variant(available: &[ModelDto], model: &str, effort: &str) -> String {
    let model = model.trim();
    if let Some((base, variant)) = model.split_once('#') {
        let base = base.trim();
        let variant = variant.trim();
        return if variant.is_empty() {
            base.to_string()
        } else {
            format!("{base}#{variant}")
        };
    }
    let effort = effort.trim();
    if effort.is_empty() {
        return model.to_string();
    }
    let (provider, id) = split_model(model);
    let has_variant = available.iter().any(|candidate| {
        let candidate_provider = candidate.provider_id.as_deref().unwrap_or_default();
        let candidate_id = candidate.model_id.as_deref().unwrap_or(&candidate.id);
        (provider.is_empty() || provider == candidate_provider)
            && id == candidate_id
            && candidate
                .variants
                .iter()
                .any(|variant| variant.id == effort)
    });
    if has_variant {
        format!("{model}#{effort}")
    } else {
        model.to_string()
    }
}

fn resolve_model(available: &[ModelDto], selected: &str, effort: &str) -> (String, Option<String>) {
    if available.is_empty() {
        return (selected.to_string(), None);
    }
    if model_exists(available, selected) {
        return (with_variant(available, selected, effort), None);
    }
    let fallback = prefer_model(available, &[selected], effort);
    let note = format!("modelo '{selected}' indisponível; usando '{fallback}'");
    (fallback, Some(note))
}

fn split_model(model: &str) -> (String, String) {
    let base = model.split('#').next().unwrap_or(model);
    match base.split_once('/') {
        Some((provider, id)) => (provider.to_string(), id.to_string()),
        None => (String::new(), base.to_string()),
    }
}

pub fn slugify(value: &str) -> String {
    let normalized: String = value
        .nfd()
        .filter(|character| !unicode_normalization::char::is_combining_mark(*character))
        .collect();
    let slug: String = normalized
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "task".into()
    } else {
        slug.chars().take(48).collect()
    }
}

pub fn output_directory(root: &Path, title: &str) -> PathBuf {
    let now = Utc::now();
    let month = format!("{:04}-{:02}", now.year(), now.month());
    let base = root.join("output").join(month).join(slugify(title));
    if !base.exists() {
        return base;
    }
    for index in 2..100 {
        let candidate = base.with_file_name(format!(
            "{}-{}",
            base.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("task"),
            index
        ));
        if !candidate.exists() {
            return candidate;
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::router::RouterRule;

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
            policy,
            rules,
            available_models: available,
            approved_plan: None,
            output_directory: None,
            budget_daily_pct: 10.0,
        }
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
}

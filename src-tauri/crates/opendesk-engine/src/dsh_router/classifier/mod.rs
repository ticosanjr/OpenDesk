use crate::opencode_client::rest_client::ModelDto;
use ::opendesk_storage::repos::router::RouterRule;
use serde::{Deserialize, Serialize};
use std::path::Path;

mod complexity;
mod model;
mod output;
mod rules;

#[cfg(test)]
mod tests;

use complexity::estimate_complexity;
use model::{cheapest_model, model_exists, prefer_model, resolve_model};
use rules::matches_rule;

pub use model::model_available;
pub use output::{output_directory, slugify};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterDecision {
    pub phase: String,
    pub selected_agent: String,
    pub selected_model: String,
    pub harness: String,
    pub effort: String,
    pub output_directory: String,
    pub rationale: String,
    pub requires_plan_approval: bool,
    pub audit_after_build: bool,
    pub policy_applied: String,
    pub rule_name: Option<String>,
    /// Where the final decision came from: default model, policy, rule,
    /// manual override or fallback.
    pub origin: String,
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
    /// Explicit harness for this task/run; wins over the workspace default.
    pub manual_harness: Option<&'a str>,
    /// Workspace default adapter, e.g. `opencode` or `claude-code`.
    pub default_harness: Option<&'a str>,
    pub policy: &'a str,
    pub rules: &'a [RouterRule],
    pub available_models: &'a [ModelDto],
    /// Model chosen for this workspace in OpenDesk (never read from
    /// `opencode.jsonc`).
    pub default_model: Option<&'a str>,
    /// Model chosen for the MANUAL policy. A task-level `manual_model` still
    /// wins over this value.
    pub manual_default_model: Option<&'a str>,
    pub approved_plan: Option<&'a str>,
    pub output_directory: Option<&'a Path>,
    pub budget_daily_pct: f64,
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
    let default_model = input
        .default_model
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut selected_model = default_model.unwrap_or_default().to_string();
    let mut origin = "DEFAULT".to_string();
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
        origin = "RULE".into();
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
                    .or(input.manual_default_model)
                    .or(default_model)
                    .unwrap_or_default()
                    .to_string();
                effort = input.manual_effort.unwrap_or("medium").to_string();
                rationale = "política manual".into();
                origin = "MANUAL".into();
            }
            "CHEAPEST" => {
                selected_model = cheapest_model(input.available_models, default_model)
                    .or_else(|| default_model.map(str::to_string))
                    .unwrap_or_default();
                effort = "low".into();
                selected_agent = if phase == "plan" { "plan" } else { "build" }.into();
                rationale = "política mais barata: modelo de menor custo, effort low".into();
                origin = "CHEAPEST".into();
            }
            "BEST_QUALITY" => {
                if phase == "plan" {
                    let mut candidates = Vec::new();
                    if let Some(model) = default_model {
                        candidates.push(model);
                    }
                    candidates.extend(["opencode-go/deepseek-v4-pro", "opencode-go/glm-5.3"]);
                    selected_model = prefer_model(input.available_models, &candidates, "high");
                    effort = "high".into();
                    selected_agent = "plan".into();
                    rationale = "máxima qualidade: Plan forte antes do Build".into();
                } else {
                    let mut candidates = Vec::new();
                    if let Some(model) = default_model {
                        candidates.push(model);
                    }
                    candidates.extend(["opencode-go/glm-5.3", "opencode-go/deepseek-v4-pro"]);
                    selected_model = prefer_model(input.available_models, &candidates, "high");
                    effort = "high".into();
                    selected_agent = "build".into();
                    audit_after_build = true;
                    rationale = "máxima qualidade: Build forte seguido de auditoria".into();
                }
                origin = "BEST_QUALITY".into();
                requires_plan_approval = true;
            }
            _ => {
                // COST_BENEFIT / unknown falls back to the default policy.
                policy_applied = "COST_BENEFIT".into();
                if phase == "plan" {
                    let mut candidates = Vec::new();
                    if let Some(model) = default_model {
                        candidates.push(model);
                    }
                    candidates.extend(["opencode-go/glm-5.3", "opencode-go/deepseek-v4-pro"]);
                    selected_model = prefer_model(input.available_models, &candidates, "medium");
                    effort = "medium".into();
                    selected_agent = "plan".into();
                    rationale = "custo-benefício: Plan com effort medium".into();
                } else {
                    let mut candidates = Vec::new();
                    if let Some(model) = default_model {
                        candidates.push(model);
                    }
                    candidates.extend(["opencode-go/deepseek-v4.1-flash", "opencode-go/glm-5.3"]);
                    selected_model = prefer_model(input.available_models, &candidates, "low");
                    effort = "low".into();
                    selected_agent = "build".into();
                    rationale = if complexity.score >= 3 {
                        "custo-benefício: Build complexo em modelo forte".into()
                    } else {
                        "custo-benefício: triagem/Build simples no modelo econômico".into()
                    };
                }
                origin = "COST_BENEFIT".into();
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
        origin = "MANUAL_OVERRIDE".into();
    }

    // If the selected model is not exposed by this OpenCode instance, fall back
    // to the closest available chat model instead of failing the run.
    let manual_model = input
        .manual_model
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (resolved_model, model_note) = if let Some(model) = manual_model {
        // Explicit choice wins over every policy. If the model is not exposed
        // right now, keep it anyway and let the provider surface the real error
        // instead of silently switching models.
        let note =
            if input.available_models.is_empty() || model_exists(input.available_models, model) {
                None
            } else {
                Some(format!(
                    "modelo '{model}' indisponível; mantendo a escolha do usuário"
                ))
            };
        (model.to_string(), note)
    } else {
        resolve_model(input.available_models, &selected_model, &effort)
    };
    if let Some(note) = model_note {
        rationale.push_str(&format!(" · {note}"));
        if input.manual_model.is_none() {
            origin = "FALLBACK".into();
        }
    }

    let output = input
        .output_directory
        .map(Path::to_path_buf)
        .unwrap_or_else(|| output_directory(input.workspace_root, input.title));

    let harness = input
        .manual_harness
        .or(input.default_harness)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("opencode")
        .to_string();

    RouterDecision {
        phase: phase.into(),
        selected_agent,
        selected_model: resolved_model,
        harness,
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
        origin,
    }
}

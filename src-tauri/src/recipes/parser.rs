use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub trigger: Trigger,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub hitl_checkpoints: Vec<HitlCheckpoint>,
    #[serde(default)]
    pub sinks: Vec<RecipeSink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    #[serde(rename = "type")]
    pub trigger_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub name: String,
    #[serde(default = "default_build")]
    pub mode: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlCheckpoint {
    pub id: String,
    pub after_step: String,
    pub interaction_type: String,
    #[serde(default)]
    pub channels: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_minutes: i64,
    #[serde(default = "default_on_timeout")]
    pub on_timeout: String,
    pub prompt_template: String,
    #[serde(default)]
    pub attach_files: Vec<String>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeSink {
    #[serde(rename = "type")]
    pub sink_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_true() -> bool {
    true
}
fn default_build() -> String {
    "build".into()
}
fn default_timeout() -> i64 {
    240
}
fn default_on_timeout() -> String {
    "PAUSE_AND_ALERT".into()
}

pub fn parse(text: &str) -> AppResult<Recipe> {
    let recipe: Recipe = serde_yaml::from_str(text)
        .map_err(|error| AppError::BadRequest(format!("YAML inválido: {error}")))?;
    validate(&recipe)?;
    Ok(recipe)
}

pub fn validate(recipe: &Recipe) -> AppResult<()> {
    if recipe.id.trim().is_empty() {
        return Err(AppError::BadRequest("recipe.id é obrigatório".into()));
    }
    if recipe.name.trim().is_empty() {
        return Err(AppError::BadRequest("recipe.name é obrigatório".into()));
    }
    if recipe.steps.is_empty() {
        return Err(AppError::BadRequest(
            "a receita precisa de ao menos um step".into(),
        ));
    }
    let mut step_ids = std::collections::HashSet::new();
    for step in &recipe.steps {
        if step.id.trim().is_empty() || !step_ids.insert(step.id.clone()) {
            return Err(AppError::BadRequest(format!(
                "step id duplicado ou vazio: {}",
                step.id
            )));
        }
        if !matches!(step.mode.as_str(), "build" | "plan" | "audit") {
            return Err(AppError::BadRequest(format!(
                "modo de step inválido '{}'",
                step.mode
            )));
        }
        if step.prompt.trim().is_empty() {
            return Err(AppError::BadRequest(format!(
                "prompt vazio no step {}",
                step.id
            )));
        }
    }
    for checkpoint in &recipe.hitl_checkpoints {
        if !step_ids.contains(&checkpoint.after_step) {
            return Err(AppError::BadRequest(format!(
                "checkpoint {} aponta para step inexistente {}",
                checkpoint.id, checkpoint.after_step
            )));
        }
        if !matches!(
            checkpoint.interaction_type.as_str(),
            "BINARY_GATE"
                | "MULTI_CHOICE"
                | "APPROVAL_WITH_FEEDBACK"
                | "ARTIFACT_EDIT_HANDOFF"
                | "TOOL_PERMISSION_PROXY"
        ) {
            return Err(AppError::BadRequest(format!(
                "interaction_type inválido '{}'",
                checkpoint.interaction_type
            )));
        }
        if !matches!(
            checkpoint.on_timeout.as_str(),
            "PAUSE_AND_ALERT" | "AUTO_APPROVE" | "ABORT"
        ) {
            return Err(AppError::BadRequest(format!(
                "on_timeout inválido '{}'",
                checkpoint.on_timeout
            )));
        }
        if checkpoint.interaction_type == "MULTI_CHOICE" && checkpoint.options.is_empty() {
            return Err(AppError::BadRequest(format!(
                "checkpoint {} MULTI_CHOICE precisa de options",
                checkpoint.id
            )));
        }
    }
    Ok(())
}

/// Resolve {{...}} variables and fail closed on unknown names.
pub fn render(
    template: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> AppResult<String> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find("}}") else {
            output.push_str(&rest[start..]);
            return Ok(output);
        };
        let expression = rest[start + 2..start + 2 + end].trim();
        let value = resolve(expression, trigger, run, steps)
            .ok_or_else(|| AppError::BadRequest(format!("variável desconhecida: {expression}")))?;
        output.push_str(&value);
        rest = &rest[start + 2 + end + 2..];
    }
    output.push_str(rest);
    Ok(output)
}

fn resolve(
    expression: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> Option<String> {
    let (root, path) = expression
        .strip_prefix("trigger.")
        .map(|path| (trigger, path))
        .or_else(|| expression.strip_prefix("run.").map(|path| (run, path)))
        .or_else(|| expression.strip_prefix("steps.").map(|path| (steps, path)))?;
    let pointer = format!("/{}", path.replace('.', "/"));
    let value = root.pointer(&pointer)?;
    Some(match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
id: teste
name: Teste
trigger:
  type: FOLDER_WATCH
  config: { path: input/dropzone }
steps:
  - id: s1
    name: Step 1
    prompt: "Analise {{trigger.file_path}}"
hitl_checkpoints:
  - id: h1
    after_step: s1
    interaction_type: BINARY_GATE
    prompt_template: "Aprovar?"
sinks:
  - type: DOC_COMPILER
    config: { format: pdf }
"#;

    #[test]
    fn parses_and_validates_sample() {
        let recipe = parse(SAMPLE).expect("recipe");
        assert_eq!(recipe.steps.len(), 1);
        assert_eq!(recipe.trigger.trigger_type, "FOLDER_WATCH");
    }

    #[test]
    fn resolves_variables_and_rejects_unknown() {
        let rendered = render(
            "x {{trigger.file_path}} {{run.output_dir}} {{steps.s1.summary}}",
            &serde_json::json!({"file_path": "input/a.pdf"}),
            &serde_json::json!({"output_dir": "/tmp/out"}),
            &serde_json::json!({"s1": {"summary": "ok"}}),
        )
        .expect("render");
        assert_eq!(rendered, "x input/a.pdf /tmp/out ok");
        assert!(
            render(
                "{{trigger.missing}}",
                &serde_json::json!({}),
                &serde_json::json!({}),
                &serde_json::json!({})
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_bad_checkpoint() {
        let text = SAMPLE.replace("after_step: s1", "after_step: nope");
        assert!(parse(&text).is_err());
    }
}

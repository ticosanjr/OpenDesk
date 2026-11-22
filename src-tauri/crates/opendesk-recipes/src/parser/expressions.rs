use super::{FlowItem, HitlCheckpoint};
use crate::error::{AppError, AppResult};

/// Checkpoints use a generated stable id when the author leaves it empty.
/// Stable id/kind for durable recipe nodes/timeline.
pub fn node_descriptor(item: &FlowItem, index: usize) -> (String, &'static str) {
    match item {
        FlowItem::Step(step) => (step.id.clone(), "step"),
        FlowItem::Hitl(checkpoint) => (checkpoint_id(checkpoint), "hitl"),
        FlowItem::Sink(sink) => (
            sink.name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("sink-{index}")),
            "sink",
        ),
        FlowItem::Branch(branch) => (branch.id.clone(), "branch"),
        FlowItem::Parallel(parallel) => (parallel.id.clone(), "parallel"),
        FlowItem::Map(map) => (map.id.clone(), "map"),
        FlowItem::Recipe(call) => (call.id.clone(), "recipe"),
        FlowItem::Loop(loop_flow) => (loop_flow.id.clone(), "loop"),
        FlowItem::Switch(switch) => (switch.id.clone(), "switch"),
    }
}

pub fn checkpoint_id(checkpoint: &HitlCheckpoint) -> String {
    if !checkpoint.id.trim().is_empty() {
        checkpoint.id.clone()
    } else {
        format!("hitl-{}", checkpoint.interaction_type.to_lowercase())
    }
}

/// Extracts the last fenced JSON block (or the first object) from an agent
/// response and parses it.
pub fn extract_json(summary: &str) -> Option<serde_json::Value> {
    let mut fenced = None;
    let mut rest = summary;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let after = after.strip_prefix("json").unwrap_or(after);
        let Some(end) = after.find("```") else {
            break;
        };
        let candidate = after[..end].trim();
        if let Ok(value) = serde_json::from_str(candidate) {
            fenced = Some(value);
        }
        rest = &after[end + 3..];
    }
    if fenced.is_some() {
        return fenced;
    }
    let start = summary.find('{')?;
    let end = summary.rfind('}')?;
    serde_json::from_str(summary[start..=end].trim()).ok()
}

pub fn validate_output(schema: &serde_json::Value, summary: &str) -> AppResult<serde_json::Value> {
    let value = extract_json(summary)
        .ok_or_else(|| AppError::BadRequest("a saída do step não contém um JSON válido".into()))?;
    let validator = jsonschema::validator_for(schema)
        .map_err(|error| AppError::BadRequest(format!("schema inválido: {error}")))?;
    validator
        .validate(&value)
        .map_err(|error| AppError::BadRequest(format!("saída fora do schema: {error}")))?;
    Ok(value)
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

pub fn evaluate_when(
    expression: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> AppResult<bool> {
    let expression = expression.trim();
    if expression.is_empty() || expression.eq_ignore_ascii_case("true") {
        return Ok(true);
    }
    if expression.eq_ignore_ascii_case("false") {
        return Ok(false);
    }
    for operator in ["!=", "=="] {
        if let Some((left, right)) = expression.split_once(operator) {
            let left = resolve(left.trim(), trigger, run, steps).unwrap_or_default();
            let right = right
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            let equal = left.eq_ignore_ascii_case(&right);
            return Ok(if operator == "==" { equal } else { !equal });
        }
    }
    if let Some(path) = expression.strip_prefix("not ") {
        let value = resolve(path.trim(), trigger, run, steps).unwrap_or_default();
        return Ok(!truthy(&value));
    }
    let value = resolve(expression, trigger, run, steps).unwrap_or_default();
    Ok(truthy(&value))
}

fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_lowercase().as_str(),
        "" | "false" | "0" | "null" | "none"
    )
}

fn resolve(
    expression: &str,
    trigger: &serde_json::Value,
    run: &serde_json::Value,
    steps: &serde_json::Value,
) -> Option<String> {
    let (root, path) = if let Some(path) = expression.strip_prefix("trigger.") {
        (trigger, path)
    } else if let Some(path) = expression.strip_prefix("run.") {
        (run, path)
    } else if let Some(path) = expression.strip_prefix("steps.") {
        (steps, path)
    } else {
        // Inside map groups the item variable is also exposed at the top level
        // (`{{arquivo.path}}`) in addition to `{{steps.arquivo.path}}`.
        (steps, expression)
    };
    let pointer = format!("/{}", path.replace('.', "/"));
    let value = root.pointer(&pointer)?;
    Some(match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

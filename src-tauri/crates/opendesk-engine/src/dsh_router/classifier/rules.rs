use super::RouterInput;
use super::complexity::Complexity;
use ::opendesk_storage::repos::router::RouterRule;
use serde_json::Value;

pub(super) fn matches_rule(
    rule: &RouterRule,
    input: &RouterInput<'_>,
    complexity: &Complexity,
) -> bool {
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

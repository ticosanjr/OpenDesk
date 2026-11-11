use anyhow::{Result, anyhow};
use serde_json::Value;

pub(super) fn ws_path(path: &str, workspace_id: &str) -> String {
    format!("{path}?workspace_id={}", urlencoding::encode(workspace_id))
}

pub(super) fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    optional_str(args, key).ok_or_else(|| anyhow!("argumento '{key}' é obrigatório"))
}

pub(super) fn optional(args: &Value, key: &str) -> Option<String> {
    optional_str(args, key).map(str::to_string)
}

pub(super) fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn bool_arg(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

pub(super) fn integer_arg(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

pub(super) fn string_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn required_argument_is_reported() {
        let error = required(&json!({}), "id").unwrap_err();
        assert!(error.to_string().contains("id"));
    }
}

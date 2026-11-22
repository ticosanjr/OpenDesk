/// Replaces `{{name}}` placeholders. Unknown placeholders are left untouched so
/// the user can see the typo in the preview instead of silently losing text.
pub fn render(template: &str, values: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, value) in values {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }
    result.trim().to_string()
}

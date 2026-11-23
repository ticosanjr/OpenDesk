pub(super) fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

pub(super) fn snippet(value: &str, needle: &str) -> String {
    let lower = value.to_lowercase();
    let position = lower.find(&needle.to_lowercase()).unwrap_or(0);
    let start = position.saturating_sub(60);
    let end = (position + needle.len() + 100).min(value.len());
    let mut text = value
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<String>();
    text = text.replace('\n', " ");
    if start > 0 {
        text = format!("…{text}");
    }
    if end < value.len() {
        text.push('…');
    }
    text
}

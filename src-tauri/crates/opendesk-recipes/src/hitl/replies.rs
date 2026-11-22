use super::HitlResolution;
use opendesk_domain::hitl::HitlKind;

/// Channel-facing instructions for a checkpoint, in the same vocabulary the
/// Desktop UI uses.
pub fn reply_instructions(interaction_type: &str, options: &[String]) -> String {
    let kind = HitlKind::parse(interaction_type).unwrap_or(HitlKind::Text);
    match kind {
        HitlKind::Approval => {
            "Responda APROVAR ou REJEITAR. Para explicar, escreva APROVAR/REJEITAR seguido do payload."
                .to_string()
        }
        HitlKind::SingleChoice | HitlKind::MultiChoice => {
            let list = options
                .iter()
                .enumerate()
                .map(|(index, option)| format!("{}. {option}", index + 1))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "Responda com o número da opção (ou números separados por vírgula).\n{list}"
            )
        }
        HitlKind::Text => "Responda com o texto desejado na primeira linha.".to_string(),
    }
}

pub fn parse_reply(
    interaction_type: &str,
    options: &[String],
    text: &str,
) -> (String, Option<String>) {
    let clean = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('>'))
        .collect::<Vec<_>>()
        .join("\n");
    let lower = clean.to_lowercase();
    if matches!(
        lower.as_str(),
        "aprovar" | "aprovado" | "sim" | "ok" | "approve" | "yes" | "👍"
    ) {
        return ("APPROVED".into(), None);
    }
    if matches!(
        lower.as_str(),
        "rejeitar" | "rejeitado" | "nao" | "não" | "reject" | "no" | "👎"
    ) {
        return ("REJECTED".into(), Some(clean));
    }
    let kind = HitlKind::parse(interaction_type).unwrap_or(HitlKind::Text);
    if kind.requires_options() {
        let mut selected = Vec::new();
        for part in clean
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let Ok(index) = part.parse::<usize>() else {
                selected.clear();
                break;
            };
            if index == 0 || index > options.len() {
                selected.clear();
                break;
            }
            selected.push(index - 1);
        }
        if let Some(first) = selected.first() {
            let value = selected
                .iter()
                .map(|index| options[*index].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return (
                "CHOICE".into(),
                Some(
                    serde_json::json!({
                        "option_index": first,
                        "option_value": value,
                    })
                    .to_string(),
                ),
            );
        }
    }
    ("FEEDBACK".into(), Some(clean))
}

pub(super) fn parse_resolution(value: &str) -> HitlResolution {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
        if let Some(path) = json.get("path").and_then(|value| value.as_str()) {
            return HitlResolution::ArtifactEdited {
                path: path.to_string(),
            };
        }
        if let Some(text) = json.get("feedback_text").and_then(|value| value.as_str()) {
            return HitlResolution::FeedbackProvided {
                feedback_text: text.to_string(),
                attached_files: json
                    .get("attached_files")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            };
        }
        if let Some(index) = json.get("option_index").and_then(|value| value.as_u64()) {
            return HitlResolution::ChoiceSelected {
                option_index: index as usize,
                option_value: json
                    .get("option_value")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
            };
        }
    }
    match value {
        "REJECTED" => HitlResolution::Rejected { reason: None },
        "FEEDBACK" => HitlResolution::FeedbackProvided {
            feedback_text: String::new(),
            attached_files: vec![],
        },
        _ => HitlResolution::Approved,
    }
}

pub fn extract_token(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    let marker = upper.find("OD-#").or_else(|| upper.find("#"))?;
    let start = if upper[marker..].starts_with("OD-#") {
        marker + 4
    } else {
        marker + 1
    };
    let token: String = upper[start..]
        .chars()
        .take_while(|character| character.is_ascii_hexdigit())
        .take(8)
        .collect();
    (token.len() >= 4).then_some(token)
}

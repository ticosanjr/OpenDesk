use super::RouterInput;

#[derive(Debug, Clone)]
pub(super) struct Complexity {
    pub(super) label: &'static str,
    pub(super) score: u8,
}

pub(super) fn estimate_complexity(input: &RouterInput<'_>) -> Complexity {
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

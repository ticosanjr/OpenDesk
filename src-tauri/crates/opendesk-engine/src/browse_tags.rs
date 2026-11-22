//! Shared browse tags/policy helpers.
//!
//! Kept in the core so DSH prompt packing and the shell's browse integration
//! agree on tags, allowlists and risk classification.

pub const BROWSE_TAG: &str = "needs-browser";
pub const BROWSE_ALLOWLIST_PREFIX: &str = "browse-allowlist:";
pub const HIGH_RISK_TAGS: [&str; 5] = [
    "browse-high-risk",
    "browse-login",
    "browse-payment",
    "browse-banking",
    "browse-2fa",
];

pub fn needs_browser(title: &str, description: &str, tags: &[String]) -> bool {
    if tags.iter().any(|tag| {
        matches!(
            tag.trim().to_lowercase().as_str(),
            BROWSE_TAG | "browse" | "web-browser" | "navegador"
        )
    }) {
        return true;
    }
    let text = format!("{title} {description}").to_lowercase();
    if text.contains("http://") || text.contains("https://") || text.contains("www.") {
        return true;
    }
    [
        "navegue",
        "navegar",
        "navegador",
        "browser",
        "página web",
        "pagina web",
        "site oficial",
        "scraping",
        "pesquise na web",
        "pesquisar na web",
        "pesquisa online",
        "acesse o site",
        "abrir o site",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// Parses `browse-allowlist:example.com,sub.example.com` tags into normalized
/// hostnames. Invalid entries are ignored.
pub fn allowlist_from_tags(tags: &[String]) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for tag in tags {
        let lower = tag.trim().to_lowercase();
        let Some(raw) = lower.strip_prefix(BROWSE_ALLOWLIST_PREFIX) else {
            continue;
        };
        for item in raw.split(',') {
            let host = item
                .trim()
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .split(['/', '?'])
                .next()
                .unwrap_or("")
                .trim();
            let valid = host.contains('.')
                && !host.contains(' ')
                && !host.contains('@')
                && !host.contains('*')
                && host.split('.').all(|label| {
                    !label.is_empty()
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label
                            .chars()
                            .all(|value| value.is_ascii_alphanumeric() || value == '-')
                });
            if valid && !hosts.iter().any(|existing| existing == host) {
                hosts.push(host.to_string());
            }
        }
    }
    hosts
}

pub fn is_high_risk(tags: &[String]) -> bool {
    tags.iter().any(|tag| {
        let tag = tag.trim().to_lowercase();
        HIGH_RISK_TAGS.iter().any(|risk| risk == &tag)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_deduplicates_allowlists() {
        let tags = vec![
            "browse-allowlist:https://Example.com/path,example.com".into(),
            "browse-allowlist:docs.example.com".into(),
        ];
        assert_eq!(
            allowlist_from_tags(&tags),
            vec!["example.com".to_string(), "docs.example.com".to_string()]
        );
        assert!(allowlist_from_tags(&["needs-browser".into()]).is_empty());
    }

    #[test]
    fn detects_browser_intent_and_risk() {
        assert!(needs_browser("Pesquise", "abra https://example.com", &[]));
        assert!(needs_browser("Navegue", "no site oficial", &[]));
        assert!(!needs_browser("Relatório", "planilha local", &[]));
        assert!(is_high_risk(&["browse-login".into()]));
    }
}

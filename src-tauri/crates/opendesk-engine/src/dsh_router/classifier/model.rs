use crate::opencode_client::rest_client::ModelDto;
use serde_json::Value;

pub(super) fn prefer_model(available: &[ModelDto], candidates: &[&str], effort: &str) -> String {
    for candidate in candidates {
        if model_exists(available, candidate) {
            return with_variant(available, candidate, effort);
        }
    }
    if let Some(model) = available.iter().find(|model| {
        model.enabled.unwrap_or(true)
            && model
                .status
                .as_deref()
                .map(|status| status != "deprecated")
                .unwrap_or(true)
    }) {
        let key = model_key(model);
        return with_variant(available, &key, effort);
    }
    candidates.first().copied().unwrap_or_default().to_string()
}

pub(super) fn cheapest_model(
    available: &[ModelDto],
    default_model: Option<&str>,
) -> Option<String> {
    let preferred_provider = default_model
        .and_then(|model| model.split('#').next())
        .and_then(|model| model.split_once('/'))
        .map(|(provider, _)| provider);
    let same_provider = available
        .iter()
        .filter(|model| {
            preferred_provider.is_none_or(|provider| model.provider_id.as_deref() == Some(provider))
        })
        .collect::<Vec<_>>();
    let pool = if same_provider.is_empty() {
        available.iter().collect::<Vec<_>>()
    } else {
        same_provider
    };
    pool.into_iter()
        .filter(|model| model.enabled.unwrap_or(true))
        .min_by(|a, b| {
            let a_price = model_price(a);
            let b_price = model_price(b);
            a_price
                .partial_cmp(&b_price)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|model| with_variant(available, &model_key(model), "low"))
}

fn model_price(model: &ModelDto) -> f64 {
    model
        .cost
        .iter()
        .map(|cost| {
            let input = cost
                .get("input")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX);
            let output = cost
                .get("output")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX);
            input + output
        })
        .fold(f64::MAX, f64::min)
}

fn model_key(model: &ModelDto) -> String {
    let provider = model.provider_id.clone().unwrap_or_default();
    let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
    format!("{provider}/{id}")
}

pub fn model_available(available: &[ModelDto], candidate: &str) -> bool {
    model_exists(available, candidate)
}

pub(super) fn model_exists(available: &[ModelDto], candidate: &str) -> bool {
    let (provider, id) = split_model(candidate);
    available.iter().any(|model| {
        let model_provider = model.provider_id.as_deref().unwrap_or_default();
        let model_id = model.model_id.as_deref().unwrap_or(&model.id);
        (provider.is_empty() || provider == model_provider) && id == model_id
    })
}

pub(super) fn with_variant(available: &[ModelDto], model: &str, effort: &str) -> String {
    let model = model.trim();
    if let Some((base, variant)) = model.split_once('#') {
        let base = base.trim();
        let variant = variant.trim();
        return if variant.is_empty() {
            base.to_string()
        } else {
            format!("{base}#{variant}")
        };
    }
    let effort = effort.trim();
    if effort.is_empty() {
        return model.to_string();
    }
    let (provider, id) = split_model(model);
    let has_variant = available.iter().any(|candidate| {
        let candidate_provider = candidate.provider_id.as_deref().unwrap_or_default();
        let candidate_id = candidate.model_id.as_deref().unwrap_or(&candidate.id);
        (provider.is_empty() || provider == candidate_provider)
            && id == candidate_id
            && candidate
                .variants
                .iter()
                .any(|variant| variant.id == effort)
    });
    if has_variant {
        format!("{model}#{effort}")
    } else {
        model.to_string()
    }
}

pub(super) fn resolve_model(
    available: &[ModelDto],
    selected: &str,
    effort: &str,
) -> (String, Option<String>) {
    if available.is_empty() {
        return (selected.to_string(), None);
    }
    if model_exists(available, selected) {
        return (with_variant(available, selected, effort), None);
    }
    let fallback = prefer_model(available, &[selected], effort);
    let note = format!("modelo '{selected}' indisponível; usando '{fallback}'");
    (fallback, Some(note))
}

fn split_model(model: &str) -> (String, String) {
    let base = model.split('#').next().unwrap_or(model);
    match base.split_once('/') {
        Some((provider, id)) => (provider.to_string(), id.to_string()),
        None => (String::new(), base.to_string()),
    }
}

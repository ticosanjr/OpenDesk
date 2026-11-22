//! Best-effort token/cost estimation for CLI harnesses that do not emit usage.
//!
//! This is deliberately approximate: chars/4 is the fallback when no tokenizer
//! is available. Costs are marked as estimates by the caller/log and should be
//! treated as a budget signal, not billing truth.

/// Conservative approximation used by CLIs without native telemetry.
pub fn estimate_tokens(text: &str) -> i64 {
    let chars = text.chars().count();
    if chars == 0 {
        0
    } else {
        ((chars as f64) / 4.0).ceil() as i64
    }
}

/// USD per million tokens (input, output) for common families. `None` means
/// unknown; callers keep cost at zero and log the gap.
pub fn price_per_million(model: &str) -> Option<(f64, f64)> {
    let model = model.to_lowercase();
    if model.contains("ollama") || model.contains("lmstudio") || model.contains("local") {
        return Some((0.0, 0.0));
    }
    if model.contains("deepseek") || model.contains("flash") {
        return Some((0.14, 0.28));
    }
    if model.contains("glm") {
        return Some((0.60, 2.20));
    }
    if model.contains("claude") {
        if model.contains("opus") {
            return Some((15.0, 75.0));
        }
        if model.contains("haiku") {
            return Some((0.80, 4.0));
        }
        return Some((3.0, 15.0));
    }
    if model.contains("gpt-4") || model.contains("o3") || model.contains("o4") {
        return Some((2.50, 10.0));
    }
    if model.contains("gpt") {
        return Some((0.15, 0.60));
    }
    if model.contains("gemini") {
        if model.contains("pro") {
            return Some((1.25, 5.0));
        }
        return Some((0.075, 0.30));
    }
    None
}

pub fn estimate_cost(model: &str, input_tokens: i64, output_tokens: i64) -> f64 {
    let Some((input_price, output_price)) = price_per_million(model) else {
        return 0.0;
    };
    (input_tokens.max(0) as f64 / 1_000_000.0) * input_price
        + (output_tokens.max(0) as f64 / 1_000_000.0) * output_price
}

pub struct Estimate {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub priced: bool,
}

pub fn estimate_usage(model: &str, prompt: &str, output: &str) -> Estimate {
    let input_tokens = estimate_tokens(prompt);
    let output_tokens = estimate_tokens(output);
    let priced = price_per_million(model).is_some();
    Estimate {
        input_tokens,
        output_tokens,
        cost_usd: estimate_cost(model, input_tokens, output_tokens),
        priced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_tokens_and_costs() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        let estimate = estimate_usage("deepseek/deepseek-flash", "abcd", "abcdefgh");
        assert_eq!(estimate.input_tokens, 1);
        assert_eq!(estimate.output_tokens, 2);
        assert!(estimate.priced);
        assert!(estimate.cost_usd > 0.0);
    }

    #[test]
    fn local_models_are_free_and_unknown_models_are_unpriced() {
        assert_eq!(estimate_cost("ollama/llama3", 1_000_000, 1_000_000), 0.0);
        assert!(price_per_million("ollama/llama3").is_some());
        assert!(price_per_million("mystery/model").is_none());
        assert_eq!(estimate_cost("mystery/model", 100, 100), 0.0);
    }
}

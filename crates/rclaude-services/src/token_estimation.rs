//! Token estimation service.
//! Provides more accurate token counting than simple chars/4.

/// Estimate tokens for a string using a character-based heuristic.
/// Uses different ratios for code vs natural language.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    // Heuristic: code has more tokens per character than prose
    let is_code = text.contains("fn ")
        || text.contains("def ")
        || text.contains("function ")
        || text.contains("class ")
        || text.contains('{')
        || text.contains("import ");

    let chars_per_token = if is_code { 3.5 } else { 4.0 };
    (text.len() as f64 / chars_per_token).ceil() as usize
}

/// Estimate tokens for a JSON value (tool input/output).
pub fn estimate_json_tokens(value: &serde_json::Value) -> usize {
    estimate_tokens(&value.to_string())
}

/// Model context window sizes.
pub fn context_window_for_model(_model: &str) -> usize {
    // All current Claude models have 200K context windows
    200_000
}

/// Model max output tokens.
pub fn max_output_tokens_for_model(model: &str) -> u32 {
    if model.contains("opus") {
        32_000
    } else if model.contains("sonnet") {
        16_000
    } else if model.contains("haiku") {
        8_192
    } else {
        16_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_prose() {
        let tokens = estimate_tokens("Hello, how are you doing today?");
        assert!(tokens > 5 && tokens < 15);
    }

    #[test]
    fn test_estimate_tokens_code() {
        let tokens = estimate_tokens("fn main() { println!(\"hello\"); }");
        assert!(tokens > 5 && tokens < 20);
    }

    #[test]
    fn test_context_window() {
        assert_eq!(
            context_window_for_model("claude-sonnet-4-20250514"),
            200_000
        );
        assert_eq!(context_window_for_model("claude-opus-4-20250514"), 200_000);
    }
}

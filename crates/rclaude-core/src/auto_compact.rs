//! AutoCompact: automatic compaction trigger with circuit breaker.

use crate::message::Message;

/// Circuit breaker state for auto-compact failures.
#[derive(Debug)]
pub struct AutoCompactState {
    pub consecutive_failures: u32,
    pub max_failures: u32,
    pub enabled: bool,
    pub last_compact_tokens: usize,
}

impl Default for AutoCompactState {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            max_failures: 3,
            enabled: true,
            last_compact_tokens: 0,
        }
    }
}

impl AutoCompactState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful compaction.
    pub fn record_success(&mut self, tokens_after: usize) {
        self.consecutive_failures = 0;
        self.last_compact_tokens = tokens_after;
    }

    /// Record a failed compaction. Returns false if circuit breaker tripped.
    pub fn record_failure(&mut self) -> bool {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.max_failures {
            self.enabled = false;
            false
        } else {
            true
        }
    }

    /// Check if auto-compact is enabled (not tripped).
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Reset the circuit breaker.
    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.enabled = true;
    }
}

/// Token warning thresholds for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenWarningState {
    /// Under 60% — no warning.
    Normal,
    /// 60-80% — micro-compact territory.
    Warning,
    /// 80-95% — auto-compact should trigger.
    Error,
    /// >95% — blocking, must compact before next turn.
    Blocking,
}

/// Calculate the effective context window size (total - reserved output).
pub fn effective_context_window(context_window: usize, max_output_tokens: usize) -> usize {
    context_window.saturating_sub(max_output_tokens)
}

/// Calculate token warning state.
pub fn calculate_warning_state(
    current_tokens: usize,
    context_window: usize,
    max_output_tokens: usize,
) -> TokenWarningState {
    let effective = effective_context_window(context_window, max_output_tokens);
    let ratio = current_tokens as f64 / effective as f64;
    if ratio > 0.95 {
        TokenWarningState::Blocking
    } else if ratio > 0.80 {
        TokenWarningState::Error
    } else if ratio > 0.60 {
        TokenWarningState::Warning
    } else {
        TokenWarningState::Normal
    }
}

/// Determine if auto-compact should trigger.
/// Uses a buffer of 13K tokens below the context window.
pub fn should_auto_compact(
    messages: &[Message],
    context_window: usize,
    max_output_tokens: usize,
    state: &AutoCompactState,
) -> bool {
    if !state.is_enabled() {
        return false;
    }
    let effective = effective_context_window(context_window, max_output_tokens);
    let threshold = effective.saturating_sub(13_000); // 13K buffer
    let current = crate::context_window::estimate_conversation_tokens(messages);
    current > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker() {
        let mut state = AutoCompactState::new();
        assert!(state.is_enabled());
        assert!(state.record_failure());
        assert!(state.record_failure());
        assert!(!state.record_failure()); // 3rd failure trips
        assert!(!state.is_enabled());
    }

    #[test]
    fn test_circuit_breaker_reset() {
        let mut state = AutoCompactState::new();
        state.record_failure();
        state.record_failure();
        state.record_failure();
        assert!(!state.is_enabled());
        state.reset();
        assert!(state.is_enabled());
    }

    #[test]
    fn test_success_resets_failures() {
        let mut state = AutoCompactState::new();
        state.record_failure();
        state.record_failure();
        state.record_success(1000);
        assert_eq!(state.consecutive_failures, 0);
        assert!(state.is_enabled());
    }

    #[test]
    fn test_warning_state() {
        assert_eq!(
            calculate_warning_state(50_000, 200_000, 20_000),
            TokenWarningState::Normal
        );
        assert_eq!(
            calculate_warning_state(120_000, 200_000, 20_000),
            TokenWarningState::Warning
        );
        assert_eq!(
            calculate_warning_state(150_000, 200_000, 20_000),
            TokenWarningState::Error
        );
        assert_eq!(
            calculate_warning_state(175_000, 200_000, 20_000),
            TokenWarningState::Blocking
        );
    }

    #[test]
    fn test_effective_context_window() {
        assert_eq!(effective_context_window(200_000, 20_000), 180_000);
    }
}

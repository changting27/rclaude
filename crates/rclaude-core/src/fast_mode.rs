//! Fast mode for reduced-latency tool execution.
//! Toggles between fast (haiku) and normal (sonnet) models.

/// Fast mode state.
#[derive(Debug, Clone)]
pub struct FastModeState {
    pub enabled: bool,
    pub fast_model: String,
    pub normal_model: String,
    pub cooldown_until: Option<std::time::Instant>,
}

impl Default for FastModeState {
    fn default() -> Self {
        Self {
            enabled: false,
            fast_model: "claude-haiku-4-5-20251001".into(),
            normal_model: "claude-sonnet-4-20250514".into(),
            cooldown_until: None,
        }
    }
}

impl FastModeState {
    /// Get the current model based on fast mode state.
    pub fn current_model(&self) -> &str {
        if self.enabled && !self.is_in_cooldown() {
            &self.fast_model
        } else {
            &self.normal_model
        }
    }

    /// Toggle fast mode.
    pub fn toggle(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.enabled
    }

    /// Check if in cooldown (API rejected fast mode).
    pub fn is_in_cooldown(&self) -> bool {
        self.cooldown_until
            .is_some_and(|t| std::time::Instant::now() < t)
    }

    /// Trigger cooldown (e.g., after API rejection).
    pub fn trigger_cooldown(&mut self, duration: std::time::Duration) {
        self.cooldown_until = Some(std::time::Instant::now() + duration);
    }

    /// Clear cooldown.
    pub fn clear_cooldown(&mut self) {
        self.cooldown_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = FastModeState::default();
        assert!(!state.enabled);
        assert!(state.current_model().contains("sonnet"));
    }

    #[test]
    fn test_toggle() {
        let mut state = FastModeState::default();
        assert!(state.toggle());
        assert!(state.current_model().contains("haiku"));
        assert!(!state.toggle());
        assert!(state.current_model().contains("sonnet"));
    }

    #[test]
    fn test_cooldown() {
        let mut state = FastModeState::default();
        state.enabled = true;
        state.trigger_cooldown(std::time::Duration::from_secs(60));
        assert!(state.is_in_cooldown());
        assert!(state.current_model().contains("sonnet")); // falls back during cooldown
    }
}

//! Denial tracking for permission classifiers.

/// Denial tracking state.
#[derive(Debug, Clone, Default)]
pub struct DenialTrackingState {
    pub consecutive_denials: u32,
    pub total_denials: u32,
}

/// Limits before falling back to prompting.
pub const MAX_CONSECUTIVE_DENIALS: u32 = 3;
pub const MAX_TOTAL_DENIALS: u32 = 20;

impl DenialTrackingState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_denial(&mut self) {
        self.consecutive_denials += 1;
        self.total_denials += 1;
    }

    pub fn record_success(&mut self) {
        self.consecutive_denials = 0;
    }

    pub fn should_fallback_to_prompting(&self) -> bool {
        self.consecutive_denials >= MAX_CONSECUTIVE_DENIALS
            || self.total_denials >= MAX_TOTAL_DENIALS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = DenialTrackingState::new();
        assert!(!state.should_fallback_to_prompting());
    }

    #[test]
    fn test_consecutive_denials_trigger() {
        let mut state = DenialTrackingState::new();
        state.record_denial();
        state.record_denial();
        assert!(!state.should_fallback_to_prompting());
        state.record_denial();
        assert!(state.should_fallback_to_prompting());
    }

    #[test]
    fn test_success_resets_consecutive() {
        let mut state = DenialTrackingState::new();
        state.record_denial();
        state.record_denial();
        state.record_success();
        assert_eq!(state.consecutive_denials, 0);
        assert_eq!(state.total_denials, 2);
        assert!(!state.should_fallback_to_prompting());
    }

    #[test]
    fn test_total_denials_trigger() {
        let mut state = DenialTrackingState::new();
        for _ in 0..20 {
            state.record_denial();
            state.record_success(); // reset consecutive each time
        }
        assert!(state.should_fallback_to_prompting());
    }
}

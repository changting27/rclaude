//! Policy limits matching services/policyLimits/.
//! Enforces usage limits and rate limiting.

/// Usage limit configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PolicyLimits {
    /// Max cost per session in USD.
    #[serde(default = "default_max_cost")]
    pub max_session_cost_usd: f64,
    /// Max turns per session.
    #[serde(default = "default_max_turns")]
    pub max_session_turns: u64,
    /// Max output tokens per turn.
    #[serde(default = "default_max_output")]
    pub max_output_tokens: u32,
    /// Whether to enforce limits.
    #[serde(default = "default_enforce")]
    pub enforce: bool,
}

fn default_max_cost() -> f64 {
    10.0
}
fn default_max_turns() -> u64 {
    100
}
fn default_max_output() -> u32 {
    16384
}
fn default_enforce() -> bool {
    false
}

impl Default for PolicyLimits {
    fn default() -> Self {
        Self {
            max_session_cost_usd: default_max_cost(),
            max_session_turns: default_max_turns(),
            max_output_tokens: default_max_output(),
            enforce: default_enforce(),
        }
    }
}

/// Check result.
#[derive(Debug)]
pub enum LimitCheck {
    Ok,
    CostExceeded { current: f64, limit: f64 },
    TurnsExceeded { current: u64, limit: u64 },
}

impl PolicyLimits {
    /// Check if current usage exceeds limits.
    pub fn check(&self, cost: f64, turns: u64) -> LimitCheck {
        if !self.enforce {
            return LimitCheck::Ok;
        }
        if cost > self.max_session_cost_usd {
            return LimitCheck::CostExceeded {
                current: cost,
                limit: self.max_session_cost_usd,
            };
        }
        if turns > self.max_session_turns {
            return LimitCheck::TurnsExceeded {
                current: turns,
                limit: self.max_session_turns,
            };
        }
        LimitCheck::Ok
    }

    /// Load from settings.
    pub fn from_settings(settings: &serde_json::Value) -> Self {
        settings
            .get("policyLimits")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_not_enforced() {
        let limits = PolicyLimits::default();
        assert!(matches!(limits.check(100.0, 1000), LimitCheck::Ok));
    }

    #[test]
    fn test_cost_exceeded() {
        let limits = PolicyLimits {
            enforce: true,
            max_session_cost_usd: 5.0,
            ..Default::default()
        };
        assert!(matches!(
            limits.check(6.0, 1),
            LimitCheck::CostExceeded { .. }
        ));
    }

    #[test]
    fn test_turns_exceeded() {
        let limits = PolicyLimits {
            enforce: true,
            max_session_turns: 10,
            ..Default::default()
        };
        assert!(matches!(
            limits.check(0.0, 11),
            LimitCheck::TurnsExceeded { .. }
        ));
    }
}

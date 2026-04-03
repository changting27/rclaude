//! Cost tracking service.

use std::collections::HashMap;

/// Per-model token pricing (USD per million tokens).
#[derive(Debug, Clone)]
struct ModelPricing {
    input: f64,
    output: f64,
    cache_write_multiplier: f64,
    cache_read_multiplier: f64,
}

/// Get pricing for a model.
fn get_pricing(model: &str) -> ModelPricing {
    if model.contains("opus") {
        ModelPricing {
            input: 15.0,
            output: 75.0,
            cache_write_multiplier: 1.25,
            cache_read_multiplier: 0.1,
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input: 0.80,
            output: 4.0,
            cache_write_multiplier: 1.25,
            cache_read_multiplier: 0.1,
        }
    } else {
        // sonnet and default
        ModelPricing {
            input: 3.0,
            output: 15.0,
            cache_write_multiplier: 1.25,
            cache_read_multiplier: 0.1,
        }
    }
}

/// Usage stats for a single model.
#[derive(Debug, Clone, Default)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub api_calls: u64,
}

/// Aggregated cost tracker.
#[derive(Debug, Clone, Default)]
pub struct CostTracker {
    pub model_usage: HashMap<String, ModelUsage>,
    pub total_cost_usd: f64,
    pub total_api_duration_ms: u64,
    pub total_tool_duration_ms: u64,
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record usage for a model and compute cost.
    pub fn record_usage(
        &mut self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation: u64,
        cache_read: u64,
    ) -> f64 {
        let pricing = get_pricing(model);
        let cost = (input_tokens as f64 * pricing.input
            + output_tokens as f64 * pricing.output
            + cache_creation as f64 * pricing.input * pricing.cache_write_multiplier
            + cache_read as f64 * pricing.input * pricing.cache_read_multiplier)
            / 1_000_000.0;

        let usage = self.model_usage.entry(model.to_string()).or_default();
        usage.input_tokens += input_tokens;
        usage.output_tokens += output_tokens;
        usage.cache_creation_tokens += cache_creation;
        usage.cache_read_tokens += cache_read;
        usage.cost_usd += cost;
        usage.api_calls += 1;
        self.total_cost_usd += cost;

        cost
    }

    /// Record API call duration.
    pub fn record_api_duration(&mut self, duration_ms: u64) {
        self.total_api_duration_ms += duration_ms;
    }

    /// Format cost for display.
    pub fn format_cost(&self) -> String {
        if self.total_cost_usd < 0.01 {
            format!("${:.4}", self.total_cost_usd)
        } else {
            format!("${:.2}", self.total_cost_usd)
        }
    }

    /// Format a summary string.
    pub fn summary(&self) -> String {
        let total_input: u64 = self.model_usage.values().map(|u| u.input_tokens).sum();
        let total_output: u64 = self.model_usage.values().map(|u| u.output_tokens).sum();
        let total_calls: u64 = self.model_usage.values().map(|u| u.api_calls).sum();
        format!(
            "Cost: {} | Tokens: {}↓ {}↑ | API calls: {}",
            self.format_cost(),
            format_number(total_input),
            format_number(total_output),
            total_calls,
        )
    }
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_usage() {
        let mut tracker = CostTracker::new();
        let cost = tracker.record_usage("claude-sonnet-4-20250514", 1000, 500, 0, 0);
        assert!(cost > 0.0);
        assert!((cost - 0.0105).abs() < 0.001);
        assert_eq!(tracker.model_usage.len(), 1);
    }

    #[test]
    fn test_format_cost() {
        let mut tracker = CostTracker::new();
        tracker.record_usage("claude-sonnet-4-20250514", 1_000_000, 100_000, 0, 0);
        let formatted = tracker.format_cost();
        assert!(formatted.starts_with('$'));
    }

    #[test]
    fn test_summary() {
        let mut tracker = CostTracker::new();
        tracker.record_usage("claude-sonnet-4-20250514", 5000, 1000, 0, 0);
        let summary = tracker.summary();
        assert!(summary.contains("Cost:"));
        assert!(summary.contains("Tokens:"));
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(500), "500");
        assert_eq!(format_number(1500), "1.5K");
        assert_eq!(format_number(1_500_000), "1.5M");
    }
}

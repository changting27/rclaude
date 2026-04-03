use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::Config;
use crate::message::Message;
use crate::permissions::PermissionMode;

/// Token usage for a specific model.
#[derive(Debug, Clone, Default)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_cost_usd: f64,
}

/// Global application state.
#[derive(Debug)]
pub struct AppState {
    /// Current working directory.
    pub cwd: PathBuf,
    /// Original working directory at startup.
    pub original_cwd: PathBuf,
    /// Current session ID.
    pub session_id: String,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Current permission mode.
    pub permission_mode: PermissionMode,
    /// Token usage per model.
    pub model_usage: HashMap<String, ModelUsage>,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Total API call duration in milliseconds.
    pub total_api_duration_ms: u64,
    /// Current model name.
    pub model: String,
    /// Whether we're in a git repository.
    pub is_git: bool,
    /// Current git branch.
    pub git_branch: Option<String>,
    /// Default (main) git branch.
    pub git_default_branch: Option<String>,
    /// Application configuration.
    pub config: Config,
    /// Whether the session is non-interactive (e.g., piped input).
    pub is_non_interactive: bool,
    /// Tracked tasks (from TaskCreate/Update tools).
    pub tasks: Vec<crate::task::TaskState>,
    /// Next task sequence number.
    pub next_task_seq: u32,
}

impl AppState {
    pub fn new(cwd: PathBuf, config: Config) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        Self {
            original_cwd: cwd.clone(),
            cwd,
            session_id,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            model_usage: HashMap::new(),
            total_cost_usd: 0.0,
            total_api_duration_ms: 0,
            model: "claude-sonnet-4-20250514".to_string(),
            is_git: false,
            git_branch: None,
            git_default_branch: None,
            config,
            is_non_interactive: false,
            tasks: Vec::new(),
            next_task_seq: 1,
        }
    }

    /// Record token usage for a model and compute cost.
    pub fn record_usage(
        &mut self,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation: u64,
        cache_read: u64,
    ) {
        let usage = self.model_usage.entry(model.to_string()).or_default();
        usage.input_tokens += input_tokens;
        usage.output_tokens += output_tokens;
        usage.cache_creation_tokens += cache_creation;
        usage.cache_read_tokens += cache_read;

        // Calculate cost
        let (input_price, output_price) = model_pricing(model);
        let cost = (input_tokens as f64 * input_price
            + output_tokens as f64 * output_price
            + cache_creation as f64 * input_price * 1.25
            + cache_read as f64 * input_price * 0.1)
            / 1_000_000.0;
        usage.total_cost_usd += cost;
        self.total_cost_usd += cost;
    }
}

/// Per-million-token pricing (input, output) for known models.
fn model_pricing(model: &str) -> (f64, f64) {
    // Prices per million tokens
    if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else if model.contains("haiku") {
        (0.25, 1.25)
    } else {
        // Default to sonnet pricing
        (3.0, 15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_record_usage_calculates_cost() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"), Config::default());
        state.record_usage("claude-sonnet-4-20250514", 1000, 500, 0, 0);

        assert!(state.total_cost_usd > 0.0);
        // 1000 * 3.0 / 1M + 500 * 15.0 / 1M = 0.003 + 0.0075 = 0.0105
        assert!((state.total_cost_usd - 0.0105).abs() < 0.001);
    }

    #[test]
    fn test_model_pricing() {
        let (i, o) = model_pricing("claude-opus-4-20250514");
        assert_eq!(i, 15.0);
        assert_eq!(o, 75.0);

        let (i, o) = model_pricing("claude-haiku-4-5-20251001");
        assert_eq!(i, 0.25);
        assert_eq!(o, 1.25);
    }
}

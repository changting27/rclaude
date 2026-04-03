//! Usage statistics tracking and reporting.

use std::collections::HashMap;

/// Session statistics.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SessionStats {
    pub session_id: String,
    pub start_time: String,
    pub duration_secs: u64,
    pub total_turns: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub tools_used: HashMap<String, u64>,
    pub files_read: u64,
    pub files_written: u64,
    pub files_edited: u64,
    pub commands_run: u64,
}

impl SessionStats {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            start_time: chrono::Utc::now().to_rfc3339(),
            ..Default::default()
        }
    }

    /// Record a tool use.
    pub fn record_tool_use(&mut self, tool_name: &str) {
        *self.tools_used.entry(tool_name.to_string()).or_default() += 1;
        match tool_name {
            "Read" => self.files_read += 1,
            "Write" => self.files_written += 1,
            "Edit" => self.files_edited += 1,
            "Bash" | "PowerShell" => self.commands_run += 1,
            _ => {}
        }
    }

    /// Record API usage.
    pub fn record_api_usage(&mut self, input_tokens: u64, output_tokens: u64, cost: f64) {
        self.total_input_tokens += input_tokens;
        self.total_output_tokens += output_tokens;
        self.total_cost_usd += cost;
        self.total_turns += 1;
    }

    /// Format stats for display.
    pub fn format(&self) -> String {
        let mut lines = vec![
            format!(
                "Session: {}",
                &self.session_id[..8.min(self.session_id.len())]
            ),
            format!("Duration: {}s", self.duration_secs),
            format!("Turns: {}", self.total_turns),
            format!(
                "Tokens: {}↓ {}↑",
                self.total_input_tokens, self.total_output_tokens
            ),
            format!("Cost: ${:.4}", self.total_cost_usd),
        ];
        if self.files_read > 0 {
            lines.push(format!("Files read: {}", self.files_read));
        }
        if self.files_written > 0 {
            lines.push(format!("Files written: {}", self.files_written));
        }
        if self.files_edited > 0 {
            lines.push(format!("Files edited: {}", self.files_edited));
        }
        if self.commands_run > 0 {
            lines.push(format!("Commands: {}", self.commands_run));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_tool_use() {
        let mut stats = SessionStats::new("test-session");
        stats.record_tool_use("Read");
        stats.record_tool_use("Read");
        stats.record_tool_use("Bash");
        assert_eq!(stats.files_read, 2);
        assert_eq!(stats.commands_run, 1);
        assert_eq!(*stats.tools_used.get("Read").unwrap(), 2);
    }

    #[test]
    fn test_format() {
        let mut stats = SessionStats::new("abc12345");
        stats.total_turns = 5;
        stats.total_cost_usd = 0.0123;
        let output = stats.format();
        assert!(output.contains("abc12345"));
        assert!(output.contains("5"));
    }
}

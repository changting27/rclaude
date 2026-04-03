//! /cost — Show token usage and estimated costs with detail.

use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct CostCommand;

#[async_trait]
impl Command for CostCommand {
    fn name(&self) -> &str {
        "cost"
    }

    fn description(&self) -> &str {
        "Show token usage and estimated costs"
    }

    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let mut output = String::new();
        output.push_str(&format!(
            "{} ${:.6}\n\n",
            "Total cost:".bold(),
            state.total_cost_usd
        ));

        if state.model_usage.is_empty() {
            output.push_str("  No API calls yet.\n");
        } else {
            // U09: Table format with per-model cost
            output.push_str(&format!(
                "  {:<30} {:>10} {:>10} {:>10} {:>10}\n",
                "Model", "Input", "Output", "Cache", "Cost"
            ));
            output.push_str(&format!("  {}\n", "─".repeat(74)));

            for (model, usage) in &state.model_usage {
                let model_short = if model.len() > 28 {
                    format!("{}…", &model[..27])
                } else {
                    model.clone()
                };
                let cache_rate = if usage.input_tokens > 0 {
                    (usage.cache_read_tokens as f64 / usage.input_tokens as f64 * 100.0) as u32
                } else {
                    0
                };
                let cache_str = if usage.cache_read_tokens > 0 {
                    format!("{} ({}%)", fmt_tok(usage.cache_read_tokens), cache_rate)
                } else {
                    "—".into()
                };
                output.push_str(&format!(
                    "  {:<30} {:>10} {:>10} {:>10} {:>10}\n",
                    model_short.cyan(),
                    fmt_tok(usage.input_tokens),
                    fmt_tok(usage.output_tokens),
                    cache_str,
                    format!("${:.4}", usage.total_cost_usd),
                ));
            }
        }

        output.push_str(&format!(
            "\n  {} {} messages, ~{} tokens",
            "Session:".bold(),
            state.messages.len(),
            rclaude_core::context_window::estimate_conversation_tokens(&state.messages),
        ));

        if state.total_api_duration_ms > 0 {
            output.push_str(&format!(
                "\n  {} {:.1}s API time",
                "Duration:".bold(),
                state.total_api_duration_ms as f64 / 1000.0
            ));
        }

        Ok(CommandResult::Ok(Some(output)))
    }
}

fn fmt_tok(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

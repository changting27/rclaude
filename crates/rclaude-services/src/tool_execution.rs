//! Tool execution service with hooks, analytics, and error handling.

use rclaude_core::hooks::{HookEvent, HookRegistry};
use rclaude_core::tool::{Tool, ToolResult, ToolUseContext};
use std::collections::HashMap;
use std::time::Instant;

/// Tool execution statistics.
#[derive(Debug, Default)]
pub struct ToolStats {
    pub total_calls: u64,
    pub total_duration_ms: u64,
    pub calls_by_tool: HashMap<String, u64>,
    pub errors_by_tool: HashMap<String, u64>,
}

/// Execute a tool with pre/post hooks and timing.
pub async fn execute_with_hooks(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tools: &[Box<dyn Tool>],
    ctx: &ToolUseContext,
    hooks: &mut HookRegistry,
    stats: &mut ToolStats,
    verbose: bool,
) -> ToolResult {
    let start = Instant::now();

    // Pre-tool hook
    let env = HashMap::from([
        ("TOOL_NAME".into(), tool_name.to_string()),
        ("TOOL_INPUT".into(), tool_input.to_string()),
    ]);
    let _ = hooks.run(HookEvent::PreToolUse, &ctx.cwd, &env).await;

    // Execute
    let result =
        rclaude_core::query::execute_tool(tool_name, tool_input, tools, ctx, verbose).await;

    let duration = start.elapsed();

    // Post-tool hook
    let env = HashMap::from([
        ("TOOL_NAME".into(), tool_name.to_string()),
        (
            "TOOL_RESULT".into(),
            if result.is_error { "error" } else { "success" }.to_string(),
        ),
        ("TOOL_DURATION_MS".into(), duration.as_millis().to_string()),
    ]);
    let _ = hooks.run(HookEvent::PostToolUse, &ctx.cwd, &env).await;

    // Update stats
    stats.total_calls += 1;
    stats.total_duration_ms += duration.as_millis() as u64;
    *stats
        .calls_by_tool
        .entry(tool_name.to_string())
        .or_default() += 1;
    if result.is_error {
        *stats
            .errors_by_tool
            .entry(tool_name.to_string())
            .or_default() += 1;
    }

    if verbose {
        tracing::info!(
            "Tool {} completed in {}ms ({})",
            tool_name,
            duration.as_millis(),
            if result.is_error { "error" } else { "ok" }
        );
    }

    result
}

/// Format tool stats for display.
pub fn format_stats(stats: &ToolStats) -> String {
    let mut lines = vec![format!(
        "Tool calls: {} total, {}ms total duration",
        stats.total_calls, stats.total_duration_ms
    )];
    let mut sorted: Vec<_> = stats.calls_by_tool.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (name, count) in sorted.iter().take(10) {
        let errors = stats.errors_by_tool.get(*name).unwrap_or(&0);
        lines.push(format!("  {name}: {count} calls, {errors} errors"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_stats() {
        let mut stats = ToolStats::default();
        stats.total_calls = 5;
        stats.total_duration_ms = 1234;
        stats.calls_by_tool.insert("Bash".into(), 3);
        stats.errors_by_tool.insert("Bash".into(), 1);
        let output = format_stats(&stats);
        assert!(output.contains("5 total"));
        assert!(output.contains("Bash: 3 calls, 1 errors"));
    }
}

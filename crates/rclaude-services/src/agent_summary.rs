//! Agent summary matching services/AgentSummary/.
//! Generates summaries of agent execution results.

/// Generate a summary of an agent's work.
pub fn summarize_agent_output(agent_type: &str, output: &str, duration_ms: u64) -> String {
    let duration = if duration_ms < 1000 {
        format!("{duration_ms}ms")
    } else {
        format!("{:.1}s", duration_ms as f64 / 1000.0)
    };

    // Extract key findings from output
    let lines: Vec<&str> = output.lines().collect();
    let total_lines = lines.len();

    // Get first meaningful line as headline
    let headline = lines
        .iter()
        .find(|l| !l.trim().is_empty() && !l.starts_with('['))
        .map(|l| {
            let trimmed = l.trim();
            if trimmed.len() > 100 {
                &trimmed[..100]
            } else {
                trimmed
            }
        })
        .unwrap_or("(no output)");

    format!(
        "Agent '{agent_type}' completed in {duration} ({total_lines} lines)\n\
         Summary: {headline}"
    )
}

/// Generate a tool use summary for display.
pub fn summarize_tool_uses(tool_uses: &[(String, bool)]) -> String {
    if tool_uses.is_empty() {
        return "No tools used".into();
    }

    let total = tool_uses.len();
    let errors = tool_uses.iter().filter(|(_, err)| *err).count();
    let tools: std::collections::HashMap<&str, usize> =
        tool_uses
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, (name, _)| {
                *acc.entry(name.as_str()).or_default() += 1;
                acc
            });

    let mut sorted: Vec<_> = tools.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let tool_list: Vec<String> = sorted
        .iter()
        .take(5)
        .map(|(name, count)| format!("{name}×{count}"))
        .collect();

    format!(
        "{total} tool calls ({errors} errors): {}",
        tool_list.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_agent() {
        let summary = summarize_agent_output(
            "Explore",
            "Found 5 files matching pattern\nfile1.rs\nfile2.rs",
            1500,
        );
        assert!(summary.contains("Explore"));
        assert!(summary.contains("1.5s"));
        assert!(summary.contains("Found 5 files"));
    }

    #[test]
    fn test_summarize_tools() {
        let uses = vec![
            ("Read".into(), false),
            ("Read".into(), false),
            ("Grep".into(), false),
            ("Bash".into(), true),
        ];
        let summary = summarize_tool_uses(&uses);
        assert!(summary.contains("4 tool calls"));
        assert!(summary.contains("1 errors"));
        assert!(summary.contains("Read×2"));
    }
}

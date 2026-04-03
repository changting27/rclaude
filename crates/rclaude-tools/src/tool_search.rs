//! ToolSearchTool: keyword search over tools with scoring.
//! Features:
//! - select:<name> direct selection (comma-separated multi-select)
//! - MCP tool prefix matching (mcp__server__)
//! - Keyword scoring: name parts, description, CamelCase splitting
//! - Required terms (+term) and optional terms

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct ToolSearchTool;

/// Parse a tool name into searchable parts.
/// Handles MCP tools (mcp__server__action) and CamelCase (FileReadTool).
fn parse_tool_name(name: &str) -> (Vec<String>, bool) {
    if name.starts_with("mcp__") {
        let without_prefix = name.strip_prefix("mcp__").unwrap_or(name).to_lowercase();
        let parts: Vec<String> = without_prefix
            .split("__")
            .flat_map(|p| p.split('_'))
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect();
        (parts, true)
    } else {
        // CamelCase split + underscore split
        let mut parts = Vec::new();
        let mut current = String::new();
        for ch in name.chars() {
            if ch.is_uppercase() && !current.is_empty() {
                parts.push(current.to_lowercase());
                current.clear();
            }
            if ch == '_' {
                if !current.is_empty() {
                    parts.push(current.to_lowercase());
                    current.clear();
                }
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            parts.push(current.to_lowercase());
        }
        (parts, false)
    }
}

/// Score a tool against search terms.
fn score_tool(name: &str, description: &str, terms: &[String]) -> i32 {
    let (parts, is_mcp) = parse_tool_name(name);
    let desc_lower = description.to_lowercase();
    let name_lower = name.to_lowercase();
    let mut score = 0;

    for term in terms {
        // Exact part match (high weight for MCP server names)
        if parts.iter().any(|p| p == term) {
            score += if is_mcp { 12 } else { 10 };
        } else if parts.iter().any(|p| p.contains(term.as_str())) {
            score += if is_mcp { 6 } else { 5 };
        }

        // Full name contains term
        if name_lower.contains(term.as_str()) && score == 0 {
            score += 3;
        }

        // Description word-boundary match
        if desc_lower.contains(term.as_str()) {
            score += 2;
        }
    }
    score
}

#[async_trait]
impl Tool for ToolSearchTool {
    fn name(&self) -> &str {
        "ToolSearch"
    }

    fn description(&self) -> &str {
        "Search for available tools by keyword or capability. \
         Use 'select:<tool_name>' for direct selection (comma-separated for multiple). \
         Use keywords to search by name or description."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Query: 'select:<name>' for direct selection, or keywords to search"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results (default: 5)"
                }
            },
            "required": ["query"]
        })).expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;
        let all_tools = crate::get_all_tools();

        // 1. select: prefix — direct tool selection (matching original)
        if let Some(names_str) = query
            .strip_prefix("select:")
            .or_else(|| query.strip_prefix("SELECT:"))
        {
            let requested: Vec<&str> = names_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            let mut found = Vec::new();
            let mut missing = Vec::new();

            for name in &requested {
                if let Some(tool) = all_tools
                    .iter()
                    .find(|t| t.name().eq_ignore_ascii_case(name))
                {
                    if !found.iter().any(|f: &String| f == tool.name()) {
                        found.push(tool.name().to_string());
                    }
                } else {
                    missing.push(name.to_string());
                }
            }

            if found.is_empty() {
                return Ok(ToolResult::text(format!(
                    "No matching tools found for: {}",
                    missing.join(", ")
                )));
            }

            let mut result = format!("Selected {} tool(s):\n", found.len());
            for name in &found {
                if let Some(tool) = all_tools.iter().find(|t| t.name() == name) {
                    result.push_str(&format!(
                        "- {}: {}\n",
                        tool.name(),
                        &tool.description()[..tool.description().len().min(100)]
                    ));
                }
            }
            if !missing.is_empty() {
                result.push_str(&format!("\nNot found: {}", missing.join(", ")));
            }
            return Ok(ToolResult::text(result));
        }

        let query_lower = query.to_lowercase().trim().to_string();

        // 2. Exact name match (fast path)
        if let Some(tool) = all_tools
            .iter()
            .find(|t| t.name().to_lowercase() == query_lower)
        {
            return Ok(ToolResult::text(format!(
                "Exact match:\n- {}: {}",
                tool.name(),
                tool.description()
            )));
        }

        // 3. MCP prefix match
        if query_lower.starts_with("mcp__") && query_lower.len() > 5 {
            let prefix_matches: Vec<String> = all_tools
                .iter()
                .filter(|t| t.name().to_lowercase().starts_with(&query_lower))
                .take(max_results)
                .map(|t| {
                    format!(
                        "- {}: {}",
                        t.name(),
                        &t.description()[..t.description().len().min(80)]
                    )
                })
                .collect();
            if !prefix_matches.is_empty() {
                return Ok(ToolResult::text(format!(
                    "MCP prefix matches ({}):\n{}",
                    prefix_matches.len(),
                    prefix_matches.join("\n")
                )));
            }
        }

        // 4. Keyword search with scoring (matching original's searchToolsWithKeywords)
        let terms: Vec<String> = query_lower
            .split_whitespace()
            .filter(|t| !t.is_empty())
            .map(|t| {
                // Strip + prefix for required terms (still used for scoring)
                t.strip_prefix('+').unwrap_or(t).to_string()
            })
            .collect();

        // Required terms (prefixed with +)
        let required_terms: Vec<String> = query_lower
            .split_whitespace()
            .filter(|t| t.starts_with('+') && t.len() > 1)
            .map(|t| t[1..].to_string())
            .collect();

        let mut scored: Vec<(String, String, i32)> = all_tools
            .iter()
            .map(|t| {
                let s = score_tool(t.name(), t.description(), &terms);
                (
                    t.name().to_string(),
                    t.description()[..t.description().len().min(100)].to_string(),
                    s,
                )
            })
            .filter(|(name, desc, score)| {
                if *score == 0 {
                    return false;
                }
                // If required terms exist, all must match in name or description
                if !required_terms.is_empty() {
                    let name_lower = name.to_lowercase();
                    let desc_lower = desc.to_lowercase();
                    required_terms.iter().all(|rt| {
                        name_lower.contains(rt.as_str()) || desc_lower.contains(rt.as_str())
                    })
                } else {
                    true
                }
            })
            .collect();

        scored.sort_by(|a, b| b.2.cmp(&a.2));
        scored.truncate(max_results);

        if scored.is_empty() {
            return Ok(ToolResult::text(format!(
                "No tools matching '{}'. {} tools available.",
                query,
                all_tools.len()
            )));
        }

        let result_lines: Vec<String> = scored
            .iter()
            .map(|(name, desc, score)| format!("- {} (score: {}): {}", name, score, desc))
            .collect();

        Ok(ToolResult::text(format!(
            "Found {} match(es) for '{}':\n{}",
            scored.len(),
            query,
            result_lines.join("\n")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_name_camel() {
        let (parts, is_mcp) = parse_tool_name("FileReadTool");
        assert!(!is_mcp);
        assert!(parts.contains(&"file".to_string()));
        assert!(parts.contains(&"read".to_string()));
    }

    #[test]
    fn test_parse_tool_name_mcp() {
        let (parts, is_mcp) = parse_tool_name("mcp__slack__send_message");
        assert!(is_mcp);
        assert!(parts.contains(&"slack".to_string()));
        assert!(parts.contains(&"send".to_string()));
        assert!(parts.contains(&"message".to_string()));
    }

    #[test]
    fn test_score_tool() {
        // Exact part match
        assert!(score_tool("FileRead", "Read files from disk", &["file".into()]) > 0);
        // Description match
        assert!(score_tool("Bash", "Execute shell commands", &["shell".into()]) > 0);
        // No match
        assert_eq!(
            score_tool("Bash", "Execute commands", &["nonexistent".into()]),
            0
        );
    }

    #[test]
    fn test_score_mcp_tool() {
        let score = score_tool(
            "mcp__slack__send_message",
            "Send a Slack message",
            &["slack".into()],
        );
        assert!(score >= 12); // MCP exact part match = 12
    }

    #[test]
    fn test_score_multiple_terms() {
        let s1 = score_tool("FileRead", "Read files", &["file".into(), "read".into()]);
        let s2 = score_tool("FileRead", "Read files", &["file".into()]);
        assert!(s1 > s2); // More matching terms = higher score
    }
}

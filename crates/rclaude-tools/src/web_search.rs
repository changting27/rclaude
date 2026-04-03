//! WebSearchTool: web search via Anthropic's built-in web_search tool.
//!
//! The tool sends a sub-request to the API with web_search enabled,
//! then extracts and returns the search results.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DESCRIPTION: &str = "Search the web for real-time information.\n\n\
Uses the Anthropic API's built-in web search capability to find current information.\n\
Returns search results with titles, URLs, and relevant snippets.";

#[allow(dead_code)]
const SYSTEM_PROMPT: &str = "You are a web search assistant. \
Search the web for the user's query and return the most relevant results. \
Be concise and factual. Include source URLs.";

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exclude results from these domains"
                }
            },
            "required": ["query"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing query".into()))?;

        let allowed_domains: Vec<String> = input
            .get("allowed_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let blocked_domains: Vec<String> = input
            .get("blocked_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Get API key from context or environment
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| RclaudeError::Tool("ANTHROPIC_API_KEY not set for web search".into()))?;

        let start = std::time::Instant::now();

        // Build the API request with web_search tool enabled
        // (matching the original's approach of using the API's built-in web search)
        let mut web_search_tool = json!({
            "type": "web_search_20250305",
            "name": "web_search",
        });
        if !allowed_domains.is_empty() {
            web_search_tool["allowed_domains"] = json!(allowed_domains);
        }
        if !blocked_domains.is_empty() {
            web_search_tool["blocked_domains"] = json!(blocked_domains);
        }

        let model = std::env::var("CLAUDE_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        let request_body = json!({
            "model": model,
            "max_tokens": 4096,
            "tools": [web_search_tool],
            "messages": [{
                "role": "user",
                "content": query,
            }],
        });

        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{base_url}/v1/messages"))
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "web-search-2025-03-05")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| RclaudeError::Tool(format!("Web search request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Ok(ToolResult::error(format!(
                "Web search API error (HTTP {status}): {body}"
            )));
        }

        let response: Value = resp
            .json()
            .await
            .map_err(|e| RclaudeError::Tool(format!("Failed to parse response: {e}")))?;

        let duration = start.elapsed().as_secs_f64();

        // Extract search results and text from the response
        let content = response.get("content").and_then(|v| v.as_array());
        let mut results = Vec::new();
        let mut text_parts = Vec::new();

        if let Some(blocks) = content {
            for block in blocks {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "web_search_tool_result" => {
                        if let Some(search_content) =
                            block.get("content").and_then(|v| v.as_array())
                        {
                            for item in search_content {
                                if item.get("type").and_then(|v| v.as_str())
                                    == Some("web_search_result")
                                {
                                    let title =
                                        item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                                    let url =
                                        item.get("url").and_then(|v| v.as_str()).unwrap_or("");
                                    let snippet = item
                                        .get("page_snippet")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    results.push(format!("- [{title}]({url})\n  {snippet}"));
                                }
                            }
                        }
                    }
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut output = format!("Web search for: \"{query}\" ({duration:.1}s)\n\n");

        if !results.is_empty() {
            output.push_str(&format!("Search results ({}):\n", results.len()));
            output.push_str(&results.join("\n\n"));
            output.push('\n');
        }

        if !text_parts.is_empty() {
            output.push_str("\nSummary:\n");
            output.push_str(&text_parts.join("\n"));
        }

        if results.is_empty() && text_parts.is_empty() {
            output.push_str("No results found.");
        }

        Ok(ToolResult::text(output))
    }
}

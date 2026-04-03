use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const MAX_BODY_BYTES: usize = 500_000;

const DESCRIPTION: &str = "Fetches content from a specified URL and returns it as text.\n\
- Takes a URL and a prompt as input\n\
- Fetches the URL content, converts HTML to text\n\
- The URL must be a fully-formed valid URL\n\
- HTTP URLs will be automatically upgraded to HTTPS\n\
- This tool is read-only and does not modify any files";

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from",
                    "format": "uri"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt to describe what information to extract"
                }
            },
            "required": ["url", "prompt"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: url".into()))?;

        let _prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Extract the main content");

        // Upgrade http to https
        let url = if url.starts_with("http://") {
            url.replacen("http://", "https://", 1)
        } else {
            url.to_string()
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| RclaudeError::Tool(format!("Failed to create HTTP client: {e}")))?;

        let resp = client
            .get(&url)
            .header("User-Agent", "rclaude/0.1")
            .send()
            .await
            .map_err(|e| RclaudeError::Tool(format!("Failed to fetch URL: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Ok(ToolResult::error(format!("HTTP {status} fetching {url}")));
        }

        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| RclaudeError::Tool(format!("Failed to read response body: {e}")))?;

        if bytes.len() > MAX_BODY_BYTES {
            let text = String::from_utf8_lossy(&bytes[..MAX_BODY_BYTES]);
            return Ok(ToolResult::text(format!(
                "{text}\n\n... (truncated, {total} bytes total)",
                total = bytes.len()
            )));
        }

        let text = String::from_utf8_lossy(&bytes);

        // Simple HTML tag stripping (basic — a real impl would use a proper HTML parser)
        let cleaned = if content_type.contains("html") {
            strip_html_tags(&text)
        } else {
            text.to_string()
        };

        Ok(ToolResult::text(cleaned))
    }
}

/// HTML to text conversion. Strips tags, removes script/style content,
/// decodes common entities, and collapses whitespace.
fn strip_html_tags(html: &str) -> String {
    // Remove script and style blocks entirely
    let re_script = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
    let re_style = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
    let re_comment = regex::Regex::new(r"(?s)<!--.*?-->").unwrap();
    let cleaned = re_script.replace_all(html, "");
    let cleaned = re_style.replace_all(&cleaned, "");
    let cleaned = re_comment.replace_all(&cleaned, "");

    // Add newlines for block elements
    let re_block = regex::Regex::new(r"(?i)</(p|div|h[1-6]|li|tr|br|hr)[^>]*>").unwrap();
    let cleaned = re_block.replace_all(&cleaned, "\n");
    let re_br = regex::Regex::new(r"(?i)<br\s*/?>").unwrap();
    let cleaned = re_br.replace_all(&cleaned, "\n");

    // Strip remaining tags
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&cleaned, "");

    // Decode common HTML entities
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse whitespace
    let mut prev_was_newline = false;
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_was_newline {
                result.push('\n');
                prev_was_newline = true;
            }
        } else {
            result.push_str(trimmed);
            result.push('\n');
            prev_was_newline = false;
        }
    }

    result.trim().to_string()
}

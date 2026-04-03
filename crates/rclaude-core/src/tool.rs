use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::Result;
use crate::permissions::PermissionMode;

/// JSON Schema for tool input parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(default)]
    pub properties: HashMap<String, Value>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Result returned by a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// The output content blocks.
    pub content: Vec<ToolResultContent>,
    /// Whether this result indicates an error.
    #[serde(default)]
    pub is_error: bool,
}

/// A single content block in a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: text.into() }],
            is_error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text {
                text: message.into(),
            }],
            is_error: true,
        }
    }
}

/// Shared state handle for tools that need to read/write app state.
pub type SharedAppState = std::sync::Arc<tokio::sync::RwLock<crate::state::AppState>>;

/// Context passed to tool execution.
pub struct ToolUseContext {
    pub cwd: std::path::PathBuf,
    pub permission_mode: PermissionMode,
    pub debug: bool,
    pub verbose: bool,
    pub abort_signal: tokio::sync::watch::Receiver<bool>,
    /// Shared access to AppState (for TaskTools, etc.)
    pub app_state: Option<SharedAppState>,
}

/// Trait that all tools must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name (e.g., "Bash", "Read").
    fn name(&self) -> &str;

    /// Human-readable display name.
    fn display_name(&self) -> &str {
        self.name()
    }

    /// Tool description for the LLM.
    fn description(&self) -> &str;

    /// JSON schema for input parameters.
    fn input_schema(&self) -> ToolInputSchema;

    /// Execute the tool with the given input.
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult>;

    /// Whether this tool is available in the current context.
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        true
    }

    /// Whether this tool can safely execute in parallel with other concurrency-safe tools.
    /// Read-only tools return true; tools that modify state (Bash, Write, Edit) return false.
    fn is_concurrency_safe(&self) -> bool {
        false
    }
}

/// Type-erased tool collection.
pub type Tools = Vec<Box<dyn Tool>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_text() {
        let r = ToolResult::text("hello");
        assert!(!r.is_error);
        assert_eq!(r.content.len(), 1);
    }

    #[test]
    fn test_tool_result_error() {
        let r = ToolResult::error("oops");
        assert!(r.is_error);
    }

    #[test]
    fn test_tool_input_schema_deserialize() {
        let schema: ToolInputSchema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "required": ["name"]
        }))
        .unwrap();
        assert_eq!(schema.schema_type, "object");
        assert!(schema.properties.contains_key("name"));
        assert_eq!(schema.required, vec!["name"]);
    }
}

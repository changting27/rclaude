//! Plan mode tools: EnterPlanMode, ExitPlanMode.
//! Actually switches the permission mode to Plan (read-only).

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::permissions::PermissionMode;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "EnterPlanMode"
    }

    fn description(&self) -> &str {
        "Enter plan mode for designing an implementation approach before writing code. \
         In plan mode, only read-only tools are allowed."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {}
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, _input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        // Switch permission mode to Plan via app state
        if let Some(ref state) = ctx.app_state {
            let mut s = state.write().await;
            s.permission_mode = PermissionMode::Plan;
        }
        Ok(ToolResult::text(
            "Entered plan mode. Only read-only tools (Read, Glob, Grep, Bash read-only) are allowed. \
             Use ExitPlanMode when ready to present the plan for user approval.",
        ))
    }
}

pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "ExitPlanMode"
    }

    fn description(&self) -> &str {
        "Exit plan mode after writing a plan. The user will review and approve the plan."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {}
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, _input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        // Restore permission mode to Default
        if let Some(ref state) = ctx.app_state {
            let mut s = state.write().await;
            s.permission_mode = PermissionMode::Default;
        }
        Ok(ToolResult::text(
            "Exited plan mode. Normal tool permissions restored. Plan submitted for user review.",
        ))
    }
}

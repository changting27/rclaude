//! ConfigTool: get/set settings with multi-layer read/write.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

/// Supported settings and their types.
const SUPPORTED_SETTINGS: &[(&str, &str, &str)] = &[
    (
        "model",
        "string",
        "Default model (e.g., sonnet, opus, haiku)",
    ),
    ("theme", "string", "Color theme (dark, light)"),
    ("max_tokens", "number", "Maximum output tokens"),
    ("verbose", "boolean", "Enable verbose output"),
    (
        "permissions.defaultMode",
        "string",
        "Default permission mode (default, auto, bypassPermissions, plan)",
    ),
    (
        "permissions.allowedTools",
        "array",
        "Tools allowed without confirmation",
    ),
    ("permissions.deniedTools", "array", "Tools always denied"),
];

pub struct ConfigTool;

#[async_trait]
impl Tool for ConfigTool {
    fn name(&self) -> &str {
        "Config"
    }
    fn description(&self) -> &str {
        "Get or set rclaude settings. Omit value to read current setting."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "setting": {
                    "type": "string",
                    "description": "Setting key (e.g., 'model', 'theme', 'permissions.defaultMode')"
                },
                "value": {
                    "description": "New value. Omit to get current value."
                }
            },
            "required": ["setting"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let setting = input
            .get("setting")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing setting".into()))?;

        // Validate setting name
        if !SUPPORTED_SETTINGS
            .iter()
            .any(|(name, _, _)| *name == setting)
        {
            let available: Vec<&str> = SUPPORTED_SETTINGS.iter().map(|(n, _t, _d)| *n).collect();
            return Ok(ToolResult::error(format!(
                "Unknown setting: \"{setting}\". Available: {}",
                available.join(", ")
            )));
        }

        let value = input.get("value");

        if value.is_none() || value == Some(&Value::Null) {
            // GET operation — read from config
            let config = rclaude_core::config::Config::load_for_cwd(&ctx.cwd);
            let current = get_config_value(&config, setting);
            return Ok(ToolResult::text(format!("Setting '{setting}' = {current}")));
        }

        // SET operation — write to project config
        let value = value.unwrap();
        let settings_path = ctx.cwd.join(".claude/settings.json");
        let mut settings: Value = if settings_path.exists() {
            let content = tokio::fs::read_to_string(&settings_path)
                .await
                .unwrap_or_default();
            serde_json::from_str(&content).unwrap_or(json!({}))
        } else {
            json!({})
        };

        // Set the value at the path
        set_nested_value(&mut settings, setting, value.clone());

        // Write back
        if let Some(parent) = settings_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json_str = serde_json::to_string_pretty(&settings)
            .map_err(|e| RclaudeError::Tool(e.to_string()))?;
        tokio::fs::write(&settings_path, json_str).await?;

        Ok(ToolResult::text(format!(
            "Setting '{setting}' updated to {value} in {}",
            settings_path.display()
        )))
    }
}

fn get_config_value(config: &rclaude_core::config::Config, setting: &str) -> String {
    match setting {
        "model" => config.model.clone(),
        "theme" => config.theme.clone(),
        "max_tokens" => config.max_tokens.to_string(),
        "verbose" => config.verbose.to_string(),
        "permissions.defaultMode" => "default".to_string(),
        "permissions.allowedTools" => format!("{:?}", config.allowed_tools),
        "permissions.deniedTools" => format!("{:?}", config.denied_tools),
        _ => "unknown".to_string(),
    }
}

fn set_nested_value(obj: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = obj;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            current[part] = value.clone();
        } else {
            if !current.get(part).is_some_and(|v| v.is_object()) {
                current[part] = json!({});
            }
            current = current.get_mut(part).unwrap();
        }
    }
}

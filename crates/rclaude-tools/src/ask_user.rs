use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DESCRIPTION: &str = "Use this tool when you need to ask the user a question. \
Allows presenting multiple-choice options to the user. \
Users will always be able to select \"Other\" to provide custom text input.";

pub struct AskUserQuestionTool;

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "AskUserQuestion"
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "description": "Questions to ask the user (1-4 questions)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": { "type": "string" },
                            "header": { "type": "string" },
                            "options": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string" },
                                        "description": { "type": "string" }
                                    },
                                    "required": ["label", "description"]
                                }
                            },
                            "multiSelect": { "type": "boolean" }
                        },
                        "required": ["question", "header", "options", "multiSelect"]
                    }
                }
            },
            "required": ["questions"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let questions = input
            .get("questions")
            .and_then(|v| v.as_array())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: questions".into()))?;

        let mut answers = serde_json::Map::new();

        for q in questions {
            let question_text = q.get("question").and_then(|v| v.as_str()).unwrap_or("?");
            let options = q
                .get("options")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // Print question and options
            println!("\n{question_text}");
            for (i, opt) in options.iter().enumerate() {
                let label = opt.get("label").and_then(|v| v.as_str()).unwrap_or("?");
                let desc = opt
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                println!("  {}. {} - {}", i + 1, label, desc);
            }
            println!("  {}. Other (type your own)", options.len() + 1);

            // Read user input
            print!("Your choice: ");
            use std::io::Write;
            std::io::stdout().flush().ok();

            let mut input_line = String::new();
            std::io::stdin()
                .read_line(&mut input_line)
                .map_err(|e| RclaudeError::Tool(format!("Failed to read input: {e}")))?;

            let choice = input_line.trim();

            // Parse as number or use as direct text
            let answer = if let Ok(num) = choice.parse::<usize>() {
                if num > 0 && num <= options.len() {
                    options[num - 1]
                        .get("label")
                        .and_then(|v| v.as_str())
                        .unwrap_or(choice)
                        .to_string()
                } else {
                    choice.to_string()
                }
            } else {
                choice.to_string()
            };

            answers.insert(question_text.to_string(), Value::String(answer));
        }

        Ok(ToolResult::text(serde_json::to_string_pretty(
            &Value::Object(answers),
        )?))
    }
}

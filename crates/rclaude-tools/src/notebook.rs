use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DESCRIPTION: &str = "Completely replaces the contents of a specific cell in a Jupyter \
notebook (.ipynb file) with new source. The notebook_path parameter must be an absolute path. \
The cell_number is 0-indexed. Use edit_mode=insert to add a new cell. \
Use edit_mode=delete to delete a cell.";

pub struct NotebookEditTool;

fn resolve_path(p: &str, cwd: &std::path::Path) -> PathBuf {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "NotebookEdit"
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "notebook_path": {
                    "type": "string",
                    "description": "The absolute path to the Jupyter notebook file"
                },
                "new_source": {
                    "type": "string",
                    "description": "The new source for the cell"
                },
                "cell_type": {
                    "type": "string",
                    "enum": ["code", "markdown"],
                    "description": "The type of the cell"
                },
                "edit_mode": {
                    "type": "string",
                    "enum": ["replace", "insert", "delete"],
                    "description": "The type of edit (default: replace)"
                },
                "cell_id": {
                    "type": "string",
                    "description": "The ID of the cell to edit"
                }
            },
            "required": ["notebook_path", "new_source"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let notebook_path = input
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: notebook_path".into()))?;

        let new_source = input
            .get("new_source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: new_source".into()))?;

        let edit_mode = input
            .get("edit_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("replace");

        let cell_type = input
            .get("cell_type")
            .and_then(|v| v.as_str())
            .unwrap_or("code");

        let cell_id = input.get("cell_id").and_then(|v| v.as_str());

        let path = resolve_path(notebook_path, &ctx.cwd);

        if !path.exists() {
            return Ok(ToolResult::error(format!(
                "Notebook does not exist: {}",
                path.display()
            )));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut notebook: Value = serde_json::from_str(&content)
            .map_err(|e| RclaudeError::Tool(format!("Invalid notebook JSON: {e}")))?;

        let cells = notebook
            .get_mut("cells")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| RclaudeError::Tool("Notebook has no 'cells' array".into()))?;

        // Find cell by ID or use first cell
        let cell_index = if let Some(id) = cell_id {
            cells
                .iter()
                .position(|c| {
                    c.get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s == id)
                        .unwrap_or(false)
                })
                .ok_or_else(|| RclaudeError::Tool(format!("Cell with id '{id}' not found")))?
        } else {
            0
        };

        let source_lines: Vec<Value> = new_source
            .lines()
            .map(|l| Value::String(format!("{l}\n")))
            .collect();

        match edit_mode {
            "replace" => {
                if cell_index >= cells.len() {
                    return Ok(ToolResult::error("Cell index out of range".to_string()));
                }
                cells[cell_index]["source"] = Value::Array(source_lines);
                if cell_type == "markdown" || cell_type == "code" {
                    cells[cell_index]["cell_type"] = Value::String(cell_type.to_string());
                }
            }
            "insert" => {
                let new_cell = json!({
                    "cell_type": cell_type,
                    "source": source_lines,
                    "metadata": {},
                    "outputs": []
                });
                let insert_at = (cell_index + 1).min(cells.len());
                cells.insert(insert_at, new_cell);
            }
            "delete" => {
                if cell_index >= cells.len() {
                    return Ok(ToolResult::error("Cell index out of range".to_string()));
                }
                cells.remove(cell_index);
            }
            _ => {
                return Ok(ToolResult::error(format!("Unknown edit_mode: {edit_mode}")));
            }
        }

        let updated = serde_json::to_string_pretty(&notebook)?;
        tokio::fs::write(&path, updated).await?;

        Ok(ToolResult::text(format!(
            "Notebook {} updated ({edit_mode} cell at index {cell_index})",
            path.display()
        )))
    }
}

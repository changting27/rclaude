use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DESCRIPTION: &str = "Performs exact string replacements in files.\n\n\
Usage:\n\
- The edit will FAIL if `old_string` is not unique in the file. Either provide a \
larger string with more surrounding context to make it unique or use `replace_all`.\n\
- Use `replace_all` for replacing and renaming strings across the file.";

pub struct FileEditTool;

fn resolve_path(file_path: &str, cwd: &std::path::Path) -> PathBuf {
    let p = PathBuf::from(file_path);
    if p.is_absolute() {
        p
    } else {
        cwd.join(p)
    }
}

/// Generate a compact unified diff between old and new content.
fn generate_diff(old: &str, new: &str, filename: &str) -> String {
    use similar::TextDiff;
    let diff = TextDiff::from_lines(old, new);
    let mut output = format!("--- a/{filename}\n+++ b/{filename}\n");
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push_str(&format!("{hunk}"));
    }
    if output.lines().count() <= 2 {
        return String::new(); // No changes
    }
    output
}

/// Show a context snippet around the edit location.
fn edit_context(content: &str, new_string: &str, max_context_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    // Find the line containing the start of new_string
    let new_lines: Vec<&str> = new_string.lines().collect();
    if new_lines.is_empty() {
        return String::new();
    }
    let first_new_line = new_lines[0];

    let mut match_line = None;
    for (i, line) in lines.iter().enumerate() {
        if line.contains(first_new_line) {
            match_line = Some(i);
            break;
        }
    }

    let center = match_line.unwrap_or(0);
    let start = center.saturating_sub(max_context_lines);
    let end = (center + new_lines.len() + max_context_lines).min(lines.len());

    let mut output = String::new();
    for (i, line) in lines[start..end].iter().enumerate() {
        output.push_str(&format!("{}\t{}\n", start + i + 1, line));
    }
    output
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: file_path".into()))?;

        let old_string = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: old_string".into()))?;

        let new_string = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: new_string".into()))?;

        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if old_string == new_string {
            return Ok(ToolResult::error(
                "old_string and new_string are identical. No edit needed.",
            ));
        }

        let path = resolve_path(file_path, &ctx.cwd);

        if !path.exists() {
            return Ok(ToolResult::error(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        let content = tokio::fs::read_to_string(&path).await?;

        // Count occurrences
        let count = content.matches(old_string).count();

        if count == 0 {
            return Ok(ToolResult::error(format!(
                "old_string not found in {}. Make sure it matches exactly (including whitespace).",
                path.display()
            )));
        }

        if !replace_all && count > 1 {
            return Ok(ToolResult::error(format!(
                "old_string found {count} times in {}. Provide more context to make it unique, \
                 or use replace_all to change every occurrence.",
                path.display()
            )));
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        // Snapshot before write (for undo/rollback)
        {
            let shadow_dir = rclaude_core::config::Config::config_dir().join("file-history");
            if tokio::fs::create_dir_all(&shadow_dir).await.is_ok() {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                path.hash(&mut h);
                let shadow = shadow_dir.join(format!("{:x}.bak", h.finish()));
                let _ = tokio::fs::write(&shadow, &content).await;
            }
        }

        tokio::fs::write(&path, &new_content).await?;

        // Generate unified diff preview
        let diff = generate_diff(&content, &new_content, file_path);
        let snippet = edit_context(&new_content, new_string, 3);
        let msg = if replace_all {
            format!(
                "The file {} has been updated successfully ({count} replacements).\n\n{diff}\n{snippet}",
                path.display()
            )
        } else {
            format!(
                "The file {} has been updated successfully.\n\n{diff}\n{snippet}",
                path.display()
            )
        };

        Ok(ToolResult::text(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_context() {
        let content = "line1\nline2\nline3\nNEW STUFF\nline5\nline6\n";
        let ctx = edit_context(content, "NEW STUFF", 2);
        assert!(ctx.contains("NEW STUFF"));
        assert!(ctx.contains("line2"));
    }
}

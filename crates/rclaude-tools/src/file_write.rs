use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::debug;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

/// Tool that writes (creates or overwrites) a file on the local filesystem.
pub struct FileWriteTool;

const TOOL_NAME: &str = "Write";
const DESCRIPTION: &str = "Write a file to the local filesystem. \
Creates parent directories as needed. Overwrites existing files.";

impl FileWriteTool {
    /// Resolve a user-supplied path against the working directory.
    /// Absolute paths are returned as-is; relative paths are joined with `cwd`.
    fn resolve_path(file_path: &str, cwd: &Path) -> PathBuf {
        let p = Path::new(file_path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        }
    }

    /// Extract a required string field from the JSON input.
    fn required_str<'a>(input: &'a Value, field: &str) -> Result<&'a str> {
        input
            .get(field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool(format!("missing required field: {field}")))
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        ToolInputSchema {
            schema_type: "object".to_string(),
            properties: [
                (
                    "file_path".to_string(),
                    json!({
                        "type": "string",
                        "description": "The absolute path to the file to write (must be absolute, not relative)"
                    }),
                ),
                (
                    "content".to_string(),
                    json!({
                        "type": "string",
                        "description": "The content to write to the file"
                    }),
                ),
            ]
            .into_iter()
            .collect(),
            required: vec!["file_path".to_string(), "content".to_string()],
            extra: Default::default(),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let file_path_str = Self::required_str(&input, "file_path")?;
        let content = Self::required_str(&input, "content")?;

        let full_path = Self::resolve_path(file_path_str, &ctx.cwd);
        debug!(path = %full_path.display(), "FileWriteTool: writing file");

        // Ensure the parent directory exists.
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                RclaudeError::Tool(format!(
                    "failed to create parent directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        // Snapshot existing file before overwrite (for undo/rollback)
        if full_path.exists() {
            let shadow_dir = rclaude_core::config::Config::config_dir().join("file-history");
            if tokio::fs::create_dir_all(&shadow_dir).await.is_ok() {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                full_path.hash(&mut h);
                let shadow = shadow_dir.join(format!("{:x}.bak", h.finish()));
                let _ = tokio::fs::copy(&full_path, &shadow).await;
            }
        }

        // Write file contents.
        let bytes = content.as_bytes();
        let byte_count = bytes.len();
        tokio::fs::write(&full_path, bytes).await.map_err(|e| {
            RclaudeError::Tool(format!("failed to write file {}: {e}", full_path.display()))
        })?;

        let message = format!(
            "Successfully wrote {} bytes to {}",
            byte_count,
            full_path.display()
        );
        debug!("{}", message);

        Ok(ToolResult::text(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_ctx(cwd: PathBuf) -> ToolUseContext {
        ToolUseContext {
            cwd,
            permission_mode: rclaude_core::permissions::PermissionMode::Default,
            debug: false,
            verbose: false,
            abort_signal: tokio::sync::watch::channel(false).1,
            app_state: None,
        }
    }

    #[tokio::test]
    async fn test_write_new_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("hello.txt");
        let ctx = make_ctx(tmp.path().to_path_buf());

        let result = FileWriteTool
            .execute(
                json!({ "file_path": file_path.to_str().unwrap(), "content": "hello world" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "hello world");
    }

    #[tokio::test]
    async fn test_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("a/b/c/deep.txt");
        let ctx = make_ctx(tmp.path().to_path_buf());

        let result = FileWriteTool
            .execute(
                json!({ "file_path": file_path.to_str().unwrap(), "content": "nested" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "nested");
    }

    #[tokio::test]
    async fn test_relative_path_resolved_from_cwd() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_ctx(tmp.path().to_path_buf());

        let result = FileWriteTool
            .execute(
                json!({ "file_path": "relative.txt", "content": "data" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let expected = tmp.path().join("relative.txt");
        assert_eq!(std::fs::read_to_string(expected).unwrap(), "data");
    }

    #[tokio::test]
    async fn test_missing_field_returns_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = make_ctx(tmp.path().to_path_buf());

        let result = FileWriteTool
            .execute(json!({ "file_path": "/tmp/x" }), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_overwrite_existing_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("existing.txt");
        std::fs::write(&file_path, "old content").unwrap();
        let ctx = make_ctx(tmp.path().to_path_buf());

        let result = FileWriteTool
            .execute(
                json!({ "file_path": file_path.to_str().unwrap(), "content": "new content" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "new content");
    }
}

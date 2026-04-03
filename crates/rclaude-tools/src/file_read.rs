use async_trait::async_trait;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

/// Default maximum number of lines to read when no limit is specified.
const DEFAULT_LIMIT: usize = 2000;

/// Number of leading bytes inspected for null-byte binary detection.
const BINARY_CHECK_BYTES: usize = 8192;

/// Device paths that would hang the process (infinite output or blocking input).
/// Safe devices like /dev/null are intentionally omitted.
const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/stdin",
    "/dev/tty",
    "/dev/console",
    "/dev/stdout",
    "/dev/stderr",
    "/dev/fd/0",
    "/dev/fd/1",
    "/dev/fd/2",
];

const DESCRIPTION: &str = "Reads a file from the local filesystem. \
You can access any file directly by using this tool.\n\
Assume this tool is able to read all files on the machine. \
If the User provides a path to a file assume that path is valid. \
It is okay to read a file that does not exist; an error will be returned.\n\n\
Usage:\n\
- The file_path parameter must be an absolute path, not a relative path\n\
- By default, it reads up to 2000 lines starting from the beginning of the file\n\
- When you already know which part of the file you need, only read that part. \
This can be important for larger files.\n\
- Results are returned using cat -n format, with line numbers starting at 1\n\
- This tool can only read files, not directories. \
To read a directory, use an ls command via the Bash tool.\n\
- If you read a file that exists but has empty contents you will receive a \
system reminder warning in place of file contents.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve `file_path`: absolute paths stay as-is, relative ones join onto `cwd`.
fn resolve_path(file_path: &str, cwd: &Path) -> PathBuf {
    let p = Path::new(file_path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// Returns `true` when the first [`BINARY_CHECK_BYTES`] bytes contain a null
/// byte, indicating a binary file.
/// Read a PDF file using pdftotext (poppler-utils).
async fn read_pdf(path: &std::path::Path, display_path: &str) -> Result<ToolResult> {
    let output = tokio::process::Command::new("pdftotext")
        .args(["-layout", path.to_str().unwrap_or(""), "-"])
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            if text.trim().is_empty() {
                Ok(ToolResult::text(format!(
                    "PDF file '{}' contains no extractable text (may be image-based).",
                    display_path
                )))
            } else {
                Ok(ToolResult::text(text.to_string()))
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Ok(ToolResult::error(format!(
                "Failed to extract text from PDF '{}': {}",
                display_path, stderr.trim()
            )))
        }
        Err(_) => {
            Ok(ToolResult::error(format!(
                "Cannot read PDF '{}': pdftotext not found. Install poppler-utils: apt install poppler-utils",
                display_path
            )))
        }
    }
}

fn is_binary(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(BINARY_CHECK_BYTES);
    bytes[..check_len].contains(&0)
}

/// Returns `true` when the resolved path matches a blocked device.
fn is_blocked_device_path(path: &str) -> bool {
    if BLOCKED_DEVICE_PATHS.contains(&path) {
        return true;
    }
    // Linux proc aliases for stdio: /proc/self/fd/0-2, /proc/<pid>/fd/0-2
    if path.starts_with("/proc/")
        && (path.ends_with("/fd/0") || path.ends_with("/fd/1") || path.ends_with("/fd/2"))
    {
        return true;
    }
    false
}

/// Format lines with `cat -n`-style numbering (6-char right-aligned number + tab).
///
/// `start_line` is the 1-based line number for the first element of `lines`.
fn add_line_numbers(lines: &[&str], start_line: usize) -> String {
    let mut out = String::with_capacity(lines.len() * 80);
    for (i, line) in lines.iter().enumerate() {
        let _ = writeln!(out, "{:>6}\t{}", start_line + i, line);
    }
    out
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

/// Reads files from the local filesystem, displaying content with line numbers
/// (`cat -n` style). Detects binary files and blocked device paths.
pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn display_name(&self) -> &str {
        "Read"
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
                    "description": "The absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "The line number to start reading from. Only provide if the file is too large to read at once",
                    "minimum": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once.",
                    "exclusiveMinimum": 0
                }
            },
            "required": ["file_path"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        // -- Parse input --------------------------------------------------
        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: file_path".into()))?;

        // offset follows the TS semantics:
        //   offset absent / 0  => start from line 1 (beginning)
        //   offset = N (>=1)   => start from that 1-based line number
        let offset: usize = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0);

        let limit: usize = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_LIMIT);

        // -- Resolve path -------------------------------------------------
        let resolved = resolve_path(file_path, &ctx.cwd);
        let resolved_str = resolved.to_string_lossy();

        // -- Block dangerous device paths ---------------------------------
        if is_blocked_device_path(&resolved_str) {
            return Ok(ToolResult::error(format!(
                "Cannot read '{}': this device file would block or produce infinite output.",
                file_path
            )));
        }

        // -- Read file bytes ----------------------------------------------
        let raw_bytes = match tokio::fs::read(&resolved).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ToolResult::error(format!(
                    "File does not exist: {}. \
                     Make sure the path is correct. \
                     Note: your current working directory is {}.",
                    file_path,
                    ctx.cwd.display()
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(ToolResult::error(format!(
                    "Permission denied reading file: {}",
                    file_path
                )));
            }
            Err(e) if e.raw_os_error() == Some(21) /* EISDIR */ => {
                return Ok(ToolResult::error(
                    "This tool can only read files, not directories. \
                     Use Bash with `ls` to list directory contents."
                        .to_string(),
                ));
            }
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "Error reading file '{}': {}",
                    file_path, e
                )));
            }
        };

        // -- Directory check (fallback for non-Linux) ---------------------
        if resolved.is_dir() {
            return Ok(ToolResult::error(
                "This tool can only read files, not directories. \
                 Use Bash with `ls` to list directory contents."
                    .to_string(),
            ));
        }

        // -- PDF support ---------------------------------------------------
        if resolved
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        {
            return read_pdf(&resolved, file_path).await;
        }

        // -- Binary detection ---------------------------------------------
        if is_binary(&raw_bytes) {
            return Ok(ToolResult::error(format!(
                "This appears to be a binary file ({}). \
                 The Read tool cannot display binary content. \
                 Use appropriate tools for binary file analysis.",
                file_path
            )));
        }

        // -- Decode to string (lossy for non-UTF-8) -----------------------
        let content = String::from_utf8_lossy(&raw_bytes);

        // -- Empty file ---------------------------------------------------
        if content.is_empty() {
            return Ok(ToolResult::text(
                "<system-reminder>Warning: the file exists but the contents are empty.</system-reminder>",
            ));
        }

        // -- Slice to requested range -------------------------------------
        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();

        // Convert offset to 0-based index (offset 0 and 1 both mean "first line").
        let start_index = offset.saturating_sub(1);

        if start_index >= total_lines {
            return Ok(ToolResult::text(format!(
                "<system-reminder>Warning: the file exists but is shorter than the provided \
                 offset ({}). The file has {} lines.</system-reminder>",
                offset, total_lines
            )));
        }

        let end_index = (start_index + limit).min(total_lines);
        let selected = &all_lines[start_index..end_index];
        let start_line = start_index + 1; // 1-based for display

        // -- Format output ------------------------------------------------
        let numbered = add_line_numbers(selected, start_line);
        Ok(ToolResult::text(numbered))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rclaude_core::permissions::PermissionMode;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_ctx(cwd: PathBuf) -> ToolUseContext {
        ToolUseContext {
            cwd,
            permission_mode: PermissionMode::Default,
            debug: false,
            verbose: false,
            abort_signal: tokio::sync::watch::channel(false).1,
            app_state: None,
        }
    }

    // -- Unit tests -------------------------------------------------------

    #[test]
    fn test_is_binary_detects_null_bytes() {
        assert!(is_binary(b"hello\x00world"));
    }

    #[test]
    fn test_is_binary_allows_text() {
        assert!(!is_binary(b"hello world\nmore text\n"));
    }

    #[test]
    fn test_is_binary_empty() {
        assert!(!is_binary(b""));
    }

    #[test]
    fn test_blocked_device_paths() {
        assert!(is_blocked_device_path("/dev/zero"));
        assert!(is_blocked_device_path("/dev/urandom"));
        assert!(is_blocked_device_path("/proc/self/fd/0"));
        assert!(is_blocked_device_path("/proc/1234/fd/2"));
        assert!(!is_blocked_device_path("/dev/null"));
        assert!(!is_blocked_device_path("/home/user/file.txt"));
    }

    #[test]
    fn test_resolve_path_absolute() {
        let cwd = Path::new("/home/user");
        assert_eq!(resolve_path("/etc/hosts", cwd), PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let cwd = Path::new("/home/user/project");
        assert_eq!(
            resolve_path("src/main.rs", cwd),
            PathBuf::from("/home/user/project/src/main.rs")
        );
    }

    #[test]
    fn test_add_line_numbers_basic() {
        let lines = vec!["hello", "world"];
        let out = add_line_numbers(&lines, 1);
        assert!(out.contains("     1\thello\n"));
        assert!(out.contains("     2\tworld\n"));
    }

    #[test]
    fn test_add_line_numbers_offset() {
        let lines = vec!["a", "b"];
        let out = add_line_numbers(&lines, 42);
        assert!(out.contains("    42\ta\n"));
        assert!(out.contains("    43\tb\n"));
    }

    // -- Integration tests ------------------------------------------------

    #[tokio::test]
    async fn test_read_normal_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "line one").unwrap();
        writeln!(tmp, "line two").unwrap();
        writeln!(tmp, "line three").unwrap();

        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({ "file_path": tmp.path().to_str().unwrap() });
        let result = tool.execute(input, &ctx).await.unwrap();

        assert!(!result.is_error);
        let text = match &result.content[0] {
            rclaude_core::tool::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("line one"));
        assert!(text.contains("line two"));
        assert!(text.contains("line three"));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let mut tmp = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(tmp, "line {}", i).unwrap();
        }

        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "offset": 3,
            "limit": 2
        });
        let result = tool.execute(input, &ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content[0] {
            rclaude_core::tool::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("     3\tline 3"));
        assert!(text.contains("     4\tline 4"));
        assert!(!text.contains("line 5"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({ "file_path": "/tmp/__nonexistent_test_file_42__.txt" });
        let result = tool.execute(input, &ctx).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let tmp = NamedTempFile::new().unwrap();
        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({ "file_path": tmp.path().to_str().unwrap() });
        let result = tool.execute(input, &ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content[0] {
            rclaude_core::tool::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("empty"));
    }

    #[tokio::test]
    async fn test_read_binary_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"hello\x00binary\x00data").unwrap();

        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({ "file_path": tmp.path().to_str().unwrap() });
        let result = tool.execute(input, &ctx).await.unwrap();
        assert!(result.is_error);
        let text = match &result.content[0] {
            rclaude_core::tool::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("binary"));
    }

    #[tokio::test]
    async fn test_read_blocked_device() {
        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({ "file_path": "/dev/zero" });
        let result = tool.execute(input, &ctx).await.unwrap();
        assert!(result.is_error);
        assert!(matches!(&result.content[0],
            rclaude_core::tool::ToolResultContent::Text { text }
            if text.contains("block")));
    }

    #[tokio::test]
    async fn test_offset_past_end() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "only line").unwrap();

        let tool = FileReadTool;
        let ctx = make_ctx(PathBuf::from("/tmp"));
        let input = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "offset": 999
        });
        let result = tool.execute(input, &ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content[0] {
            rclaude_core::tool::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("shorter than the provided offset"));
    }
}

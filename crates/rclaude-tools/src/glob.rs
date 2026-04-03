use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use ignore::WalkBuilder;
use serde_json::{json, Value};
use std::cmp::Reverse;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DEFAULT_LIMIT: usize = 200;

const DESCRIPTION: &str = "\
- Fast file pattern matching tool that works with any codebase size
- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"
- Returns matching file paths sorted by modification time
- Use this tool when you need to find files by name patterns
- When you are doing an open ended search that may require multiple rounds \
of globbing and grepping, use the Agent tool instead";

pub struct GlobTool;

/// Compile a glob pattern, automatically prepending `**/` when the pattern
/// contains no path separator so that e.g. `*.rs` matches files in any
/// subdirectory.
fn compile_glob(pattern: &str) -> std::result::Result<GlobMatcher, globset::Error> {
    let effective = if !pattern.contains('/') && !pattern.contains('\\') {
        format!("**/{pattern}")
    } else {
        pattern.to_string()
    };
    Ok(Glob::new(&effective)?.compile_matcher())
}

/// Walk `root` respecting .gitignore, collect files matching `matcher`,
/// sort by mtime descending, and return at most `limit` results.
fn collect_matches(
    root: &Path,
    matcher: &GlobMatcher,
    limit: usize,
    abort: &tokio::sync::watch::Receiver<bool>,
) -> Result<(Vec<PathBuf>, bool)> {
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .follow_links(false)
        .build();

    let mut entries: Vec<(PathBuf, u64)> = Vec::new();

    for entry in walker {
        if *abort.borrow() {
            return Err(RclaudeError::Aborted);
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let ft = match entry.file_type() {
            Some(ft) => ft,
            None => continue,
        };
        if !ft.is_file() {
            continue;
        }

        let path = entry.path();
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => path,
        };

        if !matcher.is_match(rel) {
            continue;
        }

        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        entries.push((path.to_path_buf(), mtime));
    }

    entries.sort_by_key(|(_, mtime)| Reverse(*mtime));

    let truncated = entries.len() > limit;
    entries.truncate(limit);

    let paths: Vec<PathBuf> = entries.into_iter().map(|(p, _)| p).collect();
    Ok((paths, truncated))
}

fn to_relative_display(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in. Defaults to cwd if omitted."
                }
            },
            "required": ["pattern"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: pattern".into()))?;

        let search_dir = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => {
                let pb = PathBuf::from(p);
                if pb.is_relative() {
                    ctx.cwd.join(pb)
                } else {
                    pb
                }
            }
            _ => ctx.cwd.clone(),
        };

        if !search_dir.exists() {
            return Ok(ToolResult::error(format!(
                "Directory does not exist: {}",
                search_dir.display(),
            )));
        }

        let matcher = compile_glob(pattern)
            .map_err(|e| RclaudeError::Tool(format!("Invalid glob pattern '{pattern}': {e}")))?;

        let abort = ctx.abort_signal.clone();
        let dir = search_dir.clone();
        let (paths, truncated) = tokio::task::spawn_blocking(move || {
            collect_matches(&dir, &matcher, DEFAULT_LIMIT, &abort)
        })
        .await
        .map_err(|e| RclaudeError::Other(format!("Glob task panicked: {e}")))??;

        if paths.is_empty() {
            return Ok(ToolResult::text("No files found"));
        }

        let filenames: Vec<String> = paths
            .iter()
            .map(|p| to_relative_display(p, &ctx.cwd))
            .collect();

        let mut output = filenames.join("\n");
        if truncated {
            output.push_str("\n(Results truncated. Use a more specific path or pattern.)");
        }

        Ok(ToolResult::text(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_glob_bare_pattern() {
        let m = compile_glob("*.rs").unwrap();
        assert!(m.is_match("src/main.rs"));
        assert!(m.is_match("lib.rs"));
        assert!(!m.is_match("src/main.txt"));
    }

    #[test]
    fn test_compile_glob_with_path() {
        let m = compile_glob("src/**/*.rs").unwrap();
        assert!(m.is_match("src/foo/bar.rs"));
        assert!(!m.is_match("tests/bar.rs"));
    }

    #[test]
    fn test_to_relative_display() {
        let cwd = Path::new("/home/user/project");
        let abs = Path::new("/home/user/project/src/main.rs");
        assert_eq!(to_relative_display(abs, cwd), "src/main.rs");
    }
}

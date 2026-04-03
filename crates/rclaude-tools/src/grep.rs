use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const VCS_DIRS: &[&str] = &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];
const DEFAULT_HEAD_LIMIT: usize = 250;
const MAX_LINE_LENGTH: usize = 500;

const DESCRIPTION: &str = "A powerful search tool built on regex\n\n\
  Usage:\n\
  - Supports full regex syntax (e.g., \"log.*Error\", \"function\\\\s+\\\\w+\")\n\
  - Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\")\n\
  - Output modes: \"content\" shows matching lines, \
    \"files_with_matches\" shows only file paths (default), \
    \"count\" shows match counts\n\
  - Pattern syntax: literal braces need escaping";

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }
    fn display_name(&self) -> &str {
        "Search"
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        let props: &[(&str, Value)] = &[
            (
                "pattern",
                json!({"type":"string","description":"The regex pattern to search for"}),
            ),
            (
                "path",
                json!({"type":"string","description":"Directory to search in (defaults to cwd)"}),
            ),
            (
                "glob",
                json!({"type":"string","description":"Glob filter e.g. \"*.rs\", \"*.{ts,tsx}\""}),
            ),
            (
                "output_mode",
                json!({"type":"string","enum":["content","files_with_matches","count"]}),
            ),
            (
                "-i",
                json!({"type":"boolean","description":"Case insensitive search"}),
            ),
            (
                "context",
                json!({"type":"integer","description":"Context lines around matches (content mode)"}),
            ),
            (
                "-C",
                json!({"type":"integer","description":"Alias for context"}),
            ),
            (
                "head_limit",
                json!({"type":"integer","description":"Max results (default 250, 0=unlimited)"}),
            ),
            (
                "offset",
                json!({"type":"integer","description":"Skip first N entries (default 0)"}),
            ),
        ];
        ToolInputSchema {
            schema_type: "object".into(),
            properties: props
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            required: vec!["pattern".into()],
            extra: HashMap::new(),
        }
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let pattern = input
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: pattern".into()))?;
        let case_insensitive = input.get("-i").and_then(|v| v.as_bool()).unwrap_or(false);
        let re = RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map_err(|e| RclaudeError::Tool(format!("Invalid regex pattern: {e}")))?;

        let search_path = resolve_path(&input, &ctx.cwd);
        if !search_path.exists() {
            return Err(RclaudeError::NotFound(format!(
                "Path does not exist: {}",
                search_path.display()
            )));
        }

        let glob_filter = input.get("glob").and_then(|v| v.as_str()).map(String::from);
        let output_mode = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches")
            .to_string();
        let ctx_lines = input
            .get("context")
            .and_then(|v| v.as_u64())
            .or_else(|| input.get("-C").and_then(|v| v.as_u64()))
            .map(|v| v as usize)
            .unwrap_or(0);
        let head_limit = input.get("head_limit").and_then(|v| v.as_u64());
        let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let abort = ctx.abort_signal.clone();
        let cwd = ctx.cwd.clone();

        let text = tokio::task::spawn_blocking(move || {
            let glob_matcher = build_glob_matcher(glob_filter.as_deref())?;
            let files = walk_files(&search_path, glob_matcher.as_ref(), &abort)?;
            match output_mode.as_str() {
                "content" => fmt_content(&files, &re, ctx_lines, head_limit, offset, &cwd),
                "count" => fmt_count(&files, &re, head_limit, offset, &cwd),
                _ => fmt_files(&files, &re, head_limit, offset, &cwd),
            }
        })
        .await
        .map_err(|e| RclaudeError::Other(format!("Grep task panicked: {e}")))??;
        Ok(ToolResult::text(text))
    }
}

fn resolve_path(input: &Value, cwd: &Path) -> PathBuf {
    match input.get("path").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => {
            let pb = PathBuf::from(p);
            if pb.is_relative() {
                cwd.join(pb)
            } else {
                pb
            }
        }
        _ => cwd.to_path_buf(),
    }
}

fn to_rel(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn build_glob_matcher(glob_str: Option<&str>) -> Result<Option<globset::GlobSet>> {
    let pat = match glob_str {
        Some(p) if !p.is_empty() => p,
        _ => return Ok(None),
    };
    let mut builder = globset::GlobSetBuilder::new();
    for token in pat.split_whitespace() {
        let parts = if token.contains('{') && token.contains('}') {
            vec![token.to_string()]
        } else {
            token
                .split(',')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        };
        for p in parts {
            let g = globset::GlobBuilder::new(&p)
                .literal_separator(false)
                .build()
                .map_err(|e| RclaudeError::Tool(format!("Invalid glob '{p}': {e}")))?;
            builder.add(g);
        }
    }
    Ok(Some(builder.build().map_err(|e| {
        RclaudeError::Tool(format!("Failed to compile glob set: {e}"))
    })?))
}

fn walk_files(
    root: &Path,
    glob_matcher: Option<&globset::GlobSet>,
    abort: &tokio::sync::watch::Receiver<bool>,
) -> Result<Vec<PathBuf>> {
    let mut walker = WalkBuilder::new(root);
    walker
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);
    let mut files = Vec::new();
    for entry in walker.build() {
        if *abort.borrow() {
            return Err(RclaudeError::Aborted);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.components().any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| VCS_DIRS.contains(&s))
        }) {
            continue;
        }
        if let Some(m) = glob_matcher {
            let ok = m.is_match(path) || path.file_name().is_some_and(|n| m.is_match(Path::new(n)));
            if !ok {
                continue;
            }
        }
        files.push(path.to_path_buf());
    }
    Ok(files)
}

/// Apply offset + head_limit. Returns (items, applied_limit_if_truncated).
fn apply_head_limit<T: Clone>(
    items: &[T],
    limit: Option<u64>,
    offset: usize,
) -> (Vec<T>, Option<usize>) {
    let rest = items.get(offset..).unwrap_or_default();
    if limit == Some(0) {
        return (rest.to_vec(), None);
    }
    let eff = limit.unwrap_or(DEFAULT_HEAD_LIMIT as u64) as usize;
    let sliced: Vec<T> = rest.iter().take(eff).cloned().collect();
    (sliced, if rest.len() > eff { Some(eff) } else { None })
}

fn limit_info(applied: Option<usize>, offset: usize) -> String {
    let mut p = Vec::new();
    if let Some(l) = applied {
        p.push(format!("limit: {l}"));
    }
    if offset > 0 {
        p.push(format!("offset: {offset}"));
    }
    p.join(", ")
}

fn is_matchable(line: &str, re: &regex::Regex) -> bool {
    line.len() <= MAX_LINE_LENGTH && re.is_match(line)
}

fn fmt_content(
    files: &[PathBuf],
    re: &regex::Regex,
    ctx_lines: usize,
    head_limit: Option<u64>,
    offset: usize,
    cwd: &Path,
) -> Result<String> {
    let mut all: Vec<String> = Vec::new();
    for fp in files {
        let content = match std::fs::read_to_string(fp) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        let hits: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| is_matchable(l, re))
            .map(|(i, _)| i)
            .collect();
        if hits.is_empty() {
            continue;
        }
        let rel = to_rel(fp, cwd);
        let mut include = vec![false; lines.len()];
        for &mi in &hits {
            let s = mi.saturating_sub(ctx_lines);
            for flag in &mut include[s..(mi + ctx_lines + 1).min(lines.len())] {
                *flag = true;
            }
        }
        let mut prev: Option<usize> = None;
        for (i, line) in lines.iter().enumerate() {
            if !include[i] {
                continue;
            }
            if prev.is_some_and(|p| i > p + 1) {
                all.push("--".into());
            }
            all.push(format!("{rel}:{}:{line}", i + 1));
            prev = Some(i);
        }
    }
    let (lim, applied) = apply_head_limit(&all, head_limit, offset);
    let info = limit_info(applied, offset);
    let mut text = if lim.is_empty() {
        "No matches found".into()
    } else {
        lim.join("\n")
    };
    if !info.is_empty() {
        text.push_str(&format!("\n\n[Showing results with pagination = {info}]"));
    }
    Ok(text)
}

fn fmt_count(
    files: &[PathBuf],
    re: &regex::Regex,
    head_limit: Option<u64>,
    offset: usize,
    cwd: &Path,
) -> Result<String> {
    let mut entries: Vec<(String, usize)> = Vec::new();
    for fp in files {
        let content = match std::fs::read_to_string(fp) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let count = content.lines().filter(|l| is_matchable(l, re)).count();
        if count > 0 {
            entries.push((to_rel(fp, cwd), count));
        }
    }
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let raw: Vec<String> = entries.iter().map(|(n, c)| format!("{n}:{c}")).collect();
    let (lim, applied) = apply_head_limit(&raw, head_limit, offset);
    let (mut total, mut fc) = (0usize, 0usize);
    for e in &lim {
        if let Some(c) = e.rfind(':') {
            if let Ok(n) = e[c + 1..].parse::<usize>() {
                total += n;
                fc += 1;
            }
        }
    }
    let occ = if total == 1 {
        "occurrence"
    } else {
        "occurrences"
    };
    let fil = if fc == 1 { "file" } else { "files" };
    let mut text = if lim.is_empty() {
        "No matches found".into()
    } else {
        lim.join("\n")
    };
    text.push_str(&format!("\n\nFound {total} total {occ} across {fc} {fil}."));
    let info = limit_info(applied, offset);
    if !info.is_empty() {
        text.push_str(&format!(" with pagination = {info}"));
    }
    Ok(text)
}

fn fmt_files(
    files: &[PathBuf],
    re: &regex::Regex,
    head_limit: Option<u64>,
    offset: usize,
    cwd: &Path,
) -> Result<String> {
    let mut matches: Vec<(String, u64)> = Vec::new();
    for fp in files {
        let content = match std::fs::read_to_string(fp) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !content.lines().any(|l| is_matchable(l, re)) {
            continue;
        }
        let mtime = std::fs::metadata(fp)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
            .unwrap_or(0);
        matches.push((to_rel(fp, cwd), mtime));
    }
    matches.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let paths: Vec<String> = matches.into_iter().map(|(p, _)| p).collect();
    let (lim, applied) = apply_head_limit(&paths, head_limit, offset);
    let n = lim.len();
    let info = limit_info(applied, offset);
    if n == 0 {
        return Ok("No files found".into());
    }
    let pl = if n == 1 { "file" } else { "files" };
    let hdr = if info.is_empty() {
        format!("Found {n} {pl}")
    } else {
        format!("Found {n} {pl} {info}")
    };
    Ok(format!("{hdr}\n{}", lim.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("hello.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(dir.path().join("world.txt"), "hello world\ngoodbye world\n").unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(
            dir.path().join("sub/nested.rs"),
            "// nested\nfn nested() {}\n",
        )
        .unwrap();
        dir
    }

    fn make_ctx(dir: &Path) -> ToolUseContext {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        ToolUseContext {
            cwd: dir.to_path_buf(),
            permission_mode: rclaude_core::permissions::PermissionMode::Default,
            debug: false,
            verbose: false,
            abort_signal: rx,
            app_state: None,
        }
    }

    fn text_of(r: &ToolResult) -> String {
        match &r.content[0] {
            rclaude_core::tool::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn test_files_with_matches() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(json!({"pattern":"hello"}), &make_ctx(dir.path()))
                .await
                .unwrap(),
        );
        assert!(t.contains("Found 2 files") && t.contains("hello.rs") && t.contains("world.txt"));
    }

    #[tokio::test]
    async fn test_content_mode() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(
                    json!({"pattern":"nested","output_mode":"content"}),
                    &make_ctx(dir.path()),
                )
                .await
                .unwrap(),
        );
        assert!(t.contains("nested") && t.contains(":1:"));
    }

    #[tokio::test]
    async fn test_count_mode() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(
                    json!({"pattern":"world","output_mode":"count"}),
                    &make_ctx(dir.path()),
                )
                .await
                .unwrap(),
        );
        assert!(t.contains("world.txt:2") && t.contains("2 total occurrences"));
    }

    #[tokio::test]
    async fn test_glob_filter() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(json!({"pattern":"fn","glob":"*.rs"}), &make_ctx(dir.path()))
                .await
                .unwrap(),
        );
        assert!(t.contains(".rs") && !t.contains("world.txt"));
    }

    #[tokio::test]
    async fn test_case_insensitive() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(json!({"pattern":"HELLO","-i":true}), &make_ctx(dir.path()))
                .await
                .unwrap(),
        );
        assert!(t.contains("Found 2 files"));
    }

    #[tokio::test]
    async fn test_no_matches() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(json!({"pattern":"zzzznotfound"}), &make_ctx(dir.path()))
                .await
                .unwrap(),
        );
        assert!(t.contains("No files found"));
    }

    #[tokio::test]
    async fn test_invalid_regex() {
        let dir = make_test_dir();
        assert!(GrepTool
            .execute(json!({"pattern":"[invalid"}), &make_ctx(dir.path()))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_path_not_found() {
        let dir = make_test_dir();
        assert!(GrepTool
            .execute(
                json!({"pattern":"x","path":"/nonexistent/path"}),
                &make_ctx(dir.path())
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_head_limit_truncation() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(
                    json!({"pattern":".","output_mode":"content","head_limit":2}),
                    &make_ctx(dir.path()),
                )
                .await
                .unwrap(),
        );
        assert!(t.contains("limit: 2"));
    }

    #[tokio::test]
    async fn test_context_lines() {
        let dir = make_test_dir();
        let t = text_of(
            &GrepTool
                .execute(
                    json!({"pattern":"println","output_mode":"content","context":1}),
                    &make_ctx(dir.path()),
                )
                .await
                .unwrap(),
        );
        assert!(t.contains("fn main()"));
    }
}

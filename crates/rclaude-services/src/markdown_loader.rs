//! Markdown config loader for CLAUDE.md and project instructions.
//! Loads and parses markdown files with frontmatter from config directories.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A loaded markdown file with metadata.
#[derive(Debug, Clone)]
pub struct MarkdownFile {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub content: String,
    pub frontmatter: HashMap<String, String>,
}

/// Extract description from markdown (first paragraph or heading).
pub fn extract_description(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            return trimmed.trim_start_matches('#').trim().to_string();
        }
        if !trimmed.starts_with("---") {
            return trimmed.to_string();
        }
    }
    String::new()
}

/// Get project directories from cwd up to home (for CLAUDE.md discovery).
pub fn get_project_dirs_up_to_home(cwd: &Path) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut dirs = Vec::new();
    let mut current = cwd.to_path_buf();

    loop {
        dirs.push(current.clone());
        if current == home || current.parent().is_none() {
            break;
        }
        current = current.parent().unwrap().to_path_buf();
    }
    dirs
}

/// Load all markdown files from a subdirectory.
pub async fn load_markdown_files(dir: &Path) -> Vec<MarkdownFile> {
    let mut files = Vec::new();
    let entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return files,
    };

    let mut entries = entries;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let (frontmatter, body) = parse_frontmatter(&content);
            let description = frontmatter
                .get("description")
                .cloned()
                .unwrap_or_else(|| extract_description(&body));
            files.push(MarkdownFile {
                path,
                name,
                description,
                content: body,
                frontmatter,
            });
        }
    }
    files
}

fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let mut fm = HashMap::new();
    if !content.starts_with("---") {
        return (fm, content.to_string());
    }
    let rest = &content[3..];
    if let Some(end) = rest.find("\n---") {
        for line in rest[..end].lines() {
            if let Some((k, v)) = line.split_once(':') {
                let k = k.trim().to_string();
                let v = v.trim().to_string();
                if !k.is_empty() && !v.is_empty() {
                    fm.insert(k, v);
                }
            }
        }
        (fm, rest[end + 4..].trim_start().to_string())
    } else {
        (fm, content.to_string())
    }
}

/// Parse tools from frontmatter (comma-separated).
pub fn parse_tools_from_frontmatter(tools_str: Option<&str>) -> Option<Vec<String>> {
    tools_str.map(|s| {
        s.split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_description() {
        assert_eq!(extract_description("# Hello World\nContent"), "Hello World");
        assert_eq!(extract_description("First paragraph"), "First paragraph");
        assert_eq!(extract_description(""), "");
    }

    #[test]
    fn test_project_dirs() {
        let dirs = get_project_dirs_up_to_home(Path::new("/home/user/project/src"));
        assert!(dirs.len() >= 2);
        assert_eq!(dirs[0], PathBuf::from("/home/user/project/src"));
    }

    #[test]
    fn test_parse_tools() {
        assert_eq!(
            parse_tools_from_frontmatter(Some("Read, Grep, Bash")),
            Some(vec!["Read".into(), "Grep".into(), "Bash".into()])
        );
        assert_eq!(parse_tools_from_frontmatter(None), None);
    }
}

//! Attachments system: processes @-mentioned files, images, and context injections in user input.
//! Processes @-mentioned files, images, and context injections in user input.

use std::path::{Path, PathBuf};

/// Attachment types (discriminated union for different content kinds).
#[derive(Debug, Clone)]
pub enum Attachment {
    /// File content attached via @mention.
    File {
        filename: String,
        content: String,
        truncated: bool,
        display_path: String,
    },
    /// Image attached (base64 encoded).
    Image {
        filename: String,
        media_type: String,
        data: String, // base64
        display_path: String,
    },
    /// Reference to a file already read (compact, no content).
    CompactFileReference {
        filename: String,
        display_path: String,
    },
    /// Memory file (CLAUDE.md, rules) injected as context.
    NestedMemory {
        path: String,
        content: String,
        display_path: String,
    },
    /// Agent mention (@agent-type).
    AgentMention { agent_type: String },
}

const MAX_FILE_SIZE: usize = 500_000; // 500KB
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// Process @-mentioned files in user input.
/// Returns (cleaned_input, attachments).
pub fn process_at_mentions(input: &str, cwd: &Path) -> (String, Vec<Attachment>) {
    let mut attachments = Vec::new();
    let mut cleaned = String::new();
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();

    while i < chars.len() {
        if chars[i] == '@' && (i == 0 || chars[i - 1].is_whitespace()) {
            // Try to extract a file path after @
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && !chars[end].is_whitespace() {
                end += 1;
            }
            if end > start {
                let path_str: String = chars[start..end].iter().collect();

                // #6: @url — fetch web content
                if path_str.starts_with("http://") || path_str.starts_with("https://") {
                    if let Ok(resp) = reqwest::blocking::get(&path_str) {
                        if let Ok(text) = resp.text() {
                            let truncated = if text.len() > 50_000 {
                                format!(
                                    "{}...\n[truncated, {} bytes total]",
                                    &text[..50_000],
                                    text.len()
                                )
                            } else {
                                text
                            };
                            attachments.push(Attachment::File {
                                filename: path_str.clone(),
                                content: truncated,
                                truncated: false,
                                display_path: path_str.clone(),
                            });
                            cleaned.push_str(&format!("[attached: {path_str}]"));
                            i = end;
                            continue;
                        }
                    }
                }

                let resolved = resolve_at_path(&path_str, cwd);
                if resolved.exists() {
                    if resolved.is_dir() {
                        // U04: @dir/ — list directory contents
                        if let Ok(entries) = std::fs::read_dir(&resolved) {
                            let listing: Vec<String> = entries
                                .filter_map(|e| e.ok())
                                .map(|e| {
                                    let name = e.file_name().to_string_lossy().to_string();
                                    if e.path().is_dir() {
                                        format!("{name}/")
                                    } else {
                                        name
                                    }
                                })
                                .collect();
                            attachments.push(Attachment::File {
                                filename: path_str.clone(),
                                content: format!(
                                    "Directory listing of {}:\n{}",
                                    resolved.display(),
                                    listing.join("\n")
                                ),
                                truncated: false,
                                display_path: path_str.clone(),
                            });
                            cleaned.push_str(&format!("[attached: {path_str}]"));
                            i = end;
                            continue;
                        }
                    } else if let Some(attachment) = read_file_attachment(&resolved, cwd) {
                        attachments.push(attachment);
                        // Replace @path with just the filename in cleaned input
                        cleaned.push_str(&format!(
                            "[attached: {}]",
                            resolved
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&path_str)
                        ));
                        i = end;
                        continue;
                    }
                }
            }
        }
        cleaned.push(chars[i]);
        i += 1;
    }

    (cleaned, attachments)
}

/// Resolve an @-mentioned path relative to cwd.
fn resolve_at_path(path: &str, cwd: &Path) -> PathBuf {
    if path.starts_with('/') {
        PathBuf::from(path)
    } else if path.starts_with("~/") {
        dirs::home_dir()
            .unwrap_or_default()
            .join(path.strip_prefix("~/").unwrap_or(path))
    } else {
        cwd.join(path)
    }
}

/// Read a file and create an attachment.
fn read_file_attachment(path: &Path, cwd: &Path) -> Option<Attachment> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let display_path = path
        .strip_prefix(cwd)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string());
    let filename = path.file_name()?.to_str()?.to_string();

    // Image files → base64 encode
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        let data = std::fs::read(path).ok()?;
        if data.len() > 10_000_000 {
            return None;
        } // 10MB limit for images
        let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
        let media_type = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => "application/octet-stream",
        };
        return Some(Attachment::Image {
            filename,
            media_type: media_type.to_string(),
            data: base64,
            display_path,
        });
    }

    // Text files → read content
    let content = std::fs::read_to_string(path).ok()?;
    let truncated = content.len() > MAX_FILE_SIZE;
    let content = if truncated {
        format!(
            "{}...\n[truncated, {} bytes total]",
            &content[..MAX_FILE_SIZE],
            content.len()
        )
    } else {
        content
    };

    Some(Attachment::File {
        filename,
        content,
        truncated,
        display_path,
    })
}

/// Convert attachments to API message content blocks.
pub fn attachments_to_content(attachments: &[Attachment]) -> Vec<serde_json::Value> {
    attachments
        .iter()
        .map(|a| match a {
            Attachment::File {
                content,
                display_path,
                ..
            } => {
                serde_json::json!({
                    "type": "text",
                    "text": format!("<file path=\"{display_path}\">\n{content}\n</file>")
                })
            }
            Attachment::Image {
                media_type, data, ..
            } => {
                serde_json::json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": data,
                    }
                })
            }
            Attachment::NestedMemory { path, content, .. } => {
                serde_json::json!({
                    "type": "text",
                    "text": format!("<memory path=\"{path}\">\n{content}\n</memory>")
                })
            }
            Attachment::CompactFileReference { display_path, .. } => {
                serde_json::json!({
                    "type": "text",
                    "text": format!("[File previously read: {display_path}]")
                })
            }
            Attachment::AgentMention { agent_type } => {
                serde_json::json!({
                    "type": "text",
                    "text": format!("[Agent mention: {agent_type}]")
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_process_at_mentions_no_mentions() {
        let (cleaned, attachments) = process_at_mentions("hello world", Path::new("/tmp"));
        assert_eq!(cleaned, "hello world");
        assert!(attachments.is_empty());
    }

    #[test]
    fn test_process_at_mentions_with_file() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "file content").unwrap();

        let input = format!("check @test.txt please");
        let (cleaned, attachments) = process_at_mentions(&input, dir.path());
        assert_eq!(attachments.len(), 1);
        assert!(cleaned.contains("[attached: test.txt]"));
        match &attachments[0] {
            Attachment::File { content, .. } => assert_eq!(content, "file content"),
            _ => panic!("Expected File attachment"),
        }
    }

    #[test]
    fn test_process_at_mentions_nonexistent() {
        let (cleaned, attachments) =
            process_at_mentions("check @nonexistent.txt", Path::new("/tmp"));
        assert!(attachments.is_empty());
        assert!(cleaned.contains("@nonexistent.txt"));
    }

    #[test]
    fn test_resolve_at_path() {
        assert_eq!(
            resolve_at_path("/etc/hosts", Path::new("/tmp")),
            PathBuf::from("/etc/hosts")
        );
        assert_eq!(
            resolve_at_path("file.txt", Path::new("/project")),
            PathBuf::from("/project/file.txt")
        );
    }

    #[test]
    fn test_attachments_to_content() {
        let attachments = vec![Attachment::File {
            filename: "test.rs".into(),
            content: "fn main() {}".into(),
            truncated: false,
            display_path: "src/test.rs".into(),
        }];
        let content = attachments_to_content(&attachments);
        assert_eq!(content.len(), 1);
        assert!(content[0]["text"].as_str().unwrap().contains("test.rs"));
    }
}

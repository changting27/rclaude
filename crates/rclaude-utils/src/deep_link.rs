//! Deep link handling matching utils/deepLink/.
//! Parses and builds claude:// protocol URIs.

/// Deep link action types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLinkAction {
    /// Open a conversation with a prompt.
    Prompt { text: String, cwd: Option<String> },
    /// Resume a session.
    Resume { session_id: String },
    /// Open a file at a line.
    OpenFile { path: String, line: Option<u32> },
    /// Unknown action.
    Unknown(String),
}

/// Parse a deep link URI (claude://action?params).
pub fn parse_deep_link(uri: &str) -> DeepLinkAction {
    let uri = uri.strip_prefix("claude://").unwrap_or(uri);
    let (action, query) = uri.split_once('?').unwrap_or((uri, ""));
    let params: std::collections::HashMap<&str, String> = query
        .split('&')
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k, urlencoding::decode(v).unwrap_or_default().into_owned()))
        })
        .collect();

    match action {
        "prompt" => DeepLinkAction::Prompt {
            text: params.get("text").cloned().unwrap_or_default(),
            cwd: params.get("cwd").cloned(),
        },
        "resume" => DeepLinkAction::Resume {
            session_id: params.get("session").cloned().unwrap_or_default(),
        },
        "open" => DeepLinkAction::OpenFile {
            path: params.get("path").cloned().unwrap_or_default(),
            line: params.get("line").and_then(|l| l.parse().ok()),
        },
        other => DeepLinkAction::Unknown(other.to_string()),
    }
}

/// Build a deep link URI from an action.
pub fn build_deep_link(action: &DeepLinkAction) -> String {
    match action {
        DeepLinkAction::Prompt { text, cwd } => {
            let mut uri = format!("claude://prompt?text={}", urlencoding::encode(text));
            if let Some(cwd) = cwd {
                uri.push_str(&format!("&cwd={}", urlencoding::encode(cwd)));
            }
            uri
        }
        DeepLinkAction::Resume { session_id } => format!(
            "claude://resume?session={}",
            urlencoding::encode(session_id)
        ),
        DeepLinkAction::OpenFile { path, line } => {
            let mut uri = format!("claude://open?path={}", urlencoding::encode(path));
            if let Some(line) = line {
                uri.push_str(&format!("&line={line}"));
            }
            uri
        }
        DeepLinkAction::Unknown(s) => format!("claude://{s}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prompt() {
        let action = parse_deep_link("claude://prompt?text=hello%20world&cwd=%2Ftmp");
        assert_eq!(
            action,
            DeepLinkAction::Prompt {
                text: "hello world".into(),
                cwd: Some("/tmp".into())
            }
        );
    }

    #[test]
    fn test_roundtrip() {
        let action = DeepLinkAction::OpenFile {
            path: "/src/main.rs".into(),
            line: Some(42),
        };
        let uri = build_deep_link(&action);
        let parsed = parse_deep_link(&uri);
        assert_eq!(parsed, action);
    }
}

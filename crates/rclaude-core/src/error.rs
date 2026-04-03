use thiserror::Error;

#[derive(Error, Debug)]
pub enum RclaudeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API error: {message}")]
    Api {
        message: String,
        status: Option<u16>,
    },

    #[error("Prompt too long: {message}")]
    PromptTooLong {
        message: String,
        actual_tokens: Option<u64>,
        limit_tokens: Option<u64>,
    },

    #[error("Max output tokens reached")]
    MaxOutputTokens,

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Aborted")]
    Aborted,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RclaudeError>;

impl RclaudeError {
    /// Parse prompt-too-long token counts from an API error message.
    /// Pattern: "prompt is too long: 250000 tokens > 200000"
    pub fn parse_prompt_too_long(message: &str) -> Option<(u64, u64)> {
        let re = regex::Regex::new(r"(\d+)\s*tokens?\s*>\s*(\d+)").ok()?;
        let caps = re.captures(message)?;
        let actual = caps.get(1)?.as_str().parse().ok()?;
        let limit = caps.get(2)?.as_str().parse().ok()?;
        Some((actual, limit))
    }

    /// Classify an API error response into the appropriate variant.
    pub fn from_api_error(status: u16, body: &str) -> Self {
        let lower = body.to_lowercase();
        if status == 400
            && (lower.contains("prompt is too long") || lower.contains("too many tokens"))
        {
            let counts = Self::parse_prompt_too_long(body);
            return Self::PromptTooLong {
                message: body.to_string(),
                actual_tokens: counts.map(|(a, _)| a),
                limit_tokens: counts.map(|(_, l)| l),
            };
        }
        Self::Api {
            message: body.to_string(),
            status: Some(status),
        }
    }

    /// User-friendly error message for display.
    pub fn user_message(&self) -> String {
        match self {
            Self::Api {
                status: Some(429), ..
            } => "Rate limited by API. Waiting to retry...".into(),
            Self::Api {
                status: Some(529), ..
            } => "API is overloaded. Retrying...".into(),
            Self::Api {
                status: Some(401), ..
            } => "Invalid API key. Run /login to reconfigure.".into(),
            Self::Api {
                status: Some(403), ..
            } => "Access denied. Check your API key permissions.".into(),
            Self::PromptTooLong { .. } => "Conversation too long. Auto-compacting...".into(),
            Self::MaxOutputTokens => "Output was truncated. Continuing...".into(),
            Self::Timeout(_) => "Request timed out. Retrying...".into(),
            other => other.to_string(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Api {
                status: Some(429 | 529 | 500..=599),
                ..
            } | Self::Timeout(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let e = RclaudeError::Tool("bad input".into());
        assert_eq!(e.to_string(), "Tool error: bad input");
    }

    #[test]
    fn test_api_error() {
        let e = RclaudeError::Api {
            message: "rate limit".into(),
            status: Some(429),
        };
        assert!(e.to_string().contains("rate limit"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let e: RclaudeError = io_err.into();
        assert!(e.to_string().contains("IO error"));
    }

    #[test]
    fn test_result_type() {
        let ok: Result<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);
        let err: Result<i32> = Err(RclaudeError::Aborted);
        assert!(err.is_err());
    }

    #[test]
    fn test_parse_prompt_too_long() {
        let counts = RclaudeError::parse_prompt_too_long(
            "prompt is too long: 250000 tokens > 200000 token limit",
        );
        assert_eq!(counts, Some((250000, 200000)));
    }

    #[test]
    fn test_from_api_error_ptl() {
        let e = RclaudeError::from_api_error(400, "prompt is too long: 300000 tokens > 200000");
        assert!(matches!(e, RclaudeError::PromptTooLong { .. }));
    }

    #[test]
    fn test_from_api_error_normal() {
        let e = RclaudeError::from_api_error(500, "internal server error");
        assert!(matches!(e, RclaudeError::Api { .. }));
    }

    #[test]
    fn test_user_message() {
        let e = RclaudeError::Api {
            message: "x".into(),
            status: Some(429),
        };
        assert!(e.user_message().contains("Rate limited"));
    }

    #[test]
    fn test_is_retryable() {
        assert!(RclaudeError::Api {
            message: "".into(),
            status: Some(429)
        }
        .is_retryable());
        assert!(RclaudeError::Api {
            message: "".into(),
            status: Some(529)
        }
        .is_retryable());
        assert!(!RclaudeError::Api {
            message: "".into(),
            status: Some(401)
        }
        .is_retryable());
    }
}

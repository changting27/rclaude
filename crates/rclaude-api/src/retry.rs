//! API retry logic with exponential backoff and rate-limit handling.
//!
//! Key behaviors:
//! - Exponential backoff with jitter
//! - Retry-after header respect
//! - 529 overload tracking (max 3 before fallback)
//! - 429 rate limit handling
//! - Status messages during waits
//! - Auth error detection (no retry)

use std::time::Duration;

const BASE_DELAY_MS: u64 = 1000;
const MAX_DELAY_MS: u64 = 32000;
const MAX_529_RETRIES: u32 = 3;

/// Calculate retry delay with exponential backoff + jitter.
pub fn get_retry_delay(
    attempt: u32,
    retry_after_header: Option<&str>,
    max_delay_ms: u64,
) -> Duration {
    if let Some(header) = retry_after_header {
        if let Ok(seconds) = header.parse::<u64>() {
            return Duration::from_secs(seconds);
        }
    }
    let base = (BASE_DELAY_MS * 2u64.pow(attempt.saturating_sub(1))).min(max_delay_ms);
    let jitter = (rand::random::<f64>() * 0.25 * base as f64) as u64;
    Duration::from_millis(base + jitter)
}

/// Classify API errors for retry decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// 429 rate limit or 529 overloaded or 5xx server error.
    Retryable,
    /// 400 with context overflow / prompt too long.
    ContextOverflow,
    /// 401/403 authentication error.
    AuthError,
    /// Other 4xx — not retryable.
    NotRetryable,
    /// User abort.
    Aborted,
}

pub fn classify_error(status: u16) -> ApiErrorKind {
    match status {
        429 | 529 => ApiErrorKind::Retryable,
        500..=599 => ApiErrorKind::Retryable,
        400 => ApiErrorKind::ContextOverflow,
        401 | 403 => ApiErrorKind::AuthError,
        _ => ApiErrorKind::NotRetryable,
    }
}

/// Check if a 529 error (overloaded).
pub fn is_overloaded(status: u16) -> bool {
    status == 529
}

/// Check if rate limited.
pub fn is_rate_limited(status: u16) -> bool {
    status == 429
}

/// Status message emitted during retry waits.
#[derive(Debug, Clone)]
pub enum RetryStatus {
    /// Retrying after a delay.
    Retrying {
        attempt: u32,
        max_attempts: u32,
        delay: Duration,
        reason: String,
    },
    /// Giving up after max retries.
    GaveUp { reason: String },
    /// Non-retryable error.
    Fatal { kind: ApiErrorKind, message: String },
}

/// Retry state tracker for a single request.
pub struct RetryState {
    pub max_retries: u32,
    pub attempt: u32,
    pub overload_count: u32,
    pub max_overload_retries: u32,
}

impl RetryState {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            attempt: 0,
            overload_count: 0,
            max_overload_retries: MAX_529_RETRIES,
        }
    }

    /// Determine what to do with an error. Returns None if should retry, Some(status) if should stop.
    pub fn handle_error(
        &mut self,
        status: u16,
        retry_after: Option<&str>,
        body: &str,
    ) -> (Option<RetryStatus>, Duration) {
        let kind = classify_error(status);

        match kind {
            ApiErrorKind::AuthError => {
                return (
                    Some(RetryStatus::Fatal {
                        kind,
                        message: format!("Authentication error (HTTP {status}): {body}"),
                    }),
                    Duration::ZERO,
                );
            }
            ApiErrorKind::NotRetryable => {
                return (
                    Some(RetryStatus::Fatal {
                        kind,
                        message: format!("Non-retryable error (HTTP {status}): {body}"),
                    }),
                    Duration::ZERO,
                );
            }
            ApiErrorKind::ContextOverflow => {
                // Check if it's actually a prompt-too-long error
                if body.contains("prompt is too long") || body.contains("too many tokens") {
                    return (
                        Some(RetryStatus::Fatal {
                            kind: ApiErrorKind::ContextOverflow,
                            message: format!("Context overflow: {body}"),
                        }),
                        Duration::ZERO,
                    );
                }
                // Other 400 errors are not retryable
                return (
                    Some(RetryStatus::Fatal {
                        kind: ApiErrorKind::NotRetryable,
                        message: format!("Bad request (HTTP 400): {body}"),
                    }),
                    Duration::ZERO,
                );
            }
            ApiErrorKind::Retryable => {
                if is_overloaded(status) {
                    self.overload_count += 1;
                    if self.overload_count > self.max_overload_retries {
                        return (
                            Some(RetryStatus::GaveUp {
                                reason: format!(
                                    "Too many overload errors ({} consecutive 529s)",
                                    self.overload_count
                                ),
                            }),
                            Duration::ZERO,
                        );
                    }
                }
            }
            ApiErrorKind::Aborted => {
                return (
                    Some(RetryStatus::Fatal {
                        kind,
                        message: "Request aborted".into(),
                    }),
                    Duration::ZERO,
                );
            }
        }

        self.attempt += 1;
        if self.attempt > self.max_retries {
            return (
                Some(RetryStatus::GaveUp {
                    reason: format!("Max retries ({}) exceeded", self.max_retries),
                }),
                Duration::ZERO,
            );
        }

        let delay = get_retry_delay(self.attempt, retry_after, MAX_DELAY_MS);
        let reason = if is_rate_limited(status) {
            "Rate limited (429)".to_string()
        } else if is_overloaded(status) {
            format!(
                "API overloaded (529, attempt {}/{})",
                self.overload_count, self.max_overload_retries
            )
        } else {
            format!("Server error (HTTP {status})")
        };

        (
            Some(RetryStatus::Retrying {
                attempt: self.attempt,
                max_attempts: self.max_retries,
                delay,
                reason,
            }),
            delay,
        )
    }
}

/// Default max retries.
pub fn default_max_retries() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delay_exponential() {
        let d1 = get_retry_delay(1, None, MAX_DELAY_MS);
        let d2 = get_retry_delay(2, None, MAX_DELAY_MS);
        assert!(d2 > d1);
    }

    #[test]
    fn test_retry_delay_header() {
        let d = get_retry_delay(1, Some("5"), MAX_DELAY_MS);
        assert_eq!(d, Duration::from_secs(5));
    }

    #[test]
    fn test_classify_error() {
        assert_eq!(classify_error(429), ApiErrorKind::Retryable);
        assert_eq!(classify_error(529), ApiErrorKind::Retryable);
        assert_eq!(classify_error(500), ApiErrorKind::Retryable);
        assert_eq!(classify_error(401), ApiErrorKind::AuthError);
        assert_eq!(classify_error(404), ApiErrorKind::NotRetryable);
        assert_eq!(classify_error(400), ApiErrorKind::ContextOverflow);
    }

    #[test]
    fn test_retry_state_429() {
        let mut state = RetryState::new(3);
        let (status, delay) = state.handle_error(429, None, "rate limited");
        assert!(matches!(status, Some(RetryStatus::Retrying { .. })));
        assert!(delay > Duration::ZERO);
    }

    #[test]
    fn test_retry_state_auth_error() {
        let mut state = RetryState::new(3);
        let (status, _) = state.handle_error(401, None, "unauthorized");
        assert!(matches!(
            status,
            Some(RetryStatus::Fatal {
                kind: ApiErrorKind::AuthError,
                ..
            })
        ));
    }

    #[test]
    fn test_retry_state_529_max() {
        let mut state = RetryState::new(10);
        // First 3 should retry
        for _ in 0..3 {
            let (status, _) = state.handle_error(529, None, "overloaded");
            assert!(matches!(status, Some(RetryStatus::Retrying { .. })));
        }
        // 4th should give up
        let (status, _) = state.handle_error(529, None, "overloaded");
        assert!(matches!(status, Some(RetryStatus::GaveUp { .. })));
    }

    #[test]
    fn test_retry_state_max_retries() {
        let mut state = RetryState::new(2);
        let (s1, _) = state.handle_error(500, None, "error");
        assert!(matches!(s1, Some(RetryStatus::Retrying { .. })));
        let (s2, _) = state.handle_error(500, None, "error");
        assert!(matches!(s2, Some(RetryStatus::Retrying { .. })));
        let (s3, _) = state.handle_error(500, None, "error");
        assert!(matches!(s3, Some(RetryStatus::GaveUp { .. })));
    }
}

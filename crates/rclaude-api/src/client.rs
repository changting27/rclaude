use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::Value;

use crate::streaming::MessageStream;
use crate::types::{CreateMessageRequest, CreateMessageResponse};
use rclaude_core::error::{RclaudeError, Result};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// Anthropic API client with connection pooling and proxy support.
#[derive(Debug, Clone)]
pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl AnthropicClient {
    /// Create a new client with the given API key.
    /// Respects HTTPS_PROXY / HTTP_PROXY / ALL_PROXY environment variables.
    pub fn new(api_key: impl Into<String>) -> Self {
        let mut builder = reqwest::Client::builder()
            .pool_max_idle_per_host(4)
            .tcp_keepalive(std::time::Duration::from_secs(30));

        // Proxy support: check HTTPS_PROXY / ALL_PROXY env vars
        if let Ok(proxy_url) = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("ALL_PROXY"))
            .or_else(|_| std::env::var("all_proxy"))
        {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
                tracing::debug!("Using proxy: {proxy_url}");
            }
        }

        Self {
            http: builder.build().unwrap_or_else(|_| reqwest::Client::new()),
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Get a reference to the underlying HTTP client (for connection reuse in streaming).
    pub fn http_client(&self) -> &reqwest::Client {
        &self.http
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Build default headers for API requests.
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key).unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert("anthropic-version", HeaderValue::from_static(API_VERSION));
        headers
    }

    /// Create a message with retry on transient errors (429, 529, 5xx).
    /// Uses exponential backoff with jitter.
    pub async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse> {
        let url = format!("{}/v1/messages", self.base_url);
        let max_retries = 3;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Exponential backoff with jitter
                let base_ms = 1000u64 * 2u64.pow(attempt as u32 - 1);
                let jitter = rand::random::<u64>() % (base_ms / 2 + 1);
                let delay = std::time::Duration::from_millis(base_ms + jitter);
                tracing::warn!(
                    "Retrying API call (attempt {}/{}), waiting {:?}",
                    attempt + 1,
                    max_retries + 1,
                    delay
                );
                tokio::time::sleep(delay).await;
            }

            let resp = match self
                .http
                .post(&url)
                .headers(self.headers())
                .json(request)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(RclaudeError::Api {
                        message: e.to_string(),
                        status: e.status().map(|s| s.as_u16()),
                    });
                    continue;
                }
            };

            let status = resp.status();
            let status_code = status.as_u16();

            // Retry on 429 (rate limit), 529 (overloaded), or 5xx
            if status_code == 429 || status_code == 529 || (500..600).contains(&status_code) {
                let body = resp.text().await.unwrap_or_default();
                last_error = Some(RclaudeError::Api {
                    message: format!("HTTP {status_code}: {body}"),
                    status: Some(status_code),
                });
                continue;
            }

            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                let error_msg = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
                    .unwrap_or(body);
                return Err(RclaudeError::Api {
                    message: error_msg,
                    status: Some(status_code),
                });
            }

            return resp
                .json::<CreateMessageResponse>()
                .await
                .map_err(|e| RclaudeError::Api {
                    message: format!("Failed to parse response: {e}"),
                    status: None,
                });
        }

        Err(last_error.unwrap_or_else(|| RclaudeError::Api {
            message: "Max retries exceeded".into(),
            status: None,
        }))
    }

    /// Create a streaming message request, reusing the client's connection pool.
    pub fn create_message_stream(&self, request: &CreateMessageRequest) -> Result<MessageStream> {
        crate::streaming::create_stream_with_client(
            &self.http,
            &self.api_key,
            &self.base_url,
            request,
        )
    }
}

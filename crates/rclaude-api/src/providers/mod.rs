pub mod anthropic;
pub mod bedrock;
pub mod vertex;

use crate::types::{CreateMessageRequest, CreateMessageResponse};
use rclaude_core::error::Result;

/// API provider trait for different Claude hosting backends.
#[async_trait::async_trait]
pub trait ApiProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Send a message creation request.
    async fn create_message(&self, request: &CreateMessageRequest)
        -> Result<CreateMessageResponse>;
}

/// Supported API provider types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderType {
    Anthropic,
    Bedrock,
    Vertex,
}

impl ProviderType {
    /// Detect provider from environment or base URL.
    pub fn detect(base_url: Option<&str>) -> Self {
        if let Some(url) = base_url {
            if url.contains("bedrock") || url.contains("amazonaws.com") {
                return Self::Bedrock;
            }
            if url.contains("vertex") || url.contains("googleapis.com") {
                return Self::Vertex;
            }
        }
        if std::env::var("AWS_REGION").is_ok() && std::env::var("ANTHROPIC_API_KEY").is_err() {
            return Self::Bedrock;
        }
        if std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_ok()
            && std::env::var("ANTHROPIC_API_KEY").is_err()
        {
            return Self::Vertex;
        }
        Self::Anthropic
    }
}

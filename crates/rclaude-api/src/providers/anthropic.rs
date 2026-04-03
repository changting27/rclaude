//! Direct Anthropic API provider (api.anthropic.com).

use crate::client::AnthropicClient;
use crate::types::{CreateMessageRequest, CreateMessageResponse};
use rclaude_core::error::Result;

pub struct AnthropicProvider {
    client: AnthropicClient,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, base_url: Option<&str>) -> Self {
        let mut client = AnthropicClient::new(api_key);
        if let Some(url) = base_url {
            client = client.with_base_url(url);
        }
        Self { client }
    }
}

#[async_trait::async_trait]
impl super::ApiProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse> {
        self.client.create_message(request).await
    }
}

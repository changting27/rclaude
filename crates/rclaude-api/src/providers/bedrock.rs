//! AWS Bedrock provider for Claude models.
//!
//! Uses the Bedrock Runtime converse-stream API.
//! Requires: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_REGION.
//! Optional: AWS_SESSION_TOKEN (for temporary credentials).

use crate::types::{CreateMessageRequest, CreateMessageResponse};
use rclaude_core::error::{RclaudeError, Result};

/// Default Bedrock model ID.
const DEFAULT_MODEL: &str = "anthropic.claude-sonnet-4-20250514-v1:0";

pub struct BedrockProvider {
    region: String,
    model_id: String,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
}

impl BedrockProvider {
    /// Create from environment variables.
    pub fn from_env() -> Result<Self> {
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());

        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .map_err(|_| RclaudeError::Config("AWS_ACCESS_KEY_ID not set".into()))?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .map_err(|_| RclaudeError::Config("AWS_SECRET_ACCESS_KEY not set".into()))?;
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();

        Ok(Self {
            region,
            model_id: DEFAULT_MODEL.to_string(),
            access_key,
            secret_key,
            session_token,
        })
    }

    pub fn with_model(mut self, model_id: &str) -> Self {
        self.model_id = model_id.to_string();
        self
    }

    /// Build the Bedrock endpoint URL.
    fn endpoint(&self) -> String {
        std::env::var("ANTHROPIC_BEDROCK_BASE_URL")
            .unwrap_or_else(|_| format!("https://bedrock-runtime.{}.amazonaws.com", self.region))
    }

    /// Sign a request with AWS SigV4.
    fn sign_request(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
        timestamp: &str,
        date: &str,
    ) -> std::collections::HashMap<String, String> {
        use hmac::{Hmac, Mac};
        use sha2::{Digest, Sha256};

        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_default();
        let path = url::Url::parse(url)
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| "/".to_string());

        // Payload hash
        let payload_hash = hex::encode(Sha256::digest(body));

        // Canonical request
        let signed_headers = if self.session_token.is_some() {
            "content-type;host;x-amz-date;x-amz-security-token"
        } else {
            "content-type;host;x-amz-date"
        };

        let mut canonical = format!(
            "{method}\n{path}\n\ncontent-type:application/json\nhost:{host}\nx-amz-date:{timestamp}\n",
        );
        if let Some(ref token) = self.session_token {
            canonical.push_str(&format!("x-amz-security-token:{token}\n"));
        }
        canonical.push_str(&format!("\n{signed_headers}\n{payload_hash}"));

        let canonical_hash = hex::encode(Sha256::digest(canonical.as_bytes()));

        // String to sign
        let credential_scope = format!("{date}/{}/bedrock/aws4_request", self.region);
        let string_to_sign =
            format!("AWS4-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{canonical_hash}");

        // Signing key
        type HmacSha256 = Hmac<Sha256>;
        let k_date = HmacSha256::new_from_slice(format!("AWS4{}", self.secret_key).as_bytes())
            .unwrap()
            .chain_update(date.as_bytes())
            .finalize()
            .into_bytes();
        let k_region = HmacSha256::new_from_slice(&k_date)
            .unwrap()
            .chain_update(self.region.as_bytes())
            .finalize()
            .into_bytes();
        let k_service = HmacSha256::new_from_slice(&k_region)
            .unwrap()
            .chain_update(b"bedrock")
            .finalize()
            .into_bytes();
        let k_signing = HmacSha256::new_from_slice(&k_service)
            .unwrap()
            .chain_update(b"aws4_request")
            .finalize()
            .into_bytes();

        let signature = hex::encode(
            HmacSha256::new_from_slice(&k_signing)
                .unwrap()
                .chain_update(string_to_sign.as_bytes())
                .finalize()
                .into_bytes(),
        );

        let auth = format!(
            "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key
        );

        let mut headers = std::collections::HashMap::new();
        headers.insert("Authorization".into(), auth);
        headers.insert("x-amz-date".into(), timestamp.to_string());
        headers.insert("x-amz-content-sha256".into(), payload_hash);
        if let Some(ref token) = self.session_token {
            headers.insert("x-amz-security-token".into(), token.clone());
        }
        headers
    }
}

#[async_trait::async_trait]
impl super::ApiProvider for BedrockProvider {
    fn name(&self) -> &str {
        "bedrock"
    }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse> {
        let url = format!("{}/model/{}/invoke", self.endpoint(), self.model_id);

        // Bedrock uses the same request format but without model field
        let mut body = serde_json::to_value(request)?;
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
            obj.insert("anthropic_version".into(), "bedrock-2023-05-31".into());
        }
        let body_bytes = serde_json::to_vec(&body)?;

        let now = chrono::Utc::now();
        let timestamp = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();

        let sig_headers = self.sign_request("POST", &url, &body_bytes, &timestamp, &date);

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("content-type", "application/json")
            .body(body_bytes);

        for (k, v) in &sig_headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await.map_err(|e| RclaudeError::Api {
            message: format!("Bedrock request failed: {e}"),
            status: e.status().map(|s| s.as_u16()),
        })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RclaudeError::Api {
                message: format!("Bedrock HTTP {}: {body}", status.as_u16()),
                status: Some(status.as_u16()),
            });
        }

        resp.json::<CreateMessageResponse>()
            .await
            .map_err(|e| RclaudeError::Api {
                message: format!("Failed to parse Bedrock response: {e}"),
                status: None,
            })
    }
}

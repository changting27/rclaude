//! GCP Vertex AI provider for Claude models.
//!
//! Uses the Vertex AI API with Google OAuth2 access tokens.
//! Requires: GOOGLE_CLOUD_PROJECT, and one of:
//!   - GOOGLE_APPLICATION_CREDENTIALS (service account JSON)
//!   - gcloud auth application-default login (ADC)

use crate::types::{CreateMessageRequest, CreateMessageResponse};
use rclaude_core::error::{RclaudeError, Result};

const DEFAULT_MODEL: &str = "claude-sonnet-4@20250514";

pub struct VertexProvider {
    project_id: String,
    region: String,
    model_id: String,
}

impl VertexProvider {
    /// Create from environment variables.
    pub fn from_env() -> Result<Self> {
        let project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("GCLOUD_PROJECT"))
            .or_else(|_| std::env::var("CLOUD_ML_PROJECT_ID"))
            .map_err(|_| RclaudeError::Config("GOOGLE_CLOUD_PROJECT not set".into()))?;

        let region = std::env::var("GOOGLE_CLOUD_REGION")
            .or_else(|_| std::env::var("CLOUD_ML_REGION"))
            .unwrap_or_else(|_| "us-east5".to_string());

        Ok(Self {
            project_id,
            region,
            model_id: DEFAULT_MODEL.to_string(),
        })
    }

    pub fn with_model(mut self, model_id: &str) -> Self {
        self.model_id = model_id.to_string();
        self
    }

    /// Get the Vertex AI endpoint URL.
    fn endpoint(&self) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:rawPredict",
            self.region, self.project_id, self.region, self.model_id
        )
    }

    /// Get an access token from gcloud CLI or ADC.
    async fn get_access_token(&self) -> Result<String> {
        // Try gcloud CLI first
        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "application-default", "print-access-token"])
            .output()
            .await
            .map_err(|e| RclaudeError::Config(format!(
                "Failed to get GCP access token. Run 'gcloud auth application-default login'. Error: {e}"
            )))?;

        if output.status.success() {
            let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !token.is_empty() {
                return Ok(token);
            }
        }

        Err(RclaudeError::Config(
            "Could not obtain GCP access token. Run: gcloud auth application-default login".into(),
        ))
    }
}

#[async_trait::async_trait]
impl super::ApiProvider for VertexProvider {
    fn name(&self) -> &str {
        "vertex"
    }

    async fn create_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<CreateMessageResponse> {
        let token = self.get_access_token().await?;
        let url = self.endpoint();

        // Vertex uses the same request format but with anthropic_version
        let mut body = serde_json::to_value(request)?;
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
            obj.insert("anthropic_version".into(), "vertex-2023-10-16".into());
        }

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| RclaudeError::Api {
                message: format!("Vertex request failed: {e}"),
                status: e.status().map(|s| s.as_u16()),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(RclaudeError::Api {
                message: format!("Vertex HTTP {}: {body}", status.as_u16()),
                status: Some(status.as_u16()),
            });
        }

        resp.json::<CreateMessageResponse>()
            .await
            .map_err(|e| RclaudeError::Api {
                message: format!("Failed to parse Vertex response: {e}"),
                status: None,
            })
    }
}

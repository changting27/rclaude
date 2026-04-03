//! Model management: configs, aliases, provider-specific IDs, capabilities, validation.
//! Model configs, aliases, provider-specific IDs, capabilities, validation.

/// API provider types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiProvider {
    FirstParty,
    Bedrock,
    Vertex,
}

/// Per-provider model ID strings.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub first_party: &'static str,
    pub bedrock: &'static str,
    pub vertex: &'static str,
}

/// Model info with capabilities.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub key: &'static str,
    pub config: ModelConfig,
    pub family: &'static str,
    pub context_window: usize,
    pub max_output: u32,
    pub supports_thinking: bool,
    pub supports_images: bool,
}

// ── Model configs ──

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        key: "haiku45",
        config: ModelConfig {
            first_party: "claude-haiku-4-5-20251001",
            bedrock: "us.anthropic.claude-haiku-4-5-20251001-v1:0",
            vertex: "claude-haiku-4-5@20251001",
        },
        family: "haiku",
        context_window: 200_000,
        max_output: 8_192,
        supports_thinking: true,
        supports_images: true,
    },
    ModelInfo {
        key: "sonnet40",
        config: ModelConfig {
            first_party: "claude-sonnet-4-20250514",
            bedrock: "us.anthropic.claude-sonnet-4-20250514-v1:0",
            vertex: "claude-sonnet-4@20250514",
        },
        family: "sonnet",
        context_window: 200_000,
        max_output: 16_000,
        supports_thinking: true,
        supports_images: true,
    },
    ModelInfo {
        key: "sonnet45",
        config: ModelConfig {
            first_party: "claude-sonnet-4-5-20250929",
            bedrock: "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
            vertex: "claude-sonnet-4-5@20250929",
        },
        family: "sonnet",
        context_window: 200_000,
        max_output: 16_000,
        supports_thinking: true,
        supports_images: true,
    },
    ModelInfo {
        key: "opus40",
        config: ModelConfig {
            first_party: "claude-opus-4-20250514",
            bedrock: "us.anthropic.claude-opus-4-20250514-v1:0",
            vertex: "claude-opus-4@20250514",
        },
        family: "opus",
        context_window: 200_000,
        max_output: 32_000,
        supports_thinking: true,
        supports_images: true,
    },
    ModelInfo {
        key: "opus45",
        config: ModelConfig {
            first_party: "claude-opus-4-5-20251101",
            bedrock: "us.anthropic.claude-opus-4-5-20251101-v1:0",
            vertex: "claude-opus-4-5@20251101",
        },
        family: "opus",
        context_window: 200_000,
        max_output: 32_000,
        supports_thinking: true,
        supports_images: true,
    },
    ModelInfo {
        key: "sonnet37",
        config: ModelConfig {
            first_party: "claude-3-7-sonnet-20250219",
            bedrock: "us.anthropic.claude-3-7-sonnet-20250219-v1:0",
            vertex: "claude-3-7-sonnet@20250219",
        },
        family: "sonnet",
        context_window: 200_000,
        max_output: 16_000,
        supports_thinking: true,
        supports_images: true,
    },
    ModelInfo {
        key: "haiku35",
        config: ModelConfig {
            first_party: "claude-3-5-haiku-20241022",
            bedrock: "us.anthropic.claude-3-5-haiku-20241022-v1:0",
            vertex: "claude-3-5-haiku@20241022",
        },
        family: "haiku",
        context_window: 200_000,
        max_output: 8_192,
        supports_thinking: false,
        supports_images: true,
    },
];

// ── Aliases ──

const ALIASES: &[(&str, &str)] = &[
    ("sonnet", "claude-sonnet-4-20250514"),
    ("opus", "claude-opus-4-20250514"),
    ("haiku", "claude-haiku-4-5-20251001"),
    ("best", "claude-opus-4-5-20251101"),
];

/// Resolve a model alias or return the input unchanged.
/// Checks ANTHROPIC_DEFAULT_*_MODEL env vars for custom provider model names.
pub fn resolve_model(input: &str) -> String {
    // Strip [1m] suffix first
    let stripped = input.strip_suffix("[1m]").unwrap_or(input);

    // Check env override
    if let Ok(m) = std::env::var("ANTHROPIC_MODEL") {
        if !m.is_empty() {
            return m;
        }
    }

    // Check ANTHROPIC_DEFAULT_*_MODEL env vars (for custom providers like ppio)
    let lower = stripped.to_lowercase();
    if lower == "opus" || lower.contains("opus") {
        if let Ok(m) = std::env::var("ANTHROPIC_DEFAULT_OPUS_MODEL") {
            if !m.is_empty() {
                return m;
            }
        }
    }
    if lower == "sonnet" || lower.contains("sonnet") {
        if let Ok(m) = std::env::var("ANTHROPIC_DEFAULT_SONNET_MODEL") {
            if !m.is_empty() {
                return m;
            }
        }
    }
    if lower == "haiku" || lower.contains("haiku") {
        if let Ok(m) = std::env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL") {
            if !m.is_empty() {
                return m;
            }
        }
    }

    for (alias, canonical) in ALIASES {
        if stripped.eq_ignore_ascii_case(alias) {
            return canonical.to_string();
        }
    }
    stripped.to_string()
}

/// Get model info by ID (substring match, longest first).
pub fn get_model_info(model: &str) -> Option<&'static ModelInfo> {
    let lower = model.to_lowercase();
    // Exact match first
    if let Some(m) = MODELS.iter().find(|m| m.config.first_party == lower) {
        return Some(m);
    }
    // Substring match (longest first — MODELS is ordered by specificity)
    MODELS.iter().find(|m| lower.contains(m.family))
}

/// Get the model ID for a specific provider.
pub fn model_id_for_provider(model: &str, provider: ApiProvider) -> String {
    if let Some(info) = get_model_info(model) {
        match provider {
            ApiProvider::FirstParty => info.config.first_party.to_string(),
            ApiProvider::Bedrock => info.config.bedrock.to_string(),
            ApiProvider::Vertex => info.config.vertex.to_string(),
        }
    } else {
        model.to_string() // pass through unknown models
    }
}

/// Normalize a model string for API calls (strip [1m] suffix, resolve aliases).
pub fn normalize_model_for_api(model: &str) -> String {
    let stripped = model.strip_suffix("[1m]").unwrap_or(model);
    resolve_model(stripped)
}

/// Get context window size for a model.
pub fn context_window_for_model(model: &str) -> usize {
    get_model_info(model).map_or(200_000, |m| m.context_window)
}

/// Get max output tokens for a model.
pub fn max_output_for_model(model: &str) -> u32 {
    get_model_info(model).map_or(16_000, |m| m.max_output)
}

/// Validate a model string.
pub fn validate_model(model: &str) -> Result<(), String> {
    let resolved = resolve_model(model);
    if resolved.starts_with("claude-")
        || resolved.starts_with("anthropic.")
        || resolved.contains("claude")
    {
        Ok(())
    } else {
        Err(format!(
            "Unknown model: {model}. Use 'sonnet', 'opus', 'haiku', or a full model ID."
        ))
    }
}

/// Detect API provider from environment.
pub fn detect_provider() -> ApiProvider {
    if std::env::var("AWS_REGION").is_ok() && std::env::var("ANTHROPIC_API_KEY").is_err() {
        return ApiProvider::Bedrock;
    }
    if std::env::var("GOOGLE_CLOUD_PROJECT").is_ok() && std::env::var("ANTHROPIC_API_KEY").is_err()
    {
        return ApiProvider::Vertex;
    }
    ApiProvider::FirstParty
}

/// Get the default model for the current provider.
pub fn get_default_model() -> String {
    let provider = detect_provider();
    let info = MODELS.iter().find(|m| m.key == "sonnet40").unwrap();
    match provider {
        ApiProvider::FirstParty => info.config.first_party.to_string(),
        ApiProvider::Bedrock => info.config.bedrock.to_string(),
        ApiProvider::Vertex => info.config.vertex.to_string(),
    }
}

/// Get the default subagent model (haiku for speed).
pub fn get_default_subagent_model() -> String {
    let info = MODELS.iter().find(|m| m.key == "haiku45").unwrap();
    info.config.first_party.to_string()
}

/// List all available models.
pub fn list_models() -> &'static [ModelInfo] {
    MODELS
}

/// Get canonical model name for display.
pub fn get_canonical_name(model: &str) -> String {
    if let Some(info) = get_model_info(model) {
        info.config.first_party.to_string()
    } else {
        model.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_alias() {
        assert!(resolve_model("sonnet").starts_with("claude-sonnet"));
        assert!(resolve_model("opus").starts_with("claude-opus"));
        assert!(resolve_model("haiku").starts_with("claude-haiku"));
    }

    #[test]
    fn test_resolve_passthrough() {
        assert_eq!(
            resolve_model("claude-sonnet-4-20250514"),
            "claude-sonnet-4-20250514"
        );
    }

    #[test]
    fn test_get_model_info() {
        let info = get_model_info("claude-sonnet-4-20250514").unwrap();
        assert_eq!(info.context_window, 200_000);
        assert!(info.supports_thinking);
    }

    #[test]
    fn test_model_id_for_provider() {
        let bedrock = model_id_for_provider("claude-sonnet-4-20250514", ApiProvider::Bedrock);
        assert!(bedrock.contains("anthropic"));
        let vertex = model_id_for_provider("claude-sonnet-4-20250514", ApiProvider::Vertex);
        assert!(vertex.contains("@"));
    }

    #[test]
    fn test_normalize_model() {
        assert_eq!(
            normalize_model_for_api("sonnet[1m]"),
            resolve_model("sonnet")
        );
        assert_eq!(normalize_model_for_api("opus"), resolve_model("opus"));
    }

    #[test]
    fn test_validate_model() {
        assert!(validate_model("sonnet").is_ok());
        assert!(validate_model("claude-opus-4-20250514").is_ok());
        assert!(validate_model("gpt-4").is_err());
    }

    #[test]
    fn test_context_window() {
        assert_eq!(
            context_window_for_model("claude-sonnet-4-20250514"),
            200_000
        );
    }

    #[test]
    fn test_default_model() {
        let model = get_default_model();
        assert!(model.contains("claude"));
    }

    #[test]
    fn test_canonical_name() {
        let name = get_canonical_name("claude-sonnet-4-20250514");
        assert!(name.contains("sonnet"));
    }
}

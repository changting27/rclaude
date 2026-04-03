//! OAuth service matching services/oauth/.
//! Handles OAuth2 authorization code flow for Claude AI authentication.

use std::collections::HashMap;

/// OAuth token pair.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>, // Unix timestamp
    pub token_type: String,
    pub scope: Option<String>,
}

/// OAuth configuration.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: "claude-code".into(),
            auth_url: "https://claude.ai/oauth/authorize".into(),
            token_url: "https://claude.ai/oauth/token".into(),
            redirect_uri: "http://localhost:0/oauth/callback".into(),
            scopes: vec!["user:inference".into(), "user:profile".into()],
        }
    }
}

/// Build the authorization URL for the OAuth flow.
pub fn build_auth_url(config: &OAuthConfig, state: &str, code_challenge: &str) -> String {
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        config.auth_url, config.client_id,
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(&config.scopes.join(" ")),
        state, code_challenge,
    )
}

/// Exchange an authorization code for tokens.
pub async fn exchange_code_for_tokens(
    config: &OAuthConfig,
    code: &str,
    code_verifier: &str,
) -> Result<OAuthTokens, String> {
    let client = reqwest::Client::new();
    let params = HashMap::from([
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &config.redirect_uri),
        ("client_id", &config.client_id),
        ("code_verifier", code_verifier),
    ]);

    let resp = client
        .post(&config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token exchange failed: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token exchange error: {body}"));
    }

    resp.json::<OAuthTokens>()
        .await
        .map_err(|e| format!("Parse error: {e}"))
}

/// Refresh an expired access token.
pub async fn refresh_token(
    config: &OAuthConfig,
    refresh_token: &str,
) -> Result<OAuthTokens, String> {
    let client = reqwest::Client::new();
    let params = HashMap::from([
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", &config.client_id),
    ]);

    let resp = client
        .post(&config.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh failed: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh error: {body}"));
    }

    resp.json::<OAuthTokens>()
        .await
        .map_err(|e| format!("Parse error: {e}"))
}

/// Check if a token is expired.
pub fn is_token_expired(expires_at: Option<u64>) -> bool {
    match expires_at {
        Some(exp) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now >= exp.saturating_sub(60) // 60s buffer
        }
        None => false,
    }
}

/// Save OAuth tokens to config.
pub fn save_tokens(tokens: &OAuthTokens) -> Result<(), String> {
    let config_dir = rclaude_core::config::Config::config_dir();
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let path = config_dir.join("oauth_tokens.json");
    let json = serde_json::to_string_pretty(tokens).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Load OAuth tokens from config.
pub fn load_tokens() -> Option<OAuthTokens> {
    let path = rclaude_core::config::Config::config_dir().join("oauth_tokens.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Generate PKCE code verifier and challenge.
pub fn generate_pkce() -> (String, String) {
    use base64::Engine;
    use sha2::Digest;

    let verifier: String = (0..43)
        .map(|_| {
            let idx = rand::random::<u8>() % 62;
            match idx {
                0..=25 => (b'A' + idx) as char,
                26..=51 => (b'a' + idx - 26) as char,
                _ => (b'0' + idx - 52) as char,
            }
        })
        .collect();

    let hash = sha2::Sha256::digest(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);

    (verifier, challenge)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_token_expired() {
        assert!(!is_token_expired(None));
        assert!(is_token_expired(Some(0))); // epoch = expired
        assert!(!is_token_expired(Some(u64::MAX))); // far future = not expired
    }

    #[test]
    fn test_generate_pkce() {
        let (verifier, challenge) = generate_pkce();
        assert_eq!(verifier.len(), 43);
        assert!(!challenge.is_empty());
        assert_ne!(verifier, challenge);
    }

    #[test]
    fn test_build_auth_url() {
        let config = OAuthConfig::default();
        let url = build_auth_url(&config, "state123", "challenge456");
        assert!(url.contains("claude.ai"));
        assert!(url.contains("state123"));
        assert!(url.contains("challenge456"));
    }
}

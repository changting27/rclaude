//! MCP OAuth authentication flow.
//! Steps:
//! 1. Start local HTTP callback server on available port
//! 2. Open browser for OAuth authorization
//! 3. Receive authorization code via callback
//! 4. Exchange code for access token
//! 5. Store tokens for future use

use rclaude_core::error::{RclaudeError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;

/// OAuth token pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: String,
    pub scope: Option<String>,
}

/// OAuth server metadata (RFC 8414).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthMetadata {
    pub issuer: Option<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    pub scopes_supported: Option<Vec<String>>,
    pub response_types_supported: Option<Vec<String>>,
    pub code_challenge_methods_supported: Option<Vec<String>>,
}

/// OAuth client registration result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClientInfo {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
}

/// Stored OAuth data per server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerOAuthData {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub scope: Option<String>,
}

/// Get the OAuth storage path.
fn oauth_storage_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("mcp-oauth.json")
}

/// Load stored OAuth data for all servers.
fn load_oauth_storage() -> HashMap<String, ServerOAuthData> {
    let path = oauth_storage_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Save OAuth data for a server.
fn save_oauth_data(server_key: &str, data: &ServerOAuthData) {
    let path = oauth_storage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut storage = load_oauth_storage();
    storage.insert(server_key.to_string(), data.clone());
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&storage).unwrap_or_default(),
    );
}

/// Get stored tokens for a server (if not expired).
pub fn get_stored_tokens(server_key: &str) -> Option<OAuthTokens> {
    let storage = load_oauth_storage();
    let data = storage.get(server_key)?;
    let access_token = data.access_token.as_ref()?;

    // Check expiry
    if let Some(expires_at) = data.expires_at {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now >= expires_at {
            return None; // Expired
        }
    }

    Some(OAuthTokens {
        access_token: access_token.clone(),
        refresh_token: data.refresh_token.clone(),
        expires_in: data.expires_at.map(|e| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            e.saturating_sub(now)
        }),
        token_type: "Bearer".into(),
        scope: data.scope.clone(),
    })
}

/// Clear stored tokens for a server.
pub fn clear_server_tokens(server_key: &str) {
    let path = oauth_storage_path();
    let mut storage = load_oauth_storage();
    storage.remove(server_key);
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&storage).unwrap_or_default(),
    );
}

/// Generate a server key for OAuth storage.
pub fn get_server_key(server_name: &str, server_url: &str) -> String {
    format!("{}:{}", server_name, server_url)
}

/// Find an available port for the OAuth callback server.
fn find_available_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| RclaudeError::Config(format!("Failed to find available port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| RclaudeError::Config(format!("Failed to get port: {e}")))?
        .port();
    Ok(port)
}

/// Generate PKCE code verifier and challenge (RFC 7636).
fn generate_pkce() -> (String, String) {
    use base64::Engine;
    use sha2::Digest;

    let mut verifier_bytes = [0u8; 32];
    getrandom::getrandom(&mut verifier_bytes).unwrap_or_default();
    let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(verifier_bytes);

    let mut hasher = sha2::Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}

/// Generate a random state parameter for CSRF protection.
fn generate_state() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).unwrap_or_default();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Fetch OAuth server metadata from well-known endpoint.
pub async fn fetch_oauth_metadata(server_url: &str) -> Result<OAuthMetadata> {
    let base = server_url.trim_end_matches('/');
    let url = format!("{base}/.well-known/oauth-authorization-server");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Failed to fetch OAuth metadata: {e}"),
            status: None,
        })?;

    if !resp.status().is_success() {
        return Err(RclaudeError::Api {
            message: format!("OAuth metadata endpoint returned {}", resp.status()),
            status: Some(resp.status().as_u16()),
        });
    }

    resp.json::<OAuthMetadata>()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Invalid OAuth metadata: {e}"),
            status: None,
        })
}

/// Register a dynamic OAuth client (RFC 7591).
async fn register_client(
    registration_endpoint: &str,
    redirect_uri: &str,
    server_name: &str,
) -> Result<OAuthClientInfo> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "client_name": format!("rclaude - {server_name}"),
        "redirect_uris": [redirect_uri],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none",
    });

    let resp = client
        .post(registration_endpoint)
        .json(&body)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Client registration failed: {e}"),
            status: None,
        })?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(RclaudeError::Api {
            message: format!("Client registration failed ({status}): {body}"),
            status: Some(status),
        });
    }

    let data: serde_json::Value = resp.json().await.map_err(|e| RclaudeError::Api {
        message: format!("Invalid registration response: {e}"),
        status: None,
    })?;

    Ok(OAuthClientInfo {
        client_id: data["client_id"]
            .as_str()
            .ok_or_else(|| RclaudeError::Api {
                message: "Missing client_id in registration response".into(),
                status: None,
            })?
            .to_string(),
        client_secret: data["client_secret"].as_str().map(String::from),
        redirect_uri: redirect_uri.to_string(),
    })
}

/// Exchange authorization code for tokens.
async fn exchange_code(
    token_endpoint: &str,
    code: &str,
    client_info: &OAuthClientInfo,
    code_verifier: &str,
) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let mut params = HashMap::new();
    params.insert("grant_type", "authorization_code");
    params.insert("code", code);
    params.insert("redirect_uri", &client_info.redirect_uri);
    params.insert("client_id", &client_info.client_id);
    params.insert("code_verifier", code_verifier);

    let resp = client
        .post(token_endpoint)
        .form(&params)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Token exchange failed: {e}"),
            status: None,
        })?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(RclaudeError::Api {
            message: format!("Token exchange failed ({status}): {body}"),
            status: Some(status),
        });
    }

    resp.json::<OAuthTokens>()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Invalid token response: {e}"),
            status: None,
        })
}

/// Refresh an expired access token.
pub async fn refresh_token(
    token_endpoint: &str,
    refresh_token: &str,
    client_id: &str,
) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let mut params = HashMap::new();
    params.insert("grant_type", "refresh_token");
    params.insert("refresh_token", refresh_token);
    params.insert("client_id", client_id);

    let resp = client
        .post(token_endpoint)
        .form(&params)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Token refresh failed: {e}"),
            status: None,
        })?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        return Err(RclaudeError::Api {
            message: format!("Token refresh failed ({status}): {body}"),
            status: Some(status),
        });
    }

    resp.json::<OAuthTokens>()
        .await
        .map_err(|e| RclaudeError::Api {
            message: format!("Invalid refresh response: {e}"),
            status: None,
        })
}

/// Perform the full MCP OAuth flow.
/// Steps:
/// 1. Fetch OAuth metadata from server
/// 2. Register dynamic client (if needed)
/// 3. Start local callback server
/// 4. Open browser for authorization
/// 5. Wait for callback with authorization code
/// 6. Exchange code for tokens
/// 7. Store tokens
pub async fn perform_oauth_flow(
    server_name: &str,
    server_url: &str,
    callback_port: Option<u16>,
) -> Result<OAuthTokens> {
    let server_key = get_server_key(server_name, server_url);

    // Check for cached tokens first
    if let Some(tokens) = get_stored_tokens(&server_key) {
        tracing::info!("Using cached OAuth tokens for '{server_name}'");
        return Ok(tokens);
    }

    // 1. Fetch OAuth metadata
    let metadata = fetch_oauth_metadata(server_url).await?;

    // 2. Find available port and build redirect URI
    let port = callback_port.map(Ok).unwrap_or_else(find_available_port)?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // 3. Register client (if registration endpoint available)
    let client_info = if let Some(ref reg_endpoint) = metadata.registration_endpoint {
        register_client(reg_endpoint, &redirect_uri, server_name).await?
    } else {
        // Use server_name as client_id if no dynamic registration
        OAuthClientInfo {
            client_id: format!("rclaude-{server_name}"),
            client_secret: None,
            redirect_uri: redirect_uri.clone(),
        }
    };

    // Save client info
    save_oauth_data(
        &server_key,
        &ServerOAuthData {
            client_id: Some(client_info.client_id.clone()),
            client_secret: client_info.client_secret.clone(),
            ..Default::default()
        },
    );

    // 4. Generate PKCE and state
    let (code_verifier, code_challenge) = generate_pkce();
    let state = generate_state();

    // 5. Build authorization URL
    let scope = metadata
        .scopes_supported
        .as_ref()
        .and_then(|s| s.first())
        .cloned()
        .unwrap_or_default();

    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256{}",
        metadata.authorization_endpoint,
        urlencoding::encode(&client_info.client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state),
        urlencoding::encode(&code_challenge),
        if scope.is_empty() { String::new() } else { format!("&scope={}", urlencoding::encode(&scope)) },
    );

    // 6. Start callback server and open browser
    let (tx, rx) = oneshot::channel::<std::result::Result<String, String>>();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));
    let expected_state = state.clone();

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| RclaudeError::Config(format!("Failed to bind callback port {port}: {e}")))?;

    // Spawn callback server
    let tx_clone = tx.clone();
    let server_handle = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let timeout = tokio::time::sleep(std::time::Duration::from_secs(300)); // 5 min timeout
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((mut stream, _)) => {
                            let mut buf = vec![0u8; 4096];
                            let n = stream.read(&mut buf).await.unwrap_or(0);
                            let request = String::from_utf8_lossy(&buf[..n]);

                            // Parse GET /callback?code=...&state=...
                            if let Some(path) = request.lines().next() {
                                if path.contains("/callback") {
                                    let query = path.split('?').nth(1).unwrap_or("").split(' ').next().unwrap_or("");
                                    let params: HashMap<&str, &str> = query
                                        .split('&')
                                        .filter_map(|p| p.split_once('='))
                                        .collect();

                                    if let Some(error) = params.get("error") {
                                        let desc = params.get("error_description").unwrap_or(&"");
                                        let response = format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<h1>Authentication Error</h1><p>{error}: {desc}</p><p>You can close this window.</p>"
                                        );
                                        let _ = stream.write_all(response.as_bytes()).await;
                                        if let Some(tx) = tx_clone.lock().await.take() {
                                            let _ = tx.send(Err(format!("OAuth error: {error} - {desc}")));
                                        }
                                        break;
                                    }

                                    if let Some(code) = params.get("code") {
                                        let recv_state = params.get("state").unwrap_or(&"");
                                        if *recv_state != expected_state {
                                            let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n<h1>Error</h1><p>State mismatch</p>";
                                            let _ = stream.write_all(response.as_bytes()).await;
                                            if let Some(tx) = tx_clone.lock().await.take() {
                                                let _ = tx.send(Err("OAuth state mismatch - possible CSRF attack".into()));
                                            }
                                            break;
                                        }

                                        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<h1>Authentication Successful</h1><p>You can close this window. Return to rclaude.</p>";
                                        let _ = stream.write_all(response.as_bytes()).await;
                                        if let Some(tx) = tx_clone.lock().await.take() {
                                            let _ = tx.send(Ok(code.to_string()));
                                        }
                                        break;
                                    }
                                }
                            }

                            // Not a callback request, send 404
                            let response = "HTTP/1.1 404 Not Found\r\n\r\n";
                            let _ = stream.write_all(response.as_bytes()).await;
                        }
                        Err(_) => break,
                    }
                }
                _ = &mut timeout => {
                    if let Some(tx) = tx_clone.lock().await.take() {
                        let _ = tx.send(Err("Authentication timeout (5 minutes)".into()));
                    }
                    break;
                }
            }
        }
    });

    // Open browser
    eprintln!("Opening browser for MCP server '{server_name}' authentication...");
    eprintln!("If the browser doesn't open, visit: {auth_url}");
    let _ = open::that(&auth_url);

    // 7. Wait for authorization code
    let code = rx
        .await
        .map_err(|_| RclaudeError::Api {
            message: "OAuth callback channel closed".into(),
            status: None,
        })?
        .map_err(|e| RclaudeError::Api {
            message: e,
            status: None,
        })?;

    server_handle.abort();

    // 8. Exchange code for tokens
    let tokens = exchange_code(
        &metadata.token_endpoint,
        &code,
        &client_info,
        &code_verifier,
    )
    .await?;

    // 9. Store tokens
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    save_oauth_data(
        &server_key,
        &ServerOAuthData {
            client_id: Some(client_info.client_id),
            client_secret: client_info.client_secret,
            access_token: Some(tokens.access_token.clone()),
            refresh_token: tokens.refresh_token.clone(),
            expires_at: tokens.expires_in.map(|e| now + e),
            scope: tokens.scope.clone(),
        },
    );

    eprintln!("✓ MCP server '{server_name}' authenticated successfully");
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce() {
        let (verifier, challenge) = generate_pkce();
        assert!(!verifier.is_empty());
        assert!(!challenge.is_empty());
        assert_ne!(verifier, challenge);
    }

    #[test]
    fn test_generate_state() {
        let s1 = generate_state();
        let s2 = generate_state();
        assert!(!s1.is_empty());
        assert_ne!(s1, s2); // Should be random
    }

    #[test]
    fn test_server_key() {
        assert_eq!(
            get_server_key("myserver", "https://example.com"),
            "myserver:https://example.com"
        );
    }

    #[test]
    fn test_oauth_storage_roundtrip() {
        let key = "test-server:http://localhost:9999";
        let data = ServerOAuthData {
            client_id: Some("test-client".into()),
            access_token: Some("test-token".into()),
            ..Default::default()
        };
        save_oauth_data(key, &data);
        let loaded = load_oauth_storage();
        assert_eq!(
            loaded.get(key).and_then(|d| d.client_id.as_deref()),
            Some("test-client")
        );
        // Cleanup
        clear_server_tokens(key);
    }
}

//! Remote managed settings matching services/remoteManagedSettings/.

use std::path::Path;

/// Fetch and apply remote managed settings.
pub async fn sync_remote_settings(_cwd: &Path) -> Result<SyncStatus, String> {
    // Check for remote settings endpoint
    let endpoint = std::env::var("CLAUDE_MANAGED_SETTINGS_URL").ok();
    if endpoint.is_none() {
        return Ok(SyncStatus::NoEndpoint);
    }

    let url = endpoint.unwrap();
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {e}"))?;

    if !resp.status().is_success() {
        return Ok(SyncStatus::FetchError(format!("HTTP {}", resp.status())));
    }

    let settings: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Parse failed: {e}"))?;

    // Apply to managed settings path
    let managed_dir = Path::new("/etc/claude-code");
    if managed_dir.exists() {
        let path = managed_dir.join("settings.json");
        let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        tokio::fs::write(&path, json)
            .await
            .map_err(|e| e.to_string())?;
        Ok(SyncStatus::Updated)
    } else {
        Ok(SyncStatus::NoManagedDir)
    }
}

#[derive(Debug)]
pub enum SyncStatus {
    Updated,
    NoEndpoint,
    NoManagedDir,
    FetchError(String),
}

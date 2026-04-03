//! Settings sync matching services/settingsSync/.
//! Synchronizes settings across sessions.

use std::path::Path;

/// Sync settings from remote/managed source.
pub async fn sync_settings(cwd: &Path) -> Result<SyncResult, String> {
    let managed_path = Path::new("/etc/claude-code/settings.json");
    if !managed_path.exists() {
        return Ok(SyncResult {
            synced: false,
            reason: "No managed settings".into(),
        });
    }

    let managed = tokio::fs::read_to_string(managed_path)
        .await
        .map_err(|e| format!("Failed to read managed settings: {e}"))?;
    let managed: serde_json::Value = serde_json::from_str(&managed)
        .map_err(|e| format!("Failed to parse managed settings: {e}"))?;

    // Apply managed settings to project
    let project_path = cwd.join(".claude/settings.json");
    let mut project: serde_json::Value = tokio::fs::read_to_string(&project_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    // Merge managed into project (managed takes precedence for policy fields)
    if let Some(managed_obj) = managed.as_object() {
        let project_obj = project.as_object_mut().unwrap();
        for (key, value) in managed_obj {
            if key.starts_with("policy") || key == "allowManagedPermissionRulesOnly" {
                project_obj.insert(key.clone(), value.clone());
            }
        }
    }

    Ok(SyncResult {
        synced: true,
        reason: "Managed settings applied".into(),
    })
}

#[derive(Debug)]
pub struct SyncResult {
    pub synced: bool,
    pub reason: String,
}

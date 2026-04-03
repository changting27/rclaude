//! Team memory sync matching services/teamMemorySync/.
//! Synchronizes shared memory across team members.

use std::path::{Path, PathBuf};

/// Team memory entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamMemoryEntry {
    pub key: String,
    pub value: String,
    pub author: String,
    pub updated_at: String,
}

/// Get the team memory directory.
pub fn team_memory_dir(cwd: &Path) -> PathBuf {
    cwd.join(".claude/team-memory")
}

/// Load team memory entries.
pub async fn load_team_memory(cwd: &Path) -> Vec<TeamMemoryEntry> {
    let dir = team_memory_dir(cwd);
    let path = dir.join("memory.json");
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Save a team memory entry.
pub async fn save_team_memory_entry(cwd: &Path, entry: &TeamMemoryEntry) -> Result<(), String> {
    let dir = team_memory_dir(cwd);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let path = dir.join("memory.json");
    let mut entries = load_team_memory(cwd).await;

    // Update or add
    if let Some(existing) = entries.iter_mut().find(|e| e.key == entry.key) {
        *existing = entry.clone();
    } else {
        entries.push(entry.clone());
    }

    let json = serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())
}

/// Check if content contains potential secrets.
pub fn scan_for_secrets(content: &str) -> Vec<String> {
    let mut findings = Vec::new();
    let patterns = [
        ("API key", r"(?i)(api[_-]?key|apikey)\s*[:=]\s*\S+"),
        ("Token", r"(?i)(token|bearer)\s*[:=]\s*\S+"),
        ("Password", r"(?i)(password|passwd|pwd)\s*[:=]\s*\S+"),
        ("Secret", r"(?i)(secret|private[_-]?key)\s*[:=]\s*\S+"),
    ];
    for (name, pattern) in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if re.is_match(content) {
                findings.push(format!("Potential {name} detected"));
            }
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_secrets() {
        assert!(!scan_for_secrets("normal text").is_empty() == false);
        assert!(!scan_for_secrets("api_key=sk-abc123").is_empty());
        assert!(!scan_for_secrets("password: hunter2").is_empty());
    }
}

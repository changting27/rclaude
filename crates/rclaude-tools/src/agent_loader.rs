//! Agent loading from .claude/agents/ directories.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Source of a custom agent definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentSource {
    BuiltIn,
    User,    // ~/.claude/agents/
    Project, // .claude/agents/
    Managed, // /etc/claude-code/.claude/agents/
}

/// Agent memory scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentMemoryScope {
    User,
    Project,
    Local,
}

/// A custom agent loaded from a markdown file.
#[derive(Debug, Clone)]
pub struct CustomAgent {
    pub agent_type: String,
    pub description: String,
    pub system_prompt: String,
    pub tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_turns: Option<usize>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub memory_scope: Option<AgentMemoryScope>,
    pub background: bool,
    pub read_only: bool,
    pub source: AgentSource,
    pub path: PathBuf,
}

/// Load all agent definitions from all sources.
pub async fn load_all_agents(cwd: &Path) -> Vec<CustomAgent> {
    let mut agents = Vec::new();

    // 1. Managed agents (highest priority)
    agents.extend(
        load_agents_from_dir(
            Path::new("/etc/claude-code/.claude/agents"),
            AgentSource::Managed,
        )
        .await,
    );

    // 2. User agents
    if let Some(home) = dirs::home_dir() {
        agents.extend(load_agents_from_dir(&home.join(".claude/agents"), AgentSource::User).await);
    }

    // 3. Project agents (lowest priority for overrides)
    agents.extend(load_agents_from_dir(&cwd.join(".claude/agents"), AgentSource::Project).await);

    agents
}

/// Load agents from a directory of .md files.
async fn load_agents_from_dir(dir: &Path, source: AgentSource) -> Vec<CustomAgent> {
    let mut agents = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return agents,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if let Some(agent) = parse_agent_file(&path, &content, source.clone()) {
                agents.push(agent);
            }
        }
    }
    agents
}

/// Parse an agent markdown file with frontmatter.
fn parse_agent_file(path: &Path, content: &str, source: AgentSource) -> Option<CustomAgent> {
    let (fm, body) = parse_frontmatter(content);

    let agent_type = fm.get("name").cloned().or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    })?;

    let description = fm
        .get("description")
        .cloned()
        .unwrap_or_else(|| format!("Custom agent: {agent_type}"));

    let parse_list = |key: &str| -> Option<Vec<String>> {
        fm.get(key).map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
    };

    let memory_scope = fm.get("memory").and_then(|v| match v.as_str() {
        "user" => Some(AgentMemoryScope::User),
        "project" => Some(AgentMemoryScope::Project),
        "local" => Some(AgentMemoryScope::Local),
        _ => None,
    });

    Some(CustomAgent {
        agent_type,
        description,
        system_prompt: body,
        tools: parse_list("tools"),
        disallowed_tools: parse_list("disallowed-tools").or_else(|| parse_list("disallowedTools")),
        model: fm.get("model").cloned(),
        max_turns: fm
            .get("max-turns")
            .or(fm.get("maxTurns"))
            .and_then(|v| v.parse().ok()),
        effort: fm.get("effort").cloned(),
        permission_mode: fm
            .get("permission-mode")
            .or(fm.get("permissionMode"))
            .cloned(),
        memory_scope,
        background: fm.get("background").is_some_and(|v| v == "true"),
        read_only: fm.get("read-only").is_some_and(|v| v == "true"),
        source,
        path: path.to_path_buf(),
    })
}

fn parse_frontmatter(content: &str) -> (HashMap<String, String>, String) {
    let mut fm = HashMap::new();
    if !content.starts_with("---") {
        return (fm, content.to_string());
    }
    let rest = &content[3..];
    if let Some(end) = rest.find("\n---") {
        for line in rest[..end].lines() {
            if let Some((key, value)) = line.split_once(':') {
                let k = key.trim().to_string();
                let v = value.trim().to_string();
                if !k.is_empty() && !v.is_empty() {
                    fm.insert(k, v);
                }
            }
        }
        (fm, rest[end + 4..].trim_start().to_string())
    } else {
        (fm, content.to_string())
    }
}

/// Get active agents: merge custom agents with built-ins.
/// Priority: managed > project > user > built-in (higher priority overrides by name).
pub fn get_active_agents(custom: &[CustomAgent]) -> Vec<ActiveAgent> {
    let mut by_name: HashMap<String, ActiveAgent> = HashMap::new();

    // Built-ins first (lowest priority)
    for def in crate::agent_types::get_built_in_agents() {
        by_name.insert(def.agent_type.to_lowercase(), ActiveAgent::BuiltIn(def));
    }

    // Custom agents override by name (source priority handled by load order)
    for agent in custom {
        by_name.insert(
            agent.agent_type.to_lowercase(),
            ActiveAgent::Custom(Box::new(agent.clone())),
        );
    }

    by_name.into_values().collect()
}

/// An active agent: either built-in or custom.
#[derive(Debug, Clone)]
pub enum ActiveAgent {
    BuiltIn(&'static crate::agent_types::AgentDefinition),
    Custom(Box<CustomAgent>),
}

impl ActiveAgent {
    pub fn name(&self) -> &str {
        match self {
            Self::BuiltIn(d) => d.agent_type,
            Self::Custom(c) => &c.agent_type,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::BuiltIn(d) => d.when_to_use,
            Self::Custom(c) => &c.description,
        }
    }

    pub fn system_prompt(&self) -> &str {
        match self {
            Self::BuiltIn(d) => d.system_prompt,
            Self::Custom(c) => &c.system_prompt,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::BuiltIn(d) => d.model,
            Self::Custom(c) => c.model.as_deref().unwrap_or("inherit"),
        }
    }

    pub fn is_read_only(&self) -> bool {
        match self {
            Self::BuiltIn(d) => d.read_only,
            Self::Custom(c) => c.read_only,
        }
    }
}

/// Get agent memory directory for a given scope.
pub fn get_agent_memory_dir(scope: &AgentMemoryScope, cwd: &Path) -> PathBuf {
    match scope {
        AgentMemoryScope::User => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude/agent-memory"),
        AgentMemoryScope::Project => cwd.join(".claude/agent-memory"),
        AgentMemoryScope::Local => cwd.join(".claude/agent-memory-local"),
    }
}

/// Get the memory entrypoint file (MEMORY.md) for an agent.
pub fn get_agent_memory_entrypoint(
    scope: &AgentMemoryScope,
    agent_type: &str,
    cwd: &Path,
) -> PathBuf {
    get_agent_memory_dir(scope, cwd)
        .join(agent_type)
        .join("MEMORY.md")
}

/// Load agent memory prompt if it exists.
pub async fn load_agent_memory_prompt(
    scope: &AgentMemoryScope,
    agent_type: &str,
    cwd: &Path,
) -> Option<String> {
    let path = get_agent_memory_entrypoint(scope, agent_type, cwd);
    tokio::fs::read_to_string(&path).await.ok()
}

// ── Agent Memory Snapshot ──

const SNAPSHOT_BASE: &str = "agent-memory-snapshots";
const SNAPSHOT_JSON: &str = "snapshot.json";
const SYNCED_JSON: &str = ".snapshot-synced.json";

/// What action to take based on snapshot state.
#[derive(Debug, PartialEq)]
pub enum SnapshotAction {
    /// No snapshot exists or already synced.
    None,
    /// First time: initialize local memory from snapshot.
    Initialize { timestamp: String },
    /// Snapshot is newer: prompt user to update.
    PromptUpdate { timestamp: String },
}

fn snapshot_dir_for_agent(agent_type: &str, cwd: &Path) -> PathBuf {
    cwd.join(".claude").join(SNAPSHOT_BASE).join(agent_type)
}

fn snapshot_json_path(agent_type: &str, cwd: &Path) -> PathBuf {
    snapshot_dir_for_agent(agent_type, cwd).join(SNAPSHOT_JSON)
}

fn synced_json_path(agent_type: &str, scope: &AgentMemoryScope, cwd: &Path) -> PathBuf {
    get_agent_memory_dir(scope, cwd)
        .join(agent_type)
        .join(SYNCED_JSON)
}

/// Check if a snapshot exists and whether it's newer than what we last synced.
pub async fn check_agent_memory_snapshot(
    agent_type: &str,
    scope: &AgentMemoryScope,
    cwd: &Path,
) -> SnapshotAction {
    // Read snapshot metadata
    let snap_path = snapshot_json_path(agent_type, cwd);
    let snap_meta = match tokio::fs::read_to_string(&snap_path).await {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(v) => v
                .get("updatedAt")
                .and_then(|v| v.as_str())
                .map(String::from),
            Err(_) => None,
        },
        Err(_) => None,
    };

    let snap_timestamp = match snap_meta {
        Some(t) => t,
        None => return SnapshotAction::None,
    };

    // Check if local memory exists
    let local_dir = get_agent_memory_dir(scope, cwd).join(agent_type);
    let has_local = if let Ok(mut entries) = tokio::fs::read_dir(&local_dir).await {
        let mut found = false;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|e| e == "md") {
                found = true;
                break;
            }
        }
        found
    } else {
        false
    };

    if !has_local {
        return SnapshotAction::Initialize {
            timestamp: snap_timestamp,
        };
    }

    // Check synced metadata
    let synced_path = synced_json_path(agent_type, scope, cwd);
    let synced_from = tokio::fs::read_to_string(&synced_path)
        .await
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| {
            v.get("syncedFrom")
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    match synced_from {
        Some(synced) if synced >= snap_timestamp => SnapshotAction::None,
        _ => SnapshotAction::PromptUpdate {
            timestamp: snap_timestamp,
        },
    }
}

/// Copy snapshot memory files to local agent memory directory.
async fn copy_snapshot_to_local(agent_type: &str, scope: &AgentMemoryScope, cwd: &Path) {
    let snap_dir = snapshot_dir_for_agent(agent_type, cwd);
    let local_dir = get_agent_memory_dir(scope, cwd).join(agent_type);

    if tokio::fs::create_dir_all(&local_dir).await.is_err() {
        return;
    }

    let mut entries = match tokio::fs::read_dir(&snap_dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip snapshot.json itself
        if name_str == SNAPSHOT_JSON {
            continue;
        }
        if entry.file_type().await.is_ok_and(|t| t.is_file()) {
            if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                let _ = tokio::fs::write(local_dir.join(&name), &content).await;
            }
        }
    }
}

/// Save synced metadata to track which snapshot version we've synced from.
async fn save_synced_meta(agent_type: &str, scope: &AgentMemoryScope, cwd: &Path, timestamp: &str) {
    let path = synced_json_path(agent_type, scope, cwd);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let meta = serde_json::json!({ "syncedFrom": timestamp });
    let _ = tokio::fs::write(&path, serde_json::to_string(&meta).unwrap_or_default()).await;
}

/// Initialize local agent memory from a snapshot (first-time setup).
pub async fn initialize_from_snapshot(
    agent_type: &str,
    scope: &AgentMemoryScope,
    cwd: &Path,
    timestamp: &str,
) {
    copy_snapshot_to_local(agent_type, scope, cwd).await;
    save_synced_meta(agent_type, scope, cwd, timestamp).await;
}

/// Replace local agent memory with the snapshot (remove old .md files first).
pub async fn replace_from_snapshot(
    agent_type: &str,
    scope: &AgentMemoryScope,
    cwd: &Path,
    timestamp: &str,
) {
    let local_dir = get_agent_memory_dir(scope, cwd).join(agent_type);
    // Remove existing .md files
    if let Ok(mut entries) = tokio::fs::read_dir(&local_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|e| e == "md") {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
    copy_snapshot_to_local(agent_type, scope, cwd).await;
    save_synced_meta(agent_type, scope, cwd, timestamp).await;
}

/// Mark the current snapshot as synced without changing local memory.
pub async fn mark_snapshot_synced(
    agent_type: &str,
    scope: &AgentMemoryScope,
    cwd: &Path,
    timestamp: &str,
) {
    save_synced_meta(agent_type, scope, cwd, timestamp).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_file() {
        let content = "---\nname: my-agent\ndescription: Test agent\ntools: Read, Grep\nmodel: haiku\nmemory: project\n---\nYou are a test agent.";
        let agent =
            parse_agent_file(Path::new("/tmp/my-agent.md"), content, AgentSource::Project).unwrap();
        assert_eq!(agent.agent_type, "my-agent");
        assert_eq!(agent.tools, Some(vec!["Read".into(), "Grep".into()]));
        assert_eq!(agent.memory_scope, Some(AgentMemoryScope::Project));
    }

    #[test]
    fn test_parse_agent_no_frontmatter() {
        let content = "Just a prompt with no frontmatter.";
        let agent =
            parse_agent_file(Path::new("/tmp/simple.md"), content, AgentSource::User).unwrap();
        assert_eq!(agent.agent_type, "simple");
    }

    #[test]
    fn test_get_active_agents_includes_builtins() {
        let agents = get_active_agents(&[]);
        assert!(agents.iter().any(|a| a.name() == "general-purpose"));
        assert!(agents.iter().any(|a| a.name() == "Explore"));
    }

    #[test]
    fn test_custom_overrides_builtin() {
        let custom = vec![CustomAgent {
            agent_type: "Explore".into(),
            description: "My custom explore".into(),
            system_prompt: "Custom prompt".into(),
            tools: None,
            disallowed_tools: None,
            model: None,
            max_turns: None,
            effort: None,
            permission_mode: None,
            memory_scope: None,
            background: false,
            read_only: true,
            source: AgentSource::Project,
            path: PathBuf::from("/tmp/explore.md"),
        }];
        let agents = get_active_agents(&custom);
        let explore = agents.iter().find(|a| a.name() == "Explore").unwrap();
        assert_eq!(explore.system_prompt(), "Custom prompt");
    }

    #[test]
    fn test_agent_memory_dir() {
        let dir = get_agent_memory_dir(&AgentMemoryScope::Project, Path::new("/tmp/proj"));
        assert_eq!(dir, PathBuf::from("/tmp/proj/.claude/agent-memory"));
    }
}

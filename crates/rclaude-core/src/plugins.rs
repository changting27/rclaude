//! Plugin system for extending CLI functionality.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerSpec>,
    #[serde(default)]
    pub skills: Vec<SkillSpec>,
    #[serde(default)]
    pub agents: Vec<AgentSpec>,
    #[serde(default, rename = "lspServers")]
    pub lsp_servers: HashMap<String, LspServerSpec>,
    #[serde(default)]
    pub commands: Option<String>,
    #[serde(default, rename = "agentsPath")]
    pub agents_path: Option<String>,
    #[serde(default, rename = "skillsPath")]
    pub skills_path: Option<String>,
    #[serde(default)]
    pub hooks: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuthor {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSpec {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub source: String,
    pub enabled: bool,
    pub is_builtin: bool,
}

#[derive(Debug, Clone)]
pub enum PluginError {
    GenericError {
        source: String,
        plugin: Option<String>,
        error: String,
    },
    ManifestParseError {
        source: String,
        plugin: Option<String>,
        path: String,
        error: String,
    },
    PluginNotFound {
        source: String,
        plugin_id: String,
        marketplace: String,
    },
    ComponentLoadFailed {
        source: String,
        plugin: String,
        component: String,
        path: String,
        reason: String,
    },
}

#[derive(Debug, Default)]
pub struct PluginLoadResult {
    pub enabled: Vec<LoadedPlugin>,
    pub disabled: Vec<LoadedPlugin>,
    pub errors: Vec<PluginError>,
}

pub fn get_plugin_error_message(error: &PluginError) -> String {
    match error {
        PluginError::GenericError { error, .. } => error.clone(),
        PluginError::ManifestParseError { path, error, .. } => {
            format!("Manifest parse error at {path}: {error}")
        }
        PluginError::PluginNotFound {
            plugin_id,
            marketplace,
            ..
        } => format!("Plugin {plugin_id} not found in marketplace {marketplace}"),
        PluginError::ComponentLoadFailed {
            component,
            path,
            reason,
            ..
        } => format!("{component} load failed from {path}: {reason}"),
    }
}

#[derive(Debug, Default)]
pub struct PluginManager {
    pub plugins: Vec<LoadedPlugin>,
    pub errors: Vec<PluginError>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn load_plugins(&mut self, cwd: &Path) {
        self.plugins.clear();
        self.errors.clear();
        if let Some(home) = dirs::home_dir() {
            self.load_from_dir(&home.join(".claude/plugins")).await;
        }
        self.load_from_dir(&cwd.join(".claude/plugins")).await;
        self.load_marketplace_plugins().await;
    }

    pub async fn load_from_dir(&mut self, dir: &Path) {
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(e) => e,
            Err(_) => return,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match load_manifest(&path.join("plugin.json")).await {
                Ok(manifest) => self.plugins.push(LoadedPlugin {
                    source: path.display().to_string(),
                    manifest,
                    path,
                    enabled: true,
                    is_builtin: false,
                }),
                Err(e) => self.errors.push(PluginError::GenericError {
                    source: dir.display().to_string(),
                    plugin: None,
                    error: e,
                }),
            }
        }
    }

    pub async fn load_marketplace_plugins(&mut self) {
        let enabled = get_enabled_plugins_from_settings().await;
        let cache_dir = get_plugin_cache_dir();
        for (plugin_id, is_enabled) in &enabled {
            if !is_enabled {
                continue;
            }
            let (name, _) = parse_plugin_identifier(plugin_id);
            let plugin_dir = cache_dir.join(&name);
            if !plugin_dir.is_dir() {
                continue;
            }
            match load_manifest(&plugin_dir.join("plugin.json")).await {
                Ok(manifest) => self.plugins.push(LoadedPlugin {
                    source: plugin_id.clone(),
                    manifest,
                    path: plugin_dir,
                    enabled: true,
                    is_builtin: false,
                }),
                Err(_) => self.errors.push(PluginError::PluginNotFound {
                    source: plugin_id.clone(),
                    plugin_id: name,
                    marketplace: String::new(),
                }),
            }
        }
    }

    pub fn enabled_plugins(&self) -> Vec<&LoadedPlugin> {
        self.plugins.iter().filter(|p| p.enabled).collect()
    }
    pub fn disabled_plugins(&self) -> Vec<&LoadedPlugin> {
        self.plugins.iter().filter(|p| !p.enabled).collect()
    }

    pub fn mcp_servers(&self) -> HashMap<String, &McpServerSpec> {
        let mut servers = HashMap::new();
        for p in self.enabled_plugins() {
            for (name, spec) in &p.manifest.mcp_servers {
                servers.insert(format!("{}_{name}", p.manifest.name), spec);
            }
        }
        servers
    }

    pub fn hooks_configs(&self) -> Vec<(&str, &serde_json::Value)> {
        self.enabled_plugins()
            .iter()
            .filter_map(|p| {
                p.manifest
                    .hooks
                    .as_ref()
                    .map(|h| (p.manifest.name.as_str(), h))
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

pub async fn enable_plugin(plugin_id: &str) {
    set_plugin_enabled(plugin_id, true).await;
}
pub async fn disable_plugin(plugin_id: &str) {
    set_plugin_enabled(plugin_id, false).await;
}

pub fn get_plugin_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/plugins/cache")
}

pub fn get_versioned_cache_path(plugin_id: &str, marketplace: &str, version: &str) -> PathBuf {
    let (name, _) = parse_plugin_identifier(plugin_id);
    let san = |s: &str| {
        s.replace(
            |c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.',
            "-",
        )
    };
    get_plugin_cache_dir()
        .join(san(marketplace))
        .join(san(&name))
        .join(san(version))
}

pub async fn install_plugin(plugin_id: &str, marketplace: &str) -> Result<LoadedPlugin, String> {
    let cache_dir = get_plugin_cache_dir();
    let _ = tokio::fs::create_dir_all(&cache_dir).await;
    let (name, _) = parse_plugin_identifier(plugin_id);
    let target = cache_dir.join(&name);
    if target.exists() {
        return Err(format!("Plugin {name} already installed"));
    }
    let _ = tokio::fs::create_dir_all(&target).await;
    let manifest = PluginManifest {
        name: name.clone(),
        version: String::new(),
        description: format!("Installed from {marketplace}"),
        author: None,
        repository: None,
        license: None,
        keywords: vec![],
        mcp_servers: HashMap::new(),
        skills: vec![],
        agents: vec![],
        lsp_servers: HashMap::new(),
        commands: None,
        agents_path: None,
        skills_path: None,
        hooks: None,
    };
    set_plugin_enabled(plugin_id, true).await;
    Ok(LoadedPlugin {
        manifest,
        path: target,
        source: plugin_id.to_string(),
        enabled: true,
        is_builtin: false,
    })
}

pub async fn uninstall_plugin(plugin_id: &str) -> Result<(), String> {
    let (name, _) = parse_plugin_identifier(plugin_id);
    let target = get_plugin_cache_dir().join(&name);
    if target.exists() {
        tokio::fs::remove_dir_all(&target)
            .await
            .map_err(|e| format!("Remove failed: {e}"))?;
    }
    set_plugin_enabled(plugin_id, false).await;
    Ok(())
}

fn settings_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/settings.json")
}

pub async fn get_enabled_plugins_from_settings() -> HashMap<String, bool> {
    let content = match tokio::fs::read_to_string(settings_file()).await {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    val.get("enabledPlugins")
        .and_then(|v| serde_json::from_value::<HashMap<String, bool>>(v.clone()).ok())
        .unwrap_or_default()
}

pub async fn set_plugin_enabled(plugin_id: &str, enabled: bool) {
    let path = settings_file();
    let content = tokio::fs::read_to_string(&path)
        .await
        .unwrap_or_else(|_| "{}".into());
    let mut val: serde_json::Value =
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
    let plugins = val
        .as_object_mut()
        .unwrap()
        .entry("enabledPlugins")
        .or_insert(serde_json::json!({}));
    if let Some(m) = plugins.as_object_mut() {
        m.insert(plugin_id.to_string(), serde_json::Value::Bool(enabled));
    }
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(
        &path,
        serde_json::to_string_pretty(&val).unwrap_or_default(),
    )
    .await;
}

async fn load_manifest(path: &Path) -> Result<PluginManifest, String> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

// ── DXT Support ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxtManifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub author: DxtAuthor,
    #[serde(default)]
    pub tools: Vec<DxtTool>,
    #[serde(default)]
    pub server: Option<DxtServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxtAuthor {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxtTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxtServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

pub fn generate_extension_id(manifest: &DxtManifest) -> String {
    let san = |s: &str| -> String {
        s.to_lowercase()
            .replace(char::is_whitespace, "-")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    };
    format!("{}.{}", san(&manifest.author.name), san(&manifest.name))
}

pub async fn load_dxt_manifest(path: &Path) -> Result<DxtManifest, String> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Invalid DXT manifest {}: {e}", path.display()))
}

pub async fn discover_dxt_extensions(cwd: &Path) -> Vec<(String, DxtManifest, PathBuf)> {
    let mut results = Vec::new();
    if let Some(home) = dirs::home_dir() {
        scan_dxt_dir(&home.join(".claude/extensions"), &mut results).await;
    }
    scan_dxt_dir(&cwd.join(".claude/extensions"), &mut results).await;
    results
}

async fn scan_dxt_dir(dir: &Path, results: &mut Vec<(String, DxtManifest, PathBuf)>) {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(m) = load_dxt_manifest(&path.join("manifest.json")).await {
            results.push((generate_extension_id(&m), m, path));
        }
    }
}

// ── Marketplace ──

pub const OFFICIAL_MARKETPLACE_URL: &str =
    "https://downloads.claude.ai/claude-code-releases/plugins/claude-plugins-official";
pub const OFFICIAL_MARKETPLACE_REPO: &str = "anthropics/claude-plugins-official";
pub const OFFICIAL_MARKETPLACE_NAME: &str = "claude-plugins-official";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMarketplace {
    pub name: String,
    #[serde(default)]
    pub owner: Option<MarketplaceOwner>,
    pub plugins: Vec<MarketplaceEntry>,
    #[serde(default)]
    pub force_remove_deleted_plugins: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceOwner {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source: Option<PluginSource>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: Option<MarketplaceOwner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source")]
pub enum PluginSource {
    #[serde(rename = "github")]
    GitHub {
        repo: String,
        #[serde(default)]
        path: Option<String>,
    },
    #[serde(rename = "npm")]
    Npm { package: String },
    #[serde(rename = "url")]
    Url { url: String },
    #[serde(rename = "directory")]
    Directory { path: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnownMarketplacesConfig {
    #[serde(flatten)]
    pub marketplaces: HashMap<String, KnownMarketplace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownMarketplace {
    pub install_location: String,
    #[serde(default)]
    pub source: Option<serde_json::Value>,
}

fn known_marketplaces_file() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude/plugins/known_marketplaces.json")
}

pub async fn load_known_marketplaces() -> KnownMarketplacesConfig {
    match tokio::fs::read_to_string(known_marketplaces_file()).await {
        Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
        Err(_) => KnownMarketplacesConfig::default(),
    }
}

pub async fn save_known_marketplaces(config: &KnownMarketplacesConfig) {
    let path = known_marketplaces_file();
    if let Some(p) = path.parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }
    let _ = tokio::fs::write(
        &path,
        serde_json::to_string_pretty(config).unwrap_or_default(),
    )
    .await;
}

pub async fn read_cached_marketplace(path: &Path) -> Result<PluginMarketplace, String> {
    let nested = path.join(".claude-plugin").join("marketplace.json");
    if let Ok(c) = tokio::fs::read_to_string(&nested).await {
        if let Ok(m) = serde_json::from_str::<PluginMarketplace>(&c) {
            return Ok(m);
        }
    }
    let c = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read marketplace: {e}"))?;
    serde_json::from_str(&c).map_err(|e| format!("Invalid marketplace JSON: {e}"))
}

pub async fn fetch_official_marketplace() -> Result<PluginMarketplace, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP error: {e}"))?;
    let sha = client
        .get(format!("{OFFICIAL_MARKETPLACE_URL}/latest"))
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Read error: {e}"))?;
    let resp = client
        .get(format!(
            "{OFFICIAL_MARKETPLACE_URL}/{}/marketplace.json",
            sha.trim()
        ))
        .send()
        .await
        .map_err(|e| format!("Fetch failed: {e}"))?;
    if resp.status().is_success() {
        return serde_json::from_str(&resp.text().await.map_err(|e| format!("Read error: {e}"))?)
            .map_err(|e| format!("Invalid JSON: {e}"));
    }
    let url = format!("https://raw.githubusercontent.com/{OFFICIAL_MARKETPLACE_REPO}/main/.claude-plugin/marketplace.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GitHub fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub returned {}", resp.status()));
    }
    serde_json::from_str(&resp.text().await.map_err(|e| format!("Read error: {e}"))?)
        .map_err(|e| format!("Invalid JSON: {e}"))
}

pub async fn list_marketplace_plugins() -> Vec<(String, MarketplaceEntry)> {
    let mut all = Vec::new();
    for (name, km) in &load_known_marketplaces().await.marketplaces {
        if let Ok(mp) = read_cached_marketplace(Path::new(&km.install_location)).await {
            for entry in mp.plugins {
                all.push((name.clone(), entry));
            }
        }
    }
    all
}

pub fn search_plugins<'a>(
    plugins: &'a [(String, MarketplaceEntry)],
    query: &str,
) -> Vec<&'a (String, MarketplaceEntry)> {
    let q = query.to_lowercase();
    plugins
        .iter()
        .filter(|(_, e)| {
            e.name.to_lowercase().contains(&q)
                || e.description.to_lowercase().contains(&q)
                || e.category
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
                || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
        })
        .collect()
}

pub fn format_marketplace_entry(marketplace: &str, entry: &MarketplaceEntry) -> String {
    let mut s = format!("  {} ({})", entry.name, marketplace);
    if !entry.version.is_empty() {
        s.push_str(&format!(" v{}", entry.version));
    }
    if let Some(cat) = &entry.category {
        s.push_str(&format!(" [{cat}]"));
    }
    s.push('\n');
    if !entry.description.is_empty() {
        s.push_str(&format!("    {}\n", entry.description));
    }
    if !entry.tags.is_empty() {
        s.push_str(&format!("    tags: {}\n", entry.tags.join(", ")));
    }
    s
}

pub fn parse_plugin_identifier(id: &str) -> (String, Option<String>) {
    match id.find('@') {
        Some(idx) => (id[..idx].to_string(), Some(id[idx + 1..].to_string())),
        None => (id.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manifest() {
        let json = r#"{"name":"test-plugin","version":"1.0","description":"A test","mcpServers":{"s1":{"command":"node","args":["s.js"]}}}"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, "test-plugin");
        assert_eq!(m.mcp_servers.len(), 1);
    }

    #[test]
    fn test_plugin_manager() {
        let mgr = PluginManager::new();
        assert_eq!(mgr.count(), 0);
        assert!(mgr.enabled_plugins().is_empty());
        assert!(mgr.disabled_plugins().is_empty());
    }

    #[test]
    fn test_generate_extension_id() {
        let m = DxtManifest {
            name: "My Plugin".into(),
            version: "1.0".into(),
            description: String::new(),
            author: DxtAuthor {
                name: "John Doe".into(),
                email: None,
            },
            tools: vec![],
            server: None,
        };
        assert_eq!(generate_extension_id(&m), "john-doe.my-plugin");
    }

    #[test]
    fn test_parse_marketplace() {
        let json = r#"{"name":"test-mp","plugins":[{"name":"p-a","description":"d","version":"1.0","category":"prod","tags":["t1","t2"]}]}"#;
        let m: PluginMarketplace = serde_json::from_str(json).unwrap();
        assert_eq!(m.name, "test-mp");
        assert_eq!(m.plugins.len(), 1);
        assert_eq!(m.plugins[0].tags, vec!["t1", "t2"]);
    }

    #[test]
    fn test_search_plugins() {
        let plugins = vec![
            (
                "m".into(),
                MarketplaceEntry {
                    name: "git-helper".into(),
                    description: "Git workflow".into(),
                    version: "1.0".into(),
                    source: None,
                    category: Some("dev".into()),
                    tags: vec!["git".into()],
                    author: None,
                },
            ),
            (
                "m".into(),
                MarketplaceEntry {
                    name: "slack-notify".into(),
                    description: "Slack notifs".into(),
                    version: "2.0".into(),
                    source: None,
                    category: Some("comm".into()),
                    tags: vec!["slack".into()],
                    author: None,
                },
            ),
        ];
        assert_eq!(search_plugins(&plugins, "git").len(), 1);
        assert_eq!(search_plugins(&plugins, "slack").len(), 1);
        assert_eq!(search_plugins(&plugins, "dev").len(), 1);
        assert_eq!(search_plugins(&plugins, "xyz").len(), 0);
    }

    #[test]
    fn test_format_marketplace_entry() {
        let e = MarketplaceEntry {
            name: "tp".into(),
            description: "Great".into(),
            version: "1.0".into(),
            source: None,
            category: Some("tools".into()),
            tags: vec!["t".into()],
            author: None,
        };
        let f = format_marketplace_entry("mk", &e);
        assert!(
            f.contains("tp") && f.contains("mk") && f.contains("v1.0") && f.contains("[tools]")
        );
    }

    #[test]
    fn test_parse_plugin_identifier() {
        let (name, mp) = parse_plugin_identifier("my-plugin@my-market");
        assert_eq!(name, "my-plugin");
        assert_eq!(mp, Some("my-market".into()));
        let (name, mp) = parse_plugin_identifier("solo");
        assert_eq!(name, "solo");
        assert_eq!(mp, None);
    }

    #[test]
    fn test_get_plugin_error_message() {
        assert_eq!(
            get_plugin_error_message(&PluginError::GenericError {
                source: "s".into(),
                plugin: None,
                error: "boom".into()
            }),
            "boom"
        );
        assert!(get_plugin_error_message(&PluginError::PluginNotFound {
            source: "s".into(),
            plugin_id: "p".into(),
            marketplace: "m".into()
        })
        .contains("not found"));
        assert!(get_plugin_error_message(&PluginError::ManifestParseError {
            source: "s".into(),
            plugin: None,
            path: "/a".into(),
            error: "bad".into()
        })
        .contains("bad"));
        assert!(get_plugin_error_message(&PluginError::ComponentLoadFailed {
            source: "s".into(),
            plugin: "p".into(),
            component: "hooks".into(),
            path: "/b".into(),
            reason: "missing".into()
        })
        .contains("missing"));
    }

    #[test]
    fn test_versioned_cache_path() {
        let p = get_versioned_cache_path("my-plugin@my-market", "my-market", "1.0.0");
        let s = p.to_string_lossy();
        assert!(s.contains("my-market") && s.contains("my-plugin") && s.contains("1.0.0"));
    }

    #[test]
    fn test_plugin_enable_disable() {
        let mut mgr = PluginManager::new();
        let manifest = PluginManifest {
            name: "test".into(),
            version: String::new(),
            description: String::new(),
            author: None,
            repository: None,
            license: None,
            keywords: vec![],
            mcp_servers: HashMap::new(),
            skills: vec![],
            agents: vec![],
            lsp_servers: HashMap::new(),
            commands: None,
            agents_path: None,
            skills_path: None,
            hooks: None,
        };
        mgr.plugins.push(LoadedPlugin {
            manifest: manifest.clone(),
            path: PathBuf::from("/tmp/a"),
            source: "a".into(),
            enabled: true,
            is_builtin: false,
        });
        mgr.plugins.push(LoadedPlugin {
            manifest: PluginManifest {
                name: "off".into(),
                ..manifest
            },
            path: PathBuf::from("/tmp/b"),
            source: "b".into(),
            enabled: false,
            is_builtin: false,
        });
        assert_eq!(mgr.enabled_plugins().len(), 1);
        assert_eq!(mgr.disabled_plugins().len(), 1);
        assert_eq!(mgr.count(), 2);
    }
}

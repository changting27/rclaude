//! LSP service: client, server instance, and manager.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};

/// Pending request map type alias.
type PendingMap = HashMap<u64, oneshot::Sender<Result<Value, String>>>;

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// File extensions this server handles (e.g., [".rs", ".toml"])
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Language IDs per extension (e.g., {".rs": "rust"})
    #[serde(default)]
    pub language_ids: HashMap<String, String>,
    #[serde(default)]
    pub workspace_folder: Option<String>,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
}

fn default_max_restarts() -> u32 {
    3
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

// ── LSP Client (JSON-RPC over stdio) ──

pub struct LspClient {
    name: String,
    child: Option<Child>,
    stdin: Option<Arc<Mutex<tokio::process::ChildStdin>>>,
    pending: Arc<Mutex<PendingMap>>,
    next_id: AtomicU64,
    initialized: bool,
    capabilities: Option<Value>,
}

impl LspClient {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            child: None,
            stdin: None,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            initialized: false,
            capabilities: None,
        }
    }

    /// Start the LSP server process.
    pub async fn start(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<(), String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .envs(env);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start LSP server '{}': {e}", self.name))?;

        let stdin = child.stdin.take().ok_or("No stdin")?;
        let stdout = child.stdout.take().ok_or("No stdout")?;

        let stdin = Arc::new(Mutex::new(stdin));
        self.stdin = Some(stdin.clone());

        // Spawn reader task
        let pending = self.pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut headers = String::new();

            loop {
                headers.clear();
                let mut content_length: usize = 0;

                // Read headers
                loop {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => return, // EOF
                        Ok(_) => {
                            if line.trim().is_empty() {
                                break;
                            }
                            if let Some(len) = line.strip_prefix("Content-Length: ") {
                                content_length = len.trim().parse().unwrap_or(0);
                            }
                        }
                        Err(_) => return,
                    }
                }

                if content_length == 0 {
                    continue;
                }

                // Read body
                let mut body = vec![0u8; content_length];
                if tokio::io::AsyncReadExt::read_exact(&mut reader, &mut body)
                    .await
                    .is_err()
                {
                    return;
                }

                let msg: Value = match serde_json::from_slice(&body) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                // Route response to pending request
                if let Some(id) = msg.get("id").and_then(|v| v.as_u64()) {
                    let mut pending = pending.lock().await;
                    if let Some(tx) = pending.remove(&id) {
                        if let Some(error) = msg.get("error") {
                            let _ = tx.send(Err(error.to_string()));
                        } else {
                            let _ = tx.send(Ok(msg.get("result").cloned().unwrap_or(Value::Null)));
                        }
                    }
                }
            }
        });

        self.child = Some(child);
        Ok(())
    }

    /// Send a JSON-RPC request and wait for response.
    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        self.send_raw(&msg).await?;

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Response channel closed".into()),
            Err(_) => {
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                Err("Request timed out".into())
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected).
    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), String> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send_raw(&msg).await
    }

    async fn send_raw(&self, msg: &Value) -> Result<(), String> {
        let body = serde_json::to_string(msg).map_err(|e| e.to_string())?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        let stdin = self.stdin.as_ref().ok_or("Not started")?;
        let mut stdin = stdin.lock().await;
        stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Initialize the LSP server.
    pub async fn initialize(&mut self, workspace_root: &Path) -> Result<Value, String> {
        let uri = format!("file://{}", workspace_root.display());
        let params = json!({
            "processId": std::process::id(),
            "rootUri": uri,
            "workspaceFolders": [{ "uri": uri, "name": workspace_root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace") }],
            "capabilities": {
                "textDocument": {
                    "hover": { "contentFormat": ["markdown", "plaintext"] },
                    "definition": { "linkSupport": true },
                    "references": {},
                    "documentSymbol": { "hierarchicalDocumentSymbolSupport": true },
                    "publishDiagnostics": { "relatedInformation": true },
                },
                "general": { "positionEncodings": ["utf-16"] },
            }
        });

        let result = self.send_request("initialize", params).await?;
        self.capabilities = Some(result.clone());
        self.initialized = true;

        // Send initialized notification
        self.send_notification("initialized", json!({})).await?;
        Ok(result)
    }

    /// Shutdown and exit.
    pub async fn shutdown(&mut self) -> Result<(), String> {
        if self.initialized {
            let _ = self.send_request("shutdown", Value::Null).await;
            let _ = self.send_notification("exit", Value::Null).await;
            self.initialized = false;
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

// ── LSP Server Instance ──

pub struct LspServerInstance {
    pub name: String,
    pub config: LspServerConfig,
    pub state: ServerState,
    client: LspClient,
    restart_count: u32,
}

impl LspServerInstance {
    pub fn new(name: &str, config: LspServerConfig) -> Self {
        Self {
            name: name.to_string(),
            config,
            state: ServerState::Stopped,
            client: LspClient::new(name),
            restart_count: 0,
        }
    }

    pub async fn start(&mut self, workspace_root: &Path) -> Result<(), String> {
        if self.state == ServerState::Running {
            return Ok(());
        }
        self.state = ServerState::Starting;

        self.client
            .start(
                &self.config.command,
                &self.config.args,
                &self.config.env,
                self.config.workspace_folder.as_deref(),
            )
            .await?;

        self.client.initialize(workspace_root).await?;
        self.state = ServerState::Running;
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), String> {
        if self.state == ServerState::Stopped {
            return Ok(());
        }
        self.state = ServerState::Stopping;
        self.client.shutdown().await?;
        self.state = ServerState::Stopped;
        Ok(())
    }

    pub async fn restart(&mut self, workspace_root: &Path) -> Result<(), String> {
        self.restart_count += 1;
        if self.restart_count > self.config.max_restarts {
            return Err(format!(
                "Max restarts ({}) exceeded for '{}'",
                self.config.max_restarts, self.name
            ));
        }
        self.stop().await?;
        self.start(workspace_root).await
    }

    pub fn is_healthy(&self) -> bool {
        self.state == ServerState::Running && self.client.is_initialized()
    }

    pub async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        if !self.is_healthy() {
            return Err(format!(
                "Server '{}' is not healthy (state: {:?})",
                self.name, self.state
            ));
        }
        self.client.send_request(method, params).await
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> Result<(), String> {
        if !self.is_healthy() {
            return Err(format!("Server '{}' is not healthy", self.name));
        }
        self.client.send_notification(method, params).await
    }
}

// ── LSP Server Manager ──

pub struct LspServerManager {
    servers: HashMap<String, LspServerInstance>,
    /// Extension → server name mapping
    extension_map: HashMap<String, String>,
    /// Files currently open on servers (URI → server name)
    open_files: HashMap<String, String>,
    workspace_root: PathBuf,
}

impl LspServerManager {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            servers: HashMap::new(),
            extension_map: HashMap::new(),
            open_files: HashMap::new(),
            workspace_root,
        }
    }

    /// Register a server configuration.
    pub fn register(&mut self, name: &str, config: LspServerConfig) {
        for ext in &config.extensions {
            self.extension_map.insert(ext.clone(), name.to_string());
        }
        self.servers
            .insert(name.to_string(), LspServerInstance::new(name, config));
    }

    /// Get the server for a file path based on extension.
    pub fn get_server_for_file(&self, file_path: &str) -> Option<&str> {
        let ext = Path::new(file_path).extension()?.to_str()?;
        let dotted = format!(".{ext}");
        self.extension_map.get(&dotted).map(|s| s.as_str())
    }

    /// Ensure the server for a file is started.
    pub async fn ensure_server_started(
        &mut self,
        file_path: &str,
    ) -> Result<&mut LspServerInstance, String> {
        let server_name = self
            .get_server_for_file(file_path)
            .ok_or_else(|| format!("No LSP server configured for: {file_path}"))?
            .to_string();

        let root = self.workspace_root.clone();
        let server = self
            .servers
            .get_mut(&server_name)
            .ok_or_else(|| format!("Server '{}' not found", server_name))?;

        if !server.is_healthy() {
            server.start(&root).await?;
        }

        Ok(server)
    }

    /// Send a request to the appropriate server for a file.
    pub async fn send_request(
        &mut self,
        file_path: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let server = self.ensure_server_started(file_path).await?;
        server.send_request(method, params).await
    }

    /// Open a file on the appropriate server.
    pub async fn open_file(&mut self, file_path: &str, content: &str) -> Result<(), String> {
        let server_name = self
            .get_server_for_file(file_path)
            .ok_or("No server for file")?
            .to_string();
        let uri = format!("file://{file_path}");
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let lang_id = self
            .servers
            .get(&server_name)
            .and_then(|s| s.config.language_ids.get(&format!(".{ext}")))
            .cloned()
            .unwrap_or_else(|| ext.to_string());

        let server = self.ensure_server_started(file_path).await?;
        server
            .send_notification(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": lang_id,
                        "version": 1,
                        "text": content,
                    }
                }),
            )
            .await?;

        self.open_files.insert(uri, server_name);
        Ok(())
    }

    /// Shutdown all servers.
    pub async fn shutdown(&mut self) {
        for (_, server) in self.servers.iter_mut() {
            let _ = server.stop().await;
        }
        self.open_files.clear();
    }

    /// Get all registered server names.
    pub fn server_names(&self) -> Vec<&str> {
        self.servers.keys().map(|s| s.as_str()).collect()
    }
}

// ── Default configurations for common languages ──

/// Create default LSP configs for common languages.
pub fn default_lsp_configs() -> Vec<(&'static str, LspServerConfig)> {
    vec![
        (
            "rust-analyzer",
            LspServerConfig {
                command: "rust-analyzer".into(),
                args: vec![],
                env: HashMap::new(),
                extensions: vec![".rs".into()],
                language_ids: [(".rs".into(), "rust".into())].into_iter().collect(),
                workspace_folder: None,
                max_restarts: 3,
            },
        ),
        (
            "typescript-language-server",
            LspServerConfig {
                command: "typescript-language-server".into(),
                args: vec!["--stdio".into()],
                env: HashMap::new(),
                extensions: vec![".ts".into(), ".tsx".into(), ".js".into(), ".jsx".into()],
                language_ids: [
                    (".ts".into(), "typescript".into()),
                    (".tsx".into(), "typescriptreact".into()),
                    (".js".into(), "javascript".into()),
                    (".jsx".into(), "javascriptreact".into()),
                ]
                .into_iter()
                .collect(),
                workspace_folder: None,
                max_restarts: 3,
            },
        ),
        (
            "pyright",
            LspServerConfig {
                command: "pyright-langserver".into(),
                args: vec!["--stdio".into()],
                env: HashMap::new(),
                extensions: vec![".py".into()],
                language_ids: [(".py".into(), "python".into())].into_iter().collect(),
                workspace_folder: None,
                max_restarts: 3,
            },
        ),
        (
            "gopls",
            LspServerConfig {
                command: "gopls".into(),
                args: vec!["serve".into()],
                env: HashMap::new(),
                extensions: vec![".go".into()],
                language_ids: [(".go".into(), "go".into())].into_iter().collect(),
                workspace_folder: None,
                max_restarts: 3,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_state_transitions() {
        let config = LspServerConfig {
            command: "echo".into(),
            args: vec![],
            env: HashMap::new(),
            extensions: vec![".rs".into()],
            language_ids: HashMap::new(),
            workspace_folder: None,
            max_restarts: 3,
        };
        let instance = LspServerInstance::new("test", config);
        assert_eq!(instance.state, ServerState::Stopped);
        assert!(!instance.is_healthy());
    }

    #[test]
    fn test_manager_extension_routing() {
        let mut manager = LspServerManager::new(PathBuf::from("/tmp"));
        manager.register(
            "rust-analyzer",
            LspServerConfig {
                command: "rust-analyzer".into(),
                args: vec![],
                env: HashMap::new(),
                extensions: vec![".rs".into()],
                language_ids: HashMap::new(),
                workspace_folder: None,
                max_restarts: 3,
            },
        );
        manager.register(
            "tsserver",
            LspServerConfig {
                command: "tsserver".into(),
                args: vec![],
                env: HashMap::new(),
                extensions: vec![".ts".into(), ".tsx".into()],
                language_ids: HashMap::new(),
                workspace_folder: None,
                max_restarts: 3,
            },
        );

        assert_eq!(
            manager.get_server_for_file("src/main.rs"),
            Some("rust-analyzer")
        );
        assert_eq!(manager.get_server_for_file("src/app.tsx"), Some("tsserver"));
        assert_eq!(manager.get_server_for_file("README.md"), None);
    }

    #[test]
    fn test_default_configs() {
        let configs = default_lsp_configs();
        assert!(configs.len() >= 4);
        let names: Vec<&str> = configs.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"rust-analyzer"));
        assert!(names.contains(&"pyright"));
        assert!(names.contains(&"gopls"));
    }
}

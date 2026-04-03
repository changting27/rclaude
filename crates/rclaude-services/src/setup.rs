//! Setup module: first-run initialization and environment detection.

use std::path::{Path, PathBuf};

/// Setup result with detected environment info.
#[derive(Debug)]
pub struct SetupResult {
    pub cwd: PathBuf,
    pub is_git: bool,
    pub git_root: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub git_default_branch: Option<String>,
    pub has_claude_md: bool,
    pub config_dir_exists: bool,
}

/// Run setup: detect environment, check prerequisites.
pub async fn run_setup(cwd: &Path) -> SetupResult {
    let is_git = rclaude_utils::git::is_git_repo(cwd).await;
    let git_branch = if is_git {
        rclaude_utils::git::get_branch(cwd).await.unwrap_or(None)
    } else {
        None
    };
    let git_default_branch = if is_git {
        rclaude_utils::git::get_default_branch(cwd)
            .await
            .unwrap_or(None)
    } else {
        None
    };
    let git_root = if is_git {
        find_git_root(cwd).await
    } else {
        None
    };
    let has_claude_md = cwd.join("CLAUDE.md").exists();
    let config_dir_exists = rclaude_core::config::Config::config_dir().exists();

    SetupResult {
        cwd: cwd.to_path_buf(),
        is_git,
        git_root,
        git_branch,
        git_default_branch,
        has_claude_md,
        config_dir_exists,
    }
}

/// Ensure config directory exists.
pub fn ensure_config_dir() -> std::io::Result<PathBuf> {
    let dir = rclaude_core::config::Config::config_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Check if this is the first run (no config dir or no API key from any source).
pub fn is_first_run() -> bool {
    let config_dir = rclaude_core::config::Config::config_dir();
    if !config_dir.exists() {
        return true;
    }
    // Check all auth sources (env, .credentials.json, settings.json, ~/.claude.json, helper)
    !rclaude_core::auth::has_api_key_auth()
}

/// Run first-time setup interactively.
pub async fn run_first_time_setup(cwd: &Path) -> std::io::Result<()> {
    use std::io::Write;

    let config_dir = ensure_config_dir()?;

    // Check for API key
    let config = rclaude_core::config::Config::load();
    if config.api_key.is_none() && std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Welcome to rclaude! Let's get you set up.\n");
        eprint!("Enter your Anthropic API key: ");
        std::io::stderr().flush()?;

        let mut key = String::new();
        std::io::stdin().read_line(&mut key)?;
        let key = key.trim().to_string();

        if !key.is_empty() {
            if let Err(e) = rclaude_core::auth::save_api_key(&key) {
                eprintln!("Warning: Failed to save API key: {e}");
            } else {
                eprintln!("API key saved to {}", config_dir.display());
            }
        }
    }

    // Create CLAUDE.md if it doesn't exist and we're in a git repo
    if !cwd.join("CLAUDE.md").exists() && rclaude_utils::git::is_git_repo(cwd).await {
        eprintln!("\nTip: Create a CLAUDE.md file in your project root to give rclaude context about your project.");
    }

    Ok(())
}

/// Find the git root directory.
async fn find_git_root(cwd: &Path) -> Option<PathBuf> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;
    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(PathBuf::from(root))
    } else {
        None
    }
}

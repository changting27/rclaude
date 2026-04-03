//! Doctor diagnostic checks for environment validation.
//! System health checks and environment diagnostics.

use std::path::Path;

/// Diagnostic check result.
#[derive(Debug, Clone)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

/// Run all diagnostic checks.
pub async fn run_diagnostics(cwd: &Path) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    // API key
    checks.push(check_api_key());
    // Config directory
    checks.push(check_config_dir());
    // Git
    checks.push(check_git(cwd).await);
    // Working directory
    checks.push(DiagnosticCheck {
        name: "Working directory".into(),
        status: CheckStatus::Pass,
        message: format!("{}", cwd.display()),
    });
    // Rust toolchain
    checks.push(check_command("cargo", &["--version"], "Rust toolchain").await);
    // Node.js
    checks.push(check_command("node", &["--version"], "Node.js").await);
    // Git version
    checks.push(check_command("git", &["--version"], "Git").await);

    checks
}

fn check_api_key() -> DiagnosticCheck {
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        DiagnosticCheck {
            name: "API key".into(),
            status: CheckStatus::Pass,
            message: "Set via ANTHROPIC_API_KEY".into(),
        }
    } else if rclaude_core::config::Config::config_dir()
        .join("settings.json")
        .exists()
    {
        DiagnosticCheck {
            name: "API key".into(),
            status: CheckStatus::Pass,
            message: "Found in config".into(),
        }
    } else {
        DiagnosticCheck {
            name: "API key".into(),
            status: CheckStatus::Fail,
            message: "Not configured. Set ANTHROPIC_API_KEY or run /login".into(),
        }
    }
}

fn check_config_dir() -> DiagnosticCheck {
    let dir = rclaude_core::config::Config::config_dir();
    if dir.exists() {
        DiagnosticCheck {
            name: "Config directory".into(),
            status: CheckStatus::Pass,
            message: format!("{}", dir.display()),
        }
    } else {
        DiagnosticCheck {
            name: "Config directory".into(),
            status: CheckStatus::Warn,
            message: format!("Missing: {}", dir.display()),
        }
    }
}

async fn check_git(cwd: &Path) -> DiagnosticCheck {
    if rclaude_utils::git::is_git_repo(cwd).await {
        let branch = rclaude_utils::git::get_branch(cwd)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "unknown".into());
        DiagnosticCheck {
            name: "Git repository".into(),
            status: CheckStatus::Pass,
            message: format!("Branch: {branch}"),
        }
    } else {
        DiagnosticCheck {
            name: "Git repository".into(),
            status: CheckStatus::Warn,
            message: "Not in a git repository".into(),
        }
    }
}

async fn check_command(cmd: &str, args: &[&str], label: &str) -> DiagnosticCheck {
    match tokio::process::Command::new(cmd).args(args).output().await {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            DiagnosticCheck {
                name: label.into(),
                status: CheckStatus::Pass,
                message: version,
            }
        }
        _ => DiagnosticCheck {
            name: label.into(),
            status: CheckStatus::Warn,
            message: format!("{cmd} not found"),
        },
    }
}

/// Format diagnostics for display.
pub fn format_diagnostics(checks: &[DiagnosticCheck]) -> String {
    checks
        .iter()
        .map(|c| {
            let icon = match c.status {
                CheckStatus::Pass => "✓",
                CheckStatus::Warn => "!",
                CheckStatus::Fail => "✗",
            };
            format!("{icon} {}: {}", c.name, c.message)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

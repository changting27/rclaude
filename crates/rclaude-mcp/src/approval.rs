//! MCP server approval: prompt user before connecting to new servers.

use std::collections::HashSet;
use std::path::PathBuf;

/// Load approved MCP server names from disk.
pub fn load_approved_servers() -> HashSet<String> {
    let path = approved_servers_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save approved MCP server names to disk.
pub fn save_approved_servers(servers: &HashSet<String>) {
    let path = approved_servers_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string(servers).unwrap_or_default());
}

/// Check if a server is approved. If not, prompt the user.
pub fn check_server_approved(name: &str, command: &str) -> bool {
    let mut approved = load_approved_servers();
    if approved.contains(name) {
        return true;
    }

    // Non-interactive: auto-approve
    if !atty::is(atty::Stream::Stdin) {
        return true;
    }

    eprintln!("\n  New MCP server: {name}");
    eprintln!("  Command: {command}");
    eprint!("  Allow connection? [y/N]: ");
    use std::io::Write;
    std::io::stderr().flush().ok();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let answer = input.trim().to_lowercase();
        if answer == "y" || answer == "yes" {
            approved.insert(name.to_string());
            save_approved_servers(&approved);
            return true;
        }
    }
    false
}

fn approved_servers_path() -> PathBuf {
    rclaude_core::config::Config::config_dir().join("approved-mcp.json")
}

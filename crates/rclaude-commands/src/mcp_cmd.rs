use async_trait::async_trait;
use colored::Colorize;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct McpCommand;

#[async_trait]
impl Command for McpCommand {
    fn name(&self) -> &str {
        "mcp"
    }
    fn description(&self) -> &str {
        "Manage MCP server connections"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let args = args.trim();
        match args {
            "" | "list" => {
                // List configured MCP servers
                let config = rclaude_mcp::config::load_mcp_config(&state.cwd)?;
                if config.mcp_servers.is_empty() {
                    return Ok(CommandResult::Ok(Some(
                        "No MCP servers configured. Add to .mcp.json or ~/.claude/mcp.json".into(),
                    )));
                }
                let mut output = format!("{}\n", "MCP Servers:".bold());
                for (name, cfg) in &config.mcp_servers {
                    output.push_str(&format!(
                        "  {} — {} {}\n",
                        name.cyan(),
                        cfg.command,
                        cfg.args.join(" ")
                    ));
                }
                Ok(CommandResult::Ok(Some(output)))
            }
            _ if args.starts_with("add ") => Ok(CommandResult::Ok(Some(
                "MCP add: edit .mcp.json to add servers.".into(),
            ))),
            _ => Ok(CommandResult::Ok(Some(
                "Usage: /mcp [list|add <name>]".into(),
            ))),
        }
    }
}

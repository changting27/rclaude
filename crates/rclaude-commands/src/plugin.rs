use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::plugins;
use rclaude_core::state::AppState;

pub struct PluginCommand;

#[async_trait]
impl Command for PluginCommand {
    fn name(&self) -> &str {
        "plugin"
    }
    fn description(&self) -> &str {
        "Manage plugins (list, browse, search, install, uninstall, enable, disable)"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let parts: Vec<&str> = args.trim().splitn(2, ' ').collect();
        let sub = parts.first().copied().unwrap_or("");
        let sub_args = parts.get(1).copied().unwrap_or("");
        match sub {
            "" | "list" => cmd_list(state).await,
            "browse" => cmd_browse().await,
            "search" => cmd_search(sub_args).await,
            "refresh" => cmd_refresh().await,
            "install" => cmd_install(sub_args).await,
            "uninstall" => cmd_uninstall(sub_args).await,
            "enable" => cmd_enable(sub_args).await,
            "disable" => cmd_disable(sub_args).await,
            _ => Ok(CommandResult::Ok(Some(
                "Usage: /plugin [list|browse|search <q>|install <id>|uninstall <id>|enable <id>|disable <id>|refresh]".into(),
            ))),
        }
    }
}

async fn cmd_list(state: &AppState) -> Result<CommandResult> {
    let mut mgr = plugins::PluginManager::new();
    mgr.load_plugins(&state.cwd).await;
    let enabled = mgr.enabled_plugins();
    let disabled = mgr.disabled_plugins();
    if enabled.is_empty() && disabled.is_empty() && mgr.errors.is_empty() {
        return Ok(CommandResult::Ok(Some(
            "No plugins installed.\nBrowse: /plugin browse".into(),
        )));
    }
    let mut out = String::new();
    if !enabled.is_empty() {
        out.push_str(&format!("{}\n", "Enabled Plugins".bold()));
        for p in &enabled {
            out.push_str(&format!(
                "  {} v{} — {}\n",
                p.manifest.name.cyan(),
                p.manifest.version,
                p.manifest.description
            ));
            let mc = p.manifest.mcp_servers.len();
            let sc = p.manifest.skills.len();
            let ac = p.manifest.agents.len();
            if mc + sc + ac > 0 {
                out.push_str(&format!("    {} MCP · {} skills · {} agents\n", mc, sc, ac));
            }
        }
    }
    if !disabled.is_empty() {
        out.push_str(&format!("\n{}\n", "Disabled Plugins".dimmed()));
        for p in &disabled {
            out.push_str(&format!("  {} (disabled)\n", p.manifest.name.dimmed()));
        }
    }
    if !mgr.errors.is_empty() {
        out.push_str(&format!("\n{}\n", "Errors:".red()));
        for e in &mgr.errors {
            out.push_str(&format!("  {}\n", plugins::get_plugin_error_message(e)));
        }
    }
    Ok(CommandResult::Ok(Some(out)))
}

async fn cmd_browse() -> Result<CommandResult> {
    eprintln!("{}", "Fetching marketplace...".dimmed());
    let all = plugins::list_marketplace_plugins().await;
    if all.is_empty() {
        match plugins::fetch_official_marketplace().await {
            Ok(mp) => {
                let mut out = format!(
                    "{} ({})\n\n",
                    mp.name.bold(),
                    format!("{} plugins", mp.plugins.len()).dimmed()
                );
                for e in &mp.plugins {
                    out.push_str(&plugins::format_marketplace_entry(&mp.name, e));
                }
                out.push_str(&format!(
                    "\n{}",
                    "Install: /plugin install <name>@<marketplace>".dimmed()
                ));
                return Ok(CommandResult::Ok(Some(out)));
            }
            Err(e) => {
                return Ok(CommandResult::Ok(Some(format!(
                    "Failed to fetch marketplace: {e}"
                ))))
            }
        }
    }
    let mut out = format!("{}\n\n", "Available Plugins".bold());
    for (mp, e) in &all {
        out.push_str(&plugins::format_marketplace_entry(mp, e));
    }
    out.push_str(&format!(
        "\n{} plugins\n{}",
        all.len(),
        "Install: /plugin install <name>@<marketplace>".dimmed()
    ));
    Ok(CommandResult::Ok(Some(out)))
}

async fn cmd_search(query: &str) -> Result<CommandResult> {
    if query.is_empty() {
        return Ok(CommandResult::Ok(Some(
            "Usage: /plugin search <query>".into(),
        )));
    }
    eprintln!("{}", "Searching...".dimmed());
    let mut all = plugins::list_marketplace_plugins().await;
    if all.is_empty() {
        if let Ok(mp) = plugins::fetch_official_marketplace().await {
            for e in mp.plugins {
                all.push((mp.name.clone(), e));
            }
        }
    }
    let results = plugins::search_plugins(&all, query);
    if results.is_empty() {
        return Ok(CommandResult::Ok(Some(format!(
            "No plugins matching '{query}'."
        ))));
    }
    let mut out = format!(
        "{} ({} results)\n\n",
        format!("Search: {query}").bold(),
        results.len()
    );
    for (mp, e) in results {
        out.push_str(&plugins::format_marketplace_entry(mp, e));
    }
    Ok(CommandResult::Ok(Some(out)))
}

async fn cmd_install(args: &str) -> Result<CommandResult> {
    if args.is_empty() {
        return Ok(CommandResult::Ok(Some(
            "Usage: /plugin install <name>@<marketplace>".into(),
        )));
    }
    let (name, marketplace) = plugins::parse_plugin_identifier(args);
    let mp = marketplace
        .as_deref()
        .unwrap_or(plugins::OFFICIAL_MARKETPLACE_NAME);
    eprintln!("{}", format!("Installing {name} from {mp}...").dimmed());
    match plugins::install_plugin(args, mp).await {
        Ok(p) => Ok(CommandResult::Ok(Some(format!(
            "{} Installed {} from {mp}",
            "✓".green(),
            p.manifest.name
        )))),
        Err(e) => Ok(CommandResult::Ok(Some(format!("{} {e}", "✗".red())))),
    }
}

async fn cmd_uninstall(args: &str) -> Result<CommandResult> {
    if args.is_empty() {
        return Ok(CommandResult::Ok(Some(
            "Usage: /plugin uninstall <name>".into(),
        )));
    }
    match plugins::uninstall_plugin(args).await {
        Ok(()) => Ok(CommandResult::Ok(Some(format!(
            "{} Uninstalled {args}",
            "✓".green()
        )))),
        Err(e) => Ok(CommandResult::Ok(Some(format!("{} {e}", "✗".red())))),
    }
}

async fn cmd_enable(args: &str) -> Result<CommandResult> {
    if args.is_empty() {
        return Ok(CommandResult::Ok(Some("Usage: /plugin enable <id>".into())));
    }
    plugins::enable_plugin(args).await;
    Ok(CommandResult::Ok(Some(format!(
        "{} Enabled {args}",
        "✓".green()
    ))))
}

async fn cmd_disable(args: &str) -> Result<CommandResult> {
    if args.is_empty() {
        return Ok(CommandResult::Ok(Some(
            "Usage: /plugin disable <id>".into(),
        )));
    }
    plugins::disable_plugin(args).await;
    Ok(CommandResult::Ok(Some(format!(
        "{} Disabled {args}",
        "✓".green()
    ))))
}

async fn cmd_refresh() -> Result<CommandResult> {
    eprintln!("{}", "Refreshing marketplace...".dimmed());
    match plugins::fetch_official_marketplace().await {
        Ok(mp) => {
            let cache_dir = plugins::get_plugin_cache_dir()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("marketplaces");
            let _ = tokio::fs::create_dir_all(&cache_dir).await;
            let cache_path = cache_dir.join(format!("{}.json", mp.name));
            let _ = tokio::fs::write(
                &cache_path,
                serde_json::to_string_pretty(&mp).unwrap_or_default(),
            )
            .await;
            let mut config = plugins::load_known_marketplaces().await;
            config.marketplaces.insert(mp.name.clone(), plugins::KnownMarketplace {
                install_location: cache_path.to_string_lossy().to_string(),
                source: Some(serde_json::json!({"source": "github", "repo": plugins::OFFICIAL_MARKETPLACE_REPO})),
            });
            plugins::save_known_marketplaces(&config).await;
            Ok(CommandResult::Ok(Some(format!(
                "{} Refreshed {} ({} plugins)",
                "✓".green(),
                mp.name,
                mp.plugins.len()
            ))))
        }
        Err(e) => Ok(CommandResult::Ok(Some(format!(
            "{} Failed: {e}",
            "✗".red()
        )))),
    }
}

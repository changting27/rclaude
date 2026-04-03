use clap::Parser;
use colored::Colorize;
use rclaude_core::auto_compact::AutoCompactState;
use rclaude_core::hooks::{HookEvent, HookRegistry};
use rclaude_core::message::{ContentBlock, Role};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

mod query_engine;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// rclaude - CLI tool for Anthropic Claude API
#[derive(Parser, Debug)]
#[command(name = "rclaude", version, about, long_about = None)]
struct Cli {
    /// Initial prompt to send (non-interactive mode)
    #[arg(trailing_var_arg = true)]
    prompt: Vec<String>,

    /// Model to use
    #[arg(short, long, default_value = DEFAULT_MODEL)]
    model: String,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Enable debug mode
    #[arg(long)]
    debug: bool,

    /// Maximum output tokens
    #[arg(long, default_value = "16384")]
    max_tokens: u32,

    /// Permission mode
    #[arg(long, value_enum, default_value = "default")]
    permission_mode: PermissionModeArg,

    /// Print mode (non-interactive, single response)
    #[arg(short, long)]
    print: bool,

    /// Output format for print mode
    #[arg(long, default_value = "text")]
    output_format: String,

    /// System prompt to prepend
    #[arg(long)]
    system_prompt: Option<String>,

    /// Append to system prompt
    #[arg(long)]
    append_system_prompt: Option<String>,

    /// Custom API key
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    api_key: Option<String>,

    /// Resume a previous session (optionally by ID)
    #[arg(short = 'r', long)]
    resume: Option<Option<String>>,

    /// Continue the most recent session
    #[arg(short = 'c', long)]
    r#continue: bool,

    /// Use TUI mode (interactive terminal UI)
    #[arg(long)]
    tui: bool,

    /// Allowed tools (comma-separated)
    #[arg(long, value_delimiter = ',')]
    allowed_tools: Vec<String>,

    /// Disallowed tools (comma-separated)
    #[arg(long, value_delimiter = ',')]
    disallowed_tools: Vec<String>,

    /// Maximum agentic turns (--print mode)
    #[arg(long)]
    max_turns: Option<usize>,

    /// Maximum USD budget for API calls (safety net)
    #[arg(long)]
    max_budget_usd: Option<f64>,

    /// Session name
    #[arg(short = 'n', long)]
    name: Option<String>,

    /// Additional directories to allow access
    #[arg(long)]
    add_dir: Vec<String>,

    /// Fallback model on overload
    #[arg(long)]
    fallback_model: Option<String>,

    /// Reasoning effort level
    #[arg(long)]
    effort: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum PermissionModeArg {
    Default,
    Auto,
    BypassPermissions,
    Plan,
}

type SharedState = Arc<RwLock<rclaude_core::state::AppState>>;

/// CLI renderer implementing TurnRenderer for terminal output.
struct CliRenderer {
    output_format: String,
    is_non_interactive: bool,
    at_line_start: std::sync::atomic::AtomicBool,
}

impl CliRenderer {
    fn new(output_format: &str, is_non_interactive: bool) -> Self {
        Self {
            output_format: output_format.to_string(),
            is_non_interactive,
            at_line_start: std::sync::atomic::AtomicBool::new(true),
        }
    }
}

impl query_engine::TurnRenderer for CliRenderer {
    fn on_text_delta(&self, text: &str) {
        if self.output_format != "text" {
            return;
        }
        use std::sync::atomic::Ordering;
        for ch in text.chars() {
            if self.at_line_start.load(Ordering::Relaxed) {
                print!("  ");
                self.at_line_start.store(false, Ordering::Relaxed);
            }
            print!("{ch}");
            if ch == '\n' {
                self.at_line_start.store(true, Ordering::Relaxed);
            }
        }
        use std::io::Write;
        std::io::stdout().flush().ok();
    }

    fn on_thinking_delta(&self, thinking: &str) {
        if thinking.len() > 10 {
            eprint!("{}", " [thinking...]".dimmed());
        }
    }

    fn on_tool_calls(&self, calls: &[(String, String, serde_json::Value)]) {
        if self.output_format != "text" || self.is_non_interactive {
            return;
        }
        for (_, name, input) in calls {
            let param = format_tool_param(name, input);
            eprintln!("  {} {}{}", "⏺".dimmed(), name.cyan(), param.dimmed());
        }
    }

    fn on_tool_results(&self, results: &[rclaude_core::streaming_executor::OrderedToolResult]) {
        if self.output_format != "text" || self.is_non_interactive {
            return;
        }
        for r in results {
            let summary = format_tool_result_summary(&r.tool_name, &r.result_text, r.is_error);
            if r.is_error {
                eprintln!(
                    "  {}  {} {}",
                    "⎿".dimmed(),
                    r.tool_name.red(),
                    summary.red()
                );
            } else {
                let show_preview =
                    matches!(r.tool_name.as_str(), "Read" | "Grep" | "Glob" | "Bash");
                if show_preview && !r.result_text.is_empty() {
                    let preview_lines: Vec<&str> = r.result_text.lines().take(3).collect();
                    for line in &preview_lines {
                        let truncated = if line.len() > 120 {
                            format!("{}…", &line[..119])
                        } else {
                            line.to_string()
                        };
                        eprintln!("  {}  {}", "⎿".dimmed(), truncated.dimmed());
                    }
                    let total = r.result_text.lines().count();
                    if total > 3 {
                        eprintln!(
                            "  {}  {}",
                            " ".dimmed(),
                            format!("… ({total} lines total)").dimmed()
                        );
                    }
                } else {
                    eprintln!("  {}  {}", "⎿".dimmed(), summary.dimmed());
                }
            }
        }
    }

    fn on_retry(&self, reason: &str, attempt: u32, max: u32, delay_secs: u64) {
        for remaining in (1..=delay_secs).rev() {
            eprint!(
                "\r{}",
                format!("⏳ {reason} (attempt {attempt}/{max}) retrying in {remaining}s...")
                    .yellow()
            );
            use std::io::Write;
            std::io::stderr().flush().ok();
            // Note: can't async sleep here since trait fn is not async.
            // The actual delay is handled by QueryEngine.
        }
        eprintln!(
            "\r{}",
            format!("⟳ {reason} — retrying now...        ").yellow()
        );
    }

    fn on_compact(&self, before: usize, after: usize) {
        eprintln!(
            "{}",
            format!("Auto-compacted: {before} → {after} messages").dimmed()
        );
    }

    fn on_status(&self, model: &str, cost: f64) {
        if self.output_format == "text" && !self.is_non_interactive {
            let model_display = model.rsplit('/').next().unwrap_or(model);
            eprintln!("{}", format!("  {model_display} · ${cost:.4}").dimmed());
        }
    }

    fn on_error(&self, message: &str) {
        eprintln!("{}", message.yellow());
    }
}

/// Session-level context shared across turns.
#[allow(dead_code)]
struct SessionContext {
    hooks: HookRegistry,
    auto_compact: AutoCompactState,
    tool_stats: rclaude_services::tool_execution::ToolStats,
    history: rclaude_services::history::InputHistory,
    max_output_recovery_count: u32,
    has_attempted_reactive_compact: bool,
    stream_retry_count: u32,
    /// CLI-level tool restrictions
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
    max_turns: usize,
    max_budget_usd: Option<f64>,
    #[allow(dead_code)]
    fallback_model: Option<String>,
    system_prompt_override: Option<String>,
    append_system_prompt: Option<String>,
    #[allow(dead_code)]
    output_format: String,
    consecutive_denials: u32,
    content_replacement_state: rclaude_core::tool_result_storage::ContentReplacementState,
    /// QueryEngine instance (created lazily on first turn)
    query_engine: Option<query_engine::QueryEngine>,
}

impl SessionContext {
    fn get_or_create_engine(&mut self) -> &mut query_engine::QueryEngine {
        if self.query_engine.is_none() {
            let config = query_engine::QueryEngineConfig {
                allowed_tools: self.allowed_tools.clone(),
                disallowed_tools: self.disallowed_tools.clone(),
                max_turns: self.max_turns,
                max_budget_usd: self.max_budget_usd,
                fallback_model: self.fallback_model.clone(),
                system_prompt_override: self.system_prompt_override.clone(),
                append_system_prompt: self.append_system_prompt.clone(),
                output_format: self.output_format.clone(),
                verbose: false,
            };
            self.query_engine = Some(query_engine::QueryEngine::new(config, self.hooks.clone()));
        }
        self.query_engine.as_mut().unwrap()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.debug {
        // C05: Write debug logs to ~/.claude/debug/
        let debug_dir = rclaude_core::config::Config::config_dir().join("debug");
        let _ = std::fs::create_dir_all(&debug_dir);
        let log_file = debug_dir.join(format!(
            "rclaude-{}.log",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        let file = std::fs::File::create(&log_file).ok();
        if let Some(file) = file {
            // Symlink ~/.claude/debug/latest → this log
            let latest = debug_dir.join("latest");
            let _ = std::fs::remove_file(&latest);
            let _ = std::os::unix::fs::symlink(&log_file, &latest);

            tracing_subscriber::fmt()
                .with_env_filter("rclaude=debug")
                .with_writer(std::sync::Mutex::new(file))
                .with_ansi(false)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter("rclaude=debug")
                .init();
        }
    } else if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("rclaude=info")
            .with_writer(std::io::stderr)
            .init();
    }

    let cwd = std::env::current_dir()?;

    // First-run setup
    if rclaude_services::setup::is_first_run() {
        rclaude_services::setup::run_first_time_setup(&cwd)
            .await
            .ok();
    }

    // #3: Trust dialog — confirm before running in untrusted directories
    if !cli.print && atty::is(atty::Stream::Stdin) {
        let trust_file = rclaude_core::config::Config::config_dir().join("trusted_dirs.json");
        let trusted: Vec<String> = std::fs::read_to_string(&trust_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let cwd_str = cwd.to_string_lossy().to_string();
        if !trusted.iter().any(|t| cwd_str.starts_with(t)) {
            eprintln!(
                "{}",
                format!(
                    "First time in {}. Do you trust this directory?",
                    cwd.display()
                )
                .yellow()
            );
            eprint!("  [y]es / [n]o: ");
            use std::io::Write;
            std::io::stderr().flush().ok();
            let mut answer = String::new();
            if std::io::stdin().read_line(&mut answer).is_ok() {
                let a = answer.trim().to_lowercase();
                if a == "y" || a == "yes" {
                    let mut dirs = trusted;
                    dirs.push(cwd_str);
                    let _ = std::fs::create_dir_all(trust_file.parent().unwrap());
                    let _ = std::fs::write(
                        &trust_file,
                        serde_json::to_string(&dirs).unwrap_or_default(),
                    );
                } else {
                    eprintln!("Exiting. Run in a trusted directory or use --print mode.");
                    std::process::exit(0);
                }
            }
        }
    }

    let mut config = rclaude_core::config::Config::load();
    if let Some(ref key) = cli.api_key {
        config.api_key = Some(key.clone());
    }
    config.model = cli.model.clone();
    config.max_tokens = cli.max_tokens;
    config.verbose = cli.verbose;
    if let Some(ref effort) = cli.effort {
        // Store effort in config for system prompt to reference
        config.env.insert("CLAUDE_EFFORT".into(), effort.clone());
    }

    // Load hooks from settings (both global and project)
    let mut hooks = HookRegistry::new();
    for settings_path in [
        rclaude_core::config::Config::config_dir().join("settings.json"),
        cwd.join(".claude/settings.json"),
    ] {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                hooks.load_from_settings(&val);
            }
        }
    }

    let mut state = rclaude_core::state::AppState::new(cwd.clone(), config.clone());

    state.permission_mode = match cli.permission_mode {
        PermissionModeArg::Default => rclaude_core::permissions::PermissionMode::Default,
        PermissionModeArg::Auto => rclaude_core::permissions::PermissionMode::Auto,
        PermissionModeArg::BypassPermissions => {
            rclaude_core::permissions::PermissionMode::BypassPermissions
        }
        PermissionModeArg::Plan => rclaude_core::permissions::PermissionMode::Plan,
    };

    // --name: set terminal title
    if let Some(ref name) = cli.name {
        eprint!("\x1b]0;rclaude: {name}\x07");
    }

    // Apply env vars from config
    for (k, v) in &config.env {
        std::env::set_var(k, v);
    }

    // --add-dir: register additional directories
    for dir in &cli.add_dir {
        let path = std::path::Path::new(dir);
        if path.exists() {
            std::env::set_var(
                "CLAUDE_ADD_DIRS",
                format!(
                    "{}:{}",
                    std::env::var("CLAUDE_ADD_DIRS").unwrap_or_default(),
                    dir
                ),
            );
        }
    }

    state.is_git = rclaude_utils::git::is_git_repo(&cwd).await;
    state.is_non_interactive = cli.print;
    if state.is_git {
        state.git_branch = rclaude_utils::git::get_branch(&cwd).await.unwrap_or(None);
        state.git_default_branch = rclaude_utils::git::get_default_branch(&cwd)
            .await
            .unwrap_or(None);
    }

    // Handle --resume [id]
    if let Some(ref resume_arg) = cli.resume {
        let session_opt = if let Some(id) = resume_arg {
            // Resume by specific ID
            rclaude_services::session::load_session(id, &cwd)
                .await
                .ok()
                .flatten()
        } else {
            // Resume latest
            rclaude_services::session::load_latest_session(&cwd)
                .await
                .ok()
                .flatten()
        };
        if let Some(mut session) = session_opt {
            rclaude_services::conversation_recovery::fix_incomplete_tool_calls(
                &mut session.messages,
            );
            state.messages = session.messages;
            state.session_id = session.session_id;
            let title = rclaude_services::session::generate_session_title(&state.messages)
                .unwrap_or_else(|| "untitled".into());
            eprintln!(
                "{}",
                format!("Resumed: \"{title}\" ({} messages)", state.messages.len()).dimmed()
            );
        } else {
            eprintln!("{}", "No matching session found.".dimmed());
        }
    }

    // Handle --continue: restore latest session
    if cli.r#continue && cli.resume.is_none() {
        if let Ok(Some(mut session)) = rclaude_services::session::load_latest_session(&cwd).await {
            // Fix incomplete tool calls from crashed session
            rclaude_services::conversation_recovery::fix_incomplete_tool_calls(
                &mut session.messages,
            );
            state.messages = session.messages;
            state.session_id = session.session_id;
            // U06: Rich session resume info
            let title = rclaude_services::session::generate_session_title(&state.messages)
                .unwrap_or_else(|| "untitled".into());
            let msg_count = state.messages.len();
            eprintln!(
                "{}",
                format!("Resumed: \"{title}\" ({msg_count} messages)").dimmed()
            );
        }
    }

    // Acquire session lock (detect concurrent sessions)
    let _session_lock =
        rclaude_services::session::acquire_session_lock(&state.session_id, &cwd).await;
    if let Err(ref msg) = _session_lock {
        eprintln!("{}", format!("Warning: {msg}").yellow());
    }

    let state = Arc::new(RwLock::new(state));

    let mut session_ctx = SessionContext {
        hooks,
        auto_compact: AutoCompactState::new(),
        tool_stats: rclaude_services::tool_execution::ToolStats::default(),
        history: rclaude_services::history::InputHistory::load(),
        max_output_recovery_count: 0,
        has_attempted_reactive_compact: false,
        stream_retry_count: 0,
        allowed_tools: cli.allowed_tools.clone(),
        disallowed_tools: cli.disallowed_tools.clone(),
        max_turns: cli
            .max_turns
            .unwrap_or(if cli.print { usize::MAX } else { 30 }),
        max_budget_usd: cli.max_budget_usd,
        fallback_model: cli.fallback_model.clone(),
        system_prompt_override: cli.system_prompt.clone(),
        append_system_prompt: cli.append_system_prompt.clone(),
        output_format: cli.output_format.clone(),
        consecutive_denials: 0,
        content_replacement_state: Default::default(),
        query_engine: None,
    };

    // W03: Auto-connect MCP servers at startup
    let _mcp_manager = rclaude_mcp::manager::McpConnectionManager::from_config(&cwd)
        .await
        .ok();
    if let Some(ref mgr) = _mcp_manager {
        let count = mgr.connected_count();
        if count > 0 && cli.verbose {
            eprintln!("{}", format!("Connected to {count} MCP server(s)").dimmed());
        }
    }

    // Fire SessionStart hook
    let _ = session_ctx
        .hooks
        .run(
            HookEvent::SessionStart,
            &cwd,
            &HashMap::from([("SESSION_ID".into(), state.read().await.session_id.clone())]),
        )
        .await;

    // Graceful Ctrl+C: save session before exit
    let state_for_ctrlc = state.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let s = state_for_ctrlc.read().await;
        if !s.messages.is_empty() {
            let _ = rclaude_services::session::save_session(
                &s.session_id,
                &s.model,
                &s.messages,
                &s.cwd,
            )
            .await;
        }
        eprintln!("\nSession saved. Goodbye!");
        std::process::exit(0);
    });

    let prompt = cli.prompt.join(" ");
    if !prompt.is_empty() || cli.print {
        let input = if prompt.is_empty() {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
            if buf.is_empty() {
                eprintln!("{}", "Error: No prompt provided".red());
                std::process::exit(1);
            }
            buf
        } else {
            prompt
        };
        {
            let renderer = CliRenderer::new(
                &session_ctx.output_format,
                state.read().await.is_non_interactive,
            );
            let engine = session_ctx.get_or_create_engine();
            engine.submit_message(&input, &state, &renderer).await?;
        }

        // JSON output matching claude's format
        if cli.output_format == "json" {
            let s = state.read().await;
            let result_text = s
                .messages
                .iter()
                .rev()
                .find(|m| m.role == Role::Assistant)
                .map(|m| m.text_content())
                .unwrap_or_default();
            let model_usage: serde_json::Value = s
                .model_usage
                .iter()
                .map(|(model, u)| {
                    (
                        model.clone(),
                        serde_json::json!({
                            "inputTokens": u.input_tokens,
                            "outputTokens": u.output_tokens,
                            "cacheReadInputTokens": u.cache_read_tokens,
                            "cacheCreationInputTokens": u.cache_creation_tokens,
                            "costUSD": u.total_cost_usd,
                        }),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>()
                .into();
            let output = serde_json::json!({
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "result": result_text,
                "session_id": s.session_id,
                "num_turns": s.messages.iter().filter(|m| m.role == Role::Assistant).count(),
                "total_cost_usd": s.total_cost_usd,
                "usage": {
                    "input_tokens": s.model_usage.values().map(|u| u.input_tokens).sum::<u64>(),
                    "output_tokens": s.model_usage.values().map(|u| u.output_tokens).sum::<u64>(),
                    "cache_read_input_tokens": s.model_usage.values().map(|u| u.cache_read_tokens).sum::<u64>(),
                    "cache_creation_input_tokens": s.model_usage.values().map(|u| u.cache_creation_tokens).sum::<u64>(),
                },
                "modelUsage": model_usage,
                "stop_reason": "end_turn",
                "uuid": uuid::Uuid::new_v4().to_string(),
            });
            println!("{}", serde_json::to_string(&output).unwrap_or_default());
        } else if cli.output_format == "stream-json" {
            // W06: Output all messages as JSONL events
            let s = state.read().await;
            for msg in &s.messages {
                let event = serde_json::json!({
                    "type": match msg.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        Role::System => "system",
                    },
                    "message": {
                        "role": match msg.role {
                            Role::User => "user",
                            Role::Assistant => "assistant",
                            Role::System => "system",
                        },
                        "content": msg.content.iter().map(|b| match b {
                            ContentBlock::Text { text } => serde_json::json!({"type": "text", "text": text}),
                            ContentBlock::ToolUse { id, name, input } => serde_json::json!({"type": "tool_use", "id": id, "name": name, "input": input}),
                            ContentBlock::ToolResult { tool_use_id, content, is_error } => serde_json::json!({"type": "tool_result", "tool_use_id": tool_use_id, "content": content, "is_error": is_error}),
                            _ => serde_json::json!({"type": "other"}),
                        }).collect::<Vec<_>>(),
                    },
                });
                println!("{}", serde_json::to_string(&event).unwrap_or_default());
            }
        }

        // Save session after --print mode (so --continue can restore it)
        {
            let s = state.read().await;
            if !s.messages.is_empty() {
                let _ = rclaude_services::session::save_session(
                    &s.session_id,
                    &s.model,
                    &s.messages,
                    &s.cwd,
                )
                .await;
            }
        }
    } else if cli.tui {
        run_tui_mode(&state, &mut session_ctx).await?;
    } else {
        run_repl(&state, &mut session_ctx).await?;
    }

    // Cleanup old sessions
    let _ = rclaude_services::cleanup::run_cleanup(&cwd).await;

    // Fire SessionEnd hook
    let _ = session_ctx
        .hooks
        .run(
            HookEvent::SessionEnd,
            &cwd,
            &HashMap::from([("SESSION_ID".into(), state.read().await.session_id.clone())]),
        )
        .await;

    Ok(())
}

async fn run_repl(state: &SharedState, session_ctx: &mut SessionContext) -> anyhow::Result<()> {
    {
        let s = state.read().await;
        let resolved_model = rclaude_core::model::resolve_model(&s.config.model);
        // Crab logo for rclaude (Rust port)
        println!(
            " {}   {}",
            "\\/    \\/".red(),
            format!("rclaude v{}", env!("CARGO_PKG_VERSION")).bold()
        );
        println!(
            " {}  {}",
            "{  ◕◕  }".red(),
            format!("{} · API Usage Billing", resolved_model).dimmed()
        );
        println!(
            "  {}   {}",
            "( >< )".red(),
            s.cwd.display().to_string().dimmed()
        );
    }

    println!();

    loop {
        // X02: Use ❯ prompt matching claude
        print!("{} ", "❯".bold().green());
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input)? == 0 {
            break;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // V03: REPL shortcuts
        if input == "/clear" || input == "clear" {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");
            use std::io::Write;
            std::io::stdout().flush().ok();
            continue;
        }

        // Record in history
        session_ctx.history.push(input.to_string());

        if input.starts_with('/') {
            let handled = handle_command(input, state).await;
            match handled {
                CommandAction::Continue => continue,
                CommandAction::Exit => break,
            }
        }

        if let Err(e) = {
            let renderer = CliRenderer::new(
                &session_ctx.output_format,
                state.read().await.is_non_interactive,
            );
            let engine = session_ctx.get_or_create_engine();
            engine.submit_message(input, state, &renderer).await
        } {
            eprintln!("{} {}", "Error:".red(), e);
        }
    }

    // Auto-save session
    {
        let s = state.read().await;
        if !s.messages.is_empty() {
            // Generate title from first user message
            let _title = rclaude_services::session::generate_session_title(&s.messages);
            // Save JSON session
            match rclaude_services::session::save_session(
                &s.session_id,
                &s.model,
                &s.messages,
                &s.cwd,
            )
            .await
            {
                Ok(path) => eprintln!("{}", format!("Session saved: {}", path.display()).dimmed()),
                Err(e) => eprintln!("{} {}", "Failed to save session:".dimmed(), e),
            }
            // Also save JSONL transcript for recovery
            let _ = rclaude_services::session_storage::save_transcript_jsonl(
                &s.session_id,
                &s.messages,
                &s.cwd,
            )
            .await;
        }
        // Release session lock
        rclaude_services::session::release_session_lock(&s.cwd).await;
    }

    println!("{}", "Goodbye!".dimmed());
    Ok(())
}

enum CommandAction {
    Continue,
    Exit,
}

async fn handle_command(input: &str, state: &SharedState) -> CommandAction {
    if input == "/exit" || input == "/quit" {
        return CommandAction::Exit;
    }

    let trimmed = input.strip_prefix('/').unwrap_or(input);
    let (cmd_name, args) = match trimmed.split_once(char::is_whitespace) {
        Some((name, rest)) => (name, rest.trim()),
        None => (trimmed, ""),
    };

    let commands = rclaude_commands::get_all_commands();
    let cmd = commands.iter().find(|c| c.name() == cmd_name);

    match cmd {
        Some(command) => {
            let mut s = state.write().await;
            match command.execute(args, &mut s).await {
                Ok(rclaude_core::command::CommandResult::Ok(Some(text))) => {
                    println!("{text}");
                }
                Ok(rclaude_core::command::CommandResult::Ok(None)) => {}
                Ok(rclaude_core::command::CommandResult::Message(msg)) => {
                    drop(s);
                    // Note: we can't pass session_ctx here without restructuring,
                    // but commands that generate messages are rare
                    eprintln!("{}", format!("Command generated message: {msg}").dimmed());
                }
                Ok(rclaude_core::command::CommandResult::Exit) => {
                    return CommandAction::Exit;
                }
                Err(e) => {
                    eprintln!("{} {}", "Error:".red(), e);
                }
            }
            CommandAction::Continue
        }
        None => {
            println!(
                "{} Unknown command: /{cmd_name}. Type /help for available commands.",
                "Error:".red()
            );
            CommandAction::Continue
        }
    }
}

/// Run in full TUI mode with ratatui.
async fn run_tui_mode(state: &SharedState, session_ctx: &mut SessionContext) -> anyhow::Result<()> {
    use rclaude_tui::app::{ChatRole, TuiState};
    use rclaude_tui::runner::{self, TuiEvent};

    let (model, branch) = {
        let s = state.read().await;
        (s.model.clone(), s.git_branch.clone())
    };

    let mut tui_state = TuiState::new(&model, branch);
    tui_state.add_message(
        ChatRole::System,
        "Welcome to rclaude (TUI mode). Type /help for commands.",
    );

    let mut terminal = runner::init_terminal()?;

    loop {
        let event = runner::tick(&mut terminal, &mut tui_state)?;

        match event {
            Some(TuiEvent::Quit) => break,
            Some(TuiEvent::UserMessage(text)) => {
                tui_state.add_message(ChatRole::User, &text);
                tui_state.is_loading = true;
                tui_state.loading_status = Some("Sending to API...".into());

                terminal.draw(|f| rclaude_tui::ui::render(f, &tui_state))?;

                let turn_result = {
                    let renderer = CliRenderer::new(&session_ctx.output_format, true);
                    let engine = session_ctx.get_or_create_engine();
                    engine.submit_message(&text, state, &renderer).await
                };
                match turn_result {
                    Ok(_) => {
                        let s = state.read().await;
                        if let Some(msg) = s.messages.last() {
                            if msg.role == Role::Assistant {
                                tui_state.add_message(ChatRole::Assistant, msg.text_content());
                            }
                        }
                        tui_state.status_cost = s.total_cost_usd;
                    }
                    Err(e) => {
                        tui_state.add_message(ChatRole::System, format!("Error: {e}"));
                    }
                }
                tui_state.is_loading = false;
                tui_state.loading_status = None;
            }
            Some(TuiEvent::SlashCommand(cmd)) => {
                let action = handle_command(&cmd, state).await;
                match action {
                    CommandAction::Exit => break,
                    CommandAction::Continue => {}
                }
            }
            None => {}
        }
    }

    runner::restore_terminal();

    {
        let s = state.read().await;
        if !s.messages.is_empty() {
            if let Ok(path) = rclaude_services::session::save_session(
                &s.session_id,
                &s.model,
                &s.messages,
                &s.cwd,
            )
            .await
            {
                eprintln!("Session saved: {}", path.display());
            }
        }
    }

    Ok(())
}

/// V01: Extract key parameter from tool input for display.
fn format_tool_param(tool_name: &str, input: &serde_json::Value) -> String {
    let s = |field: &str| -> String {
        input
            .get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let truncate = |text: &str, max: usize| -> String {
        if text.len() > max {
            format!(" {}…", &text[..max])
        } else if text.is_empty() {
            String::new()
        } else {
            format!(" {text}")
        }
    };
    match tool_name {
        "Bash" => truncate(&s("command"), 80),
        "Read" | "Write" | "Edit" => truncate(&s("file_path"), 60),
        "Glob" => truncate(&s("pattern"), 40),
        "Grep" => {
            let pattern = s("pattern");
            let path = s("path");
            if path.is_empty() {
                truncate(&pattern, 40)
            } else {
                truncate(&format!("{pattern} in {path}"), 60)
            }
        }
        "Agent" => {
            let desc = s("description");
            let agent_type = s("subagent_type");
            if agent_type.is_empty() {
                truncate(&desc, 50)
            } else {
                truncate(&format!("{agent_type}: {desc}"), 50)
            }
        }
        "WebFetch" => truncate(&s("url"), 60),
        "WebSearch" => truncate(&s("query"), 50),
        _ => String::new(),
    }
}

/// V05: Extract result summary from tool output.
fn format_tool_result_summary(tool_name: &str, result_text: &str, is_error: bool) -> String {
    if is_error {
        return format!("{tool_name} failed");
    }
    let lines = result_text.lines().count();
    match tool_name {
        "Bash" => format!("Ran command ({lines} lines output)"),
        "Read" => format!("Read {lines} lines"),
        "Edit" => "Applied edit".to_string(),
        "Write" => {
            if let Some(pos) = result_text.find("bytes") {
                let start = result_text[..pos].rfind(' ').unwrap_or(0);
                format!("Wrote {}", result_text[start..pos + 5].trim())
            } else {
                "Wrote file".to_string()
            }
        }
        "Glob" => {
            let count = result_text.lines().filter(|l| !l.is_empty()).count();
            format!("Found {count} files")
        }
        "Grep" => {
            let count = result_text.lines().filter(|l| !l.is_empty()).count();
            format!("Found {count} matches")
        }
        "Agent" => "Agent completed".to_string(),
        _ => format!("{tool_name} completed"),
    }
}

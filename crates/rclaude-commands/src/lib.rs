pub mod branch;
pub mod clear;
pub mod color;
pub mod commit;
pub mod compact;
pub mod config;
pub mod context;
pub mod copy;
pub mod cost;
pub mod diff;
pub mod doctor;
pub mod effort;
pub mod env;
pub mod exit;
pub mod export;
pub mod fast;
pub mod feedback;
pub mod files;
pub mod help;
pub mod hooks;
pub mod keybindings;
pub mod login;
pub mod mcp_cmd;
pub mod memory;
pub mod model;
pub mod permissions;
pub mod plan;
pub mod resume_cmd;
pub mod review;
pub mod session;
pub mod share;
pub mod skills;
pub mod status;
pub mod tasks;
pub mod theme;
pub mod upgrade;
pub mod usage;
pub mod vim;
// Tier 1
pub mod add_dir;
pub mod init;
pub mod issue;
pub mod logout;
pub mod onboarding;
pub mod plugin;
pub mod pr_comments;
pub mod release_notes;
pub mod rename;
pub mod security_review;
pub mod stats;
pub mod tag;
// Tier 2
pub mod autofix_pr;
pub mod btw;
pub mod bughunter;
pub mod commit_push_pr;
pub mod output_style;
pub mod privacy_settings;
pub mod rewind;
pub mod sandbox_toggle;
pub mod summary;
// Achievable
pub mod advisor;
pub mod agents;
pub mod ctx_viz;
pub mod debug_tool_call;
pub mod desktop;
pub mod good_claude;
pub mod ide;
pub mod insights;
pub mod install_github_app;
pub mod install_slack_app;
pub mod mobile;
pub mod stickers;
pub mod terminal_setup;
pub mod version;

use rclaude_core::command::Command;

pub fn get_all_commands() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(help::HelpCommand),
        Box::new(config::ConfigCommand),
        Box::new(clear::ClearCommand),
        Box::new(cost::CostCommand),
        Box::new(model::ModelCommand),
        Box::new(session::SessionCommand),
        Box::new(compact::CompactCommand),
        Box::new(doctor::DoctorCommand),
        Box::new(login::LoginCommand),
        Box::new(logout::LogoutCommand),
        Box::new(commit::CommitCommand),
        Box::new(review::ReviewCommand),
        Box::new(diff::DiffCommand),
        Box::new(status::StatusCommand),
        Box::new(mcp_cmd::McpCommand),
        Box::new(upgrade::UpgradeCommand),
        Box::new(theme::ThemeCommand),
        Box::new(memory::MemoryCommand),
        Box::new(export::ExportCommand),
        Box::new(vim::VimCommand),
        Box::new(env::EnvCommand),
        Box::new(exit::ExitCommand),
        Box::new(fast::FastCommand),
        Box::new(files::FilesCommand),
        Box::new(copy::CopyCommand),
        Box::new(color::ColorCommand),
        Box::new(branch::BranchCommand),
        Box::new(hooks::HooksCommand),
        Box::new(plan::PlanCommand),
        Box::new(tasks::TasksCommand),
        Box::new(permissions::PermissionsCommand),
        Box::new(keybindings::KeybindingsCommand),
        Box::new(resume_cmd::ResumeCommand),
        Box::new(share::ShareCommand),
        Box::new(skills::SkillsCommand),
        Box::new(usage::UsageCommand),
        Box::new(feedback::FeedbackCommand),
        Box::new(context::ContextCommand),
        Box::new(effort::EffortCommand),
        Box::new(init::InitCommand),
        Box::new(add_dir::AddDirCommand),
        Box::new(rename::RenameCommand),
        Box::new(tag::TagCommand),
        Box::new(stats::StatsCommand),
        Box::new(pr_comments::PrCommentsCommand),
        Box::new(issue::IssueCommand),
        Box::new(release_notes::ReleaseNotesCommand),
        Box::new(security_review::SecurityReviewCommand),
        Box::new(onboarding::OnboardingCommand),
        Box::new(plugin::PluginCommand),
        // Tier 2
        Box::new(output_style::OutputStyleCommand),
        Box::new(privacy_settings::PrivacySettingsCommand),
        Box::new(sandbox_toggle::SandboxToggleCommand),
        Box::new(rewind::RewindCommand),
        Box::new(summary::SummaryCommand),
        Box::new(commit_push_pr::CommitPushPrCommand),
        Box::new(autofix_pr::AutofixPrCommand),
        Box::new(bughunter::BughunterCommand),
        Box::new(btw::BtwCommand),
        Box::new(version::VersionCommand),
        Box::new(stickers::StickersCommand),
        Box::new(good_claude::GoodClaudeCommand),
        Box::new(advisor::AdvisorCommand),
        Box::new(insights::InsightsCommand),
        Box::new(agents::AgentsCommand),
        Box::new(ide::IdeCommand),
        Box::new(desktop::DesktopCommand),
        Box::new(mobile::MobileCommand),
        Box::new(terminal_setup::TerminalSetupCommand),
        Box::new(install_github_app::InstallGithubAppCommand),
        Box::new(install_slack_app::InstallSlackAppCommand),
        Box::new(ctx_viz::CtxVizCommand),
        Box::new(debug_tool_call::DebugToolCallCommand),
    ]
}

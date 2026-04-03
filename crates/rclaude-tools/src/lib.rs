pub mod agent;
pub mod agent_fork;
pub mod agent_loader;
pub mod agent_types;
pub mod ask_user;
pub mod bash;
pub mod bash_ast;
pub mod bash_classifier;
pub mod bash_path;
pub mod bash_sed;
pub mod brief;
pub mod config_tool;
pub mod cron;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob;
pub mod grep;
pub mod list_mcp_resources;
pub mod lsp;
pub mod mcp_auth;
pub mod mcp_tool;
pub mod monitor;
pub mod notebook;
pub mod plan_mode;
pub mod powershell;
pub mod push_notification;
pub mod read_mcp_resource;
pub mod remote_trigger;
pub mod send_user_file;
pub mod skill;
pub mod sleep;
pub mod subscribe_pr;
pub mod suggest_background_pr;
pub mod synthetic_output;
pub mod task_tools;
pub mod team;
pub mod testing_permission;
pub mod todo;
pub mod tool_search;
pub mod verify_plan;
pub mod web_fetch;
pub mod web_search;
pub mod workflow;
pub mod worktree;

use rclaude_core::tool::Tool;

/// Get all built-in tools.
pub fn get_all_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(bash::BashTool),
        Box::new(file_read::FileReadTool),
        Box::new(file_write::FileWriteTool),
        Box::new(file_edit::FileEditTool),
        Box::new(glob::GlobTool),
        Box::new(grep::GrepTool),
        Box::new(web_fetch::WebFetchTool),
        Box::new(web_search::WebSearchTool),
        Box::new(ask_user::AskUserQuestionTool),
        Box::new(notebook::NotebookEditTool),
        Box::new(agent::AgentTool),
        Box::new(task_tools::TaskCreateTool),
        Box::new(task_tools::TaskListTool),
        Box::new(task_tools::TaskGetTool),
        Box::new(task_tools::TaskUpdateTool),
        Box::new(task_tools::TaskOutputTool),
        Box::new(task_tools::TaskStopTool),
        Box::new(team::TeamCreateTool),
        Box::new(team::TeamDeleteTool),
        Box::new(team::SendMessageTool),
        Box::new(plan_mode::EnterPlanModeTool),
        Box::new(plan_mode::ExitPlanModeTool),
        Box::new(cron::CronCreateTool),
        Box::new(cron::CronDeleteTool),
        Box::new(cron::CronListTool),
        Box::new(skill::SkillTool),
        Box::new(worktree::EnterWorktreeTool),
        Box::new(worktree::ExitWorktreeTool),
        Box::new(lsp::LSPTool),
        Box::new(config_tool::ConfigTool),
        Box::new(sleep::SleepTool),
        Box::new(list_mcp_resources::ListMcpResourcesTool),
        Box::new(read_mcp_resource::ReadMcpResourceTool),
        Box::new(mcp_auth::McpAuthTool),
        Box::new(tool_search::ToolSearchTool),
        Box::new(brief::BriefTool),
        Box::new(remote_trigger::RemoteTriggerTool),
        Box::new(monitor::MonitorTool),
        Box::new(workflow::WorkflowTool),
        Box::new(verify_plan::VerifyPlanExecutionTool),
        Box::new(synthetic_output::SyntheticOutputTool),
        Box::new(powershell::PowerShellTool),
        // Feature-gated
        Box::new(suggest_background_pr::SuggestBackgroundPRTool),
        Box::new(push_notification::PushNotificationTool),
        Box::new(subscribe_pr::SubscribePRTool),
        Box::new(send_user_file::SendUserFileTool),
        Box::new(testing_permission::TestingPermissionTool),
        Box::new(todo::TodoWriteTool),
    ]
}

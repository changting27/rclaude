use async_trait::async_trait;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct PrivacySettingsCommand;

const PRIVACY_URL: &str = "https://claude.ai/settings/data-privacy-controls";

#[async_trait]
impl Command for PrivacySettingsCommand {
    fn name(&self) -> &str {
        "privacy-settings"
    }

    fn description(&self) -> &str {
        "Review and manage your privacy settings"
    }

    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        // Open browser to privacy settings page
        match open::that(PRIVACY_URL) {
            Ok(_) => Ok(CommandResult::Ok(Some(format!(
                "Opened privacy settings: {PRIVACY_URL}"
            )))),
            Err(_) => Ok(CommandResult::Ok(Some(format!(
                "Review and manage your privacy settings at {PRIVACY_URL}"
            )))),
        }
    }
}

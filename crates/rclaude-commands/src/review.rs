use async_trait::async_trait;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ReviewCommand;

/// Build review prompt.
fn build_review_prompt(args: &str) -> String {
    format!(
        r#"You are an expert code reviewer. Follow these steps:

1. If no PR number is provided in the args, run `gh pr list` to show open PRs
2. If a PR number is provided, run `gh pr view <number>` to get PR details
3. Run `gh pr diff <number>` to get the diff
4. Analyze the changes and provide a thorough code review that includes:
   - Overview of what the PR does
   - Analysis of code quality and style
   - Specific suggestions for improvements
   - Any potential issues or risks

Keep your review concise but thorough. Focus on:
- Code correctness
- Following project conventions
- Performance implications
- Test coverage
- Security considerations

Format your review with clear sections and bullet points.

PR number: {args}"#
    )
}

#[async_trait]
impl Command for ReviewCommand {
    fn name(&self) -> &str {
        "review"
    }

    fn description(&self) -> &str {
        "Review a pull request"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some(
                "Not in a git repository.".to_string(),
            )));
        }

        Ok(CommandResult::Message(build_review_prompt(args.trim())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_prompt() {
        let prompt = build_review_prompt("42");
        assert!(prompt.contains("gh pr diff"));
        assert!(prompt.contains("42"));
        assert!(prompt.contains("Security considerations"));
    }
}

use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct CommitCommand;

/// Build the commit prompt.
/// Includes git context, safety protocol, and commit message guidelines.
fn build_commit_prompt(args: &str) -> String {
    let user_message = if args.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n\nThe user wants the commit message to be about: {}\n",
            args.trim()
        )
    };

    format!(
        r#"## Context

- Current git status: !`git status`
- Current git diff (staged and unstaged changes): !`git diff HEAD`
- Current branch: !`git branch --show-current`
- Recent commits: !`git log --oneline -10`

## Git Safety Protocol

- NEVER update the git config
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it
- CRITICAL: ALWAYS create NEW commits. NEVER use git commit --amend, unless the user explicitly requests it
- Do not commit files that likely contain secrets (.env, credentials.json, etc). Warn the user if they specifically request to commit those files
- If there are no changes to commit (i.e., no untracked files and no modifications), do not create an empty commit
- Never use git commands with the -i flag (like git rebase -i or git add -i) since they require interactive input which is not supported
{user_message}
## Your task

Based on the above changes, create a single git commit:

1. First run `git status` and `git diff HEAD` to see all changes
2. Analyze all changes and draft a commit message:
   - Look at the recent commits above to follow this repository's commit message style
   - Summarize the nature of the changes (new feature, enhancement, bug fix, refactoring, test, docs, etc.)
   - Ensure the message accurately reflects the changes and their purpose
   - Draft a concise (1-2 sentences) commit message that focuses on the "why" rather than the "what"
3. Stage relevant files and create the commit using HEREDOC syntax:
```
git commit -m "$(cat <<'EOF'
Commit message here.
EOF
)"
```

Stage and create the commit using a single message. Do not use any other tools or do anything else."#
    )
}

#[async_trait]
impl Command for CommitCommand {
    fn name(&self) -> &str {
        "commit"
    }

    fn description(&self) -> &str {
        "Create a git commit with AI-generated message"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some(
                "Not in a git repository.".red().to_string(),
            )));
        }

        Ok(CommandResult::Message(build_commit_prompt(args)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_prompt_contains_safety() {
        let prompt = build_commit_prompt("");
        assert!(prompt.contains("NEVER use git commit --amend"));
        assert!(prompt.contains("NEVER skip hooks"));
        assert!(prompt.contains("git status"));
        assert!(prompt.contains("git diff HEAD"));
        assert!(prompt.contains("HEREDOC"));
    }

    #[test]
    fn test_commit_prompt_with_args() {
        let prompt = build_commit_prompt("fix login bug");
        assert!(prompt.contains("fix login bug"));
    }
}

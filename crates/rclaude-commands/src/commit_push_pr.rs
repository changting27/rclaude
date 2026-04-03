use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct CommitPushPrCommand;

/// Build the commit-push-pr prompt.
fn build_prompt(args: &str, default_branch: &str) -> String {
    let user_instructions = if args.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n\n## Additional instructions from user\n\n{}",
            args.trim()
        )
    };

    format!(
        r#"## Context

- `git status`: !`git status`
- `git diff HEAD`: !`git diff HEAD`
- `git branch --show-current`: !`git branch --show-current`
- `git diff {default_branch}...HEAD`: !`git diff {default_branch}...HEAD`
- `gh pr view --json number 2>/dev/null || true`: !`gh pr view --json number 2>/dev/null || true`

## Git Safety Protocol

- NEVER update the git config
- NEVER run destructive/irreversible git commands (like push --force, hard reset, etc) unless the user explicitly requests them
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it
- NEVER run force push to main/master, warn the user if they request it
- Do not commit files that likely contain secrets (.env, credentials.json, etc)
- Never use git commands with the -i flag (like git rebase -i or git add -i) since they require interactive input which is not supported

## Your task

Analyze all changes that will be included in the pull request, making sure to look at all relevant commits (NOT just the latest commit, but ALL commits from the git diff {default_branch}...HEAD output above).

Based on the above changes:
1. Create a new branch if on {default_branch} (e.g., `username/feature-name`)
2. Create a single commit with an appropriate message using heredoc syntax:
```
git commit -m "$(cat <<'EOF'
Commit message here.
EOF
)"
```
3. Push the branch to origin
4. If a PR already exists for this branch (check the gh pr view output above), update the PR title and body using `gh pr edit`. Otherwise, create a pull request using `gh pr create` with heredoc syntax for the body.
   - IMPORTANT: Keep PR titles short (under 70 characters). Use the body for details.
```
gh pr create --title "Short, descriptive title" --body "$(cat <<'EOF'
## Summary
<1-3 bullet points>

## Test plan
[Bulleted markdown checklist of TODOs for testing the pull request...]
EOF
)"
```

You have the capability to call multiple tools in a single response. You MUST do all of the above in a single message.

Return the PR URL when you're done, so the user can see it.{user_instructions}"#
    )
}

#[async_trait]
impl Command for CommitPushPrCommand {
    fn name(&self) -> &str {
        "commit-push-pr"
    }

    fn description(&self) -> &str {
        "Commit, push, and create a pull request"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some("Not in a git repository.".into())));
        }

        let default_branch = state.git_default_branch.as_deref().unwrap_or("main");

        Ok(CommandResult::Message(build_prompt(args, default_branch)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_contains_safety() {
        let prompt = build_prompt("", "main");
        assert!(prompt.contains("NEVER run force push"));
        assert!(prompt.contains("gh pr create"));
        assert!(prompt.contains("heredoc"));
        assert!(prompt.contains("git diff main...HEAD"));
    }

    #[test]
    fn test_prompt_with_custom_branch() {
        let prompt = build_prompt("", "develop");
        assert!(prompt.contains("git diff develop...HEAD"));
    }

    #[test]
    fn test_prompt_with_args() {
        let prompt = build_prompt("fix the login page", "main");
        assert!(prompt.contains("fix the login page"));
    }
}

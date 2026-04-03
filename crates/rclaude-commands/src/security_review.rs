use async_trait::async_trait;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct SecurityReviewCommand;

/// Build security review prompt.
fn build_prompt() -> String {
    r#"You are a senior security engineer conducting a focused security review of the changes on this branch.

GIT STATUS:
```
!`git status`
```

FILES MODIFIED:
```
!`git diff --name-only origin/HEAD...`
```

DIFF CONTENT:
```
!`git diff origin/HEAD...`
```

Review the complete diff above. This contains all code changes in the PR.

OBJECTIVE:
Perform a security-focused code review to identify HIGH-CONFIDENCE security vulnerabilities that could have real exploitation potential. Focus ONLY on security implications newly added by this PR.

CRITICAL INSTRUCTIONS:
1. MINIMIZE FALSE POSITIVES: Only flag issues where you're >80% confident of actual exploitability
2. AVOID NOISE: Skip theoretical issues, style concerns, or low-impact findings
3. FOCUS ON IMPACT: Prioritize vulnerabilities that could lead to unauthorized access, data breaches, or system compromise

SECURITY CATEGORIES TO EXAMINE:
- Input Validation: SQL injection, command injection, path traversal, template injection
- Auth & Authorization: Authentication bypass, privilege escalation, session flaws
- Crypto & Secrets: Hardcoded keys, weak algorithms, improper key storage
- Injection & Code Execution: RCE via deserialization, eval injection, XSS
- Data Exposure: Sensitive data logging, PII handling, API data leakage

REQUIRED OUTPUT FORMAT:
Output findings in markdown with file, line number, severity, category, description, exploit scenario, and fix recommendation.

SEVERITY GUIDELINES:
- HIGH: Directly exploitable (RCE, data breach, auth bypass)
- MEDIUM: Requires specific conditions but significant impact
- LOW: Defense-in-depth issues

CONFIDENCE SCORING:
- 0.9-1.0: Certain exploit path
- 0.8-0.9: Clear vulnerability pattern
- Below 0.8: Don't report

Focus on HIGH and MEDIUM findings only. Better to miss theoretical issues than flood with false positives."#.to_string()
}

#[async_trait]
impl Command for SecurityReviewCommand {
    fn name(&self) -> &str {
        "security-review"
    }

    fn description(&self) -> &str {
        "Security-focused code review of current branch changes"
    }

    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some("Not in a git repository.".into())));
        }
        Ok(CommandResult::Message(build_prompt()))
    }
}

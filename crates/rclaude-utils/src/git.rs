use rclaude_core::error::Result;
use tokio::process::Command;

/// Check if current directory is inside a git repository.
pub async fn is_git_repo(cwd: &std::path::Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(cwd)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current git branch name.
pub async fn get_branch(cwd: &std::path::Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    } else {
        Ok(None)
    }
}

/// Get the default branch (main/master).
pub async fn get_default_branch(cwd: &std::path::Path) -> Result<Option<String>> {
    // Try `main` first
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "refs/heads/main"])
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() {
        return Ok(Some("main".to_string()));
    }

    // Fall back to `master`
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "refs/heads/master"])
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() {
        return Ok(Some("master".to_string()));
    }

    // Try remote default
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() {
        let full = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // "origin/main" -> "main"
        Ok(full.strip_prefix("origin/").map(|s| s.to_string()))
    } else {
        Ok(None)
    }
}

/// Get the current git user name.
pub async fn get_user_name(cwd: &std::path::Path) -> Option<String> {
    Command::new("git")
        .args(["config", "user.name"])
        .current_dir(cwd)
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

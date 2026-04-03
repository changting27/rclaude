//! Enhanced git operations for repository management.

use std::path::Path;

/// Git repository state.
#[derive(Debug, Clone)]
pub struct GitState {
    pub branch: Option<String>,
    pub default_branch: Option<String>,
    pub status: String,
    pub recent_log: String,
    pub user_name: Option<String>,
    pub remote_url: Option<String>,
    pub has_uncommitted: bool,
}

/// Get comprehensive git state for the current directory.
pub async fn get_git_state(cwd: &Path) -> Option<GitState> {
    if !crate::git::is_git_repo(cwd).await {
        return None;
    }

    let (branch, default_branch, status, log, user_name, remote_url) = tokio::join!(
        crate::git::get_branch(cwd),
        crate::git::get_default_branch(cwd),
        run_git(cwd, &["status", "--short"]),
        run_git(cwd, &["log", "--oneline", "-n", "5"]),
        run_git(cwd, &["config", "user.name"]),
        run_git(cwd, &["remote", "get-url", "origin"]),
    );

    let status_text = status.unwrap_or_default();
    let has_uncommitted = !status_text.trim().is_empty();

    Some(GitState {
        branch: branch.ok().flatten(),
        default_branch: default_branch.ok().flatten(),
        status: status_text,
        recent_log: log.unwrap_or_default(),
        user_name: user_name
            .map(|s| if s.is_empty() { None } else { Some(s) })
            .unwrap_or(None),
        remote_url: remote_url
            .map(|s| if s.is_empty() { None } else { Some(s) })
            .unwrap_or(None),
        has_uncommitted,
    })
}

/// Get the diff of staged changes.
pub async fn get_staged_diff(cwd: &Path) -> String {
    run_git(cwd, &["diff", "--cached"])
        .await
        .unwrap_or_default()
}

/// Get the diff of unstaged changes.
pub async fn get_unstaged_diff(cwd: &Path) -> String {
    run_git(cwd, &["diff"]).await.unwrap_or_default()
}

/// Get diff between current branch and default branch.
pub async fn get_branch_diff(cwd: &Path, default_branch: &str) -> String {
    run_git(cwd, &["diff", &format!("{default_branch}...HEAD")])
        .await
        .unwrap_or_default()
}

/// Find the git root directory.
pub async fn find_git_root(cwd: &Path) -> Option<String> {
    let output = run_git(cwd, &["rev-parse", "--show-toplevel"]).await.ok()?;
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

/// Normalize a git remote URL for comparison.
pub fn normalize_remote_url(url: &str) -> Option<String> {
    let url = url.trim();
    // SSH format: git@github.com:user/repo.git
    if let Some(rest) = url.strip_prefix("git@") {
        let normalized = rest.replace(':', "/").trim_end_matches(".git").to_string();
        return Some(normalized);
    }
    // HTTPS format: https://github.com/user/repo.git
    if url.starts_with("https://") || url.starts_with("http://") {
        let normalized = url
            .split("://")
            .nth(1)?
            .trim_end_matches(".git")
            .to_string();
        return Some(normalized);
    }
    None
}

async fn run_git(cwd: &Path, args: &[&str]) -> Result<String, ()> {
    let output = tokio::process::Command::new("git")
        .args(["--no-optional-locks"])
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|_| ())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_ssh_url() {
        assert_eq!(
            normalize_remote_url("git@github.com:user/repo.git"),
            Some("github.com/user/repo".into())
        );
    }

    #[test]
    fn test_normalize_https_url() {
        assert_eq!(
            normalize_remote_url("https://github.com/user/repo.git"),
            Some("github.com/user/repo".into())
        );
    }

    #[test]
    fn test_normalize_no_git_suffix() {
        assert_eq!(
            normalize_remote_url("https://github.com/user/repo"),
            Some("github.com/user/repo".into())
        );
    }
}

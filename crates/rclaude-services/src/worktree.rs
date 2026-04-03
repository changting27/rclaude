//! Git worktree management for parallel branch operations.

use std::path::{Path, PathBuf};

/// Worktree session info.
#[derive(Debug, Clone)]
pub struct WorktreeSession {
    pub slug: String,
    pub branch: String,
    pub path: PathBuf,
    pub original_cwd: PathBuf,
}

/// Validate a worktree slug (alphanumeric + hyphens only).
pub fn validate_slug(slug: &str) -> Result<(), String> {
    if slug.is_empty() {
        return Err("Slug cannot be empty".into());
    }
    if slug.len() > 50 {
        return Err("Slug too long (max 50 chars)".into());
    }
    if !slug.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err("Slug must be alphanumeric with hyphens only".into());
    }
    Ok(())
}

/// Generate branch name for a worktree.
pub fn worktree_branch_name(slug: &str) -> String {
    format!("claude/{slug}")
}

/// Create a git worktree for an agent session.
pub async fn create_worktree(cwd: &Path, slug: &str) -> Result<WorktreeSession, String> {
    validate_slug(slug)?;
    let branch = worktree_branch_name(slug);
    let worktree_path = cwd.join(format!("../.claude-worktrees/{slug}"));

    // Create branch from current HEAD
    let output = tokio::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &branch,
            worktree_path.to_str().unwrap_or(""),
        ])
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| format!("Failed to create worktree: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {stderr}"));
    }

    Ok(WorktreeSession {
        slug: slug.to_string(),
        branch,
        path: worktree_path,
        original_cwd: cwd.to_path_buf(),
    })
}

/// Clean up a worktree.
pub async fn cleanup_worktree(cwd: &Path, slug: &str) -> Result<(), String> {
    let branch = worktree_branch_name(slug);
    let worktree_path = cwd.join(format!("../.claude-worktrees/{slug}"));

    // Remove worktree
    let _ = tokio::process::Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            worktree_path.to_str().unwrap_or(""),
        ])
        .current_dir(cwd)
        .output()
        .await;

    // Delete branch
    let _ = tokio::process::Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(cwd)
        .output()
        .await;

    Ok(())
}

/// List existing worktrees.
pub async fn list_worktrees(cwd: &Path) -> Vec<String> {
    let output = tokio::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(cwd)
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.starts_with("worktree "))
            .map(|l| l.strip_prefix("worktree ").unwrap_or(l).to_string())
            .collect(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_slug() {
        assert!(validate_slug("my-feature").is_ok());
        assert!(validate_slug("fix123").is_ok());
        assert!(validate_slug("").is_err());
        assert!(validate_slug("has spaces").is_err());
        assert!(validate_slug(&"a".repeat(51)).is_err());
    }

    #[test]
    fn test_branch_name() {
        assert_eq!(worktree_branch_name("my-feature"), "claude/my-feature");
    }
}

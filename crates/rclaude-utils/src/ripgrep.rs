//! Ripgrep integration for fast file content search.
//! Provides fast file search using rg binary.

use std::path::Path;

/// Search result from ripgrep.
#[derive(Debug, Clone)]
pub struct RgMatch {
    pub file: String,
    pub line_number: u32,
    pub content: String,
}

/// Check if ripgrep is available.
pub async fn is_rg_available() -> bool {
    tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .is_ok_and(|o| o.status.success())
}

/// Search files using ripgrep.
pub async fn search(
    cwd: &Path,
    pattern: &str,
    glob: Option<&str>,
    max_count: Option<u32>,
    case_insensitive: bool,
) -> Result<Vec<RgMatch>, String> {
    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--line-number")
        .arg("--no-heading")
        .arg("--color=never");

    if case_insensitive {
        cmd.arg("-i");
    }
    if let Some(g) = glob {
        cmd.arg("--glob").arg(g);
    }
    if let Some(m) = max_count {
        cmd.arg("--max-count").arg(m.to_string());
    }

    cmd.arg(pattern).current_dir(cwd);

    let output = cmd.output().await.map_err(|e| format!("rg failed: {e}"))?;

    // rg returns exit code 1 for no matches (not an error)
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let matches: Vec<RgMatch> = stdout
        .lines()
        .filter_map(|line| {
            // Format: file:line:content
            let mut parts = line.splitn(3, ':');
            let file = parts.next()?.to_string();
            let line_num: u32 = parts.next()?.parse().ok()?;
            let content = parts.next()?.to_string();
            Some(RgMatch {
                file,
                line_number: line_num,
                content,
            })
        })
        .collect();

    Ok(matches)
}

/// Count matches per file using ripgrep.
pub async fn count_matches(cwd: &Path, pattern: &str) -> Result<Vec<(String, u32)>, String> {
    let output = tokio::process::Command::new("rg")
        .args(["--count", "--color=never", pattern])
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| format!("rg failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| {
            let (file, count) = line.rsplit_once(':')?;
            Some((file.to_string(), count.parse().ok()?))
        })
        .collect())
}

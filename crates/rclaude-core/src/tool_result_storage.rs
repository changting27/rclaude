//! Tool result storage: persist large results to disk, use preview in API.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Max chars per tool result before persisting to disk.
const PERSISTENCE_THRESHOLD: usize = 50_000;
/// Preview size when result is persisted.
const PREVIEW_SIZE: usize = 2_000;

/// State tracking for tool result replacements (prompt cache stability).
#[derive(Debug, Default)]
pub struct ContentReplacementState {
    /// Tool use IDs we've seen (frozen — won't change fate).
    pub seen_ids: HashSet<String>,
    /// Tool use IDs that were replaced with previews.
    pub replacements: HashMap<String, PathBuf>,
}

/// Persist a large tool result to disk, return the preview text.
pub async fn persist_tool_result(
    tool_use_id: &str,
    _tool_name: &str,
    content: &str,
    session_dir: &Path,
) -> Result<(PathBuf, String), String> {
    let results_dir = session_dir.join("tool-results");
    tokio::fs::create_dir_all(&results_dir)
        .await
        .map_err(|e| e.to_string())?;

    let file_path = results_dir.join(format!("{tool_use_id}.txt"));
    tokio::fs::write(&file_path, content)
        .await
        .map_err(|e| e.to_string())?;

    // Build preview: first PREVIEW_SIZE chars + metadata
    let preview = if content.len() > PREVIEW_SIZE {
        let cut = content[..PREVIEW_SIZE].rfind('\n').unwrap_or(PREVIEW_SIZE);
        format!(
            "{}...\n\n[Full result ({} chars) saved to disk. Key information has been preserved above.]",
            &content[..cut],
            content.len()
        )
    } else {
        content.to_string()
    };

    Ok((file_path, preview))
}

/// Process tool results: persist large ones, return (possibly modified) results.
pub async fn process_results(
    results: &mut [crate::streaming_executor::OrderedToolResult],
    state: &mut ContentReplacementState,
    session_dir: &Path,
) {
    for r in results.iter_mut() {
        // Already seen — don't change (prompt cache stability)
        if state.seen_ids.contains(&r.tool_use_id) {
            if let Some(path) = state.replacements.get(&r.tool_use_id) {
                // Re-apply the same preview
                if let Ok(preview) = tokio::fs::read_to_string(path).await {
                    let cut = preview.len().min(PREVIEW_SIZE);
                    r.result_text =
                        format!("{}...\n\n[Full result saved to disk.]", &preview[..cut]);
                }
            }
            continue;
        }

        state.seen_ids.insert(r.tool_use_id.clone());

        // Persist if over threshold
        if r.result_text.len() > PERSISTENCE_THRESHOLD {
            if let Ok((path, preview)) =
                persist_tool_result(&r.tool_use_id, &r.tool_name, &r.result_text, session_dir).await
            {
                state.replacements.insert(r.tool_use_id.clone(), path);
                r.result_text = preview;
            }
        }
    }
}

/// Get the session directory for tool result storage.
pub fn get_session_results_dir(session_id: &str, cwd: &Path) -> PathBuf {
    let hash: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    crate::config::Config::projects_dir()
        .join(&hash[..hash.len().min(80)])
        .join("sessions")
        .join(session_id)
}

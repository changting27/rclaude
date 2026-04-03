use rclaude_core::error::Result;
use std::path::Path;

/// Read a file's content as string.
pub async fn read_text_file(path: &Path) -> Result<String> {
    Ok(tokio::fs::read_to_string(path).await?)
}

/// Write text content to a file, creating parent directories if needed.
pub async fn write_text_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    Ok(tokio::fs::write(path, content).await?)
}

/// Check if a path exists.
pub async fn path_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Get file size in bytes.
pub async fn file_size(path: &Path) -> Result<u64> {
    let meta = tokio::fs::metadata(path).await?;
    Ok(meta.len())
}

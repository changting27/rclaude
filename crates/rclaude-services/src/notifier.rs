//! Notification service for desktop alerts and sounds.

/// Send a desktop notification.
pub async fn send_notification(title: &str, body: &str) -> Result<(), String> {
    // Try notify-send (Linux)
    let result = tokio::process::Command::new("notify-send")
        .args([title, body])
        .output()
        .await;

    if let Ok(output) = result {
        if output.status.success() {
            return Ok(());
        }
    }

    // Try osascript (macOS)
    let result = tokio::process::Command::new("osascript")
        .args([
            "-e",
            &format!("display notification \"{body}\" with title \"{title}\""),
        ])
        .output()
        .await;

    if let Ok(output) = result {
        if output.status.success() {
            return Ok(());
        }
    }

    Err("No notification system available".into())
}

/// Send a notification when a long-running task completes.
pub async fn notify_task_complete(task_name: &str, success: bool) {
    let status = if success { "completed" } else { "failed" };
    let _ = send_notification("rclaude", &format!("Task '{task_name}' {status}")).await;
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Task type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    LocalBash,
    LocalAgent,
    RemoteAgent,
    InProcessTeammate,
}

/// Task lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

impl TaskStatus {
    /// Whether the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed
        )
    }
}

/// ID prefix for task IDs.
fn task_id_prefix(task_type: TaskType) -> char {
    match task_type {
        TaskType::LocalBash => 'b',
        TaskType::LocalAgent => 'a',
        TaskType::RemoteAgent => 'r',
        TaskType::InProcessTeammate => 't',
    }
}

/// Generate a unique task ID.
pub fn generate_task_id(task_type: TaskType) -> String {
    let prefix = task_id_prefix(task_type);
    let random: String = (0..8)
        .map(|_| {
            let idx = rand::random::<u8>() % 36;
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("{prefix}{random}")
}

/// A running or completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub id: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub description: String,
    pub start_time: u64,
    pub end_time: Option<u64>,
    pub output_file: PathBuf,
    pub output_offset: usize,
    /// Whether the user has been notified of completion.
    pub notified: bool,
}

/// Handle returned when spawning a task.
#[derive(Debug)]
pub struct TaskHandle {
    pub task_id: String,
    pub abort_sender: Option<tokio::sync::watch::Sender<bool>>,
}

impl TaskHandle {
    /// Signal the task to abort.
    pub fn abort(&self) {
        if let Some(ref sender) = self.abort_sender {
            let _ = sender.send(true);
        }
    }
}

/// Spawn a background shell task.
pub async fn spawn_shell_task(
    command: &str,
    description: &str,
    cwd: &std::path::Path,
    output_dir: &std::path::Path,
) -> crate::error::Result<(TaskState, TaskHandle)> {
    let task_id = generate_task_id(TaskType::LocalBash);
    let output_file = output_dir.join(format!("{task_id}.output"));

    // Ensure output directory exists
    tokio::fs::create_dir_all(output_dir).await?;

    let (abort_tx, _abort_rx) = tokio::sync::watch::channel(false);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let state = TaskState {
        id: task_id.clone(),
        task_type: TaskType::LocalBash,
        status: TaskStatus::Running,
        description: description.to_string(),
        start_time: now,
        end_time: None,
        output_file: output_file.clone(),
        output_offset: 0,
        notified: false,
    };

    let handle = TaskHandle {
        task_id: task_id.clone(),
        abort_sender: Some(abort_tx),
    };

    // Spawn background process
    let cmd = command.to_string();
    let cwd = cwd.to_path_buf();
    let out_file = output_file.clone();

    tokio::spawn(async move {
        use tokio::process::Command;
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&cwd)
            .output()
            .await;

        let content = match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                format!("{stdout}{stderr}")
            }
            Err(e) => format!("Error: {e}"),
        };

        tokio::fs::write(&out_file, content).await.ok();
    });

    Ok((state, handle))
}

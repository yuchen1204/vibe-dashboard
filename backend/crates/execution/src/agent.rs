use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use shared::{AppError, AppResult};

/// A running coding agent subprocess.
pub struct AgentProcess {
    child: Child,
    pub job_id: String,
}

/// Output emitted by the agent process.
#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub job_id: String,
    pub text: String,
}

/// Spawn a Claude Code agent subprocess to execute a task.
///
/// Returns a handle to the process and a receiver for stdout/stderr output.
pub async fn spawn_claude_code(
    job_id: &str,
    worktree_path: &str,
    prompt: &str,
) -> AppResult<(AgentProcess, mpsc::UnboundedReceiver<AgentOutput>)> {
    let (tx, rx) = mpsc::unbounded_channel();
    let tx_clone = tx.clone();
    let job_id_owned = job_id.to_string();

    // Build the claude code command
    // claude is expected to be in PATH; we pass the prompt via stdin
    let mut child = Command::new("claude")
        .args(["-p", prompt])
        .current_dir(worktree_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::Internal(format!("failed to spawn claude: {e}")))?;

    let job_id_clone = job_id_owned.clone();

    // Spawn a task to read stdout and send to channel
    if let Some(stdout) = child.stdout.take() {
        let jid = job_id_owned.clone();
        let tx_stdout = tx_clone.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx_stdout
                    .send(AgentOutput {
                        job_id: jid.clone(),
                        text: format!("{line}\n"),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    // Spawn a task to read stderr and send to channel
    if let Some(stderr) = child.stderr.take() {
        let tx_stderr = tx.clone();
        let jid = job_id_owned.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx_stderr
                    .send(AgentOutput {
                        job_id: jid.clone(),
                        text: format!("{line}\n"),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    Ok((
        AgentProcess {
            child,
            job_id: job_id_clone,
        },
        rx,
    ))
}

impl AgentProcess {
    /// Wait for the agent process to finish and return the exit code.
    pub async fn wait(&mut self) -> AppResult<i32> {
        let status = self
            .child
            .wait()
            .await
            .map_err(|e| AppError::Internal(format!("agent process error: {e}")))?;
        Ok(status.code().unwrap_or(-1))
    }

    /// Kill the agent process.
    pub async fn kill(&mut self) -> AppResult<()> {
        self.child
            .kill()
            .await
            .map_err(|e| AppError::Internal(format!("failed to kill agent: {e}")))?;
        Ok(())
    }
}

/// Spawn a generic command for testing purposes.
/// Used in tests to simulate an agent without requiring claude CLI.
pub async fn spawn_test_agent(
    job_id: &str,
    worktree_path: &str,
) -> AppResult<(AgentProcess, mpsc::UnboundedReceiver<AgentOutput>)> {
    let (tx, rx) = mpsc::unbounded_channel();
    let tx_out = tx.clone();
    let jid = job_id.to_string();

    // On Windows, use cmd /c echo to simulate output
    let mut child = Command::new("cmd")
        .args(["/c", "echo", "test agent output"])
        .current_dir(worktree_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| AppError::Internal(format!("failed to spawn test agent: {e}")))?;

    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if tx_out
                    .send(AgentOutput {
                        job_id: jid.clone(),
                        text: format!("{line}\n"),
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    Ok((
        AgentProcess {
            child,
            job_id: job_id.to_string(),
        },
        rx,
    ))
}

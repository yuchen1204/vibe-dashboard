use std::path::Path;
use tokio::process::Command;

use shared::{AppError, AppResult};

/// Run `git worktree add` in the given repo path.
/// Returns the path of the created worktree.
pub async fn create_worktree(
    repo_path: &Path,
    branch: &str,
    worktree_path: &Path,
) -> AppResult<()> {
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap_or(""),
            branch,
        ])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("failed to spawn git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!(
            "git worktree add failed: {stderr}"
        )));
    }

    Ok(())
}

/// Run `git worktree remove` and clean up.
pub async fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> AppResult<()> {
    // Try `git worktree remove` first
    let output = Command::new("git")
        .args(["worktree", "remove", worktree_path.to_str().unwrap_or("")])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("failed to spawn git: {e}")))?;

    if !output.status.success() {
        // Fallback: delete the directory manually
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("git worktree remove failed, falling back to fs delete: {stderr}");
        tokio::fs::remove_dir_all(worktree_path)
            .await
            .map_err(|e| AppError::Internal(format!("failed to remove worktree dir: {e}")))?;

        // Also prune the worktree metadata
        let _ = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(repo_path)
            .output()
            .await;
    }

    Ok(())
}

/// List existing worktree paths for a repo.
pub async fn list_worktree_paths(repo_path: &Path) -> AppResult<Vec<String>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("failed to spawn git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!(
            "git worktree list failed: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut paths = Vec::new();
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            paths.push(path.to_string());
        }
    }

    Ok(paths)
}

/// Create a new branch from the current HEAD (or default branch) in the repo.
pub async fn create_branch(repo_path: &Path, branch: &str) -> AppResult<()> {
    let output = Command::new("git")
        .args(["checkout", "-b", branch])
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("failed to spawn git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // If branch already exists locally, try to just use it
        if stderr.contains("already exists") {
            return Ok(());
        }
        return Err(AppError::Internal(format!(
            "git checkout -b failed: {stderr}"
        )));
    }

    Ok(())
}

/// Check if a path is a valid git repository.
pub async fn is_git_repo(path: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(path)
        .output()
        .await;
    matches!(output, Ok(o) if o.status.success())
}

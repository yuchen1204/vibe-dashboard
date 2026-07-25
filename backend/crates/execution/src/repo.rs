use chrono::Utc;
use shared::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::{ExecutionJob, JobStatus, Worktree, WorktreeStatus};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

// ---------- Worktree ----------

pub async fn list_worktrees(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Worktree>> {
    let rows = sqlx::query_as!(
        Worktree,
        r#"SELECT id, workspace_id, target_id, branch, path, status, created_at, updated_at
           FROM worktrees
           WHERE workspace_id = ?
           ORDER BY created_at DESC"#,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_worktree(pool: &SqlitePool, id: &str) -> AppResult<Worktree> {
    sqlx::query_as!(
        Worktree,
        r#"SELECT id, workspace_id, target_id, branch, path, status, created_at, updated_at
           FROM worktrees WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("worktree {id} not found")))
}

pub async fn create_worktree(
    pool: &SqlitePool,
    workspace_id: &str,
    branch: &str,
    path: &str,
    target_id: Option<&str>,
) -> AppResult<Worktree> {
    if branch.trim().is_empty() {
        return Err(AppError::BadRequest("branch must not be empty".into()));
    }
    if path.trim().is_empty() {
        return Err(AppError::BadRequest("path must not be empty".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = WorktreeStatus::default().as_str();

    sqlx::query!(
        r#"INSERT INTO worktrees (id, workspace_id, target_id, branch, path, status, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        id,
        workspace_id,
        target_id,
        branch,
        path,
        status,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(Worktree {
        id,
        workspace_id: workspace_id.to_string(),
        target_id: target_id.map(|s| s.to_string()),
        branch: branch.to_string(),
        path: path.to_string(),
        status: status.to_string(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_worktree_status(
    pool: &SqlitePool,
    id: &str,
    status: WorktreeStatus,
) -> AppResult<Worktree> {
    let existing = get_worktree(pool, id).await?;
    let now = now_rfc3339();
    let status_str = status.as_str();

    sqlx::query!(
        r#"UPDATE worktrees SET status = ?, updated_at = ? WHERE id = ?"#,
        status_str,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(Worktree {
        status: status_str.to_string(),
        updated_at: now,
        ..existing
    })
}

pub async fn delete_worktree(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM worktrees WHERE id = ?", id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("worktree {id} not found")));
    }
    Ok(())
}

// ---------- ExecutionJob ----------

pub async fn list_jobs_by_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<ExecutionJob>> {
    let rows = sqlx::query_as!(
        ExecutionJob,
        r#"SELECT ej.id, ej.todo_id, ej.worktree_id, ej.status, ej.agent_type,
                  ej.prompt, ej.output, ej.started_at, ej.finished_at,
                  ej.created_at, ej.updated_at
           FROM execution_jobs ej
           JOIN todos t ON ej.todo_id = t.id
           JOIN targets tg ON t.target_id = tg.id
           WHERE tg.workspace_id = ?
           ORDER BY ej.created_at DESC"#,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_jobs_by_todo(pool: &SqlitePool, todo_id: &str) -> AppResult<Vec<ExecutionJob>> {
    let rows = sqlx::query_as!(
        ExecutionJob,
        r#"SELECT id, todo_id, worktree_id, status, agent_type, prompt, output,
                  started_at, finished_at, created_at, updated_at
           FROM execution_jobs
           WHERE todo_id = ?
           ORDER BY created_at DESC"#,
        todo_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_job(pool: &SqlitePool, id: &str) -> AppResult<ExecutionJob> {
    sqlx::query_as!(
        ExecutionJob,
        r#"SELECT id, todo_id, worktree_id, status, agent_type, prompt, output,
                  started_at, finished_at, created_at, updated_at
           FROM execution_jobs WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("job {id} not found")))
}

pub async fn create_job(
    pool: &SqlitePool,
    todo_id: &str,
    prompt: &str,
    agent_type: &str,
) -> AppResult<ExecutionJob> {
    if prompt.trim().is_empty() {
        return Err(AppError::BadRequest("prompt must not be empty".into()));
    }
    // Verify todo exists
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM todos WHERE id = ?")
        .bind(todo_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("todo {todo_id} not found")));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = JobStatus::default().as_str();

    sqlx::query!(
        r#"INSERT INTO execution_jobs (id, todo_id, status, agent_type, prompt, output, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, '', ?, ?)"#,
        id,
        todo_id,
        status,
        agent_type,
        prompt,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(ExecutionJob {
        id,
        todo_id: todo_id.to_string(),
        worktree_id: None,
        status: status.to_string(),
        agent_type: agent_type.to_string(),
        prompt: prompt.to_string(),
        output: String::new(),
        started_at: None,
        finished_at: None,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_job_status(
    pool: &SqlitePool,
    id: &str,
    status: JobStatus,
    worktree_id: Option<&str>,
) -> AppResult<ExecutionJob> {
    let existing = get_job(pool, id).await?;
    let now = now_rfc3339();
    let status_str = status.as_str();

    let (started_at, finished_at): (Option<String>, Option<String>) = match status {
        JobStatus::Running => (Some(now.clone()), None),
        JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled => {
            (existing.started_at.clone(), Some(now.clone()))
        }
        _ => (existing.started_at.clone(), None),
    };

    sqlx::query!(
        r#"UPDATE execution_jobs
           SET status = ?, worktree_id = COALESCE(?, worktree_id),
               started_at = COALESCE(?, started_at),
               finished_at = COALESCE(?, finished_at),
               updated_at = ?
           WHERE id = ?"#,
        status_str,
        worktree_id,
        started_at,
        finished_at,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(ExecutionJob {
        status: status_str.to_string(),
        worktree_id: worktree_id.map(|s| s.to_string()).or(existing.worktree_id),
        started_at,
        finished_at,
        updated_at: now,
        ..existing
    })
}

pub async fn append_job_output(pool: &SqlitePool, id: &str, text: &str) -> AppResult<()> {
    // Use raw SQL to append to output
    let existing = get_job(pool, id).await?;
    let new_output = format!("{}{}", existing.output, text);
    let now = now_rfc3339();

    sqlx::query!(
        r#"UPDATE execution_jobs SET output = ?, updated_at = ? WHERE id = ?"#,
        new_output,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn cancel_job(pool: &SqlitePool, id: &str) -> AppResult<ExecutionJob> {
    let job = get_job(pool, id).await?;
    if job.status != "running" && job.status != "pending" {
        return Err(AppError::BadRequest(
            "can only cancel pending or running jobs".into(),
        ));
    }
    update_job_status(pool, id, JobStatus::Cancelled, None).await
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::state::AppState;
use crate::ws::message::ServerMsg;
use execution::repo;
use execution::{ExecuteTodo, JobStatus};

/// Currently executing jobs (in-memory tracking for cancellation).
/// Maps job_id -> channel to signal cancellation.
pub type ActiveJobs = Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>>;

// ---------- Worktrees ----------

pub async fn list_worktrees(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<execution::Worktree>>> {
    Ok(Json(repo::list_worktrees(&state.db, &wid).await?))
}

pub async fn create_worktree(
    State(state): State<AppState>,
    Path(wid): Path<String>,
    Json(input): Json<execution::CreateWorktree>,
) -> AppResult<Json<execution::Worktree>> {
    // We need the workspace path to create the git worktree
    // First verify the workspace exists and get its path
    let ws = tasks::repo::get_workspace(&state.db, &wid).await?;
    let ws_path = std::path::Path::new(&ws.workspace.path);

    // Determine the worktree path: <workspace_path>/../<branch>-worktree
    let wt_path = ws_path
        .parent()
        .unwrap_or(ws_path)
        .join(format!("{}-worktree", input.branch));

    let wt_path_str = wt_path.to_string_lossy().to_string();

    // Create the git worktree
    execution::worktree::create_worktree(ws_path, &input.branch, &wt_path).await?;

    // Record in DB
    let wt = repo::create_worktree(
        &state.db,
        &wid,
        &input.branch,
        &wt_path_str,
        input.target_id.as_deref(),
    )
    .await?;

    Ok(Json(wt))
}

pub async fn delete_worktree(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let wt = repo::get_worktree(&state.db, &id).await?;
    let ws = tasks::repo::get_workspace(&state.db, &wt.workspace_id).await?;
    let ws_path = std::path::Path::new(&ws.workspace.path);
    let wt_path = std::path::Path::new(&wt.path);

    // Remove the git worktree
    let _ = execution::worktree::remove_worktree(ws_path, wt_path).await;

    // Delete from DB
    repo::delete_worktree(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Jobs ----------

pub async fn list_jobs(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<execution::ExecutionJob>>> {
    Ok(Json(repo::list_jobs_by_workspace(&state.db, &wid).await?))
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<execution::ExecutionJob>> {
    Ok(Json(repo::get_job(&state.db, &id).await?))
}

pub async fn execute_todo(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<ExecuteTodo>,
) -> AppResult<Json<execution::ExecutionJob>> {
    // Get the todo
    let todo = tasks::repo::get_todo(&state.db, &tid).await?;

    // Get the target to find workspace
    let target = tasks::repo::get_target(&state.db, &todo.target_id).await?;

    // Get the workspace path
    let ws = tasks::repo::get_workspace(&state.db, &target.workspace_id).await?;
    let ws_path = std::path::Path::new(&ws.workspace.path);

    // Create the job
    let agent_type = input.agent_type.unwrap_or_else(|| "claude-code".to_string());
    let prompt = format!("Execute the following task:\n\nTitle: {}\nDescription: {}\n\nPlease implement this change.", todo.title, todo.description);
    let job = repo::create_job(&state.db, &tid, &prompt, &agent_type).await?;

    // Determine worktree path
    let branch = format!("todo-{}", &todo.id[..8]);
    // Create a branch and worktree
    // Note: This is simplified — in production we'd handle existing branches
    let wt_path = ws_path
        .parent()
        .unwrap_or(ws_path)
        .join(format!("{}-worktree", branch));

    // First check if repo is a git repo
    if !execution::worktree::is_git_repo(ws_path).await {
        // Not a git repo — just update status directly
        let job = repo::update_job_status(&state.db, &job.id, JobStatus::Running, None).await?;
        let hub = state.hub.clone();
        hub.broadcast(ServerMsg::job_status(
            job.id.clone(),
            tid.clone(),
            "running".to_string(),
        ));

        // Simulate execution (just mark as success since no git repo)
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let job = repo::update_job_status(&state.db, &job.id, JobStatus::Success, None).await?;
        hub.broadcast(ServerMsg::job_status(
            job.id.clone(),
            tid.clone(),
            "success".to_string(),
        ));

        // Update todo status to done
        let _ = tasks::repo::update_todo(
            &state.db,
            &tid,
            tasks::UpdateTodo {
                status: Some(tasks::TodoStatus::Done),
                ..Default::default()
            },
        )
        .await;

        return Ok(Json(job));
    }

    // Create worktree branch
    let _ = execution::worktree::create_branch(ws_path, &branch).await;

    // Create worktree
    let _ = execution::worktree::create_worktree(ws_path, &branch, &wt_path).await;

    let wt_path_str = wt_path.to_string_lossy().to_string();

    // Record worktree in DB
    let _ = repo::create_worktree(&state.db, &target.workspace_id, &branch, &wt_path_str, Some(&target.id)).await;

    // Update job to running
    let job = repo::update_job_status(
        &state.db,
        &job.id,
        JobStatus::Running,
        Some(&job.id),
    ).await?;

    let hub = state.hub.clone();
    let db = state.db.clone();
    let todo_id = tid.clone();
    let job_id = job.id.clone();

    // Broadcast running status
    hub.broadcast(ServerMsg::job_status(
        job_id.clone(),
        todo_id.clone(),
        "running".to_string(),
    ));

    // Spawn the agent process
    tokio::spawn(async move {
        match execution::agent::spawn_claude_code(&job_id, &wt_path_str, &prompt).await {
            Ok((mut agent, mut rx)) => {
                // Stream output
                while let Some(output) = rx.recv().await {
                    let _ = repo::append_job_output(&db, &job_id, &output.text).await;
                    hub.broadcast(ServerMsg::job_output(
                        job_id.clone(),
                        output.text,
                    ));
                }

                // Wait for exit
                match agent.wait().await {
                    Ok(0) => {
                        let _ = repo::update_job_status(&db, &job_id, JobStatus::Success, None).await;
                        let _ = tasks::repo::update_todo(
                            &db,
                            &todo_id,
                            tasks::UpdateTodo {
                                status: Some(tasks::TodoStatus::Done),
                                ..Default::default()
                            },
                        ).await;
                        hub.broadcast(ServerMsg::job_status(
                            job_id.clone(),
                            todo_id.clone(),
                            "success".to_string(),
                        ));
                    }
                    Ok(_) => {
                        let _ = repo::update_job_status(&db, &job_id, JobStatus::Failed, None).await;
                        let _ = tasks::repo::update_todo(
                            &db,
                            &todo_id,
                            tasks::UpdateTodo {
                                status: Some(tasks::TodoStatus::Blocked),
                                ..Default::default()
                            },
                        ).await;
                        hub.broadcast(ServerMsg::job_status(
                            job_id.clone(),
                            todo_id.clone(),
                            "failed".to_string(),
                        ));
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "agent process error");
                        let _ = repo::update_job_status(&db, &job_id, JobStatus::Failed, None).await;
                        hub.broadcast(ServerMsg::job_status(
                            job_id.clone(),
                            todo_id.clone(),
                            "failed".to_string(),
                        ));
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to spawn agent");
                let _ = repo::update_job_status(&db, &job_id, JobStatus::Failed, None).await;
                hub.broadcast(ServerMsg::job_status(
                    job_id.clone(),
                    todo_id.clone(),
                    "failed".to_string(),
                ));
            }
        }
    });

    Ok(Json(job))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<execution::ExecutionJob>> {
    let job = repo::cancel_job(&state.db, &id).await?;
    let hub = state.hub.clone();
    hub.broadcast(ServerMsg::job_status(
        job.id.clone(),
        job.todo_id.clone(),
        "cancelled".to_string(),
    ));
    Ok(Json(job))
}
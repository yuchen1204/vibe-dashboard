use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppResult;
use crate::state::AppState;
use crate::ws::message::ServerMsg;
use execution::executor::ExecContext;
use execution::repo;
use execution::{ExecuteTodo, JobStatus};

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
    let ws = tasks::repo::get_workspace(&state.db, &wid).await?;
    let ws_path = std::path::Path::new(&ws.workspace.path);

    let wt_path = ws_path
        .parent()
        .unwrap_or(ws_path)
        .join(format!("{}-worktree", input.branch));

    let wt_path_str = wt_path.to_string_lossy().to_string();

    execution::worktree::create_worktree(ws_path, &input.branch, &wt_path).await?;

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

    let _ = execution::worktree::remove_worktree(ws_path, wt_path).await;
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
    let todo = tasks::repo::get_todo(&state.db, &tid).await?;
    let target = tasks::repo::get_target(&state.db, &todo.target_id).await?;
    let ws = tasks::repo::get_workspace(&state.db, &target.workspace_id).await?;

    let ws_path = std::path::Path::new(&ws.workspace.path);
    let agent_type = input.agent_type.unwrap_or_else(|| "claude-code".to_string());
    let prompt = format!(
        "Execute the following task:\n\nTitle: {}\nDescription: {}\n\nPlease implement this change.",
        todo.title, todo.description
    );

    // Create job record
    let job = repo::create_job(&state.db, &tid, &prompt, &agent_type).await?;
    let job_id = job.id.clone();

    // Determine worktree path
    let branch = format!("todo-{}", &todo.id[..8]);
    let wt_path = ws_path
        .parent()
        .unwrap_or(ws_path)
        .join(format!("{}-worktree", branch));

    // Check if git repo and prepare worktree
    let is_git_repo = execution::worktree::is_git_repo(ws_path).await;
    let wt_path_str = if is_git_repo {
        let _ = execution::worktree::create_branch(ws_path, &branch).await;
        let _ = execution::worktree::create_worktree(ws_path, &branch, &wt_path).await;
        let _ = repo::create_worktree(
            &state.db,
            &target.workspace_id,
            &branch,
            &wt_path.to_string_lossy(),
            Some(&target.id),
        )
        .await;
        wt_path.to_string_lossy().to_string()
    } else {
        ws_path.to_string_lossy().to_string()
    };

    // Update job to running
    let _ = repo::update_job_status(&state.db, &job_id, JobStatus::Running, None).await?;
    state.hub.broadcast(ServerMsg::job_status(
        job_id.clone(),
        tid.clone(),
        "running".to_string(),
    ));

    // Build exec context
    let ctx = ExecContext::new(job_id.clone(), wt_path_str, prompt);

    // Spawn via executor manager
    let hub = state.hub.clone();
    let db = state.db.clone();
    let todo_id = tid.clone();
    let executor = state.executor.clone();

    tokio::spawn(async move {
        match executor.spawn(&agent_type, ctx).await {
            Ok(mut rx) => {
                use execution::executor::ExecutorEvent;
                loop {
                    match rx.recv().await {
                        Some(ExecutorEvent::Log { text }) => {
                            let _ = repo::append_job_output(&db, &job_id, &text).await;
                            hub.broadcast(ServerMsg::job_output(job_id.clone(), text));
                        }
                        Some(ExecutorEvent::Status { status }) => {
                            let status_str = status.as_str().to_string();
                            let _ = repo::update_job_status(
                                &db,
                                &job_id,
                                status,
                                None,
                            )
                            .await;
                            hub.broadcast(ServerMsg::job_status(
                                job_id.clone(),
                                todo_id.clone(),
                                status_str,
                            ));
                            if matches!(
                                status,
                                JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled
                            ) {
                                if status == JobStatus::Success {
                                    let _ = tasks::repo::update_todo(
                                        &db,
                                        &todo_id,
                                        tasks::UpdateTodo {
                                            status: Some(tasks::TodoStatus::Done),
                                            ..Default::default()
                                        },
                                    )
                                    .await;
                                } else if status == JobStatus::Failed {
                                    let _ = tasks::repo::update_todo(
                                        &db,
                                        &todo_id,
                                        tasks::UpdateTodo {
                                            status: Some(tasks::TodoStatus::Blocked),
                                            ..Default::default()
                                        },
                                    )
                                    .await;
                                }
                                executor.remove(&job_id);
                                break;
                            }
                        }
                        Some(ExecutorEvent::Heartbeat) => {
                            // No-op, keep waiting
                        }
                        None => {
                            // Channel closed, process likely exited
                            break;
                        }
                    }
                }

                // If we exited without a terminal status, check exit code
                let current = repo::get_job(&db, &job_id).await;
                if let Ok(j) = current {
                    if j.status == "running" || j.status == "pending" {
                        // Channel closed but no status update — mark as failed
                        let _ = repo::update_job_status(&db, &job_id, JobStatus::Failed, None).await;
                        hub.broadcast(ServerMsg::job_status(
                            job_id.clone(),
                            todo_id.clone(),
                            "failed".to_string(),
                        ));
                        let _ = tasks::repo::update_todo(
                            &db,
                            &todo_id,
                            tasks::UpdateTodo {
                                status: Some(tasks::TodoStatus::Blocked),
                                ..Default::default()
                            },
                        )
                        .await;
                        executor.remove(&job_id);
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to spawn executor");
                let _ = repo::update_job_status(&db, &job_id, JobStatus::Failed, None).await;
                hub.broadcast(ServerMsg::job_status(
                    job_id.clone(),
                    todo_id.clone(),
                    "failed".to_string(),
                ));
                executor.remove(&job_id);
            }
        }
    });

    Ok(Json(job))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<execution::ExecutionJob>> {
    // Cancel via manager first (kill process)
    let _ = state.executor.cancel(&id).await;
    // Then update DB
    let job = repo::cancel_job(&state.db, &id).await?;
    state.hub.broadcast(ServerMsg::job_status(
        job.id.clone(),
        job.todo_id.clone(),
        "cancelled".to_string(),
    ));
    Ok(Json(job))
}
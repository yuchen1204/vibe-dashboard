use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::error::AppResult;
use crate::state::AppState;
use crate::ws::message::ServerMsg;
use execution::dispatch::JobNotifier;
use execution::ExecuteTodo;

/// HubNotifier - 将 job 事件广播到所有 WS 连接
struct HubNotifier(std::sync::Arc<crate::ws::Hub>);

#[async_trait::async_trait]
impl JobNotifier for HubNotifier {
    async fn on_job_output(&self, job_id: &str, text: &str) {
        self.0.broadcast(ServerMsg::job_output(job_id.to_string(), text.to_string()));
    }

    async fn on_job_status(&self, job_id: &str, todo_id: &str, status: &str) {
        self.0.broadcast(ServerMsg::job_status(
            job_id.to_string(),
            todo_id.to_string(),
            status.to_string(),
        ));
    }
}

// ---------- Worktrees ----------

pub async fn list_worktrees(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<execution::Worktree>>> {
    Ok(Json(execution::repo::list_worktrees(&state.db, &wid).await?))
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

    let wt = execution::repo::create_worktree(
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
    let wt = execution::repo::get_worktree(&state.db, &id).await?;
    let ws = tasks::repo::get_workspace(&state.db, &wt.workspace_id).await?;
    let ws_path = std::path::Path::new(&ws.workspace.path);
    let wt_path = std::path::Path::new(&wt.path);

    let _ = execution::worktree::remove_worktree(ws_path, wt_path).await;
    execution::repo::delete_worktree(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Jobs ----------

pub async fn list_jobs(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<execution::ExecutionJob>>> {
    Ok(Json(execution::repo::list_jobs_by_workspace(&state.db, &wid).await?))
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<execution::ExecutionJob>> {
    Ok(Json(execution::repo::get_job(&state.db, &id).await?))
}

pub async fn execute_todo(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<ExecuteTodo>,
) -> AppResult<Json<execution::ExecutionJob>> {
    let agent_type = input.agent_type.unwrap_or_else(|| "claude-code".to_string());
    let notifier = std::sync::Arc::new(HubNotifier(state.hub.clone()));

    let job = execution::dispatch::execute_todo(
        &state.db,
        state.executor.clone(),
        notifier as std::sync::Arc<dyn JobNotifier>,
        &tid,
        &agent_type,
        input.prompt.as_deref(),
    )
    .await?;

    Ok(Json(job))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<execution::ExecutionJob>> {
    // Cancel via manager first (kill process)
    let _ = state.executor.cancel(&id).await;
    // Then update DB
    let job = execution::repo::cancel_job(&state.db, &id).await?;
    state.hub.broadcast(ServerMsg::job_status(
        job.id.clone(),
        job.todo_id.clone(),
        "cancelled".to_string(),
    ));
    Ok(Json(job))
}
use axum::{
    extract::{Path, State},
    Json,
};

use crate::error::AppResult;
use crate::state::AppState;
use crate::ws::message::ServerMsg;
use execution::dispatch::JobNotifier;

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

// ---------- Feedback ----------

/// GET /api/reviews/:rid/feedback - 获取某次审查的 feedback 列表
pub async fn list_feedback(
    State(state): State<AppState>,
    Path(rid): Path<String>,
) -> AppResult<Json<Vec<orchestrator::feedback::ReviewFeedback>>> {
    Ok(Json(
        orchestrator::feedback::get_feedback_by_review(&state.db, &rid).await?,
    ))
}

/// POST /api/feedback/:finding_id/accept - 接受某个 finding，创建新 todo
pub async fn accept_finding(
    State(state): State<AppState>,
    Path(finding_id): Path<String>,
    Json(input): Json<serde_json::Value>,
) -> AppResult<Json<orchestrator::feedback::ReviewFeedback>> {
    let target_id = input.get("target_id").and_then(|v| v.as_str());
    Ok(Json(
        orchestrator::feedback::accept_finding(&state.db, &finding_id, target_id).await?,
    ))
}

/// POST /api/feedback/:finding_id/ignore - 忽略某个 finding
pub async fn ignore_finding(
    State(state): State<AppState>,
    Path(finding_id): Path<String>,
) -> AppResult<Json<orchestrator::feedback::ReviewFeedback>> {
    Ok(Json(
        orchestrator::feedback::ignore_finding(&state.db, &finding_id).await?,
    ))
}

// ---------- Iterations ----------

/// GET /api/todos/:tid/iterations - 列出某个 todo 的所有迭代
pub async fn list_iterations(
    State(state): State<AppState>,
    Path(tid): Path<String>,
) -> AppResult<Json<Vec<orchestrator::feedback::ReviewIteration>>> {
    Ok(Json(
        orchestrator::feedback::list_iterations(&state.db, &tid).await?,
    ))
}

/// POST /api/todos/:tid/auto-fix - 触发自动修复循环
pub async fn trigger_auto_fix(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<serde_json::Value>,
) -> AppResult<Json<orchestrator::feedback::ReviewIteration>> {
    let config = {
        let cfg = state.llm_config.read().unwrap();
        cfg.clone()
    };
    let prompt = input.get("prompt").and_then(|v| v.as_str());

    let pool = state.db.clone();
    let hub = state.hub.clone();
    let executor = state.executor.clone();
    let todo_id = tid.clone();
    let custom_prompt = prompt.map(|s| s.to_string());

    // 创建迭代记录
    let iteration = orchestrator::feedback::get_or_create_iteration(&pool, &todo_id).await?;

    // 后台运行 auto-fix 循环
    tokio::spawn(async move {
        let notifier = std::sync::Arc::new(HubNotifier(hub.clone()));

        if let Err(e) = orchestrator::feedback::run_auto_fix(
            &pool,
            &config,
            executor,
            notifier,
            &todo_id,
            custom_prompt.as_deref(),
            None,
        )
        .await
        {
            tracing::error!(todo_id = %todo_id, error = %e, "auto-fix loop failed");
        }
    });

    Ok(Json(iteration))
}

/// POST /api/todos/:tid/auto-fix-sync - 同步触发自动修复（等待完成）
pub async fn trigger_auto_fix_sync(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<serde_json::Value>,
) -> AppResult<Json<Vec<orchestrator::feedback::ReviewIteration>>> {
    let config = {
        let cfg = state.llm_config.read().unwrap();
        cfg.clone()
    };
    let prompt = input.get("prompt").and_then(|v| v.as_str());

    let notifier = std::sync::Arc::new(HubNotifier(state.hub.clone()));

    let results = orchestrator::feedback::run_auto_fix(
        &state.db,
        &config,
        state.executor.clone(),
        notifier,
        &tid,
        prompt,
        None,
    )
    .await
    .map_err(|e| shared::AppError::Internal(e))?;

    Ok(Json(results))
}
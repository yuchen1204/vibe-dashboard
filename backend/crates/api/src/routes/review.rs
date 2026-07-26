use axum::{
    extract::{Path, State},
    Json,
};

use crate::error::AppResult;
use crate::state::AppState;
use orchestrator::review::CreateFinding;
use crate::ws::message::ServerMsg;

/// GET /api/reviews/todo/:todo_id - 列出某个 todo 的所有审查
pub async fn list_reviews_by_todo(
    State(state): State<AppState>,
    Path(todo_id): Path<String>,
) -> AppResult<Json<Vec<orchestrator::review::Review>>> {
    Ok(Json(
        orchestrator::review::list_reviews_by_todo(&state.db, &todo_id).await?,
    ))
}

/// GET /api/reviews/job/:job_id - 列出某个 job 的所有审查
pub async fn list_reviews_by_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> AppResult<Json<Vec<orchestrator::review::Review>>> {
    Ok(Json(
        orchestrator::review::list_reviews_by_job(&state.db, &job_id).await?,
    ))
}

/// GET /api/reviews/:id - 获取审查详情（含 findings）
pub async fn get_review(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<orchestrator::review::ReviewDetail>> {
    Ok(Json(
        orchestrator::review::get_review_with_findings(&state.db, &id).await?,
    ))
}

/// POST /api/reviews - 创建审查
pub async fn create_review(
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> AppResult<Json<orchestrator::review::Review>> {
    let job_id = input
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| shared::AppError::BadRequest("missing job_id".into()))?;
    let todo_id = input
        .get("todo_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| shared::AppError::BadRequest("missing todo_id".into()))?;

    Ok(Json(
        orchestrator::review::create_review(&state.db, job_id, todo_id).await?,
    ))
}

/// POST /api/reviews/:id/findings - 添加审查发现
pub async fn add_finding(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CreateFinding>,
) -> AppResult<Json<orchestrator::review::ReviewFinding>> {
    Ok(Json(
        orchestrator::review::add_finding(&state.db, &id, input).await?,
    ))
}

/// PUT /api/reviews/:id/summary - 更新审查总结
pub async fn update_review_summary(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<serde_json::Value>,
) -> AppResult<Json<orchestrator::review::Review>> {
    let summary = input
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let score = input.get("score").and_then(|v| v.as_i64());
    let total_findings = input
        .get("total_findings")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    Ok(Json(
        orchestrator::review::update_review_summary(
            &state.db,
            &id,
            summary,
            score,
            total_findings,
        )
        .await?,
    ))
}

/// POST /api/reviews/trigger - 触发 LLM 代码审查
pub async fn trigger_review(
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> AppResult<Json<orchestrator::review::Review>> {
    let job_id = input
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| shared::AppError::BadRequest("missing job_id".into()))?;
    let todo_id = input
        .get("todo_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| shared::AppError::BadRequest("missing todo_id".into()))?;

    let config = {
        let cfg = state.llm_config.read().unwrap();
        cfg.clone()
    };

    let pool = state.db.clone();
    let hub = state.hub.clone();
    let jid = job_id.to_string();
    let tid = todo_id.to_string();

    // 先在 DB 创建 review 记录并返回给调用方
    let review = orchestrator::review::create_review(&pool, &jid, &tid).await?;
    let review_id = review.id.clone();

    // 后台运行审查 agent，通过 WS 推送实时事件
    tokio::spawn(async move {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

        // 转发事件到 WS
        let hub_clone = hub.clone();
        let rid = review_id.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    orchestrator::review_agent::ReviewEvent::Started { review_id, job_id, todo_id } => {
                        hub_clone.broadcast(ServerMsg::review_started(review_id, job_id, todo_id));
                    }
                    orchestrator::review_agent::ReviewEvent::Finding { review_id, finding } => {
                        hub_clone.broadcast(ServerMsg::review_finding(
                            review_id,
                            crate::ws::message::ReviewFindingPayload {
                                id: finding.id,
                                severity: finding.severity,
                                file_path: finding.file_path,
                                line_number: finding.line_number,
                                category: finding.category,
                                title: finding.title,
                                description: finding.description,
                                suggestion: finding.suggestion,
                            },
                        ));
                    }
                    orchestrator::review_agent::ReviewEvent::Completed {
                        review_id,
                        summary,
                        score,
                        finding_count,
                    } => {
                        hub_clone.broadcast(ServerMsg::review_completed(
                            review_id, summary, score, finding_count,
                        ));
                    }
                    orchestrator::review_agent::ReviewEvent::Error {
                        review_id,
                        message,
                    } => {
                        hub_clone.broadcast(ServerMsg::review_error(review_id, message));
                    }
                }
            }
        });

        if let Err(e) = orchestrator::review_agent::run_review(
            &pool,
            &config,
            &jid,
            &tid,
            Some(&review_id),  // 传入已创建的 review_id
            Some(event_tx),
        )
        .await
        {
            tracing::error!(review_id = %rid, error = %e, "review agent failed");
            hub.broadcast(ServerMsg::review_error(rid, e));
        }
    });

    Ok(Json(review))
}
use std::sync::Arc;

use sqlx::SqlitePool;

use shared::{AppError, AppResult};

use crate::executor::{ExecContext, ExecutorEvent, ExecutorManager};
use crate::models::{ExecutionJob, JobStatus};
use crate::repo;

/// 共享的 job 事件通知接口
///
/// 由 `api` crate 的 Hub（广播给所有 WS 连接）或单个 WS 连接实现，
/// 这样 `execution::dispatch` 无需感知具体传输层。
#[async_trait::async_trait]
pub trait JobNotifier: Send + Sync {
    async fn on_job_output(&self, job_id: &str, text: &str);
    async fn on_job_status(&self, job_id: &str, todo_id: &str, status: &str);
}

/// 完整的 execute_todo 流程，供 HTTP handler 和 orchestrator 工具共享调用。
///
/// 1. 从 DB 获取 todo → target → workspace 层级
/// 2. 创建 job 记录
/// 3. 准备 git worktree（`git worktree add -b` 一步完成）
/// 4. 通过 ExecutorManager 启动 coding agent 子进程
/// 5. 后台任务：将输出事件写入 DB + 通过 notifier 通知前端
/// 6. 终态时更新 todo 状态，从 active 表移除
///
/// 返回刚创建的 job（status: pending）。后台任务会异步更新状态。
///
/// `custom_prompt` 可选，如果提供则覆盖 todo.title + description 生成的默认 prompt。
pub async fn execute_todo(
    pool: &SqlitePool,
    executor: Arc<ExecutorManager>,
    notifier: Arc<dyn JobNotifier>,
    todo_id: &str,
    agent_type: &str,
    custom_prompt: Option<&str>,
) -> AppResult<ExecutionJob> {
    // ---------- 1. 获取任务层级 ----------
    let todo = tasks::repo::get_todo(pool, todo_id).await?;
    let target = tasks::repo::get_target(pool, &todo.target_id).await?;
    let ws = tasks::repo::get_workspace(pool, &target.workspace_id).await?;

    let ws_path = std::path::Path::new(&ws.workspace.path);
    let prompt = custom_prompt
        .map(|p| p.to_string())
        .unwrap_or_else(|| {
            format!(
                "Task: {}. Description: {}. Implement this change in the codebase. Create or modify files as needed. Do not ask for clarification.",
                todo.title, todo.description
            )
        });

    // ---------- 2. 创建 job 记录 ----------
    let job = repo::create_job(pool, todo_id, &prompt, agent_type).await?;
    let job_id = job.id.clone();

    // ---------- 3. 准备 worktree ----------
    // 分支名带时间戳后缀，避免重跑同一个 todo 时分支名冲突
    let timestamp = chrono::Utc::now().format("%H%M%S%3f").to_string();
    let branch = format!("todo-{}-{}", &todo.id[..8], timestamp);
    let wt_path = ws_path
        .parent()
        .unwrap_or(ws_path)
        .join(format!("{}-worktree", branch));

    let is_git_repo = crate::worktree::is_git_repo(ws_path).await;
    let wt_path_str = if is_git_repo {
        crate::worktree::create_worktree(ws_path, &branch, &wt_path)
            .await
            .map_err(|e| {
                let msg = format!("worktree creation failed: {e}");
                tracing::error!(todo_id = %todo_id, "{}", msg);
                AppError::Internal(msg)
            })?;

        let _ = repo::create_worktree(
            pool,
            &target.workspace_id,
            &branch,
            &wt_path.to_string_lossy(),
            Some(&target.id),
        )
        .await
        .map_err(|e| {
            tracing::warn!(todo_id = %todo_id, error = %e, "failed to persist worktree record, execution continues");
            // Don't propagate — agent can still work without the DB record
        });

        wt_path.to_string_lossy().to_string()
    } else {
        ws_path.to_string_lossy().to_string()
    };

    // ---------- 4. 更新 job 为 running ----------
    let _ = repo::update_job_status(pool, &job_id, JobStatus::Running, None).await?;
    notifier.on_job_status(&job_id, todo_id, "running").await;

    // ---------- 5. 启动 executor ----------
    let ctx = ExecContext::new(job_id.clone(), wt_path_str, prompt);

    let db = pool.clone();
    let jid = job_id.clone();
    let tid = todo_id.to_string();
    let notif = notifier.clone();

    match executor.spawn(agent_type, ctx).await {
        Ok(mut rx) => {
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Some(ExecutorEvent::Log { text }) => {
                            let _ = repo::append_job_output(&db, &jid, &text).await;
                            notif.on_job_output(&jid, &text).await;
                        }
                        Some(ExecutorEvent::Status { status }) => {
                            let status_str = status.as_str().to_string();
                            let _ = repo::update_job_status(&db, &jid, status, None).await;
                            notif.on_job_status(&jid, &tid, &status_str).await;

                            if matches!(
                                status,
                                JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled
                            ) {
                                let todo_status = if status == JobStatus::Success {
                                    Some(tasks::TodoStatus::Done)
                                } else {
                                    Some(tasks::TodoStatus::Blocked)
                                };
                                let _ = tasks::repo::update_todo(
                                    &db,
                                    &tid,
                                    tasks::UpdateTodo {
                                        status: todo_status,
                                        ..Default::default()
                                    },
                                )
                                .await;
                                executor.remove(&jid);
                                break;
                            }
                        }
                        Some(ExecutorEvent::Heartbeat) => {}
                        None => break,
                    }
                }
            });
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to spawn executor");
            let _ = repo::update_job_status(pool, &job_id, JobStatus::Failed, None).await;
            notifier.on_job_status(&job_id, todo_id, "failed").await;
            executor.remove(&job_id);
        }
    }

    Ok(job)
}
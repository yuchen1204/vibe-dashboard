use serde::{Deserialize, Serialize};
use shared::{AppError, AppResult};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::review::{self as review_repo, ReviewFinding};

// ---------- Models ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackAction {
    #[default]
    Pending,
    Accepted,
    Ignored,
    AutoFix,
}

impl FeedbackAction {
    pub fn as_str(self) -> &'static str {
        match self {
            FeedbackAction::Pending => "pending",
            FeedbackAction::Accepted => "accepted",
            FeedbackAction::Ignored => "ignored",
            FeedbackAction::AutoFix => "auto_fix",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewFeedback {
    pub id: String,
    pub review_id: String,
    pub finding_id: String,
    pub todo_id: Option<String>,
    pub action: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IterationStatus {
    #[default]
    Pending,
    Running,
    Passed,
    Failed,
    MaxedOut,
}

impl IterationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IterationStatus::Pending => "pending",
            IterationStatus::Running => "running",
            IterationStatus::Passed => "passed",
            IterationStatus::Failed => "failed",
            IterationStatus::MaxedOut => "maxed_out",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewIteration {
    pub id: String,
    pub todo_id: String,
    pub iteration: i64,
    pub job_id: Option<String>,
    pub review_id: Option<String>,
    pub status: String,
    pub score: Option<i64>,
    pub threshold: i64,
    pub summary: String,
    pub created_at: String,
    pub updated_at: String,
}

// ---------- DTOs ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackWithFinding {
    pub feedback: ReviewFeedback,
    pub finding: Option<ReviewFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationDetail {
    #[serde(flatten)]
    pub iteration: ReviewIteration,
    pub review: Option<review_repo::ReviewDetail>,
}

// ---------- Helpers ----------

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ---------- Feedback Repo ----------

pub async fn get_feedback_by_review(
    pool: &SqlitePool,
    review_id: &str,
) -> AppResult<Vec<ReviewFeedback>> {
    let rows = sqlx::query(
        r#"SELECT id, review_id, finding_id, todo_id, action, created_at, updated_at
           FROM review_feedback WHERE review_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(review_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|row| ReviewFeedback {
        id: row.get("id"),
        review_id: row.get("review_id"),
        finding_id: row.get("finding_id"),
        todo_id: row.get("todo_id"),
        action: row.get("action"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }).collect())
}

pub async fn get_feedback_by_finding(
    pool: &SqlitePool,
    finding_id: &str,
) -> AppResult<ReviewFeedback> {
    let rows = sqlx::query(
        r#"SELECT id, review_id, finding_id, todo_id, action, created_at, updated_at
           FROM review_feedback WHERE finding_id = ?1"#,
    )
    .bind(finding_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().next().map(|row| ReviewFeedback {
        id: row.get("id"),
        review_id: row.get("review_id"),
        finding_id: row.get("finding_id"),
        todo_id: row.get("todo_id"),
        action: row.get("action"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }).ok_or_else(|| AppError::NotFound(format!("feedback for finding {finding_id} not found")))
}

pub async fn ensure_feedback_records(
    pool: &SqlitePool,
    review_id: &str,
    finding_ids: &[String],
) -> AppResult<Vec<ReviewFeedback>> {
    let mut results = Vec::new();
    let now = now_rfc3339();

    for fid in finding_ids {
        // Check if already exists
        let existing = sqlx::query(
            r#"SELECT id, review_id, finding_id, todo_id, action, created_at, updated_at
               FROM review_feedback WHERE finding_id = ?1"#,
        )
        .bind(fid)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = existing {
            results.push(ReviewFeedback {
                id: row.get("id"),
                review_id: row.get("review_id"),
                finding_id: row.get("finding_id"),
                todo_id: row.get("todo_id"),
                action: row.get("action"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
            continue;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO review_feedback (id, review_id, finding_id, action, created_at, updated_at)
               VALUES (?1, ?2, ?3, 'pending', ?4, ?5)"#,
        )
        .bind(&id).bind(review_id).bind(fid).bind(&now).bind(&now)
        .execute(pool)
        .await?;

        results.push(ReviewFeedback {
            id,
            review_id: review_id.to_string(),
            finding_id: fid.to_string(),
            todo_id: None,
            action: "pending".to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }

    Ok(results)
}

pub async fn accept_finding(
    pool: &SqlitePool,
    finding_id: &str,
    target_id: Option<&str>,
) -> AppResult<ReviewFeedback> {
    let feedback = get_feedback_by_finding(pool, finding_id).await?;
    if feedback.action != "pending" {
        return Err(AppError::BadRequest(format!(
            "finding {finding_id} already has action '{}'",
            feedback.action
        )));
    }

    // Get the finding detail
    let rows = sqlx::query(
        r#"SELECT id, review_id, severity, file_path, line_number, category, title, description, suggestion, created_at
           FROM review_findings WHERE id = ?1"#,
    )
    .bind(finding_id)
    .fetch_all(pool)
    .await?;

    let finding = rows.into_iter().next()
        .ok_or_else(|| AppError::NotFound(format!("finding {finding_id} not found")))?;

    let title: String = finding.get("title");
    let description: String = finding.get("description");
    let suggestion: String = finding.get("suggestion");

    // Get the review to find the review's todo_id
    let review_rows = sqlx::query(
        r#"SELECT todo_id FROM reviews WHERE id = ?1"#,
    )
    .bind(&feedback.review_id)
    .fetch_all(pool)
    .await?;

    let review_todo_id: Option<String> = review_rows.into_iter().next().map(|r| r.get("todo_id"));

    // Create a new todo from the finding
    let todo = if let Some(ref tid) = review_todo_id {
        // Find which target the original todo belongs to
        let target_rows = sqlx::query(
            r#"SELECT target_id FROM todos WHERE id = ?1"#,
        )
        .bind(tid)
        .fetch_all(pool)
        .await?;

        let target_id = if let Some(tid) = target_id {
            Some(tid.to_string())
        } else {
            target_rows.into_iter().next().map(|r| {
                r.get::<String, _>("target_id")
            })
        };

        if let Some(ref tid) = target_id {
            let todo = tasks::repo::create_todo(
                pool,
                tid,
                tasks::CreateTodo {
                    title: format!("[审查] {}", title),
                    description: format!(
                        "## 问题描述\n{}\n\n## 修改建议\n{}\n\n## 来源\n审查 finding: {}\n文件: {}:{}",
                        description,
                        suggestion,
                        finding_id,
                        finding.get::<String, _>("file_path"),
                        finding.get::<Option<i64>, _>("line_number").map(|l| l.to_string()).unwrap_or_default(),
                    ),
                },
            )
            .await?;
            Some(todo)
        } else {
            None
        }
    } else {
        None
    };

    let now = now_rfc3339();
    let todo_id = todo.as_ref().map(|t| t.id.clone());

    sqlx::query(
        r#"UPDATE review_feedback SET action = 'accepted', todo_id = ?1, updated_at = ?2 WHERE id = ?3"#,
    )
    .bind(&todo_id).bind(&now).bind(&feedback.id)
    .execute(pool)
    .await?;

    Ok(ReviewFeedback {
        action: "accepted".to_string(),
        todo_id,
        updated_at: now,
        ..feedback
    })
}

pub async fn ignore_finding(
    pool: &SqlitePool,
    finding_id: &str,
) -> AppResult<ReviewFeedback> {
    let feedback = get_feedback_by_finding(pool, finding_id).await?;
    if feedback.action != "pending" {
        return Err(AppError::BadRequest(format!(
            "finding {finding_id} already has action '{}'",
            feedback.action
        )));
    }

    let now = now_rfc3339();
    sqlx::query(
        r#"UPDATE review_feedback SET action = 'ignored', updated_at = ?1 WHERE id = ?2"#,
    )
    .bind(&now).bind(&feedback.id)
    .execute(pool)
    .await?;

    Ok(ReviewFeedback {
        action: "ignored".to_string(),
        updated_at: now,
        ..feedback
    })
}

// ---------- Iteration Repo ----------

pub async fn get_or_create_iteration(
    pool: &SqlitePool,
    todo_id: &str,
) -> AppResult<ReviewIteration> {
    // Find the latest iteration for this todo
    let rows = sqlx::query(
        r#"SELECT id, todo_id, iteration, job_id, review_id, status, score, threshold, summary, created_at, updated_at
           FROM review_iterations WHERE todo_id = ?1 ORDER BY iteration DESC LIMIT 1"#,
    )
    .bind(todo_id)
    .fetch_all(pool)
    .await?;

    if let Some(row) = rows.into_iter().next() {
        let status: String = row.get("status");
        let sid = IterationStatus::Failed.as_str();
        let mid = IterationStatus::MaxedOut.as_str();

        // Only return existing if it's still pending/running
        if status == sid || status == mid {
            // Start a new iteration
            let iter: i64 = row.get("iteration");
            return create_iteration(pool, todo_id, iter + 1, None).await;
        }
        // If it's running or passed, return current
        // (if passed, caller should check score against threshold)
        if status == IterationStatus::Passed.as_str() {
            // Check score against threshold
            let score: Option<i64> = row.get("score");
            let threshold: i64 = row.get("threshold");
            if let Some(s) = score {
                if s >= threshold {
                    return Ok(ReviewIteration {
                        id: row.get("id"),
                        todo_id: row.get("todo_id"),
                        iteration: row.get("iteration"),
                        job_id: row.get("job_id"),
                        review_id: row.get("review_id"),
                        status: status.to_string(),
                        score,
                        threshold,
                        summary: row.get("summary"),
                        created_at: row.get("created_at"),
                        updated_at: row.get("updated_at"),
                    });
                }
            }
            // Score below threshold, start new iteration
            let iter: i64 = row.get("iteration");
            return create_iteration(pool, todo_id, iter + 1, None).await;
        }

        return Ok(ReviewIteration {
            id: row.get("id"),
            todo_id: row.get("todo_id"),
            iteration: row.get("iteration"),
            job_id: row.get("job_id"),
            review_id: row.get("review_id"),
            status: status.to_string(),
            score: row.get("score"),
            threshold: row.get("threshold"),
            summary: row.get("summary"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        });
    }

    // No existing iteration, create first one
    create_iteration(pool, todo_id, 1, None).await
}

async fn create_iteration(
    pool: &SqlitePool,
    todo_id: &str,
    iteration: i64,
    threshold: Option<i64>,
) -> AppResult<ReviewIteration> {
    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = IterationStatus::default().as_str();
    let threshold = threshold.unwrap_or(8);

    sqlx::query(
        r#"INSERT INTO review_iterations (id, todo_id, iteration, status, threshold, summary, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, ?7)"#,
    )
    .bind(&id).bind(todo_id).bind(iteration).bind(status).bind(threshold).bind(&now).bind(&now)
    .execute(pool)
    .await?;

    Ok(ReviewIteration {
        id,
        todo_id: todo_id.to_string(),
        iteration,
        job_id: None,
        review_id: None,
        status: status.to_string(),
        score: None,
        threshold,
        summary: String::new(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_iteration_job(
    pool: &SqlitePool,
    iteration_id: &str,
    job_id: &str,
) -> AppResult<ReviewIteration> {
    let now = now_rfc3339();
    let status = IterationStatus::Running.as_str();

    sqlx::query(
        r#"UPDATE review_iterations SET job_id = ?1, status = ?2, updated_at = ?3 WHERE id = ?4"#,
    )
    .bind(job_id).bind(status).bind(&now).bind(iteration_id)
    .execute(pool)
    .await?;

    let rows = sqlx::query(
        r#"SELECT id, todo_id, iteration, job_id, review_id, status, score, threshold, summary, created_at, updated_at
           FROM review_iterations WHERE id = ?1"#,
    )
    .bind(iteration_id)
    .fetch_all(pool)
    .await?;

    let row = rows.into_iter().next().ok_or_else(|| AppError::NotFound(format!("iteration {iteration_id}")))?;
    Ok(ReviewIteration {
        id: row.get("id"),
        todo_id: row.get("todo_id"),
        iteration: row.get("iteration"),
        job_id: Some(job_id.to_string()),
        review_id: row.get("review_id"),
        status: status.to_string(),
        score: row.get("score"),
        threshold: row.get("threshold"),
        summary: row.get("summary"),
        created_at: row.get("created_at"),
        updated_at: now,
    })
}

pub async fn complete_iteration(
    pool: &SqlitePool,
    iteration_id: &str,
    review_id: &str,
    score: i64,
    summary: &str,
) -> AppResult<ReviewIteration> {
    let now = now_rfc3339();
    let status = IterationStatus::Passed.as_str();

    sqlx::query(
        r#"UPDATE review_iterations
           SET review_id = ?1, status = ?2, score = ?3, summary = ?4, updated_at = ?5
           WHERE id = ?6"#,
    )
    .bind(review_id).bind(status).bind(score).bind(summary).bind(&now).bind(iteration_id)
    .execute(pool)
    .await?;

    let rows = sqlx::query(
        r#"SELECT id, todo_id, iteration, job_id, review_id, status, score, threshold, summary, created_at, updated_at
           FROM review_iterations WHERE id = ?1"#,
    )
    .bind(iteration_id)
    .fetch_all(pool)
    .await?;

    let row = rows.into_iter().next().ok_or_else(|| AppError::NotFound(format!("iteration {iteration_id}")))?;
    Ok(ReviewIteration {
        id: row.get("id"),
        todo_id: row.get("todo_id"),
        iteration: row.get("iteration"),
        job_id: row.get("job_id"),
        review_id: Some(review_id.to_string()),
        status: status.to_string(),
        score: Some(score),
        threshold: row.get("threshold"),
        summary: summary.to_string(),
        created_at: row.get("created_at"),
        updated_at: now,
    })
}

pub async fn max_out_iteration(
    pool: &SqlitePool,
    iteration_id: &str,
    review_id: &str,
    score: i64,
    summary: &str,
) -> AppResult<ReviewIteration> {
    let now = now_rfc3339();
    let status = IterationStatus::MaxedOut.as_str();

    sqlx::query(
        r#"UPDATE review_iterations
           SET review_id = ?1, status = ?2, score = ?3, summary = ?4, updated_at = ?5
           WHERE id = ?6"#,
    )
    .bind(review_id).bind(status).bind(score).bind(summary).bind(&now).bind(iteration_id)
    .execute(pool)
    .await?;

    let rows = sqlx::query(
        r#"SELECT id, todo_id, iteration, job_id, review_id, status, score, threshold, summary, created_at, updated_at
           FROM review_iterations WHERE id = ?1"#,
    )
    .bind(iteration_id)
    .fetch_all(pool)
    .await?;

    let row = rows.into_iter().next().ok_or_else(|| AppError::NotFound(format!("iteration {iteration_id}")))?;
    Ok(ReviewIteration {
        id: row.get("id"),
        todo_id: row.get("todo_id"),
        iteration: row.get("iteration"),
        job_id: row.get("job_id"),
        review_id: Some(review_id.to_string()),
        status: status.to_string(),
        score: Some(score),
        threshold: row.get("threshold"),
        summary: summary.to_string(),
        created_at: row.get("created_at"),
        updated_at: now,
    })
}

pub async fn list_iterations(
    pool: &SqlitePool,
    todo_id: &str,
) -> AppResult<Vec<ReviewIteration>> {
    let rows = sqlx::query(
        r#"SELECT id, todo_id, iteration, job_id, review_id, status, score, threshold, summary, created_at, updated_at
           FROM review_iterations WHERE todo_id = ?1 ORDER BY iteration ASC"#,
    )
    .bind(todo_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|row| ReviewIteration {
        id: row.get("id"),
        todo_id: row.get("todo_id"),
        iteration: row.get("iteration"),
        job_id: row.get("job_id"),
        review_id: row.get("review_id"),
        status: row.get("status"),
        score: row.get("score"),
        threshold: row.get("threshold"),
        summary: row.get("summary"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }).collect())
}

// ---------- Auto-Fix Orchestration ----------

const MAX_ITERATIONS: i64 = 5;

/// 运行自动修复循环：
/// 1. 获取/创建迭代记录
/// 2. 执行 todo（如果是首次，或 score < threshold）
/// 3. 审查执行结果
/// 4. 如果 score >= threshold，标记通过
/// 5. 如果 score < threshold 且 iteration < max，自动重试（用审查建议改进 prompt）
/// 6. 如果达到 max，标记 maxed_out
pub async fn run_auto_fix(
    pool: &SqlitePool,
    config: &crate::llm::LlmConfig,
    executor: std::sync::Arc<execution::executor::ExecutorManager>,
    notifier: std::sync::Arc<dyn execution::dispatch::JobNotifier>,
    todo_id: &str,
    custom_prompt: Option<&str>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<AutoFixEvent>>,
) -> Result<Vec<ReviewIteration>, String> {
    if !config.is_configured() {
        return Err("LLM 未配置。请先设置 API Key 来启用自动修复。".to_string());
    }

    let mut results = Vec::new();

    for round in 1..=MAX_ITERATIONS {
        send_event(&event_tx, AutoFixEvent::IterationStarted { todo_id: todo_id.to_string(), iteration: round });

        // 1. 获取或创建迭代记录
        let iteration = crate::feedback::get_or_create_iteration(pool, todo_id)
            .await
            .map_err(|e| format!("创建迭代记录失败: {e}"))?;

        // 如果已经通过，停止
        if iteration.status == IterationStatus::Passed.as_str() {
            send_event(&event_tx, AutoFixEvent::IterationPassed {
                todo_id: todo_id.to_string(),
                iteration: iteration.iteration,
                score: iteration.score.unwrap_or(0),
            });
            results.push(iteration);
            break;
        }

        // 2. 执行 todo
        let effective_prompt = if round > 1 {
            // 后续轮次：用上一轮审查结果增强 prompt
            if let Some(ref last_result) = results.last() {
                if let Some(ref review_id) = last_result.review_id {
                    if let Ok(detail) = crate::review::get_review_with_findings(pool, review_id).await {
                        let findings_summary: Vec<String> = detail.findings.iter().map(|f| {
                            format!("- [{}] {}: {} (建议: {})", f.severity, f.file_path, f.title, f.suggestion)
                        }).collect();
                        let enhancement = format!(
                            "\n\n## 上一轮审查反馈 (评分: {}/10)\n请根据以下审查发现修复代码：\n{}\n\n修复后重新运行测试确认。",
                            last_result.score.unwrap_or(0),
                            findings_summary.join("\n"),
                        );

                        let base = custom_prompt.unwrap_or("");
                        format!("{}{}", base, enhancement)
                    } else {
                        custom_prompt.unwrap_or("").to_string()
                    }
                } else {
                    custom_prompt.unwrap_or("").to_string()
                }
            } else {
                custom_prompt.unwrap_or("").to_string()
            }
        } else {
            custom_prompt.unwrap_or("").to_string()
        };

        send_event(&event_tx, AutoFixEvent::Executing { todo_id: todo_id.to_string(), iteration: round });

        let job = execution::dispatch::execute_todo(
            pool,
            executor.clone(),
            notifier.clone(),
            todo_id,
            "claude-code",
            Some(&effective_prompt),
        )
        .await
        .map_err(|e| format!("执行失败: {e}"))?;

        // 更新迭代记录中的 job_id
        let iteration = update_iteration_job(pool, &iteration.id, &job.id)
            .await
            .map_err(|e| format!("更新迭代 job 失败: {e}"))?;

        send_event(&event_tx, AutoFixEvent::JobCreated {
            todo_id: todo_id.to_string(),
            iteration: round,
            job_id: job.id.clone(),
        });

        // 3. 等待 job 完成
        wait_for_job_completion(pool, &job.id).await;

        // 4. 审查结果
        send_event(&event_tx, AutoFixEvent::Reviewing { todo_id: todo_id.to_string(), iteration: round });

        let review = crate::review_agent::run_review(
            pool,
            config,
            &job.id,
            todo_id,
            None,
            None,
        )
        .await?;

        // 5. 为 findings 创建 feedback 记录
        let finding_ids: Vec<String> = review.findings.iter().map(|f| f.id.clone()).collect();
        if !finding_ids.is_empty() {
            let _ = crate::feedback::ensure_feedback_records(pool, &review.review.id, &finding_ids).await;
        }

        let score = review.review.score.unwrap_or(0);
        let summary = review.review.summary.clone();
        let threshold = iteration.threshold;

        // 6. 判断是否通过
        if score >= threshold {
            let completed = complete_iteration(pool, &iteration.id, &review.review.id, score, &summary)
                .await
                .map_err(|e| format!("完成迭代失败: {e}"))?;

            send_event(&event_tx, AutoFixEvent::IterationPassed {
                todo_id: todo_id.to_string(),
                iteration: round,
                score,
            });

            results.push(completed);
            break;
        }

        if round >= MAX_ITERATIONS {
            let maxed = max_out_iteration(pool, &iteration.id, &review.review.id, score, &summary)
                .await
                .map_err(|e| format!("标记 maxed_out 失败: {e}"))?;

            send_event(&event_tx, AutoFixEvent::IterationMaxedOut {
                todo_id: todo_id.to_string(),
                iteration: round,
                score,
            });

            results.push(maxed);
        } else {
            send_event(&event_tx, AutoFixEvent::IterationFailed {
                todo_id: todo_id.to_string(),
                iteration: round,
                score,
                threshold,
            });

            results.push(iteration);
            // 继续下一轮
        }
    }

    Ok(results)
}

/// 等待 job 完成（通过轮询 DB）
async fn wait_for_job_completion(pool: &SqlitePool, job_id: &str) {
    let terminal = ["success", "failed", "cancelled"];
    for _ in 0..300 {
        // 最多等 5 分钟（每秒一次）
        match execution::repo::get_job(pool, job_id).await {
            Ok(job) => {
                if terminal.contains(&job.status.as_str()) {
                    return;
                }
            }
            Err(_) => return,
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

fn send_event(tx: &Option<tokio::sync::mpsc::UnboundedSender<AutoFixEvent>>, event: AutoFixEvent) {
    if let Some(ref tx) = tx {
        let _ = tx.send(event);
    }
}

// ---------- AutoFix Events ----------

#[derive(Debug, Clone)]
pub enum AutoFixEvent {
    IterationStarted { todo_id: String, iteration: i64 },
    Executing { todo_id: String, iteration: i64 },
    JobCreated { todo_id: String, iteration: i64, job_id: String },
    Reviewing { todo_id: String, iteration: i64 },
    IterationPassed { todo_id: String, iteration: i64, score: i64 },
    IterationFailed { todo_id: String, iteration: i64, score: i64, threshold: i64 },
    IterationMaxedOut { todo_id: String, iteration: i64, score: i64 },
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feedback_roundtrip() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        // Create tables
        sqlx::query(
            r#"CREATE TABLE reviews (
                id TEXT PRIMARY KEY, job_id TEXT, todo_id TEXT, status TEXT,
                summary TEXT, score INTEGER, total_findings INTEGER,
                created_at TEXT, updated_at TEXT, completed_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE review_findings (
                id TEXT PRIMARY KEY, review_id TEXT, severity TEXT, file_path TEXT,
                line_number INTEGER, category TEXT, title TEXT, description TEXT,
                suggestion TEXT, created_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE review_feedback (
                id TEXT PRIMARY KEY, review_id TEXT, finding_id TEXT, todo_id TEXT,
                action TEXT, created_at TEXT, updated_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a finding
        let finding_id = Uuid::new_v4().to_string();
        let now = now_rfc3339();
        sqlx::query(
            r#"INSERT INTO review_findings (id, review_id, severity, file_path, category, title, description, suggestion, created_at)
               VALUES (?1, 'review-1', 'major', 'src/main.rs', 'bug', 'Test', 'Test desc', 'Fix it', ?2)"#,
        )
        .bind(&finding_id).bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        // Ensure feedback records
        let feedbacks = ensure_feedback_records(&pool, "review-1", &[finding_id.clone()])
            .await
            .unwrap();
        assert_eq!(feedbacks.len(), 1);
        assert_eq!(feedbacks[0].action, "pending");

        // Ignore
        let ignored = ignore_finding(&pool, &finding_id).await.unwrap();
        assert_eq!(ignored.action, "ignored");
    }

    #[tokio::test]
    async fn test_iteration_roundtrip() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query(
            r#"CREATE TABLE todos (
                id TEXT PRIMARY KEY, target_id TEXT, title TEXT, description TEXT,
                status TEXT, sort_order INTEGER, created_at TEXT, updated_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"CREATE TABLE review_iterations (
                id TEXT PRIMARY KEY, todo_id TEXT, iteration INTEGER, job_id TEXT,
                review_id TEXT, status TEXT, score INTEGER, threshold INTEGER,
                summary TEXT, created_at TEXT, updated_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

        let todo_id = "todo-1";
        let iter = get_or_create_iteration(&pool, todo_id).await.unwrap();
        assert_eq!(iter.iteration, 1);
        assert_eq!(iter.status, "pending");

        // Second call should return same iteration
        let iter2 = get_or_create_iteration(&pool, todo_id).await.unwrap();
        assert_eq!(iter2.id, iter.id);
    }
}
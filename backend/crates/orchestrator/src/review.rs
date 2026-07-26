use serde::{Deserialize, Serialize};
use shared::{AppError, AppResult};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

// ---------- Review ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl ReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReviewStatus::Pending => "pending",
            ReviewStatus::InProgress => "in_progress",
            ReviewStatus::Completed => "completed",
            ReviewStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Review {
    pub id: String,
    pub job_id: String,
    pub todo_id: String,
    pub status: String,
    pub summary: String,
    pub score: Option<i64>,
    pub total_findings: i64,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

// ---------- Review Finding ----------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    #[default]
    Minor,
    Major,
    Critical,
    Suggestion,
}

impl FindingSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingSeverity::Critical => "critical",
            FindingSeverity::Major => "major",
            FindingSeverity::Minor => "minor",
            FindingSeverity::Suggestion => "suggestion",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewFinding {
    pub id: String,
    pub review_id: String,
    pub severity: String,
    pub file_path: String,
    pub line_number: Option<i64>,
    pub category: String,
    pub title: String,
    pub description: String,
    pub suggestion: String,
    pub created_at: String,
}

// ---------- DTOs ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDetail {
    #[serde(flatten)]
    pub review: Review,
    pub findings: Vec<ReviewFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFinding {
    pub severity: FindingSeverity,
    pub file_path: String,
    pub line_number: Option<i64>,
    pub category: String,
    pub title: String,
    pub description: String,
    pub suggestion: String,
}

// ---------- Repo ----------

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub async fn create_review(pool: &SqlitePool, job_id: &str, todo_id: &str) -> AppResult<Review> {
    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = ReviewStatus::default().as_str();

    sqlx::query(
        "INSERT INTO reviews (id, job_id, todo_id, status, summary, total_findings, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, '', 0, ?5, ?6)",
    )
    .bind(&id).bind(job_id).bind(todo_id).bind(status).bind(&now).bind(&now)
    .execute(pool)
    .await?;

    Ok(Review {
        id,
        job_id: job_id.to_string(),
        todo_id: todo_id.to_string(),
        status: status.to_string(),
        summary: String::new(),
        score: None,
        total_findings: 0,
        created_at: now.clone(),
        updated_at: now,
        completed_at: None,
    })
}

pub async fn get_review(pool: &SqlitePool, id: &str) -> AppResult<Review> {
    let rows = sqlx::query(
        "SELECT id, job_id, todo_id, status, summary, score, total_findings, created_at, updated_at, completed_at FROM reviews WHERE id = ?1",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    rows.into_iter().next().map(|row| Review {
        id: row.get("id"),
        job_id: row.get("job_id"),
        todo_id: row.get("todo_id"),
        status: row.get("status"),
        summary: row.get("summary"),
        score: row.get("score"),
        total_findings: row.get("total_findings"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }).ok_or_else(|| AppError::NotFound(format!("review {id} not found")))
}

pub async fn get_review_with_findings(pool: &SqlitePool, id: &str) -> AppResult<ReviewDetail> {
    let review = get_review(pool, id).await?;

    let rows = sqlx::query(
        r#"SELECT id, review_id, severity, file_path, line_number, category, title, description, suggestion, created_at
           FROM review_findings
           WHERE review_id = ?1
           ORDER BY
               CASE severity
                   WHEN 'critical' THEN 0
                   WHEN 'major' THEN 1
                   WHEN 'minor' THEN 2
                   WHEN 'suggestion' THEN 3
               END,
               created_at ASC"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let findings: Vec<ReviewFinding> = rows.iter().map(|row| ReviewFinding {
        id: row.get("id"),
        review_id: row.get("review_id"),
        severity: row.get("severity"),
        file_path: row.get("file_path"),
        line_number: row.get("line_number"),
        category: row.get("category"),
        title: row.get("title"),
        description: row.get("description"),
        suggestion: row.get("suggestion"),
        created_at: row.get("created_at"),
    }).collect();

    Ok(ReviewDetail { review, findings })
}

pub async fn list_reviews_by_todo(pool: &SqlitePool, todo_id: &str) -> AppResult<Vec<Review>> {
    let rows = sqlx::query(
        "SELECT id, job_id, todo_id, status, summary, score, total_findings, created_at, updated_at, completed_at FROM reviews WHERE todo_id = ?1 ORDER BY created_at DESC",
    )
    .bind(todo_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|row| Review {
        id: row.get("id"),
        job_id: row.get("job_id"),
        todo_id: row.get("todo_id"),
        status: row.get("status"),
        summary: row.get("summary"),
        score: row.get("score"),
        total_findings: row.get("total_findings"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }).collect())
}

pub async fn list_reviews_by_job(pool: &SqlitePool, job_id: &str) -> AppResult<Vec<Review>> {
    let rows = sqlx::query(
        "SELECT id, job_id, todo_id, status, summary, score, total_findings, created_at, updated_at, completed_at FROM reviews WHERE job_id = ?1 ORDER BY created_at DESC",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|row| Review {
        id: row.get("id"),
        job_id: row.get("job_id"),
        todo_id: row.get("todo_id"),
        status: row.get("status"),
        summary: row.get("summary"),
        score: row.get("score"),
        total_findings: row.get("total_findings"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }).collect())
}

pub async fn update_review_status(
    pool: &SqlitePool,
    id: &str,
    status: ReviewStatus,
) -> AppResult<Review> {
    let existing = get_review(pool, id).await?;
    let now = now_rfc3339();
    let status_str = status.as_str();

    match status {
        ReviewStatus::Completed | ReviewStatus::Failed => {
            sqlx::query(
                "UPDATE reviews SET status = ?1, completed_at = ?2, updated_at = ?3 WHERE id = ?4",
            )
            .bind(status_str).bind(&now).bind(&now).bind(id)
            .execute(pool)
            .await?;
        }
        _ => {
            sqlx::query(
                "UPDATE reviews SET status = ?1, completed_at = NULL, updated_at = ?2 WHERE id = ?3",
            )
            .bind(status_str).bind(&now).bind(id)
            .execute(pool)
            .await?;
        }
    }

    Ok(Review {
        status: status_str.to_string(),
        completed_at: if matches!(status, ReviewStatus::Completed | ReviewStatus::Failed) {
            Some(now)
        } else {
            existing.completed_at
        },
        ..existing
    })
}

pub async fn update_review_summary(
    pool: &SqlitePool,
    id: &str,
    summary: &str,
    score: Option<i64>,
    total_findings: i64,
) -> AppResult<Review> {
    let existing = get_review(pool, id).await?;
    let now = now_rfc3339();

    sqlx::query(
        "UPDATE reviews SET summary = ?1, score = ?2, total_findings = ?3, status = 'completed', completed_at = ?4, updated_at = ?5 WHERE id = ?6",
    )
    .bind(summary).bind(score).bind(total_findings).bind(&now).bind(&now).bind(id)
    .execute(pool)
    .await?;

    Ok(Review {
        summary: summary.to_string(),
        score,
        total_findings,
        status: "completed".to_string(),
        completed_at: Some(now.clone()),
        updated_at: now.clone(),
        ..existing
    })
}

pub async fn add_finding(
    pool: &SqlitePool,
    review_id: &str,
    input: CreateFinding,
) -> AppResult<ReviewFinding> {
    // Verify review exists
    let _ = get_review(pool, review_id).await?;

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let severity = input.severity.as_str();

    sqlx::query(
        "INSERT INTO review_findings (id, review_id, severity, file_path, line_number, category, title, description, suggestion, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(&id).bind(review_id).bind(severity)
    .bind(&input.file_path).bind(input.line_number).bind(&input.category)
    .bind(&input.title).bind(&input.description).bind(&input.suggestion)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(ReviewFinding {
        id,
        review_id: review_id.to_string(),
        severity: severity.to_string(),
        file_path: input.file_path,
        line_number: input.line_number,
        category: input.category,
        title: input.title,
        description: input.description,
        suggestion: input.suggestion,
        created_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_review_roundtrip() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .unwrap();

        // Create tables
        sqlx::query(
            r#"CREATE TABLE execution_jobs (
                id TEXT PRIMARY KEY, todo_id TEXT, status TEXT, agent_type TEXT,
                prompt TEXT, output TEXT, created_at TEXT, updated_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

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

        // Create a review
        let review = create_review(&pool, "job-1", "todo-1").await.unwrap();
        assert_eq!(review.status, "pending");
        assert_eq!(review.job_id, "job-1");

        // Add findings
        let finding = add_finding(
            &pool,
            &review.id,
            CreateFinding {
                severity: FindingSeverity::Major,
                file_path: "src/main.rs".into(),
                line_number: Some(42),
                category: "bug".into(),
                title: "Null pointer risk".into(),
                description: "Potential null dereference".into(),
                suggestion: "Add a null check".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(finding.severity, "major");
        assert_eq!(finding.file_path, "src/main.rs");

        // Get with findings
        let detail = get_review_with_findings(&pool, &review.id).await.unwrap();
        assert_eq!(detail.findings.len(), 1);
        assert_eq!(detail.findings[0].title, "Null pointer risk");

        // Update summary
        let updated = update_review_summary(&pool, &review.id, "Looks good", Some(8), 1)
            .await
            .unwrap();
        assert_eq!(updated.status, "completed");
        assert_eq!(updated.score, Some(8));
    }

    #[tokio::test]
    async fn test_list_reviews() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::query(
            r#"CREATE TABLE execution_jobs (
                id TEXT PRIMARY KEY, todo_id TEXT, status TEXT, agent_type TEXT,
                prompt TEXT, output TEXT, created_at TEXT, updated_at TEXT
            )"#,
        )
        .execute(&pool)
        .await
        .unwrap();

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

        create_review(&pool, "job-1", "todo-1").await.unwrap();
        create_review(&pool, "job-1", "todo-1").await.unwrap();

        let list = list_reviews_by_todo(&pool, "todo-1").await.unwrap();
        assert_eq!(list.len(), 2);
    }
}
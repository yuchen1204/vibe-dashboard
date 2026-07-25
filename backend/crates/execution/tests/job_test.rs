use db::{init_pool, run_migrations};
use execution::repo;
use execution::JobStatus;

struct TestSeed {
    pool: sqlx::SqlitePool,
    _dir: tempfile::TempDir,
    todo_id: String,
}

async fn setup() -> TestSeed {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");

    let ws_id = uuid::Uuid::new_v4().to_string();
    let tgt_id = uuid::Uuid::new_v4().to_string();
    let todo_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Use raw sqlx::query (not macro) to avoid needing prepare for test seed data
    sqlx::query("INSERT INTO workspaces (id, name, path, created_at, updated_at) VALUES (?1, 'test', '/tmp', ?2, ?3)")
        .bind(&ws_id).bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO targets (id, workspace_id, title, description, status, sort_order, created_at, updated_at) VALUES (?1, ?2, 'tgt', '', 'planned', 0, ?3, ?4)")
        .bind(&tgt_id).bind(&ws_id).bind(&now).bind(&now)
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO todos (id, target_id, title, description, status, sort_order, created_at, updated_at) VALUES (?1, ?2, 'todo', '', 'todo', 0, ?3, ?4)")
        .bind(&todo_id).bind(&tgt_id).bind(&now).bind(&now)
        .execute(&pool).await.unwrap();

    TestSeed { pool, _dir: dir, todo_id }
}

#[tokio::test]
async fn create_and_get_job() {
    let seed = setup().await;
    let job = repo::create_job(&seed.pool, &seed.todo_id, "do the thing", "claude-code")
        .await
        .expect("create");
    assert_eq!(job.status, "pending");
    assert_eq!(job.agent_type, "claude-code");
    assert_eq!(job.prompt, "do the thing");

    let got = repo::get_job(&seed.pool, &job.id).await.expect("get");
    assert_eq!(got.id, job.id);
}

#[tokio::test]
async fn list_jobs_by_todo() {
    let seed = setup().await;
    repo::create_job(&seed.pool, &seed.todo_id, "job 1", "claude-code")
        .await
        .unwrap();
    repo::create_job(&seed.pool, &seed.todo_id, "job 2", "claude-code")
        .await
        .unwrap();

    let list = repo::list_jobs_by_todo(&seed.pool, &seed.todo_id).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn update_job_status_sets_timestamps() {
    let seed = setup().await;
    let job = repo::create_job(&seed.pool, &seed.todo_id, "test", "claude-code")
        .await
        .unwrap();

    let running = repo::update_job_status(&seed.pool, &job.id, JobStatus::Running, None)
        .await
        .unwrap();
    assert_eq!(running.status, "running");
    assert!(running.started_at.is_some());
    assert!(running.finished_at.is_none());

    let done = repo::update_job_status(&seed.pool, &job.id, JobStatus::Success, None)
        .await
        .unwrap();
    assert_eq!(done.status, "success");
    assert!(done.finished_at.is_some());
}

#[tokio::test]
async fn append_job_output() {
    let seed = setup().await;
    let job = repo::create_job(&seed.pool, &seed.todo_id, "test", "claude-code")
        .await
        .unwrap();

    repo::append_job_output(&seed.pool, &job.id, "line 1\n")
        .await
        .unwrap();
    repo::append_job_output(&seed.pool, &job.id, "line 2\n")
        .await
        .unwrap();

    let updated = repo::get_job(&seed.pool, &job.id).await.unwrap();
    assert_eq!(updated.output, "line 1\nline 2\n");
}

#[tokio::test]
async fn cancel_job() {
    let seed = setup().await;
    let job = repo::create_job(&seed.pool, &seed.todo_id, "test", "claude-code")
        .await
        .unwrap();

    repo::update_job_status(&seed.pool, &job.id, JobStatus::Running, None)
        .await
        .unwrap();

    let cancelled = repo::cancel_job(&seed.pool, &job.id).await.unwrap();
    assert_eq!(cancelled.status, "cancelled");
}

#[tokio::test]
async fn cancel_non_running_job_fails() {
    let seed = setup().await;
    let job = repo::create_job(&seed.pool, &seed.todo_id, "test", "claude-code")
        .await
        .unwrap();

    repo::update_job_status(&seed.pool, &job.id, JobStatus::Success, None)
        .await
        .unwrap();
    let err = repo::cancel_job(&seed.pool, &job.id).await.unwrap_err();
    assert!(matches!(err, shared::AppError::BadRequest(_)));
}

#[tokio::test]
async fn create_job_rejects_empty_prompt() {
    let seed = setup().await;
    let err = repo::create_job(&seed.pool, "todo-1", "  ", "claude-code")
        .await
        .unwrap_err();
    assert!(matches!(err, shared::AppError::BadRequest(_)));
}

#[tokio::test]
async fn create_job_unknown_todo_returns_404() {
    let seed = setup().await;
    let err = repo::create_job(&seed.pool, "nope", "do it", "claude-code")
        .await
        .unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}
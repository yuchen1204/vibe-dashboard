use db::{init_pool, run_migrations};
use execution::repo;
use execution::WorktreeStatus;

struct TestSeed {
    pool: sqlx::SqlitePool,
    _dir: tempfile::TempDir,
    ws_id: String,
}

async fn setup() -> TestSeed {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");

    let ws_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query("INSERT INTO workspaces (id, name, path, created_at, updated_at) VALUES (?1, 'test', '/tmp', ?2, ?3)")
        .bind(&ws_id).bind(&now).bind(&now)
        .execute(&pool).await.unwrap();

    TestSeed { pool, _dir: dir, ws_id }
}

#[tokio::test]
async fn create_and_get_worktree() {
    let seed = setup().await;
    let wt = repo::create_worktree(&seed.pool, &seed.ws_id, "feature/x", "/tmp/wt", None)
        .await
        .expect("create");
    assert_eq!(wt.branch, "feature/x");
    assert_eq!(wt.status, "active");

    let got = repo::get_worktree(&seed.pool, &wt.id).await.expect("get");
    assert_eq!(got.id, wt.id);
}

#[tokio::test]
async fn list_worktrees_by_workspace() {
    let seed = setup().await;
    repo::create_worktree(&seed.pool, &seed.ws_id, "b1", "/tmp/b1", None)
        .await
        .unwrap();
    repo::create_worktree(&seed.pool, &seed.ws_id, "b2", "/tmp/b2", None)
        .await
        .unwrap();

    // Also create a second workspace to verify isolation
    let ws2_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO workspaces (id, name, path, created_at, updated_at) VALUES (?1, 'test2', '/tmp2', ?2, ?3)")
        .bind(&ws2_id).bind(&now).bind(&now)
        .execute(&seed.pool).await.unwrap();
    repo::create_worktree(&seed.pool, &ws2_id, "b3", "/tmp/b3", None)
        .await
        .unwrap();

    let list = repo::list_worktrees(&seed.pool, &seed.ws_id).await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn update_worktree_status() {
    let seed = setup().await;
    let wt = repo::create_worktree(&seed.pool, &seed.ws_id, "feature/x", "/tmp/wt", None)
        .await
        .unwrap();

    let updated = repo::update_worktree_status(&seed.pool, &wt.id, WorktreeStatus::Merged)
        .await
        .unwrap();
    assert_eq!(updated.status, "merged");
}

#[tokio::test]
async fn delete_worktree() {
    let seed = setup().await;
    let wt = repo::create_worktree(&seed.pool, &seed.ws_id, "feature/x", "/tmp/wt", None)
        .await
        .unwrap();

    repo::delete_worktree(&seed.pool, &wt.id).await.unwrap();
    let err = repo::get_worktree(&seed.pool, &wt.id).await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

#[tokio::test]
async fn create_worktree_rejects_empty_branch() {
    let seed = setup().await;
    let err = repo::create_worktree(&seed.pool, &seed.ws_id, "  ", "/tmp/wt", None)
        .await
        .unwrap_err();
    assert!(matches!(err, shared::AppError::BadRequest(_)));
}

#[tokio::test]
async fn get_unknown_worktree_returns_404() {
    let seed = setup().await;
    let err = repo::get_worktree(&seed.pool, "nope").await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}
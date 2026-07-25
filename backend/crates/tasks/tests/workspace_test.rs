use db::{init_pool, run_migrations};
use tasks::repo;
use tasks::CreateWorkspace;

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    (pool, dir)
}

#[tokio::test]
async fn create_and_get_workspace() {
    let (pool, _dir) = setup().await;
    let ws = repo::create_workspace(
        &pool,
        CreateWorkspace {
            name: "demo".into(),
            path: "/tmp/demo".into(),
        },
    )
    .await
    .expect("create");

    let detail = repo::get_workspace(&pool, &ws.id).await.expect("get");
    assert_eq!(detail.workspace.name, "demo");
    assert_eq!(detail.target_count, 0);
    assert_eq!(detail.todo_count, 0);
}

#[tokio::test]
async fn list_workspaces_orders_by_updated_desc() {
    let (pool, _dir) = setup().await;
    let a = repo::create_workspace(
        &pool,
        CreateWorkspace {
            name: "a".into(),
            path: "/a".into(),
        },
    )
    .await
    .unwrap();
    let b = repo::create_workspace(
        &pool,
        CreateWorkspace {
            name: "b".into(),
            path: "/b".into(),
        },
    )
    .await
    .unwrap();
    repo::update_workspace(
        &pool,
        &a.id,
        tasks::UpdateWorkspace {
            name: Some("a2".into()),
            path: None,
        },
    )
    .await
    .unwrap();

    let list = repo::list_workspaces(&pool).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, a.id, "recently updated a should come first");
    assert_eq!(list[1].id, b.id);
}

#[tokio::test]
async fn update_workspace_not_found() {
    let (pool, _dir) = setup().await;
    let err = repo::update_workspace(
        &pool,
        "nope",
        tasks::UpdateWorkspace {
            name: Some("x".into()),
            path: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

#[tokio::test]
async fn create_workspace_rejects_empty_name() {
    let (pool, _dir) = setup().await;
    let err = repo::create_workspace(
        &pool,
        CreateWorkspace {
            name: "  ".into(),
            path: "/x".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, shared::AppError::BadRequest(_)));
}

#[tokio::test]
async fn delete_workspace_not_found() {
    let (pool, _dir) = setup().await;
    let err = repo::delete_workspace(&pool, "nope").await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

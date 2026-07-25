use db::{init_pool, run_migrations};
use tasks::repo;
use tasks::{CreateTarget, CreateWorkspace, TargetStatus, UpdateTarget};

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    (pool, dir)
}

async fn seed_workspace(pool: &sqlx::SqlitePool) -> String {
    let ws = repo::create_workspace(
        pool,
        CreateWorkspace {
            name: "w".into(),
            path: "/w".into(),
        },
    )
    .await
    .unwrap();
    ws.id
}

#[tokio::test]
async fn create_and_list_targets() {
    let (pool, _dir) = setup().await;
    let wid = seed_workspace(&pool).await;
    let t = repo::create_target(
        &pool,
        &wid,
        CreateTarget {
            title: "t1".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(t.status, "planned");

    let list = repo::list_targets(&pool, &wid).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, t.id);
}

#[tokio::test]
async fn create_target_workspace_not_found() {
    let (pool, _dir) = setup().await;
    let err = repo::create_target(
        &pool,
        "nope",
        CreateTarget {
            title: "x".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

#[tokio::test]
async fn update_target_status() {
    let (pool, _dir) = setup().await;
    let wid = seed_workspace(&pool).await;
    let t = repo::create_target(
        &pool,
        &wid,
        CreateTarget {
            title: "t".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();

    let updated = repo::update_target(
        &pool,
        &t.id,
        UpdateTarget {
            status: Some(TargetStatus::Done),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.status, "done");
}

#[tokio::test]
async fn delete_target_removes_it() {
    let (pool, _dir) = setup().await;
    let wid = seed_workspace(&pool).await;
    let t = repo::create_target(
        &pool,
        &wid,
        CreateTarget {
            title: "t".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();
    repo::delete_target(&pool, &t.id).await.unwrap();
    let err = repo::get_target(&pool, &t.id).await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

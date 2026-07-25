use db::{init_pool, run_migrations};
use tasks::repo;
use tasks::{CreateTarget, CreateTodo, CreateWorkspace, TodoStatus, UpdateTodo};

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    (pool, dir)
}

async fn seed(pool: &sqlx::SqlitePool) -> (String, String) {
    let ws = repo::create_workspace(
        pool,
        CreateWorkspace {
            name: "w".into(),
            path: "/w".into(),
        },
    )
    .await
    .unwrap();
    let t = repo::create_target(
        pool,
        &ws.id,
        CreateTarget {
            title: "t".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();
    (ws.id, t.id)
}

#[tokio::test]
async fn create_and_get_todo() {
    let (pool, _dir) = setup().await;
    let (_wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(
        &pool,
        &tid,
        CreateTodo {
            title: "do thing".into(),
            description: "desc".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(todo.status, "todo");

    let got = repo::get_todo(&pool, &todo.id).await.unwrap();
    assert_eq!(got.title, "do thing");
}

#[tokio::test]
async fn list_todos_by_workspace_cross_target() {
    let (pool, _dir) = setup().await;
    let (wid, tid) = seed(&pool).await;
    let t2 = repo::create_target(
        &pool,
        &wid,
        CreateTarget {
            title: "t2".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();
    repo::create_todo(
        &pool,
        &tid,
        CreateTodo {
            title: "a".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();
    repo::create_todo(
        &pool,
        &t2.id,
        CreateTodo {
            title: "b".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();

    let todos = repo::list_todos_by_workspace(&pool, &wid).await.unwrap();
    assert_eq!(todos.len(), 2);
}

#[tokio::test]
async fn update_todo_status() {
    let (pool, _dir) = setup().await;
    let (_wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(
        &pool,
        &tid,
        CreateTodo {
            title: "x".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();

    let updated = repo::update_todo(
        &pool,
        &todo.id,
        UpdateTodo {
            status: Some(TodoStatus::Doing),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.status, "doing");
}

#[tokio::test]
async fn delete_target_cascades_todos() {
    let (pool, _dir) = setup().await;
    let (_wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(
        &pool,
        &tid,
        CreateTodo {
            title: "x".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();

    repo::delete_target(&pool, &tid).await.unwrap();
    let err = repo::get_todo(&pool, &todo.id).await.unwrap_err();
    assert!(
        matches!(err, shared::AppError::NotFound(_)),
        "todo should be gone after target cascade delete"
    );
}

#[tokio::test]
async fn delete_workspace_cascades_targets_and_todos() {
    let (pool, _dir) = setup().await;
    let (wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(
        &pool,
        &tid,
        CreateTodo {
            title: "x".into(),
            description: "".into(),
        },
    )
    .await
    .unwrap();

    repo::delete_workspace(&pool, &wid).await.unwrap();
    assert!(matches!(
        repo::get_target(&pool, &tid).await.unwrap_err(),
        shared::AppError::NotFound(_)
    ));
    assert!(matches!(
        repo::get_todo(&pool, &todo.id).await.unwrap_err(),
        shared::AppError::NotFound(_)
    ));
}

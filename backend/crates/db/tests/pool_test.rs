use db::{init_pool, run_migrations};

async fn setup_pool() -> sqlx::SqlitePool {
    let url = "sqlite::memory:";
    let pool = init_pool(url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    pool
}

#[tokio::test]
async fn init_pool_creates_working_pool() {
    let pool = setup_pool().await;
    let (val,): (String,) = sqlx::query_as("SELECT 'ok' AS val")
        .fetch_one(&pool)
        .await
        .expect("query failed");
    assert_eq!(val, "ok");
}

#[tokio::test]
async fn migrations_create_schema_meta_table() {
    let pool = setup_pool().await;
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) AS count FROM schema_meta")
        .fetch_one(&pool)
        .await
        .expect("query failed");
    assert!(
        count >= 2,
        "schema_meta should have schema_version and created_at"
    );
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let pool = setup_pool().await;
    run_migrations(&pool)
        .await
        .expect("re-run migrations failed");
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) AS count FROM schema_meta")
        .fetch_one(&pool)
        .await
        .expect("query failed");
    assert_eq!(
        count, 2,
        "schema_meta should still have 2 rows after re-run"
    );
}

// NOTE: WAL mode cannot be verified on an in-memory database (sqlite::memory:),
// because WAL requires a separate sidecar file on disk. SQLite silently falls
// back to MEMORY journal mode for in-memory DBs (see SQLite docs for
// PRAGMA journal_mode). This test uses a file-backed temp database so the WAL
// assertion is meaningful.
#[tokio::test]
async fn wal_mode_enabled() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("wal_test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));

    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");

    let (mode,): (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("pragma failed");
    assert_eq!(mode.to_lowercase(), "wal");

    drop(pool);
}

use axum::body::Body;
use axum::http::{Request, StatusCode};
use db::{init_pool, run_migrations};
use tower::ServiceExt;

async fn setup() -> axum::Router {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("t.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hub = api::Hub::new();
    let config = api::Config {
        db_path: String::new(),
        http_port: 0,
        log_level: "info".to_string(),
    };
    let state = api::AppState::new(pool, hub, config);
    api::app(state)
}

async fn seed_workspace_target_todo(app: axum::Router) -> (String, String, String) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"w","path":"/w"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let ws: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let wid = ws["id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/workspaces/{wid}/targets"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"t"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let t: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tid = t["id"].as_str().unwrap().to_string();

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/targets/{tid}/todos"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"do x"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let todo: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let todo_id = todo["id"].as_str().unwrap().to_string();

    (wid, tid, todo_id)
}

#[tokio::test]
async fn workspace_target_todo_roundtrip() {
    let app = setup().await;
    let (wid, _tid, todo_id) = seed_workspace_target_todo(app.clone()).await;

    // list todos by workspace returns the seeded todo
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/workspaces/{wid}/todos"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let todos: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(todos.as_array().unwrap().len(), 1);
    assert_eq!(todos[0]["status"], "todo");

    // update todo status -> doing
    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/todos/{todo_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"doing"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn delete_workspace_returns_204_and_cascades() {
    let app = setup().await;
    let (wid, tid, todo_id) = seed_workspace_target_todo(app.clone()).await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/workspaces/{wid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // target and todo are gone (404)
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/targets/{tid}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/todos/{todo_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_unknown_todo_returns_404() {
    let app = setup().await;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/todos/nope")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn invalid_status_returns_400() {
    let app = setup().await;
    let (_wid, _tid, todo_id) = seed_workspace_target_todo(app.clone()).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/todos/{todo_id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"bogus"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // axum's Json extractor rejects malformed/invalid payloads with 422
    // (Unprocessable Entity); app-level BadRequest (empty title etc.) is 400.
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_workspace_missing_field_returns_422() {
    let app = setup().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"w"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    // missing required field -> axum Json rejection (422)
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_workspace_empty_name_returns_400() {
    let app = setup().await;
    // present-but-empty name passes deserialization but fails app-level validation (400)
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"  ","path":"/w"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

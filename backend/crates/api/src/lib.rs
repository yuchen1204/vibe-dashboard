pub mod config;
pub mod error;
pub mod routes;
pub mod state;
pub mod ws;

pub use config::Config;
pub use state::AppState;
pub use ws::Hub;

use axum::{routing::{delete, get, post}, Router};
use tower_http::trace::TraceLayer;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health::health))
        .route("/api/path-suggest", get(routes::path::path_suggest))
        .route(
            "/api/workspaces",
            get(routes::tasks::list_workspaces).post(routes::tasks::create_workspace),
        )
        .route(
            "/api/workspaces/:id",
            get(routes::tasks::get_workspace)
                .put(routes::tasks::update_workspace)
                .delete(routes::tasks::delete_workspace),
        )
        .route(
            "/api/workspaces/:wid/targets",
            get(routes::tasks::list_targets).post(routes::tasks::create_target),
        )
        .route(
            "/api/targets/:id",
            get(routes::tasks::get_target)
                .put(routes::tasks::update_target)
                .delete(routes::tasks::delete_target),
        )
        .route(
            "/api/workspaces/:wid/todos",
            get(routes::tasks::list_todos_by_workspace),
        )
        .route(
            "/api/targets/:tid/todos",
            get(routes::tasks::list_todos_by_target).post(routes::tasks::create_todo),
        )
        .route(
            "/api/todos/:id",
            get(routes::tasks::get_todo)
                .put(routes::tasks::update_todo)
                .delete(routes::tasks::delete_todo),
        )
        // L3 execution routes
        .route(
            "/api/workspaces/:wid/worktrees",
            get(routes::execution::list_worktrees).post(routes::execution::create_worktree),
        )
        .route(
            "/api/worktrees/:id",
            delete(routes::execution::delete_worktree),
        )
        .route(
            "/api/workspaces/:wid/jobs",
            get(routes::execution::list_jobs),
        )
        .route(
            "/api/todos/:tid/execute",
            post(routes::execution::execute_todo),
        )
        .route(
            "/api/jobs/:id",
            get(routes::execution::get_job),
        )
        .route(
            "/api/jobs/:id/cancel",
            post(routes::execution::cancel_job),
        )
        .route("/ws", get(routes::ws::ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

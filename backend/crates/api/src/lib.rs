pub mod config;
pub mod error;
pub mod routes;
pub mod state;
pub mod ws;

pub use config::Config;
pub use state::AppState;
pub use ws::Hub;

use axum::{routing::{delete, get, post, put}, Router};
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
        // L4 settings routes
        .route(
            "/api/settings/llm",
            get(routes::settings::get_llm_config)
                .put(routes::settings::set_llm_config)
                .delete(routes::settings::clear_llm_config),
        )
        // L5 review routes
        .route(
            "/api/reviews/todo/:todo_id",
            get(routes::review::list_reviews_by_todo),
        )
        .route(
            "/api/reviews/job/:job_id",
            get(routes::review::list_reviews_by_job),
        )
        .route(
            "/api/reviews/:id",
            get(routes::review::get_review),
        )
        .route(
            "/api/reviews",
            post(routes::review::create_review),
        )
        .route(
            "/api/reviews/trigger",
            post(routes::review::trigger_review),
        )
        .route(
            "/api/reviews/:id/findings",
            post(routes::review::add_finding),
        )
        .route(
            "/api/reviews/:id/summary",
            put(routes::review::update_review_summary),
        )
        // L6 feedback loop routes
        .route(
            "/api/reviews/:rid/feedback",
            get(routes::feedback::list_feedback),
        )
        .route(
            "/api/feedback/:finding_id/accept",
            post(routes::feedback::accept_finding),
        )
        .route(
            "/api/feedback/:finding_id/ignore",
            post(routes::feedback::ignore_finding),
        )
        .route(
            "/api/todos/:tid/iterations",
            get(routes::feedback::list_iterations),
        )
        .route(
            "/api/todos/:tid/auto-fix",
            post(routes::feedback::trigger_auto_fix),
        )
        .route(
            "/api/todos/:tid/auto-fix-sync",
            post(routes::feedback::trigger_auto_fix_sync),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

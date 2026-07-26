use tracing::info;

use api::config::Config;
use api::state::AppState;
use api::ws::Hub;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    shared::logging::init(&config.log_level);

    let db_url = format!("sqlite:{}", config.db_path);
    let pool = db::init_pool(&db_url).await?;
    db::run_migrations(&pool).await?;

    let hub = Hub::new();
    let state = AppState::new(pool, hub, config.clone()).await;

    let app = api::app(state.clone());

    let addr = format!("127.0.0.1:{}", config.http_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "server starting");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("server stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed installing Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed installing signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    info!("shutdown signal received");
}

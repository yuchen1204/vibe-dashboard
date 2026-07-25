use std::sync::Arc;

use chrono::{DateTime, Utc};
use execution::executor::ExecutorManager;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub hub: Arc<Hub>,
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub executor: Arc<ExecutorManager>,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new(db: SqlitePool, hub: Arc<Hub>, config: Config) -> Self {
        let mut manager = ExecutorManager::new();
        manager.register_all(vec![
            Box::new(execution::executor::claude::ClaudeCodeExecutor::new()),
            Box::new(execution::executor::opencode::OpenCodeExecutor::new()),
        ]);
        Self {
            db,
            hub,
            config: Arc::new(config),
            executor: Arc::new(manager),
            started_at: Utc::now(),
        }
    }
}

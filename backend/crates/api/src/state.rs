use std::sync::Arc;

use chrono::{DateTime, Utc};
use execution::executor::ExecutorManager;
use orchestrator::llm::LlmConfig;
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
    pub llm_config: LlmConfig,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new(db: SqlitePool, hub: Arc<Hub>, config: Config) -> Self {
        let mut manager = ExecutorManager::new();

        // 自动发现 PATH 中的 coding agent
        let discovered = execution::discovery::discover_agents();
        for (agent_type, bin_path) in &discovered {
            match *agent_type {
                "claude-code" => {
                    manager.register(Box::new(
                        execution::executor::claude::ClaudeCodeExecutor::new()
                            .with_bin_path(bin_path.clone()),
                    ));
                }
                "opencode" => {
                    manager.register(Box::new(
                        execution::executor::opencode::OpenCodeExecutor::new()
                            .with_bin_path(bin_path.clone()),
                    ));
                }
                _ => {
                    tracing::warn!(agent_type = %agent_type, "unknown discovered agent type");
                }
            }
        }

        if discovered.is_empty() {
            tracing::warn!(
                "no coding agents discovered on PATH (searched for: claude, opencode). \
                 Execute will fail until one is installed."
            );
        }

        Self {
            db,
            hub,
            config: Arc::new(config),
            executor: Arc::new(manager),
            llm_config: LlmConfig::from_env(),
            started_at: Utc::now(),
        }
    }
}

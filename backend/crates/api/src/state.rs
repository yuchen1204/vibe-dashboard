use std::sync::{Arc, RwLock};

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
    pub llm_config: Arc<RwLock<LlmConfig>>,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub async fn new(db: SqlitePool, hub: Arc<Hub>, config: Config) -> Self {
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

        // 从 DB 加载 LLM 配置，环境变量优先级更高（覆盖 DB）
        let llm_config = Arc::new(RwLock::new(load_llm_config(&db).await));

        Self {
            db,
            hub,
            config: Arc::new(config),
            executor: Arc::new(manager),
            llm_config,
            started_at: Utc::now(),
        }
    }
}

async fn load_llm_config(db: &SqlitePool) -> LlmConfig {
    let mut cfg = LlmConfig::from_env();

    // 如果环境变量没有设置，尝试从 DB 加载
    if cfg.api_key.is_empty() {
        match tasks::settings::get_llm_config(db).await {
            Ok((api_base, api_key, model)) => {
                if let Some(key) = api_key.filter(|k| !k.is_empty()) {
                    cfg.api_key = key;
                }
                if let Some(base) = api_base {
                    cfg.api_base = base;
                }
                if let Some(model) = model {
                    cfg.model = model;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load LLM config from DB");
            }
        }
    }

    cfg
}

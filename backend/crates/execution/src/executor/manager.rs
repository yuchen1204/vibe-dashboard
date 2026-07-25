use std::collections::HashMap;

use dashmap::DashMap;
use tokio::sync::mpsc;

use shared::{AppError, AppResult};

use super::{ExecContext, Executor, ExecutorEvent, ExecutorHandle};

/// 统一管理所有 executor 实例 + 运行中的进程
pub struct ExecutorManager {
    /// agent_type → Box<dyn Executor>
    registry: HashMap<&'static str, Box<dyn Executor>>,
    /// job_id → ExecutorHandle（用于取消）
    active: DashMap<String, ExecutorHandle>,
    /// 默认 executor（agent_type 不匹配时降级）
    default: &'static str,
}

impl ExecutorManager {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            active: DashMap::new(),
            default: "claude-code",
        }
    }

    /// 注册一个 executor
    pub fn register(&mut self, executor: Box<dyn Executor>) {
        let name = executor.name();
        tracing::info!(executor = %name, "registered executor");
        self.registry.insert(name, executor);
    }

    /// 批量注册
    pub fn register_all(&mut self, executors: Vec<Box<dyn Executor>>) {
        for ex in executors {
            self.register(ex);
        }
    }

    /// 设置默认 executor
    pub fn set_default(&mut self, name: &'static str) {
        self.default = name;
    }

    /// 获取 executor（找不到时降级到默认）
    pub fn get(&self, agent_type: &str) -> &dyn Executor {
        self.registry
            .get(agent_type)
            .map(|b| b.as_ref())
            .unwrap_or_else(|| {
                tracing::warn!(
                    agent_type = %agent_type,
                    default = %self.default,
                    "unknown agent_type, falling back to default"
                );
                self.registry
                    .get(self.default)
                    .map(|b| b.as_ref())
                    .expect("default executor must be registered")
            })
    }

    /// 启动一个 job
    pub async fn spawn(
        &self,
        agent_type: &str,
        ctx: ExecContext,
    ) -> AppResult<mpsc::UnboundedReceiver<ExecutorEvent>> {
        let executor = self.get(agent_type);
        let (handle, rx) = executor.spawn(ctx).await?;
        self.active.insert(handle.job_id.clone(), handle);
        Ok(rx)
    }

    /// 取消一个运行中的 job
    pub async fn cancel(&self, job_id: &str) -> AppResult<()> {
        if let Some(entry) = self.active.get(job_id) {
            entry.kill().await?;
            drop(entry);
            self.active.remove(job_id);
            tracing::info!(job_id = %job_id, "cancelled active job");
            Ok(())
        } else {
            Err(AppError::NotFound(format!(
                "no active job {job_id} found"
            )))
        }
    }

    /// job 完成后从 active 表移除
    pub fn remove(&self, job_id: &str) {
        self.active.remove(job_id);
    }

    /// 当前活跃 job 数
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// 列出所有活跃 job_id
    pub fn active_jobs(&self) -> Vec<String> {
        self.active.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for ExecutorManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::claude::ClaudeCodeExecutor;

    #[test]
    fn manager_register_and_get() {
        let mut mgr = ExecutorManager::new();
        mgr.register(Box::new(ClaudeCodeExecutor::new()));
        let ex = mgr.get("claude-code");
        assert_eq!(ex.name(), "claude-code");
    }

    #[test]
    fn manager_fallback_to_default() {
        let mut mgr = ExecutorManager::new();
        mgr.register(Box::new(ClaudeCodeExecutor::new()));
        let ex = mgr.get("opencode"); // not registered, falls back to claude-code
        assert_eq!(ex.name(), "claude-code");
    }

    #[test]
    fn manager_active_count() {
        let mgr = ExecutorManager::new();
        assert_eq!(mgr.active_count(), 0);
    }
}
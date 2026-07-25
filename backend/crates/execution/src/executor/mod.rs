use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

pub mod claude;
pub mod manager;
pub mod opencode;

pub use manager::ExecutorManager;

use shared::{AppError, AppResult};

use crate::models::JobStatus;

/// 统一输出事件 - 所有 executor 实现都把异构输出转换成这个
#[derive(Debug, Clone)]
pub enum ExecutorEvent {
    /// 普通日志行（stdout/stderr 文本）
    Log { text: String },
    /// 状态变更（executor 主动报告，如 opencode 解析出 task_completed 事件）
    Status { status: JobStatus },
    /// 保活心跳（用于长时间无输出的任务，前端可显示"仍在跑"）
    Heartbeat,
}

/// 执行上下文 - 一次 agent 执行的全部输入
#[derive(Debug, Clone)]
pub struct ExecContext {
    pub job_id: String,
    pub worktree_path: String,
    pub prompt: String,
    pub envs: HashMap<String, String>,
    pub timeout: Option<Duration>,
}

impl ExecContext {
    pub fn new(job_id: String, worktree_path: String, prompt: String) -> Self {
        Self {
            job_id,
            worktree_path,
            prompt,
            envs: HashMap::new(),
            timeout: None,
        }
    }

    pub fn with_env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.envs.insert(key.into(), val.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// 运行中的 executor 句柄
pub struct ExecutorHandle {
    child: Child,
    pub job_id: String,
}

impl ExecutorHandle {
    /// 等待进程退出，返回 exit code
    pub async fn wait(&mut self) -> AppResult<i32> {
        let status = self
            .child
            .wait()
            .await
            .map_err(|e| AppError::Internal(format!("agent process error: {e}")))?;
        Ok(status.code().unwrap_or(-1))
    }

    /// 强制 kill 进程
    pub async fn kill(&mut self) -> AppResult<()> {
        self.child
            .kill()
            .await
            .map_err(|e| AppError::Internal(format!("failed to kill agent: {e}")))?;
        Ok(())
    }
}

/// Executor 核心 trait - 每个 coding agent 实现这个
#[async_trait]
pub trait Executor: Send + Sync {
    /// executor 名称，用于 registry 注册和日志
    fn name(&self) -> &'static str;

    /// 构造子进程命令（不含 spawn，便于测试断言）
    fn build_command(&self, ctx: &ExecContext) -> Command;

    /// 默认超时时间（None = 无超时）
    fn default_timeout(&self) -> Option<Duration> {
        None
    }

    /// 启动 agent 进程，返回 handle + 事件流 receiver
    async fn spawn(
        &self,
        ctx: ExecContext,
    ) -> AppResult<(ExecutorHandle, mpsc::UnboundedReceiver<ExecutorEvent>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut cmd = self.build_command(&ctx);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // 注入 env
        for (k, v) in &ctx.envs {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Internal(format!("failed to spawn {}: {e}", self.name())))?;

        // 启动超时看门狗
        if let Some(timeout) = ctx.timeout.or(self.default_timeout()) {
            let tx_timeout = tx.clone();
            let job_id = ctx.job_id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;
                // 超时只发 Status，让上层决定是否 kill
                let _ = tx_timeout.send(ExecutorEvent::Heartbeat);
                tracing::warn!(job_id = %job_id, ?timeout, "executor timeout fired");
            });
        }

        // 读 stdout -> Log 事件
        if let Some(stdout) = child.stdout.take() {
            let tx = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(ExecutorEvent::Log { text: format!("{line}\n") }).is_err() {
                        break;
                    }
                }
            });
        }

        // 读 stderr -> Log 事件
        if let Some(stderr) = child.stderr.take() {
            let tx = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx.send(ExecutorEvent::Log { text: format!("{line}\n") }).is_err() {
                        break;
                    }
                }
            });
        }

        Ok((
            ExecutorHandle {
                child,
                job_id: ctx.job_id.clone(),
            },
            rx,
        ))
    }

    /// 取消运行中的 agent（默认实现：直接 kill）
    async fn cancel(&self, handle: &mut ExecutorHandle) -> AppResult<()> {
        handle.kill().await
    }
}

/// 输出解析 trait - 把原始文本行解析成结构化事件
/// 简单的纯文本 executor 可以不实现，默认每行都是 Log
pub trait OutputParser: Send + Sync {
    /// 把一行原始输出解析成 0 或多个事件
    fn parse_line(&self, line: &str) -> Vec<ExecutorEvent>;
}

/// 纯文本解析器 - 每行都是一条 Log
pub struct PlainTextParser;

impl OutputParser for PlainTextParser {
    fn parse_line(&self, line: &str) -> Vec<ExecutorEvent> {
        vec![ExecutorEvent::Log {
            text: format!("{line}\n"),
        }]
    }
}

/// JSON Lines 解析器 - 尝试解析每行为 JSON，失败回退到纯文本
pub struct JsonLinesParser;

impl OutputParser for JsonLinesParser {
    fn parse_line(&self, line: &str) -> Vec<ExecutorEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return vec![];
        }
        // 尝试解析为 JSON object
        if trimmed.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
                // 常见字段：type / event / status
                if let Some(event_type) = v.get("type").and_then(|t| t.as_str()) {
                    match event_type {
                        "status" | "state" => {
                            if let Some(status) = v.get("status").and_then(|s| s.as_str()) {
                                if let Some(job_status) = parse_status(status) {
                                    return vec![ExecutorEvent::Status { status: job_status }];
                                }
                            }
                        }
                        "result" | "completed" => {
                            return vec![ExecutorEvent::Status {
                                status: JobStatus::Success,
                            }];
                        }
                        "error" => {
                            return vec![ExecutorEvent::Status {
                                status: JobStatus::Failed,
                            }];
                        }
                        _ => {}
                    }
                }
                // 无法识别的 JSON 结构，原样输出
                return vec![ExecutorEvent::Log {
                    text: format!("{line}\n"),
                }];
            }
        }
        // 不是 JSON，按纯文本处理
        vec![ExecutorEvent::Log {
            text: format!("{line}\n"),
        }]
    }
}

fn parse_status(s: &str) -> Option<JobStatus> {
    match s {
        "pending" | "queued" => Some(JobStatus::Pending),
        "running" | "active" => Some(JobStatus::Running),
        "success" | "done" | "completed" => Some(JobStatus::Success),
        "failed" | "error" => Some(JobStatus::Failed),
        "cancelled" | "canceled" => Some(JobStatus::Cancelled),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_parser_emits_log() {
        let parser = PlainTextParser;
        let events = parser.parse_line("hello world");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ExecutorEvent::Log { .. }));
    }

    #[test]
    fn json_lines_parser_parses_status_event() {
        let parser = JsonLinesParser;
        let events = parser.parse_line(r#"{"type":"status","status":"running"}"#);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            ExecutorEvent::Status {
                status: JobStatus::Running
            }
        ));
    }

    #[test]
    fn json_lines_parser_parses_result_event() {
        let parser = JsonLinesParser;
        let events = parser.parse_line(r#"{"type":"result"}"#);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            ExecutorEvent::Status {
                status: JobStatus::Success
            }
        ));
    }

    #[test]
    fn json_lines_parser_falls_back_to_text_for_invalid_json() {
        let parser = JsonLinesParser;
        let events = parser.parse_line("not json at all");
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ExecutorEvent::Log { .. }));
    }

    #[test]
    fn json_lines_parser_skips_empty_lines() {
        let parser = JsonLinesParser;
        let events = parser.parse_line("   ");
        assert!(events.is_empty());
    }

    #[test]
    fn exec_context_builder() {
        let ctx = ExecContext::new("job-1".into(), "/tmp/wt".into(), "do thing".into())
            .with_env("API_KEY", "secret")
            .with_timeout(Duration::from_secs(300));
        assert_eq!(ctx.job_id, "job-1");
        assert_eq!(ctx.envs.get("API_KEY").unwrap(), "secret");
        assert_eq!(ctx.timeout, Some(Duration::from_secs(300)));
    }
}
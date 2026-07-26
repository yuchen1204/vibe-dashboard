use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Notify};
use tokio::time::sleep;

pub mod claude;
pub mod manager;
pub mod opencode;

pub use manager::ExecutorManager;

use shared::{AppError, AppResult};

use crate::models::JobStatus;

// ---------- Windows shell wrapper ----------

/// 在 Windows 上，通过 npm 全局安装的 CLI（claude、opencode 等）本质是 `.cmd` 批处理脚本，
/// 不是 PE 可执行文件。`CreateProcessW` 无法直接执行这类脚本，会报 `ERROR_BAD_EXE_FORMAT (193)`。
///
/// 这里的 wrapper 将命令通过 `cmd /C` 包装一层，让 shell 去解析扩展名和 PATHEXT。
#[cfg(windows)]
pub fn prepare_command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(program);
    for a in args {
        cmd.arg(a);
    }
    cmd
}

#[cfg(not(windows))]
pub fn prepare_command(program: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new(program);
    for a in args {
        cmd.arg(a);
    }
    cmd
}

/// 统一输出事件 - 所有 executor 实现都把异构输出转换成这个
#[derive(Debug, Clone)]
pub enum ExecutorEvent {
    /// 普通日志行（stdout/stderr 文本）
    Log { text: String },
    /// 状态变更（executor 主动报告，或退出码兜底）
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
///
/// 只持有取消信号 - 子进程由 spawn 内部启动的 wait task 拥有，
/// wait task 负责等待退出码并发送终态 Status 事件。
pub struct ExecutorHandle {
    cancel: Arc<Notify>,
    pub job_id: String,
}

impl ExecutorHandle {
    /// 请求取消：通知 wait task 中断 select 并 kill 进程
    pub async fn kill(&self) -> AppResult<()> {
        self.cancel.notify_one();
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

    /// 此 executor 的输出解析器，默认按纯文本逐行发 Log
    fn parser(&self) -> Arc<dyn OutputParser> {
        Arc::new(PlainTextParser)
    }

    /// 启动 agent 进程，返回 handle + 事件流 receiver
    ///
    /// 默认实现做了三件事：
    /// 1. 用 parser 解析 stdout/stderr 每一行（而非无脑发 Log）
    /// 2. 起一个 wait task，在子进程退出后按退出码发终态 Status
    /// 3. wait task 同时监听取消信号和超时，任一触发先 kill 再发 Status
    async fn spawn(
        &self,
        ctx: ExecContext,
    ) -> AppResult<(ExecutorHandle, mpsc::UnboundedReceiver<ExecutorEvent>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut cmd = self.build_command(&ctx);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        for (k, v) in &ctx.envs {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Internal(format!("failed to spawn {}: {e}", self.name())))?;

        // 通过 stdin 将 prompt 写入子进程，避免命令行参数截断/转义问题
        // Windows cmd.exe 会重新解析命令行参数，prompt 中的换行、引号、特殊字符
        // 在通过 -p 参数传递时会被截断或误解析。改用 stdin 管道写入则完全绕过
        // cmd.exe 的 tokenizer，同时也规避了 Windows 命令行 32KB 长度上限。
        if let Some(mut stdin) = child.stdin.take() {
            let prompt = ctx.prompt.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(prompt.as_bytes()).await;
                // stdin 在这里 drop，自动关闭管道 = 发送 EOF
            });
        }

        let parser = self.parser();

        // 读 stdout -> 用 parser 解析
        if let Some(stdout) = child.stdout.take() {
            let tx = tx.clone();
            let parser = parser.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    for event in parser.parse_line(&line) {
                        if tx.send(event).is_err() {
                            return;
                        }
                    }
                }
            });
        }

        // 读 stderr -> 用 parser 解析
        if let Some(stderr) = child.stderr.take() {
            let tx = tx.clone();
            let parser = parser.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    for event in parser.parse_line(&line) {
                        if tx.send(event).is_err() {
                            return;
                        }
                    }
                }
            });
        }

        // wait task - 拥有 child 和原始 tx，负责发终态 Status
        let cancel = Arc::new(Notify::new());
        let cancel_wait = cancel.clone();
        let timeout = ctx.timeout.or(self.default_timeout());
        let job_id = ctx.job_id.clone();
        let tx_wait = tx; // 原始 tx 移入 wait task，drop 后 channel 关闭

        tokio::spawn(async move {
            let mut child = child;

            let status = tokio::select! {
                exit = child.wait() => {
                    match exit {
                        Ok(s) => {
                            let code = s.code().unwrap_or(-1);
                            tracing::info!(job_id = %job_id, exit_code = code, "agent exited");
                            if code == 0 { JobStatus::Success } else { JobStatus::Failed }
                        }
                        Err(e) => {
                            tracing::error!(job_id = %job_id, error = %e, "agent wait error");
                            JobStatus::Failed
                        }
                    }
                }
                _ = cancel_wait.notified() => {
                    tracing::info!(job_id = %job_id, "agent cancelled by user");
                    JobStatus::Cancelled
                }
                _ = maybe_timeout(timeout) => {
                    tracing::warn!(job_id = %job_id, "agent timed out, killing");
                    JobStatus::Failed
                }
            };

            // 显式 drop child：kill_on_drop 会在进程仍在跑时发 SIGKILL
            drop(child);

            let _ = tx_wait.send(ExecutorEvent::Status { status });
            // tx_wait drop 后 channel 关闭，rx.recv() 返回 None
        });

        Ok((
            ExecutorHandle {
                cancel,
                job_id: ctx.job_id.clone(),
            },
            rx,
        ))
    }

    /// 取消运行中的 agent（默认实现：通知 wait task）
    async fn cancel(&self, handle: &ExecutorHandle) -> AppResult<()> {
        handle.kill().await
    }
}

/// timeout 为 None 时永远 pending，否则 sleep
async fn maybe_timeout(timeout: Option<Duration>) {
    match timeout {
        Some(t) => sleep(t).await,
        None => std::future::pending::<()>().await,
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

    // ---------- 退出码兜底集成测试 ----------

    struct ExitZeroExecutor;

    #[async_trait]
    impl Executor for ExitZeroExecutor {
        fn name(&self) -> &'static str {
            "exit-zero"
        }
        fn build_command(&self, _ctx: &ExecContext) -> Command {
            if cfg!(windows) {
                let mut cmd = Command::new("cmd");
                cmd.args(["/c", "exit", "0"]);
                cmd
            } else {
                Command::new("true")
            }
        }
    }

    struct ExitNonZeroExecutor;

    #[async_trait]
    impl Executor for ExitNonZeroExecutor {
        fn name(&self) -> &'static str {
            "exit-nonzero"
        }
        fn build_command(&self, _ctx: &ExecContext) -> Command {
            if cfg!(windows) {
                let mut cmd = Command::new("cmd");
                cmd.args(["/c", "exit", "1"]);
                cmd
            } else {
                Command::new("false")
            }
        }
    }

    #[tokio::test]
    async fn spawn_reports_success_on_exit_zero() {
        let ex = ExitZeroExecutor;
        let ctx = ExecContext::new("j1".into(), ".".into(), "test".into());
        let (_handle, mut rx) = ex.spawn(ctx).await.unwrap();

        let mut got_status = false;
        while let Some(event) = rx.recv().await {
            if let ExecutorEvent::Status { status } = event {
                assert_eq!(status, JobStatus::Success);
                got_status = true;
                break;
            }
        }
        assert!(got_status, "should receive Status::Success for exit 0");
    }

    #[tokio::test]
    async fn spawn_reports_failed_on_exit_nonzero() {
        let ex = ExitNonZeroExecutor;
        let ctx = ExecContext::new("j2".into(), ".".into(), "test".into());
        let (_handle, mut rx) = ex.spawn(ctx).await.unwrap();

        let mut got_status = false;
        while let Some(event) = rx.recv().await {
            if let ExecutorEvent::Status { status } = event {
                assert_eq!(status, JobStatus::Failed);
                got_status = true;
                break;
            }
        }
        assert!(got_status, "should receive Status::Failed for exit != 0");
    }
}

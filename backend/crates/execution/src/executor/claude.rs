use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

use super::{ExecContext, Executor, OutputParser, PlainTextParser};

/// Claude Code executor
///
/// 命令构造: `claude -p` [--model {model}] [--permission-mode bypassPermissions]
/// prompt 通过 stdin 传入，避免 cmd.exe 命令行截断/转义问题
/// 输出解析: 纯文本流（逐行 stdout/stderr → Log 事件）
/// 默认超时: 5 分钟
pub struct ClaudeCodeExecutor {
    pub model: Option<String>,
    pub bin_path: String,
    /// 自动批准文件写入和命令执行（对应 --permission-mode bypassPermissions）
    /// 仅在 worktree 隔离的自动化场景下启用（CI/CD、批量重构、无人值守）
    pub auto_approve: bool,
}

impl Default for ClaudeCodeExecutor {
    fn default() -> Self {
        Self {
            model: None,
            bin_path: "claude".into(),
            auto_approve: true,
        }
    }
}

impl ClaudeCodeExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_bin_path(mut self, path: impl Into<String>) -> Self {
        self.bin_path = path.into();
        self
    }

    pub fn with_auto_approve(mut self, yes: bool) -> Self {
        self.auto_approve = yes;
        self
    }
}

#[async_trait::async_trait]
impl Executor for ClaudeCodeExecutor {
    fn name(&self) -> &'static str {
        "claude-code"
    }

    fn build_command(&self, ctx: &ExecContext) -> Command {
        let mut args: Vec<&str> = Vec::new();
        if let Some(model) = &self.model {
            args.push("--model");
            args.push(model);
        }
        if self.auto_approve {
            args.push("--permission-mode");
            args.push("bypassPermissions");
        }
        args.push("-p"); // 不带 prompt 文本，由 spawn 从 stdin 写入
        let mut cmd = super::prepare_command(&self.bin_path, &args);
        cmd.current_dir(&ctx.worktree_path);
        cmd
    }

    fn default_timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(300)) // 5 分钟
    }

    fn parser(&self) -> Arc<dyn OutputParser> {
        Arc::new(PlainTextParser)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_build_command_default() {
        let ex = ClaudeCodeExecutor::new();
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let program = cmd.as_std().get_program().to_str().unwrap().to_string();
        if cfg!(windows) {
            assert_eq!(program, "cmd", "on Windows should wrap with cmd /C");
        } else {
            assert_eq!(program, "claude");
        }
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        // prompt 不再作为命令行参数传递，而是通过 stdin 写入
        assert!(args.contains(&"-p".to_string()), "should have -p flag: {args:?}");
        assert!(!args.contains(&"do it".to_string()), "prompt should NOT be in args, it goes via stdin: {args:?}");
    }

    #[test]
    fn claude_build_command_includes_auto_approve() {
        let ex = ClaudeCodeExecutor::new().with_auto_approve(true);
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        assert!(args.contains(&"--permission-mode".to_string()), "auto_approve should add --permission-mode flag: {args:?}");
        assert!(args.contains(&"bypassPermissions".to_string()), "auto_approve should set bypassPermissions: {args:?}");
    }

    #[test]
    fn claude_build_command_with_model() {
        let ex = ClaudeCodeExecutor::new().with_model("claude-sonnet-5");
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-5".to_string()));
    }

    #[test]
    fn claude_build_command_skips_auto_approve_when_false() {
        let ex = ClaudeCodeExecutor::new().with_auto_approve(false);
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        assert!(!args.contains(&"--permission-mode".to_string()));
    }
}
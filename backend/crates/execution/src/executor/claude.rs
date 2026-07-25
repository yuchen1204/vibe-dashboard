use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

use super::{ExecContext, Executor, OutputParser, PlainTextParser};

/// Claude Code executor
///
/// 命令构造: `claude -p "{prompt}" [--model {model}] [-y]`
/// 输出解析: 纯文本流（逐行 stdout/stderr → Log 事件）
/// 默认超时: 5 分钟
pub struct ClaudeCodeExecutor {
    pub model: Option<String>,
    pub bin_path: String,
    /// 自动批准文件写入和命令执行（对应 claude -y 标志）
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
        let mut args = vec!["-p", &ctx.prompt];
        if let Some(model) = &self.model {
            args.push("--model");
            args.push(model);
        }
        if self.auto_approve {
            args.push("-y");
        }
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
    }

    #[test]
    fn claude_build_command_includes_auto_approve() {
        let ex = ClaudeCodeExecutor::new().with_auto_approve(true);
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        assert!(args.contains(&"-y".to_string()), "auto_approve should add -y flag: {args:?}");
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
        assert!(!args.contains(&"-y".to_string()));
    }
}
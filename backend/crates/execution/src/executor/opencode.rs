use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

use super::{ExecContext, Executor, JsonLinesParser, OutputParser};

/// OpenCode executor
///
/// 命令构造: `opencode run [--model {model}]`  + prompt 通过 stdin 传入
/// 输出解析: JSON Lines（尝试解析结构化事件，回退到纯文本）
/// 默认超时: 无（用户不主动 cancel 就一直跑）
pub struct OpenCodeExecutor {
    pub model: Option<String>,
    pub bin_path: String,
}

impl Default for OpenCodeExecutor {
    fn default() -> Self {
        Self {
            model: None,
            bin_path: "opencode".into(),
        }
    }
}

impl OpenCodeExecutor {
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
}

#[async_trait::async_trait]
impl Executor for OpenCodeExecutor {
    fn name(&self) -> &'static str {
        "opencode"
    }

    fn build_command(&self, ctx: &ExecContext) -> Command {
        let mut args = vec!["--task"]; // 不带 prompt 文本，由 spawn 从 stdin 写入
        if let Some(model) = &self.model {
            args.push("--model");
            args.push(model);
        }
        let mut cmd = super::prepare_command(&self.bin_path, &args);
        cmd.current_dir(&ctx.worktree_path);
        cmd
    }

    /// OpenCode 默认无超时，用户不 cancel 就一直跑
    fn default_timeout(&self) -> Option<Duration> {
        None
    }

    /// OpenCode 输出 JSON Lines，使用 JsonLinesParser 解析结构化事件
    fn parser(&self) -> Arc<dyn OutputParser> {
        Arc::new(JsonLinesParser)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_build_command_default() {
        let ex = OpenCodeExecutor::new();
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let program = cmd.as_std().get_program().to_str().unwrap().to_string();
        if cfg!(windows) {
            assert_eq!(program, "cmd", "on Windows should wrap with cmd /C");
        } else {
            assert_eq!(program, "opencode");
        }
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        assert!(args.contains(&"--task".to_string()));
        assert!(!args.contains(&"do it".to_string()), "prompt should NOT be in args, it goes via stdin: {args:?}");
    }

    #[test]
    fn opencode_build_command_with_model() {
        let ex = OpenCodeExecutor::new().with_model("gpt-4");
        let ctx = ExecContext::new("j1".into(), "/tmp/wt".into(), "do it".into());
        let cmd = ex.build_command(&ctx);
        let args: Vec<_> = cmd.as_std().get_args().map(|a| a.to_str().unwrap().to_string()).collect();
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"gpt-4".to_string()));
    }
}
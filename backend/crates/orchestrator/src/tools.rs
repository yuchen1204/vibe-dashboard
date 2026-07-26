use std::sync::Arc;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::llm::ToolDefinition;
use crate::llm::ToolFunction;

/// 工具执行上下文 - 包含需要执行 todo 时所需的额外依赖
pub struct ToolContext {
    pub executor: Option<Arc<execution::executor::ExecutorManager>>,
    pub notifier: Option<Arc<dyn execution::dispatch::JobNotifier>>,
}

impl ToolContext {
    pub fn new() -> Self {
        Self {
            executor: None,
            notifier: None,
        }
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 工具执行结果
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub name: String,
    pub result: String,
}

/// 执行一个工具调用
pub async fn execute_tool(
    pool: &SqlitePool,
    workspace_id: &str,
    name: &str,
    arguments: &Value,
    ctx: &ToolContext,
) -> Result<String, String> {
    match name {
        "list_targets" => cmd_list_targets(pool, workspace_id, arguments).await,
        "list_todos" => cmd_list_todos(pool, workspace_id, arguments).await,
        "create_todo" => cmd_create_todo(pool, arguments).await,
        "execute_todo" => cmd_execute_todo(pool, arguments, ctx).await,
        "get_job_result" => cmd_get_job_result(pool, arguments).await,
        "read_file" => cmd_read_file(pool, workspace_id, arguments).await,
        "grep_files" => cmd_grep_files(pool, workspace_id, arguments).await,
        _ => Err(format!("unknown tool: {name}")),
    }
}

/// 所有可用工具的定义
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "list_targets".to_string(),
                description: "List all targets (milestones) in the current workspace".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "list_todos".to_string(),
                description: "List todos (tasks), optionally filtered by target_id".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "target_id": {
                            "type": "string",
                            "description": "Optional target ID to filter by"
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "create_todo".to_string(),
                description: "Create a new todo (task) under a target".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "target_id": {
                            "type": "string",
                            "description": "Target ID to create the todo under"
                        },
                        "title": {
                            "type": "string",
                            "description": "Title of the todo"
                        },
                        "description": {
                            "type": "string",
                            "description": "Optional description"
                        }
                    },
                    "required": ["target_id", "title"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "execute_todo".to_string(),
                description: "Execute a todo by dispatching a coding agent. You can provide a custom prompt to guide what the coding agent should do. Use read_file and grep_files first to understand the codebase, then craft a detailed prompt.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "todo_id": {
                            "type": "string",
                            "description": "ID of the todo to execute"
                        },
                        "prompt": {
                            "type": "string",
                            "description": "Optional custom prompt for the coding agent. If omitted, the todo title and description are used as the prompt."
                        }
                    },
                    "required": ["todo_id"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "get_job_result".to_string(),
                description: "Get the result of a previously executed job".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "job_id": {
                            "type": "string",
                            "description": "Job ID to query"
                        }
                    },
                    "required": ["job_id"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "read_file".to_string(),
                description: "Read the contents of a file in the workspace. Can specify start_line to read from a specific line (1-indexed).".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative path to the file from the workspace root (e.g. src/main.rs, Cargo.toml)"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional: line number to start reading from (1-indexed, default: 1)"
                        },
                        "max_lines": {
                            "type": "integer",
                            "description": "Optional: maximum number of lines to read (default: 200)"
                        }
                    },
                    "required": ["path"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "grep_files".to_string(),
                description: "Search for a pattern in files within the workspace. Returns matching lines with file paths.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "The search pattern (supports regex)"
                        },
                        "glob": {
                            "type": "string",
                            "description": "Optional: file glob pattern to filter by (e.g. **/*.rs, **/*.tsx). If omitted, searches all files."
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Optional: maximum number of results to return (default: 50)"
                        }
                    },
                    "required": ["pattern"]
                }),
            },
        },
    ]
}

async fn cmd_list_targets(
    pool: &SqlitePool,
    workspace_id: &str,
    _args: &Value,
) -> Result<String, String> {
    let targets = tasks::repo::list_targets(pool, workspace_id)
        .await
        .map_err(|e| e.to_string())?;
    if targets.is_empty() {
        return Ok("No targets found.".to_string());
    }
    let lines: Vec<String> = targets
        .iter()
        .map(|t| format!("- {} (id: {}, status: {})", t.title, t.id, t.status))
        .collect();
    Ok(lines.join("\n"))
}

async fn cmd_list_todos(
    pool: &SqlitePool,
    workspace_id: &str,
    args: &Value,
) -> Result<String, String> {
    let target_id = args.get("target_id").and_then(|v| v.as_str());

    let todos = if let Some(tid) = target_id {
        tasks::repo::list_todos_by_target(pool, tid)
            .await
            .map_err(|e| e.to_string())?
    } else {
        tasks::repo::list_todos_by_workspace(pool, workspace_id)
            .await
            .map_err(|e| e.to_string())?
    };

    if todos.is_empty() {
        return Ok("No todos found.".to_string());
    }
    let lines: Vec<String> = todos
        .iter()
        .map(|t| format!("- {} (id: {}, status: {})", t.title, t.id, t.status))
        .collect();
    Ok(lines.join("\n"))
}

async fn cmd_create_todo(pool: &SqlitePool, args: &Value) -> Result<String, String> {
    let target_id = args
        .get("target_id")
        .and_then(|v| v.as_str())
        .ok_or("missing target_id")?;
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or("missing title")?;
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let todo = tasks::repo::create_todo(
        pool,
        target_id,
        tasks::CreateTodo {
            title: title.to_string(),
            description: description.to_string(),
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(format!(
        "Created todo: {} (id: {}, status: {})",
        todo.title, todo.id, todo.status
    ))
}

async fn cmd_execute_todo(pool: &SqlitePool, args: &Value, ctx: &ToolContext) -> Result<String, String> {
    let todo_id = args
        .get("todo_id")
        .and_then(|v| v.as_str())
        .ok_or("missing todo_id")?;

    let custom_prompt = args.get("prompt").and_then(|v| v.as_str());

    let (executor, notifier) = match (&ctx.executor, &ctx.notifier) {
        (Some(exec), Some(notif)) => (exec.clone(), notif.clone()),
        _ => {
            // Fallback: no executor configured, just create a job record (legacy path)
            let todo = tasks::repo::get_todo(pool, todo_id)
                .await
                .map_err(|e| e.to_string())?;

            let default_prompt = format!(
                "Execute the following task:\n\nTitle: {}\nDescription: {}\n\nPlease implement this change.",
                todo.title, todo.description
            );
            let prompt = custom_prompt.unwrap_or(&default_prompt);

            let job = execution::repo::create_job(pool, todo_id, prompt, "claude-code")
                .await
                .map_err(|e| e.to_string())?;

            return Ok(format!(
                "Job created (id: {}, status: {}). The todo is queued for execution.",
                job.id, job.status
            ));
        }
    };

    let todo = tasks::repo::get_todo(pool, todo_id)
        .await
        .map_err(|e| e.to_string())?;

    // If a custom prompt is provided, override the todo's description for this execution
    let effective_prompt = if let Some(p) = custom_prompt {
        p.to_string()
    } else {
        format!(
            "Task: {}. Description: {}. Implement this change in the codebase. Create or modify files as needed. Do not ask for clarification.",
            todo.title, todo.description
        )
    };

    let job = execution::dispatch::execute_todo(
        pool, executor, notifier, todo_id, "claude-code", Some(&effective_prompt),
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(format!(
        "Job created and dispatched (id: {}, status: {}). A coding agent is now working on this task.",
        job.id, job.status
    ))
}

async fn cmd_get_job_result(pool: &SqlitePool, args: &Value) -> Result<String, String> {
    let job_id = args
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or("missing job_id")?;

    let job = execution::repo::get_job(pool, job_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "Job {}: status={}, started_at={:?}, finished_at={:?}, output_length={}",
        job.id,
        job.status,
        job.started_at,
        job.finished_at,
        job.output.len()
    ))
}

/// read_file - 读取工作区中的文件内容
async fn cmd_read_file(
    pool: &SqlitePool,
    workspace_id: &str,
    args: &Value,
) -> Result<String, String> {
    let rel_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("missing path")?;
    let start_line = args
        .get("start_line")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(1);
    let max_lines = args
        .get("max_lines")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;

    // 获取 workspace 实际路径
    let ws = tasks::repo::get_workspace(pool, workspace_id)
        .await
        .map_err(|e| format!("failed to get workspace: {e}"))?;

    let full_path = std::path::Path::new(&ws.workspace.path).join(rel_path);

    // 安全校验：确保路径在 workspace 内，防止路径穿越
    let canonical_ws = std::path::Path::new(&ws.workspace.path)
        .canonicalize()
        .map_err(|e| format!("failed to canonicalize workspace path: {e}"))?;
    let canonical_target = full_path
        .canonicalize()
        .map_err(|e| format!("file not found or inaccessible: {e}"))?;
    if !canonical_target.starts_with(&canonical_ws) {
        return Err("path is outside the workspace directory".to_string());
    }

    // 读取文件
    let content = tokio::fs::read_to_string(&full_path)
        .await
        .map_err(|e| format!("failed to read file: {e}"))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    // start_line 是 1-indexed，转为 0-indexed
    let start = if start_line > 0 { start_line - 1 } else { 0 };
    let start = start.min(total);

    let end = start + max_lines;
    let end = end.min(total);

    let snippet = lines[start..end].join("\n");
    let actual_start = start + 1; // 显示给用户时转回 1-indexed

    if start == 0 && end == total {
        Ok(format!("```\n{}\n```\n\n*{} lines total*", content, total))
    } else {
        Ok(format!(
            "```\n{}\n```\n\n*Showing lines {}-{} of {}*",
            snippet, actual_start, end, total
        ))
    }
}

/// grep_files - 在工作区中搜索文件内容
async fn cmd_grep_files(
    pool: &SqlitePool,
    workspace_id: &str,
    args: &Value,
) -> Result<String, String> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or("missing pattern")?;
    let glob = args.get("glob").and_then(|v| v.as_str());
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    // 获取 workspace 实际路径
    let ws = tasks::repo::get_workspace(pool, workspace_id)
        .await
        .map_err(|e| format!("failed to get workspace: {e}"))?;

    let ws_path = std::path::Path::new(&ws.workspace.path);

    // 编译正则
    let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;

    // 构建文件匹配器
    let glob_matcher = glob.map(|g| glob::Pattern::new(g).ok());

    // 使用 walkdir 遍历文件
    let mut results: Vec<String> = Vec::new();
    let mut count = 0;

    for entry in walkdir::WalkDir::new(ws_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // 跳过 .git 目录和 node_modules
            if e.file_type().is_dir() {
                return name != ".git" && name != "node_modules" && name != "target";
            }
            true
        })
    {
        if count >= max_results {
            break;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        // 检查 glob 匹配
        if let Some(Some(ref m)) = glob_matcher.as_ref() {
            if !m.matches(entry.path().to_string_lossy().as_ref()) {
                continue;
            }
        }

        // 读取文件内容
        let content = match tokio::fs::read_to_string(entry.path()).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        // 搜索匹配行
        let rel_path = entry
            .path()
            .strip_prefix(ws_path)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        for (line_no, line) in content.lines().enumerate() {
            if count >= max_results {
                break;
            }
            if re.is_match(line) {
                results.push(format!("{}:{}: {}", rel_path, line_no + 1, line.trim()));
                count += 1;
            }
        }
    }

    if results.is_empty() {
        return Ok("No matches found.".to_string());
    }

    let output = results.join("\n");
    Ok(format!(
        "Found {} matches:\n```\n{}\n```",
        results.len(),
        output
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_count() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 7);
    }

    #[test]
    fn tool_definitions_have_valid_json() {
        let defs = tool_definitions();
        for tool in &defs {
            let json = serde_json::to_string(tool).unwrap();
            assert!(json.contains(&tool.function.name));
        }
    }
}
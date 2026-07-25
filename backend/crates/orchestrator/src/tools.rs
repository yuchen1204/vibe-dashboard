use serde_json::Value;
use sqlx::SqlitePool;

use crate::llm::ToolDefinition;
use crate::llm::ToolFunction;

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
) -> Result<String, String> {
    match name {
        "list_targets" => cmd_list_targets(pool, workspace_id, arguments).await,
        "list_todos" => cmd_list_todos(pool, workspace_id, arguments).await,
        "create_todo" => cmd_create_todo(pool, arguments).await,
        "execute_todo" => cmd_execute_todo(pool, arguments).await,
        "get_job_result" => cmd_get_job_result(pool, arguments).await,
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
                description: "Execute a todo by dispatching a coding agent. Returns the job ID.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "todo_id": {
                            "type": "string",
                            "description": "ID of the todo to execute"
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

async fn cmd_execute_todo(pool: &SqlitePool, args: &Value) -> Result<String, String> {
    let todo_id = args
        .get("todo_id")
        .and_then(|v| v.as_str())
        .ok_or("missing todo_id")?;

    // We can't actually call the API route from here, so we create a job
    // and let the caller poll for results
    let todo = tasks::repo::get_todo(pool, todo_id)
        .await
        .map_err(|e| e.to_string())?;

    let prompt = format!(
        "Execute the following task:\n\nTitle: {}\nDescription: {}\n\nPlease implement this change.",
        todo.title, todo.description
    );

    let job = execution::repo::create_job(pool, todo_id, &prompt, "claude-code")
        .await
        .map_err(|e| e.to_string())?;

    Ok(format!(
        "Job created (id: {}, status: {}). The todo is queued for execution.",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_count() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 5);
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
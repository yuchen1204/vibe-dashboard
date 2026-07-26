use sqlx::SqlitePool;

use crate::llm::{self, ChatCompletionRequest, LlmConfig};
use crate::session::{ChatMessage, Role, Session};
use crate::tools::{self, ToolContext};

/// 编排 Agent 单次对话的结果
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub response: String,
    pub tool_calls: Vec<ToolCallInfo>,
}

#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: String,
}

/// 运行编排 Agent 一圈（用户输入 → LLM → 工具调用 → LLM → 回复）
pub async fn run_agent(
    session: &mut Session,
    pool: &SqlitePool,
    config: &LlmConfig,
    tool_ctx: &ToolContext,
) -> Result<AgentResponse, String> {
    if !config.is_configured() {
        // 无 LLM 配置时返回模拟回复
        return Ok(AgentResponse {
            response: "LLM 未配置。请设置 VIBE_LLM_API_KEY 环境变量来启用 AI 编排助手。".to_string(),
            tool_calls: vec![],
        });
    }

    let tools = tools::tool_definitions();

    // 确保 system prompt 在开头
    if session.messages.is_empty() || session.messages[0].role != Role::System {
        let system_prompt = r#"你是一个 Vibe Dashboard 的 AI 编排助手。你的工作是帮助用户管理开发任务。

你有以下能力：
1. 查看当前 workspace 的 targets（里程碑）和 todos（任务）
2. 创建新的 todo
3. 阅读 workspace 中的文件内容（read_file），搜索文件内容（grep_files）
4. 执行 todo（调度 coding agent 去实现，可以自定义 prompt 来指导 coding agent）
5. 查询执行结果

=== 推荐工作流 ===

当你接到一个任务时，建议按以下步骤进行：

1. 理解项目结构 — 先用 read_file 阅读关键文件（如 Cargo.toml、package.json、src/lib.rs、src/main.rs 等），了解项目架构
2. 分析现有代码 — 用 grep_files 搜索相关代码，找到需要修改的位置，理解现有实现
3. 制定计划 — 根据分析结果创建 todo 或直接规划执行方案
4. 执行时 — 用之前阅读代码获得的信息，编写详细的 prompt 传给 coding agent

=== 如何编写 coding agent 的 prompt ===

当你调用 execute_todo 的 prompt 参数时，注意以下几点：

- 必须包含具体文件路径，不要只说"修改配置文件"，要说"修改 src/config.rs 的第 42-50 行"
- 引用现有代码模式，让 coding agent 知道你看到了什么，它可以直接在此基础上修改
- 说明为什么要改，而不仅是什么要改——提供业务上下文
- 提供上下文但不要过长，coding agent 也有上下文限制
- 如果需要修改多个文件，在 prompt 中逐一列出
- 示例格式：
  "在 src/api/routes.rs 中，find_user 函数缺少错误处理。
  当前代码（第 85 行）：
    let user = db.find_user(id).unwrap();
  需要改为：
    let user = db.find_user(id).map_err(|e| AppError::NotFound(e.to_string()))?;
  请做这个修改，并添加相应的测试。"

=== 上下文管理 ===

- 读文件时尽量用 max_lines 限制行数，避免把大文件全部塞入上下文
- 如果文件很大，先读开头部分了解结构，再用 grep_files 定位关键代码
- 不需要在最终回复中重复所有文件内容，给用户总结即可
- 每次调用只做一个工具，等待结果后再做下一步

请用中文回答。每次只调用一个工具，不要同时调用多个。
在调用工具后，根据工具返回的结果给用户一个清晰的总结。"#.to_string();
        session.messages.insert(0, ChatMessage::system(system_prompt));
    }

    let api_messages = llm::to_api_messages(&session.messages_for_llm());

    let request = ChatCompletionRequest {
        model: config.model.clone(),
        messages: api_messages,
        tools: Some(tools.clone()),
        tool_choice: Some(serde_json::json!("auto")),
        max_tokens: config.max_tokens,
        temperature: config.temperature,
    };

    let response = llm::chat_completion(config, request)
        .await
        .map_err(|e| format!("LLM error: {e}"))?;

    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or("no response from LLM")?;

    let content = choice.message.content.clone();
    let tool_calls = choice.message.tool_calls.clone();

    // 记录 assistant 消息
    let mut assistant_msg = ChatMessage::assistant(content.clone());
    if let Some(ref calls) = tool_calls {
        assistant_msg = assistant_msg.with_tool_calls(calls.clone());
    }
    session.add(assistant_msg);

    // 处理工具调用
    let mut tool_infos = Vec::new();

    if let Some(calls) = tool_calls {
        for call in calls {
            // 解析参数
            let args: serde_json::Value = serde_json::from_str(&call.function.arguments)
                .unwrap_or(serde_json::json!({"error": "invalid arguments"}));

            // 执行工具
            let result = tools::execute_tool(pool, &session.workspace_id, &call.function.name, &args, tool_ctx)
                .await
                .unwrap_or_else(|e| format!("Error: {e}"));

            tool_infos.push(ToolCallInfo {
                name: call.function.name.clone(),
                arguments: args,
                result: result.clone(),
            });

            // 记录 tool 结果到消息历史
            session.add(ChatMessage::tool(
                &result,
                &call.id,
                &call.function.name,
            ));
        }

        // 有工具调用 → 再调一次 LLM 生成最终回复
        let api_messages = llm::to_api_messages(&session.messages_for_llm());
        let follow_up_request = ChatCompletionRequest {
            model: config.model.clone(),
            messages: api_messages,
            tools: None,
            tool_choice: None,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
        };

        let follow_up = llm::chat_completion(config, follow_up_request)
            .await
            .map_err(|e| format!("LLM follow-up error: {e}"))?;

        let final_content = follow_up
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        session.add(ChatMessage::assistant(&final_content));

        return Ok(AgentResponse {
            response: final_content,
            tool_calls: tool_infos,
        });
    }

    Ok(AgentResponse {
        response: content,
        tool_calls: tool_infos,
    })
}

/// 非 LLM 模式下的简单回复（用于 mock 测试）
pub async fn run_agent_mock(
    session: &mut Session,
    _pool: &SqlitePool,
) -> AgentResponse {
    // 检查用户消息中是否包含"列出"或"target"关键词
    let last_msg = session.messages.last().map(|m| m.content.clone()).unwrap_or_default();

    if last_msg.contains("target") || last_msg.contains("列出") {
        // 模拟 list_targets
        let result = "模拟 target 列表:\n- 重构数据库 (id: mock-1, status: active)\n- 添加测试 (id: mock-2, status: planned)".to_string();
        let result_clone = result.clone();
        session.add(ChatMessage::assistant(&result));
        AgentResponse {
            response: result,
            tool_calls: vec![ToolCallInfo {
                name: "list_targets".to_string(),
                arguments: serde_json::json!({}),
                result: result_clone,
            }],
        }
    } else {
        let response = format!("已收到你的消息。你说: {last_msg}\n\n这是模拟回复（LLM 未配置时使用）。要启用 AI 编排，请设置 VIBE_LLM_API_KEY 环境变量。");
        session.add(ChatMessage::assistant(&response));
        AgentResponse {
            response,
            tool_calls: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn agent_mock_responds() {
        let mut session = Session::new("ws-1");
        session.add(ChatMessage::user("帮我列出所有 target"));
        // We can't create a real pool in unit test, so we test the mock path
        // by checking the response is not empty
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let resp = run_agent_mock(&mut session, &pool).await;
        assert!(!resp.response.is_empty());
    }

    #[tokio::test]
    async fn agent_mock_echoes() {
        let mut session = Session::new("ws-1");
        session.add(ChatMessage::user("你好"));
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let resp = run_agent_mock(&mut session, &pool).await;
        assert!(resp.response.contains("你好"));
    }
}
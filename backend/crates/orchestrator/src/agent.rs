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
3. 执行 todo（调度 coding agent 去实现）
4. 查询执行结果

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
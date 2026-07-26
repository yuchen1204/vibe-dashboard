use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 计算重试延迟：1s, 3s, 5s, 7s, 10s, 13s, 16s, 19s, 22s, 25s
fn retry_delay(attempt: usize) -> Duration {
    let secs: u64 = if attempt < 4 {
        (2 * attempt + 1) as u64
    } else {
        7 + (attempt as u64 - 3) * 3
    };
    Duration::from_secs(secs)
}

const MAX_RETRIES: usize = 10;

/// 判断错误是否可重试（仅网络错误和 5xx 服务端错误）
fn is_retryable(err: &LlmError) -> bool {
    match err {
        LlmError::HttpError(_) => true,
        LlmError::ApiError(code, _) => *code >= 500,
        LlmError::DeserializeError(_) => false,
        LlmError::NotConfigured => false,
    }
}

/// LLM 配置
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_base: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o".to_string(),
            max_tokens: 4096,
            temperature: 0.0,
        }
    }
}

impl LlmConfig {
    pub fn from_env() -> Self {
        Self {
            api_base: std::env::var("VIBE_LLM_API_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            api_key: std::env::var("VIBE_LLM_API_KEY").unwrap_or_default(),
            model: std::env::var("VIBE_LLM_MODEL")
                .unwrap_or_else(|_| "gpt-4o".to_string()),
            max_tokens: 4096,
            temperature: 0.0,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }
}

/// OpenAI 兼容的 chat/completions 请求
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// 工具定义（OpenAI function calling 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// LLM 响应
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<Choice>,
    #[allow(dead_code)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub message: ChatCompletionMessage,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 发送 chat/completions 请求（带自动重试）
pub async fn chat_completion(
    config: &LlmConfig,
    request: ChatCompletionRequest,
) -> Result<ChatCompletionResponse, LlmError> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", config.api_base.trim_end_matches('/'));

    let mut last_err = LlmError::HttpError("max retries exceeded".to_string());

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = retry_delay(attempt);
            tracing::warn!(
                attempt, delay_ms = delay.as_millis(),
                "LLM API call failed, retrying"
            );
            tokio::time::sleep(delay).await;
        }

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last_err = LlmError::HttpError(e.to_string());
                if !is_retryable(&last_err) {
                    return Err(last_err);
                }
                continue;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            last_err = LlmError::ApiError(status, body);
            if !is_retryable(&last_err) {
                return Err(last_err);
            }
            continue;
        }

        match resp.json().await {
            Ok(data) => return Ok(data),
            Err(e) => {
                last_err = LlmError::DeserializeError(e.to_string());
                // 反序列化错误不可重试
                return Err(last_err);
            }
        }
    }

    Err(last_err)
}

/// 把内部 ChatMessage 转为 API 格式
pub fn to_api_messages(messages: &[crate::ChatMessage]) -> Vec<ChatCompletionMessage> {
    messages
        .iter()
        .map(|m| {
            let role = match m.role {
                crate::Role::System => "system",
                crate::Role::User => "user",
                crate::Role::Assistant => "assistant",
                crate::Role::Tool => "tool",
            }
            .to_string();

            ChatCompletionMessage {
                role,
                content: Some(m.content.clone()),
                tool_calls: m.tool_calls.clone(),
                tool_call_id: m.tool_call_id.clone(),
                name: m.name.clone(),
            }
        })
        .collect()
}

#[derive(Debug)]
pub enum LlmError {
    HttpError(String),
    ApiError(u16, String),
    DeserializeError(String),
    NotConfigured,
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::HttpError(e) => write!(f, "HTTP error: {e}"),
            LlmError::ApiError(code, body) => write!(f, "API error {code}: {body}"),
            LlmError::DeserializeError(e) => write!(f, "deserialize error: {e}"),
            LlmError::NotConfigured => write!(f, "LLM not configured (VIBE_LLM_API_KEY)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatMessage;

    #[test]
    fn llm_config_default() {
        let cfg = LlmConfig::default();
        assert_eq!(cfg.model, "gpt-4o");
        assert!(!cfg.is_configured());
    }

    #[test]
    fn to_api_messages_converts_roles() {
        let msgs = vec![
            ChatMessage::system("system prompt"),
            ChatMessage::user("user msg"),
            ChatMessage::assistant("assistant msg"),
            ChatMessage::tool("tool result", "call-1", "my_tool"),
        ];
        let api = to_api_messages(&msgs);
        assert_eq!(api.len(), 4);
        assert_eq!(api[0].role, "system");
        assert_eq!(api[1].role, "user");
        assert_eq!(api[2].role, "assistant");
        assert_eq!(api[3].role, "tool");
        assert_eq!(api[0].content.as_deref(), Some("system prompt"));
        assert_eq!(api[3].tool_call_id.as_deref(), Some("call-1"));
    }

    #[test]
    fn tool_definition_serialization() {
        let tool = ToolDefinition {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "arg1": {"type": "string"}
                    },
                    "required": ["arg1"]
                }),
            },
        };
        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("test_tool"));
        assert!(json.contains("function"));
    }
}
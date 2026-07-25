use serde::{Deserialize, Serialize};

use crate::llm::ToolCall;

/// 对话角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// 单条聊天消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: Some(name.into()),
        }
    }

    pub fn with_tool_calls(mut self, calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(calls);
        self
    }
}

/// 会话 — 管理消息历史
pub struct Session {
    pub messages: Vec<ChatMessage>,
    pub max_messages: usize,
    pub workspace_id: String,
}

impl Session {
    pub fn new(workspace_id: impl Into<String>) -> Self {
        Self {
            messages: Vec::new(),
            max_messages: 50,
            workspace_id: workspace_id.into(),
        }
    }

    pub fn add(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
        if self.messages.len() > self.max_messages {
            // 保留 system prompt + 最近的消息
            let system_msgs: Vec<_> = self.messages.iter().filter(|m| m.role == Role::System).cloned().collect();
            let keep_count = self.max_messages.saturating_sub(system_msgs.len());
            let tail_start = self.messages.len().saturating_sub(keep_count).max(system_msgs.len());
            let tail = self.messages.split_off(tail_start);
            self.messages = system_msgs;
            self.messages.extend(tail);
        }
    }

    pub fn messages_for_llm(&self) -> Vec<ChatMessage> {
        self.messages.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_add_message() {
        let mut s = Session::new("ws-1");
        s.add(ChatMessage::user("hello"));
        assert_eq!(s.messages.len(), 1);
    }

    #[test]
    fn session_truncates() {
        let mut s = Session::new("ws-1");
        s.max_messages = 5;
        for i in 0..10 {
            s.add(ChatMessage::user(format!("msg {i}")));
        }
        assert!(s.messages.len() <= 5);
    }

    #[test]
    fn chat_message_builders() {
        let sys = ChatMessage::system("you are a helper");
        assert_eq!(sys.role, Role::System);
        assert_eq!(sys.content, "you are a helper");

        let tool = ChatMessage::tool("result", "call-1", "my_tool");
        assert_eq!(tool.role, Role::Tool);
        assert_eq!(tool.tool_call_id.as_deref(), Some("call-1"));
    }
}
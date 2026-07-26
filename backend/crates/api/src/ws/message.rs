use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 用于 WS 推送的 review finding 载荷（不含内部 id/review_id）
#[derive(Debug, Clone, Serialize)]
pub struct ReviewFindingPayload {
    pub id: String,
    pub severity: String,
    pub file_path: String,
    pub line_number: Option<i64>,
    pub category: String,
    pub title: String,
    pub description: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientMsg {
    Ping,
    ChatMessage {
        text: String,
        workspace_id: String,
    },
    NewSession {
        workspace_id: String,
    },
    GetHistory {
        workspace_id: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ServerMsg {
    Hello {
        connection_id: Uuid,
        server_time: DateTime<Utc>,
    },
    Pong {
        server_time: DateTime<Utc>,
    },
    JobOutput {
        job_id: String,
        text: String,
    },
    JobStatus {
        job_id: String,
        todo_id: String,
        status: String,
    },
    ChatResponse {
        text: String,
    },
    ChatThinking {
        text: String,
        iteration: usize,
    },
    ChatToolCall {
        tool_name: String,
        args: serde_json::Value,
    },
    ChatToolResult {
        tool_name: String,
        result: String,
    },
    ChatError {
        message: String,
    },
    SessionHistory {
        messages: Vec<SessionMessage>,
    },
    // L5 review events
    ReviewStarted {
        review_id: String,
        job_id: String,
        todo_id: String,
    },
    ReviewFinding {
        review_id: String,
        finding: ReviewFindingPayload,
    },
    ReviewCompleted {
        review_id: String,
        summary: String,
        score: i64,
        finding_count: i64,
    },
    ReviewError {
        review_id: String,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
}

impl ServerMsg {
    pub fn hello(connection_id: Uuid) -> Self {
        Self::Hello {
            connection_id,
            server_time: Utc::now(),
        }
    }

    pub fn pong() -> Self {
        Self::Pong {
            server_time: Utc::now(),
        }
    }

    pub fn job_output(job_id: String, text: String) -> Self {
        Self::JobOutput { job_id, text }
    }

    pub fn job_status(job_id: String, todo_id: String, status: String) -> Self {
        Self::JobStatus {
            job_id,
            todo_id,
            status,
        }
    }

    pub fn chat_response(text: String) -> Self {
        Self::ChatResponse { text }
    }

    pub fn chat_thinking(text: String, iteration: usize) -> Self {
        Self::ChatThinking { text, iteration }
    }

    pub fn chat_tool_call(tool_name: String, args: serde_json::Value) -> Self {
        Self::ChatToolCall { tool_name, args }
    }

    pub fn chat_tool_result(tool_name: String, result: String) -> Self {
        Self::ChatToolResult { tool_name, result }
    }

    pub fn chat_error(message: String) -> Self {
        Self::ChatError { message }
    }

    pub fn session_history(messages: Vec<SessionMessage>) -> Self {
        Self::SessionHistory { messages }
    }

    pub fn review_started(review_id: String, job_id: String, todo_id: String) -> Self {
        Self::ReviewStarted { review_id, job_id, todo_id }
    }

    pub fn review_finding(review_id: String, finding: ReviewFindingPayload) -> Self {
        Self::ReviewFinding { review_id, finding }
    }

    pub fn review_completed(review_id: String, summary: String, score: i64, finding_count: i64) -> Self {
        Self::ReviewCompleted { review_id, summary, score, finding_count }
    }

    pub fn review_error(review_id: String, message: String) -> Self {
        Self::ReviewError { review_id, message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_ping() {
        let json = r#"{"type":"ping"}"#;
        let msg: ClientMsg = serde_json::from_str(json).expect("parse");
        assert!(matches!(msg, ClientMsg::Ping));
    }

    #[test]
    fn serialize_hello() {
        let id = Uuid::new_v4();
        let msg = ServerMsg::hello(id);
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"hello\""));
        assert!(json.contains(&id.to_string()));
    }

    #[test]
    fn serialize_pong() {
        let msg = ServerMsg::pong();
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"pong\""));
    }

    #[test]
    fn serialize_job_output() {
        let msg = ServerMsg::job_output("job-1".into(), "hello".into());
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"job_output\""));
        assert!(json.contains("job-1"));
    }

    #[test]
    fn serialize_job_status() {
        let msg = ServerMsg::job_status("job-1".into(), "todo-1".into(), "running".into());
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"job_status\""));
        assert!(json.contains("running"));
    }
}

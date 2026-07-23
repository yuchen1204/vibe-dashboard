use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientMsg {
    Ping,
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
}

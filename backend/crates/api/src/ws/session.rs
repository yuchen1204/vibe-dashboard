use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use dashmap::DashMap;
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, Instant};

use crate::state::AppState;
use super::hub::ConnId;
use super::message::{ClientMsg, ServerMsg};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handle_connection(ws: WebSocket, state: AppState) {
    let hub = state.hub.clone();
    let (id, mut rx) = hub.register();
    let (mut ws_sink, mut ws_stream) = ws.split();

    let hello = ServerMsg::hello(id);
    let hello_json = serde_json::to_string(&hello).expect("serialize hello");
    if ws_sink.send(Message::Text(hello_json)).await.is_err() {
        hub.unregister(id);
        return;
    }

    let send_task = tokio::spawn(async move {
        let mut ticker = interval(HEARTBEAT_INTERVAL);
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            let json = serde_json::to_string(&msg).expect("serialize");
                            if ws_sink.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = ticker.tick() => {
                    if ws_sink.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        let mut last_pong = Instant::now();
        let mut sessions: DashMap<String, orchestrator::session::Session> = DashMap::new();
        let llm_config = state.llm_config;
        let pool = state.db.clone();
        let hub = state.hub.clone();
        let executor = state.executor.clone();

        // Build ToolContext with executor + HubNotifier
        let tool_ctx = orchestrator::ToolContext {
            executor: Some(executor),
            notifier: Some(Arc::new(HubNotifier(hub.clone()))),
        };

        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(client_msg) => {
                        match client_msg {
                            ClientMsg::Ping => {
                                let _ = hub.send_to(id, ServerMsg::pong());
                            }
                            ClientMsg::ChatMessage { text, workspace_id } => {
                                handle_chat_message(
                                    hub.clone(), id, &mut sessions, &llm_config, &pool,
                                    &workspace_id, &text, &tool_ctx,
                                ).await;
                            }
                            ClientMsg::NewSession { workspace_id } => {
                                sessions.insert(workspace_id.clone(), orchestrator::session::Session::new(&workspace_id));
                                let _ = hub.send_to(id, ServerMsg::chat_response("已开启新会话，之前的对话历史已清除。".to_string()));
                            }
                            ClientMsg::GetHistory { workspace_id } => {
                                if let Some(session) = sessions.get(&workspace_id) {
                                    let history = session.messages.iter().filter_map(|m| {
                                        let role = match m.role {
                                            orchestrator::Role::System => return None,
                                            orchestrator::Role::User => "user",
                                            orchestrator::Role::Assistant => "assistant",
                                            orchestrator::Role::Tool => return None,
                                        };
                                        Some(super::message::SessionMessage {
                                            role: role.to_string(),
                                            content: m.content.clone(),
                                            tool_name: m.name.clone(),
                                        })
                                    }).collect();
                                    let _ = hub.send_to(id, ServerMsg::session_history(history));
                                } else {
                                    let _ = hub.send_to(id, ServerMsg::session_history(vec![]));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(conn_id = %id, error = %e, "invalid ws message");
                    }
                },
                Message::Pong(_) => {
                    last_pong = Instant::now();
                }
                Message::Close(_) => break,
                _ => {}
            }
            if last_pong.elapsed() > CLIENT_TIMEOUT {
                tracing::warn!(conn_id = %id, "ws client timeout, dropping");
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    hub.unregister(id);
}

/// HubNotifier - 将 job 事件广播到所有 WS 连接
struct HubNotifier(Arc<super::hub::Hub>);

#[async_trait::async_trait]
impl execution::dispatch::JobNotifier for HubNotifier {
    async fn on_job_output(&self, job_id: &str, text: &str) {
        self.0.broadcast(ServerMsg::job_output(job_id.to_string(), text.to_string()));
    }

    async fn on_job_status(&self, job_id: &str, todo_id: &str, status: &str) {
        self.0.broadcast(ServerMsg::job_status(
            job_id.to_string(),
            todo_id.to_string(),
            status.to_string(),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_chat_message(
    hub: Arc<super::hub::Hub>,
    conn_id: ConnId,
    sessions: &mut DashMap<String, orchestrator::session::Session>,
    llm_config: &Arc<RwLock<orchestrator::llm::LlmConfig>>,
    pool: &sqlx::SqlitePool,
    workspace_id: &str,
    text: &str,
    tool_ctx: &orchestrator::ToolContext,
) {
    // Get or create session for this workspace
    let mut session = sessions
        .entry(workspace_id.to_string())
        .or_insert_with(|| orchestrator::session::Session::new(workspace_id));

    // Add user message
    session.add(orchestrator::ChatMessage::user(text));

    // 每次处理消息时从全局共享状态读取最新配置
    let is_configured = {
        let cfg = llm_config.read().unwrap();
        cfg.is_configured()
    };

    // 创建事件 channel，用于接收 agent 循环的中间事件
    let (event_tx, event_rx) = mpsc::unbounded_channel::<orchestrator::agent::AgentEvent>();

    // 启动一个后台任务：从 event_rx 读取事件并实时推送到前端
    let event_forwarder = spawn_event_forwarder(hub.clone(), conn_id, event_rx);

    // Run the agent loop
    if is_configured {
        let config_snapshot = {
            let cfg = llm_config.read().unwrap();
            cfg.clone()
        };
        match orchestrator::agent::run_agent(&mut session, pool, &config_snapshot, tool_ctx, Some(event_tx)).await {
            Ok(response) => {
                // Send final response
                hub.send_to(conn_id, ServerMsg::chat_response(response.response));
            }
            Err(e) => {
                hub.send_to(conn_id, ServerMsg::chat_error(e));
            }
        }
    } else {
        let resp = orchestrator::agent::run_agent_mock(&mut session, pool).await;
        for tc in &resp.tool_calls {
            hub.send_to(conn_id, ServerMsg::chat_tool_call(
                tc.name.clone(),
                tc.arguments.clone(),
            ));
            hub.send_to(conn_id, ServerMsg::chat_tool_result(
                tc.name.clone(),
                tc.result.clone(),
            ));
        }
        hub.send_to(conn_id, ServerMsg::chat_response(resp.response));
    }

    // 停止事件转发任务
    event_forwarder.abort();
}

fn spawn_event_forwarder(
    hub: Arc<super::hub::Hub>,
    conn_id: ConnId,
    mut event_rx: mpsc::UnboundedReceiver<orchestrator::agent::AgentEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                orchestrator::agent::AgentEvent::Thinking { text, iteration } => {
                    hub.send_to(conn_id, ServerMsg::chat_thinking(text, iteration));
                }
                orchestrator::agent::AgentEvent::ToolCall { name, arguments, iteration: _ } => {
                    hub.send_to(conn_id, ServerMsg::chat_tool_call(name, arguments));
                }
                orchestrator::agent::AgentEvent::ToolResult { name, result, iteration: _ } => {
                    hub.send_to(conn_id, ServerMsg::chat_tool_result(name, result));
                }
            }
        }
    })
}
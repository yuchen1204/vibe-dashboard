use axum::extract::ws::{Message, WebSocket};
use dashmap::DashMap;
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use std::time::Duration;
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
        let llm_config = state.llm_config.clone();
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
                                    &hub, id, &mut sessions, &llm_config, &pool,
                                    &workspace_id, &text, &tool_ctx,
                                ).await;
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
    hub: &super::hub::Hub,
    conn_id: ConnId,
    sessions: &mut DashMap<String, orchestrator::session::Session>,
    llm_config: &orchestrator::llm::LlmConfig,
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

    // Run the agent
    if llm_config.is_configured() {
        match orchestrator::agent::run_agent(&mut session, pool, llm_config, tool_ctx).await {
            Ok(response) => {
                // Send tool calls
                for tc in &response.tool_calls {
                    hub.send_to(conn_id, ServerMsg::chat_tool_call(
                        tc.name.clone(),
                        tc.arguments.clone(),
                    ));
                    hub.send_to(conn_id, ServerMsg::chat_tool_result(
                        tc.name.clone(),
                        tc.result.clone(),
                    ));
                }
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
}
use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use std::time::Duration;
use tokio::time::{interval, Instant};

use super::hub::Hub;
use super::message::{ClientMsg, ServerMsg};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handle_connection(ws: WebSocket, hub: std::sync::Arc<Hub>) {
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

    let hub_recv = std::sync::Arc::clone(&hub);
    let recv_task = tokio::spawn(async move {
        let mut last_pong = Instant::now();
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(client_msg) => hub_recv.handle_client_msg(id, client_msg),
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
        let _ = id;
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    hub.unregister(id);
}

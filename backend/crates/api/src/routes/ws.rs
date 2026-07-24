use axum::{
    extract::{ws::WebSocketUpgrade, State},
    response::Response,
};
use std::sync::Arc;

use crate::state::AppState;
use crate::ws::{session::handle_connection, Hub};

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    let hub: Arc<Hub> = state.hub;
    ws.on_upgrade(move |socket| handle_connection(socket, hub))
}

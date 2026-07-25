use axum::{
    extract::{ws::WebSocketUpgrade, State},
    response::Response,
};

use crate::state::AppState;
use crate::ws::session::handle_connection;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

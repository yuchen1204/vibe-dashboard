use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: f64,
}

pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    let uptime = (Utc::now() - state.started_at).num_milliseconds() as f64 / 1000.0;
    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
    }))
}

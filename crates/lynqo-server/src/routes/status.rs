use axum::{extract::State, Json};
use serde::Serialize;
use crate::state::AppState;

#[derive(Serialize)]
pub struct StatusResponse {
    version: &'static str,
    hostname: String,
    port: u16,
    platform: &'static str,
}

pub async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        hostname: state.config.hostname,
        port: state.config.port,
        platform: std::env::consts::OS,
    })
}

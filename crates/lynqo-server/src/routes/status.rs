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

#[derive(serde::Deserialize)]
pub struct PublicDomainPayload {
    pub domain: String,
}

pub async fn set_public_domain(
    State(state): State<AppState>,
    Json(payload): Json<PublicDomainPayload>,
) -> impl axum::response::IntoResponse {
    let domain = payload.domain.trim();
    let _ = state.db.set_setting("public_domain", domain).await;
    Json(serde_json::json!({"success": true, "public_domain": domain}))
}

pub async fn get_public_domain(
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    let domain = state.db.get_setting("public_domain").await.ok().flatten();
    Json(serde_json::json!({"public_domain": domain}))
}

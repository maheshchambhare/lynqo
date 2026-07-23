use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use crate::state::AppState;

pub async fn list_devices(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_devices().await {
        Ok(devices) => Json(devices).into_response(),
        Err(e) => {
            tracing::error!("list_devices: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

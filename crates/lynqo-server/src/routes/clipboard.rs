use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use crate::state::AppState;
use crate::ws::broadcast_clipboard;

#[derive(Deserialize)]
pub struct PushRequest {
    pub text: String,
}

pub async fn get_history(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.get_clipboard_history(100).await {
        Ok(items) => Json(items).into_response(),
        Err(e) => {
            tracing::error!("get_history: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn push_clipboard(
    State(state): State<AppState>,
    Json(body): Json<PushRequest>,
) -> impl IntoResponse {
    let entry = lynqo_core::ClipboardEntry::new(body.text.clone(), "text/plain".to_string(), "browser".to_string());

    if let Err(e) = state.db.add_clipboard_entry(&entry).await {
        tracing::error!("push_clipboard db: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Copy to system clipboard
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(&body.text);
    }

    broadcast_clipboard(&state).await;
    StatusCode::CREATED.into_response()
}

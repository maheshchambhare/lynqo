use axum::{
    body::Body,
    extract::{Path, State},
    http::{Response, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use crate::state::AppState;
use lynqo_core::TransferTask;
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
pub struct ShareRequest {
    pub path: String,
}

pub struct ProgressStream<S> {
    inner: S,
    transferred: u64,
    total: u64,
    task_id: String,
    db: lynqo_db::Database,
    ws_hub: crate::ws::WsHub,
    last_update: std::time::Instant,
}

impl<S> Stream for ProgressStream<S>
where
    S: Stream<Item = Result<axum::body::Bytes, std::io::Error>> + Unpin,
{
    type Item = Result<axum::body::Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                let len = bytes.len() as u64;
                self.transferred += len;
                
                let now = std::time::Instant::now();
                if now.duration_since(self.last_update).as_millis() >= 250 || self.transferred == self.total {
                    self.last_update = now;
                    let task_id = self.task_id.clone();
                    let transferred = self.transferred;
                    let total = self.total;
                    let db = self.db.clone();
                    let ws_hub = self.ws_hub.clone();
                    
                    tokio::spawn(async move {
                        let task = TransferTask {
                            id: task_id,
                            file_id: None,
                            file_name: None,
                            device_id: "client".to_string(),
                            action: "download".to_string(),
                            status: if transferred == total { "completed".to_string() } else { "transferring".to_string() },
                            transferred_bytes: transferred,
                            total_bytes: total,
                            created_at: 0,
                        };
                        let _ = db.update_transfer_task(&task).await;
                        let event = if transferred == total {
                            lynqo_core::WsEvent::TransferCompleted { task }
                        } else {
                            lynqo_core::WsEvent::TransferProgress { task }
                        };
                        if let Ok(json) = serde_json::to_string(&event) {
                            ws_hub.broadcast(json);
                        }
                    });
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => {
                let task_id = self.task_id.clone();
                let transferred = self.transferred;
                let total = self.total;
                let db = self.db.clone();
                let ws_hub = self.ws_hub.clone();
                tokio::spawn(async move {
                    let task = TransferTask {
                        id: task_id,
                        file_id: None,
                        file_name: None,
                        device_id: "client".to_string(),
                        action: "download".to_string(),
                        status: "failed".to_string(),
                        transferred_bytes: transferred,
                        total_bytes: total,
                        created_at: 0,
                    };
                    let _ = db.update_transfer_task(&task).await;
                    let event = lynqo_core::WsEvent::TransferFailed { task };
                    if let Ok(json) = serde_json::to_string(&event) {
                        ws_hub.broadcast(json);
                    }
                });
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub async fn list_files(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_shared_files().await {
        Ok(files) => Json(files).into_response(),
        Err(e) => {
            tracing::error!("list_files: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn share_file(
    State(state): State<AppState>,
    Json(body): Json<ShareRequest>,
) -> impl IntoResponse {
    let path = std::path::PathBuf::from(&body.path);
    if !path.exists() {
        return (StatusCode::NOT_FOUND, "file not found").into_response();
    }
    match lynqo_files::share_file(path, &state.db).await {
        Ok(file) => {
            spawn_thumbnail_generation(file.id.clone(), file.file_path.clone());
            let event = lynqo_core::WsEvent::FileShared { file: file.clone() };
            if let Ok(json) = serde_json::to_string(&event) {
                state.ws_hub.broadcast(json);
            }
            (StatusCode::CREATED, Json(file)).into_response()
        }
        Err(e) => {
            tracing::error!("share_file: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn stream_file(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let file = match state.db.get_shared_file(&id).await {
        Ok(Some(f)) => f,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("get_shared_file: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let f = match tokio::fs::File::open(&file.file_path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let _ = state.db.increment_download_count(&id).await;

    // Start download task log
    let task_id = uuid::Uuid::new_v4().to_string();
    let task = TransferTask {
        id: task_id.clone(),
        file_id: Some(file.id.clone()),
        file_name: Some(file.file_name.clone()),
        device_id: "client".to_string(),
        action: "download".to_string(),
        status: "transferring".to_string(),
        transferred_bytes: 0,
        total_bytes: file.file_size,
        created_at: chrono::Utc::now().timestamp(),
    };
    let _ = state.db.add_transfer_task(&task).await;
    let event = lynqo_core::WsEvent::TransferStarted { task: task.clone() };
    if let Ok(json) = serde_json::to_string(&event) {
        state.ws_hub.broadcast(json);
    }

    let content_type = file
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());
    
    let stream = ReaderStream::new(f);
    let progress_stream = ProgressStream {
        inner: stream,
        transferred: 0,
        total: file.file_size,
        task_id,
        db: state.db.clone(),
        ws_hub: state.ws_hub.clone(),
        last_update: std::time::Instant::now(),
    };

    Response::builder()
        .header("Content-Type", content_type)
        .header("Content-Length", file.file_size)
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", file.file_name),
        )
        .body(Body::from_stream(progress_stream))
        .unwrap()
}

pub async fn revoke_file(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.revoke_shared_file(&id).await {
        Ok(true) => {
            let event = lynqo_core::WsEvent::FileRevoked { id };
            if let Ok(json) = serde_json::to_string(&event) {
                state.ws_hub.broadcast(json);
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("revoke_file: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    let mut file_name = None;
    let mut content_type = None;
    let mut file_path_str = None;
    let mut file_size = 0;
    let file_id = uuid::Uuid::new_v4().to_string();
    let task_id = uuid::Uuid::new_v4().to_string();

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("lynqo")
        .join("uploads");

    if let Err(e) = tokio::fs::create_dir_all(&data_dir).await {
        tracing::error!("failed to create uploads dir: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            file_name = field.file_name().map(|s| s.to_string());
            content_type = field.content_type().map(|s| s.to_string());
            
            let original_name = file_name.clone().unwrap_or_else(|| "uploaded_file".to_string());
            let ext = std::path::Path::new(&original_name)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let dest_filename = if ext.is_empty() {
                file_id.clone()
            } else {
                format!("{}.{}", file_id, ext)
            };
            let path = data_dir.join(dest_filename);
            file_path_str = Some(path.to_string_lossy().to_string());

            let mut out_file = match tokio::fs::File::create(&path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!("failed to create dest file: {e}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            let mut task = TransferTask {
                id: task_id.clone(),
                file_id: Some(file_id.clone()),
                file_name: Some(original_name.clone()),
                device_id: "client".to_string(),
                action: "upload".to_string(),
                status: "transferring".to_string(),
                transferred_bytes: 0,
                total_bytes: 0,
                created_at: chrono::Utc::now().timestamp(),
            };
            let _ = state.db.add_transfer_task(&task).await;
            let event = lynqo_core::WsEvent::TransferStarted { task: task.clone() };
            if let Ok(json) = serde_json::to_string(&event) {
                state.ws_hub.broadcast(json);
            }

            while let Ok(Some(chunk)) = field.chunk().await {
                if let Err(e) = out_file.write_all(&chunk).await {
                    tracing::error!("failed to write chunk: {e}");
                    task.status = "failed".to_string();
                    let _ = state.db.update_transfer_task(&task).await;
                    let event = lynqo_core::WsEvent::TransferFailed { task: task.clone() };
                    if let Ok(json) = serde_json::to_string(&event) {
                        state.ws_hub.broadcast(json);
                    }
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
                file_size += chunk.len() as u64;
                task.transferred_bytes = file_size;
                task.total_bytes = file_size;
                
                let event = lynqo_core::WsEvent::TransferProgress { task: task.clone() };
                if let Ok(json) = serde_json::to_string(&event) {
                    state.ws_hub.broadcast(json);
                }
            }

            task.status = "completed".to_string();
            let _ = state.db.update_transfer_task(&task).await;
            let event = lynqo_core::WsEvent::TransferCompleted { task: task.clone() };
            if let Ok(json) = serde_json::to_string(&event) {
                state.ws_hub.broadcast(json);
            }
        }
    }

    let original_name = match file_name {
        Some(name) => name,
        None => return (StatusCode::BAD_REQUEST, "Missing file field").into_response(),
    };
    let file_path = match file_path_str {
        Some(path) => path,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "File path missing").into_response(),
    };

    let file = lynqo_core::SharedFile {
        id: file_id.clone(),
        file_path,
        file_name: original_name,
        file_size,
        mime_type: content_type.or_else(|| {
            mime_guess::from_path(std::path::Path::new(&file_id))
                .first()
                .map(|m| m.to_string())
        }),
        created_at: chrono::Utc::now().timestamp(),
        expires_at: None,
        download_count: 0,
        revoked: false,
    };

    match state.db.save_shared_file(&file).await {
        Ok(_) => {
            spawn_thumbnail_generation(file.id.clone(), file.file_path.clone());
            let event = lynqo_core::WsEvent::FileShared { file: file.clone() };
            if let Ok(json) = serde_json::to_string(&event) {
                state.ws_hub.broadcast(json);
            }
            (StatusCode::CREATED, Json(file)).into_response()
        }
        Err(e) => {
            tracing::error!("save_shared_file: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn get_thumbnail(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let file = match state.db.get_shared_file(&id).await {
        Ok(Some(f)) => f,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let ext = std::path::Path::new(&file.file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_image = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"].contains(&ext.as_str());

    let thumb_dir = match dirs::data_dir() {
        Some(d) => d.join("lynqo").join("thumbnails"),
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let thumb_path = thumb_dir.join(format!("{}.jpg", id));

    if let Ok(f) = tokio::fs::File::open(&thumb_path).await {
        return Response::builder()
            .header("Content-Type", "image/jpeg")
            .body(Body::from_stream(ReaderStream::new(f)))
            .unwrap();
    }

    if is_image {
        let f = match tokio::fs::File::open(&file.file_path).await {
            Ok(f) => f,
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        };
        let content_type = file.mime_type.unwrap_or_else(|| "image/jpeg".to_string());
        return Response::builder()
            .header("Content-Type", content_type)
            .body(Body::from_stream(ReaderStream::new(f)))
            .unwrap();
    }

    StatusCode::NOT_FOUND.into_response()
}

fn spawn_thumbnail_generation(file_id: String, file_path: String) {
    tokio::spawn(async move {
        let ext = std::path::Path::new(&file_path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_video = ["mp4", "webm", "ogg", "mov", "m4v"].contains(&ext.as_str());
        let is_image = ["png", "jpg", "jpeg", "gif", "webp", "bmp"].contains(&ext.as_str());
        if !is_video && !is_image {
            return;
        }

        let thumb_dir = match dirs::data_dir() {
            Some(d) => d.join("lynqo").join("thumbnails"),
            None => return,
        };

        if let Err(e) = tokio::fs::create_dir_all(&thumb_dir).await {
            tracing::error!("failed to create thumbnails dir: {e}");
            return;
        }

        let thumb_path = thumb_dir.join(format!("{}.jpg", file_id));

        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        };
        let bin_path = exe_path.parent().unwrap().join("video_thumbnail");
        let bin_path = if bin_path.exists() {
            bin_path
        } else {
            std::path::PathBuf::from("crates/lynqo-server/video_thumbnail")
        };

        if !bin_path.exists() {
            tracing::error!("video_thumbnail binary not found at {:?}", bin_path);
            return;
        }

        let output = tokio::process::Command::new(bin_path)
            .arg(&file_path)
            .arg(thumb_path)
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                tracing::info!("generated thumbnail for {}", file_id);
            }
            Ok(out) => {
                tracing::error!(
                    "thumbnail generation failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            Err(e) => {
                tracing::error!("failed to execute thumbnail tool: {e}");
            }
        }
    });
}

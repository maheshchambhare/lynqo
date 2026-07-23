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

// ── Public File Share Handlers ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreatePublicSharePayload {
    pub password: Option<String>,
    pub max_downloads: Option<u64>,
    pub expires_hours: Option<u64>,
}

#[derive(Deserialize)]
pub struct PublicQuery {
    pub pwd: Option<String>,
    pub dl: Option<String>,
    pub download: Option<String>,
}

pub async fn create_public_share_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<CreatePublicSharePayload>,
) -> impl IntoResponse {
    let file = match state.db.get_shared_file(&id).await {
        Ok(Some(f)) => f,
        _ => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "File not found"}))).into_response(),
    };

    let token = format!("pub_{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..12]);
    let password_hash = payload.password.as_ref().map(|p| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        p.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    });

    let expires_at = payload.expires_hours.map(|h| chrono::Utc::now().timestamp() + (h as i64 * 3600));

    let share = lynqo_core::PublicShare {
        token: token.clone(),
        file_id: file.id.clone(),
        password_hash,
        max_downloads: payload.max_downloads,
        download_count: 0,
        expires_at,
        created_at: chrono::Utc::now().timestamp(),
        revoked: false,
    };

    if let Err(e) = state.db.create_public_share(&share).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let db_domain = state.db.get_setting("public_domain").await.ok().flatten();
    let public_domain = db_domain
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| "https://share.lynqo.app".to_string());

    let public_url = format!("{}/public/s/{}", public_domain.trim_end_matches('/'), token);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "token": token,
            "public_url": public_url,
            "file_name": file.file_name,
            "has_password": share.password_hash.is_some(),
            "max_downloads": share.max_downloads,
            "expires_at": share.expires_at,
        })),
    )
        .into_response()
}

pub async fn serve_public_share_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    axum::extract::Query(query): axum::extract::Query<PublicQuery>,
) -> impl IntoResponse {
    let share = match state.db.get_public_share(&token).await {
        Ok(Some(s)) => s,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [("content-type", "text/html")],
                "<html><body style='font-family:sans-serif;text-align:center;padding:4rem;'><h2>Link Expired or Invalid</h2><p>This public file share link is no longer available.</p></body></html>",
            ).into_response();
        }
    };

    let now = chrono::Utc::now().timestamp();
    if let Some(exp) = share.expires_at {
        if now > exp {
            return (
                StatusCode::GONE,
                [("content-type", "text/html")],
                "<html><body style='font-family:sans-serif;text-align:center;padding:4rem;'><h2>Link Expired</h2><p>This download link has passed its expiration time.</p></body></html>",
            ).into_response();
        }
    }

    if let Some(max_d) = share.max_downloads {
        if share.download_count >= max_d {
            return (
                StatusCode::GONE,
                [("content-type", "text/html")],
                "<html><body style='font-family:sans-serif;text-align:center;padding:4rem;'><h2>Download Limit Reached</h2><p>This file link has reached its maximum allowed downloads.</p></body></html>",
            ).into_response();
        }
    }

    // Get underlying file
    let file = match state.db.get_shared_file(&share.file_id).await {
        Ok(Some(f)) => f,
        _ => return (StatusCode::NOT_FOUND, [("content-type", "text/html")], "File content unavailable").into_response(),
    };

    // Password verification check
    if let Some(expected_hash) = &share.password_hash {
        let provided_pwd = query.pwd.as_deref().unwrap_or("");
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        provided_pwd.hash(&mut hasher);
        let provided_hash = format!("{:x}", hasher.finish());

        if provided_hash != *expected_hash {
            let has_error = !provided_pwd.is_empty();
            let html = crate::routes::video_review::render_password_page(&file.file_name, has_error);
            return (StatusCode::OK, [("content-type", "text/html")], html).into_response();
        }
    }

    let is_download_request = query.dl.as_deref() == Some("1") || query.download.as_deref() == Some("true");

    if is_download_request {
        let path = std::path::PathBuf::from(&file.file_path);
        let fs_file = match tokio::fs::File::open(&path).await {
            Ok(f) => f,
            Err(_) => return (StatusCode::NOT_FOUND, [("content-type", "text/html")], "File not found on server disk").into_response(),
        };

        let _ = state.db.increment_public_share_download(&token).await;
        let _ = state.db.increment_download_count(&file.id).await;

        let mime = file.mime_type.as_deref().unwrap_or("application/octet-stream");
        let stream = ReaderStream::new(fs_file);
        let body = Body::from_stream(stream);

        return Response::builder()
            .header("content-type", mime)
            .header(
                "content-disposition",
                format!("attachment; filename=\"{}\"", file.file_name),
            )
            .body(body)
            .unwrap()
            .into_response();
    }

    // Default View: Serve Interactive Client Preview Page based on file type
    let ext = std::path::Path::new(&file.file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let pwd_param = query.pwd.as_deref().unwrap_or("");

    let is_video = ["mp4", "webm", "mov", "mkv", "avi", "m4v"].contains(&ext.as_str())
        || file.mime_type.as_deref().unwrap_or("").starts_with("video/");
    let is_image = ["png", "jpg", "jpeg", "webp", "gif", "svg", "bmp"].contains(&ext.as_str())
        || file.mime_type.as_deref().unwrap_or("").starts_with("image/");
    let is_audio = ["mp3", "wav", "flac", "aac", "m4a", "ogg"].contains(&ext.as_str())
        || file.mime_type.as_deref().unwrap_or("").starts_with("audio/");

    if is_video {
        let redirect_url = if pwd_param.is_empty() {
            format!("/public/v/{}", token)
        } else {
            format!("/public/v/{}?pwd={}", token, pwd_param)
        };
        return axum::response::Redirect::temporary(&redirect_url).into_response();
    } else if is_image {
        let html = generate_image_preview_html(&token, &file.file_name, file.file_size, pwd_param);
        return (StatusCode::OK, [("content-type", "text/html")], html).into_response();
    } else if is_audio {
        let html = generate_audio_preview_html(&token, &file.file_name, file.file_size, pwd_param);
        return (StatusCode::OK, [("content-type", "text/html")], html).into_response();
    } else {
        let html = generate_file_preview_html(&token, &file.file_name, file.file_size, pwd_param);
        return (StatusCode::OK, [("content-type", "text/html")], html).into_response();
    }
}

fn generate_image_preview_html(token: &str, file_name: &str, file_size: u64, pwd: &str) -> String {
    let size_mb = format!("{:.2} MB", file_size as f64 / 1024.0 / 1024.0);
    let pwd_qs = if pwd.is_empty() { String::new() } else { format!("&pwd={}", pwd) };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Lynqo Studio Preview - {file_name}</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg: #080C14;
      --card: #121826;
      --primary: #00F0FF;
      --accent: #8B5CF6;
      --emerald: #10B981;
      --text: #F8FAFC;
      --text-sub: #94A3B8;
      --border: rgba(255, 255, 255, 0.08);
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: 'Plus Jakarta Sans', sans-serif; }}
    body {{ background: var(--bg); color: var(--text); min-height: 100vh; display: flex; flex-direction: column; }}
    
    header {{
      background: rgba(18, 24, 38, 0.85);
      backdrop-filter: blur(20px);
      border-bottom: 1px solid var(--border);
      padding: 0.85rem 1.5rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    .brand {{ display: flex; align-items: center; gap: 0.85rem; }}
    .logo-mark {{ width: 40px; height: 40px; background: linear-gradient(135deg, var(--primary), var(--accent)); border-radius: 12px; display: flex; align-items: center; justify-content: center; font-weight: 800; color: #000; font-size: 1.25rem; }}
    .title-block h3 {{ font-size: 1rem; font-weight: 800; color: var(--text); }}
    .title-block .meta {{ font-size: 0.775rem; color: var(--text-sub); display: flex; gap: 0.5rem; margin-top: 2px; }}

    .preview-container {{
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 2rem 1.5rem;
      gap: 1.5rem;
      max-width: 1400px;
      margin: 0 auto;
      width: 100%;
    }}

    .image-stage {{
      background: rgba(18, 24, 38, 0.6);
      border: 1px solid var(--border);
      border-radius: 24px;
      padding: 1.5rem;
      display: flex;
      align-items: center;
      justify-content: center;
      width: 100%;
      max-height: 70vh;
      box-shadow: 0 25px 50px -12px rgba(0,0,0,0.5);
      position: relative;
      overflow: hidden;
    }}
    .image-stage img {{
      max-width: 100%;
      max-height: 65vh;
      object-fit: contain;
      border-radius: 12px;
      box-shadow: 0 10px 30px rgba(0,0,0,0.6);
    }}

    .info-bar {{
      background: var(--card);
      border: 1px solid var(--border);
      border-radius: 18px;
      padding: 1.25rem 2rem;
      display: flex;
      align-items: center;
      justify-content: space-between;
      width: 100%;
      gap: 1.5rem;
      flex-wrap: wrap;
    }}

    .file-details {{ display: flex; align-items: center; gap: 1rem; }}
    .file-icon {{ width: 48px; height: 48px; background: rgba(0,240,255,0.1); border-radius: 14px; display: flex; align-items: center; justify-content: center; font-size: 1.5rem; border: 1px solid rgba(0,240,255,0.25); }}

    .btn-download {{
      background: linear-gradient(135deg, var(--primary), #00A3FF);
      color: #000;
      border: none;
      padding: 0.8rem 1.75rem;
      border-radius: 14px;
      font-weight: 800;
      font-size: 0.95rem;
      text-decoration: none;
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      box-shadow: 0 4px 20px rgba(0, 240, 255, 0.3);
      transition: all 0.2s;
    }}
    .btn-download:hover {{ transform: translateY(-2px); filter: brightness(1.1); box-shadow: 0 6px 25px rgba(0, 240, 255, 0.4); }}
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="logo-mark">L</div>
      <div class="title-block">
        <h3>{file_name}</h3>
        <div class="meta"><span>{size_mb}</span> · <span>Client Image Preview</span></div>
      </div>
    </div>
  </header>

  <div class="preview-container">
    <div class="image-stage">
      <img src="/public/s/{token}?dl=1{pwd_qs}" alt="{file_name}" />
    </div>

    <div class="info-bar">
      <div class="file-details">
        <div class="file-icon">🖼️</div>
        <div>
          <h4 style="font-size: 1rem; font-weight: 800;">{file_name}</h4>
          <p style="font-size: 0.825rem; color: var(--text-sub);">{size_mb} · Ready for Client Review & Download</p>
        </div>
      </div>
      <a href="/public/s/{token}?dl=1{pwd_qs}" download="{file_name}" class="btn-download">
        <span>⬇️ Download Original Image</span>
      </a>
    </div>
  </div>
</body>
</html>"#
    )
}

fn generate_audio_preview_html(token: &str, file_name: &str, file_size: u64, pwd: &str) -> String {
    let size_mb = format!("{:.2} MB", file_size as f64 / 1024.0 / 1024.0);
    let pwd_qs = if pwd.is_empty() { String::new() } else { format!("&pwd={}", pwd) };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Lynqo Audio Review - {file_name}</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg: #080C14;
      --card: #121826;
      --primary: #00F0FF;
      --accent: #8B5CF6;
      --emerald: #10B981;
      --text: #F8FAFC;
      --text-sub: #94A3B8;
      --border: rgba(255, 255, 255, 0.08);
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: 'Plus Jakarta Sans', sans-serif; }}
    body {{ background: var(--bg); color: var(--text); min-height: 100vh; display: flex; flex-direction: column; }}

    header {{
      background: rgba(18, 24, 38, 0.85);
      backdrop-filter: blur(20px);
      border-bottom: 1px solid var(--border);
      padding: 0.85rem 1.5rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    .brand {{ display: flex; align-items: center; gap: 0.85rem; }}
    .logo-mark {{ width: 40px; height: 40px; background: linear-gradient(135deg, var(--primary), var(--accent)); border-radius: 12px; display: flex; align-items: center; justify-content: center; font-weight: 800; color: #000; font-size: 1.25rem; }}

    .preview-container {{
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 2rem 1.5rem;
      max-width: 800px;
      margin: 0 auto;
      width: 100%;
    }}

    .audio-card {{
      background: var(--card);
      border: 1px solid var(--border);
      border-radius: 24px;
      padding: 2.5rem;
      width: 100%;
      box-shadow: 0 25px 50px -12px rgba(0,0,0,0.5);
      text-align: center;
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
      align-items: center;
    }}

    .audio-icon {{
      width: 80px;
      height: 80px;
      background: linear-gradient(135deg, rgba(0,240,255,0.1), rgba(139,92,246,0.15));
      border: 1px solid rgba(0,240,255,0.3);
      border-radius: 24px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 2.5rem;
      box-shadow: 0 0 30px rgba(0,240,255,0.2);
    }}

    audio {{ width: 100%; outline: none; border-radius: 12px; }}

    .btn-download {{
      background: linear-gradient(135deg, var(--primary), #00A3FF);
      color: #000;
      border: none;
      padding: 0.85rem 2rem;
      border-radius: 14px;
      font-weight: 800;
      font-size: 0.95rem;
      text-decoration: none;
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      box-shadow: 0 4px 20px rgba(0, 240, 255, 0.3);
      transition: all 0.2s;
    }}
    .btn-download:hover {{ transform: translateY(-2px); filter: brightness(1.1); box-shadow: 0 6px 25px rgba(0, 240, 255, 0.4); }}
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="logo-mark">L</div>
      <div style="font-weight:800; font-size:1.1rem;">Lynqo Studio Audio</div>
    </div>
  </header>

  <div class="preview-container">
    <div class="audio-card">
      <div class="audio-icon">🎵</div>
      <div>
        <h3 style="font-size: 1.25rem; font-weight: 800; margin-bottom: 0.25rem;">{file_name}</h3>
        <p style="color: var(--text-sub); font-size: 0.875rem;">{size_mb} · Audio File Stream</p>
      </div>

      <audio controls preload="metadata">
        <source src="/public/s/{token}?dl=1{pwd_qs}">
        Your browser does not support HTML5 audio playback.
      </audio>

      <a href="/public/s/{token}?dl=1{pwd_qs}" download="{file_name}" class="btn-download">
        <span>⬇️ Download Audio File</span>
      </a>
    </div>
  </div>
</body>
</html>"#
    )
}

fn generate_file_preview_html(token: &str, file_name: &str, file_size: u64, pwd: &str) -> String {
    let size_mb = format!("{:.2} MB", file_size as f64 / 1024.0 / 1024.0);
    let pwd_qs = if pwd.is_empty() { String::new() } else { format!("&pwd={}", pwd) };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Lynqo Studio Share - {file_name}</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg: #080C14;
      --card: #121826;
      --primary: #00F0FF;
      --accent: #8B5CF6;
      --emerald: #10B981;
      --text: #F8FAFC;
      --text-sub: #94A3B8;
      --border: rgba(255, 255, 255, 0.08);
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: 'Plus Jakarta Sans', sans-serif; }}
    body {{ background: var(--bg); color: var(--text); min-height: 100vh; display: flex; flex-direction: column; }}

    header {{
      background: rgba(18, 24, 38, 0.85);
      backdrop-filter: blur(20px);
      border-bottom: 1px solid var(--border);
      padding: 0.85rem 1.5rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    .brand {{ display: flex; align-items: center; gap: 0.85rem; }}
    .logo-mark {{ width: 40px; height: 40px; background: linear-gradient(135deg, var(--primary), var(--accent)); border-radius: 12px; display: flex; align-items: center; justify-content: center; font-weight: 800; color: #000; font-size: 1.25rem; }}

    .preview-container {{
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 2rem 1.5rem;
      max-width: 540px;
      margin: 0 auto;
      width: 100%;
    }}

    .file-card {{
      background: var(--card);
      border: 1px solid var(--border);
      border-radius: 24px;
      padding: 2.5rem;
      width: 100%;
      box-shadow: 0 25px 50px -12px rgba(0,0,0,0.5);
      text-align: center;
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
      align-items: center;
    }}

    .file-icon {{
      width: 80px;
      height: 80px;
      background: linear-gradient(135deg, rgba(0,240,255,0.1), rgba(139,92,246,0.15));
      border: 1px solid rgba(0,240,255,0.3);
      border-radius: 24px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 2.5rem;
      box-shadow: 0 0 30px rgba(0,240,255,0.2);
    }}

    .btn-download {{
      width: 100%;
      background: linear-gradient(135deg, var(--primary), #00A3FF);
      color: #000;
      border: none;
      padding: 0.85rem 2rem;
      border-radius: 14px;
      font-weight: 800;
      font-size: 1rem;
      text-decoration: none;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 0.5rem;
      box-shadow: 0 4px 20px rgba(0, 240, 255, 0.3);
      transition: all 0.2s;
    }}
    .btn-download:hover {{ transform: translateY(-2px); filter: brightness(1.1); box-shadow: 0 6px 25px rgba(0, 240, 255, 0.4); }}
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="logo-mark">L</div>
      <div style="font-weight:800; font-size:1.1rem;">Lynqo Studio Share</div>
    </div>
  </header>

  <div class="preview-container">
    <div class="file-card">
      <div class="file-icon">📦</div>
      <div>
        <h3 style="font-size: 1.2rem; font-weight: 800; margin-bottom: 0.35rem; word-break: break-word;">{file_name}</h3>
        <p style="color: var(--text-sub); font-size: 0.875rem;">{size_mb} · Shared File Ready for Download</p>
      </div>

      <a href="/public/s/{token}?dl=1{pwd_qs}" download="{file_name}" class="btn-download">
        <span>⬇️ Download Shared File</span>
      </a>
    </div>
  </div>
</body>
</html>"#
    )
}

pub async fn revoke_public_share_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    match state.db.revoke_public_share(&token).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Token not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

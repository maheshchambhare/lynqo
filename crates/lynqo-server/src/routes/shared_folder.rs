use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, Response, StatusCode},
    response::IntoResponse,
    Json, Router,
    routing::{get, post},
};
use lynqo_core::{SharedFolderConfig, SharedFolderItem, WsEvent};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Mutex as StdMutex;
use std::time::SystemTime;
use tokio::fs;
use tokio_util::io::ReaderStream;
use crate::state::AppState;

const SETTING_KEY_SHARED_FOLDER: &str = "shared_folder_path";

#[derive(Deserialize)]
pub struct SetConfigPayload {
    pub path: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_config).post(set_config))
        .route("/files", get(list_files))
        .route("/upload", post(upload_file))
        .route("/file/*path", get(get_file).delete(delete_file))
}

pub async fn get_default_or_configured_path(state: &AppState) -> Option<PathBuf> {
    if let Ok(Some(custom_path)) = state.db.get_setting(SETTING_KEY_SHARED_FOLDER).await {
        if !custom_path.trim().is_empty() {
            let p = PathBuf::from(&custom_path);
            if p.exists() && p.is_dir() {
                return Some(p);
            }
        }
    }

    // Default fallback: ~/.lynqo/shared_folder
    if let Some(home) = dirs::home_dir() {
        let default_dir = home.join(".lynqo").join("shared_folder");
        let _ = fs::create_dir_all(&default_dir).await;
        if default_dir.exists() {
            return Some(default_dir);
        }
    }

    None
}

pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let path = get_default_or_configured_path(&state).await;
    let config = SharedFolderConfig {
        path: path.as_ref().map(|p| p.to_string_lossy().to_string()),
        is_active: path.is_some(),
    };
    Json(config).into_response()
}

pub async fn set_config(
    State(state): State<AppState>,
    Json(payload): Json<SetConfigPayload>,
) -> impl IntoResponse {
    let path_buf = PathBuf::from(&payload.path);
    if !path_buf.exists() || !path_buf.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Folder path does not exist or is not a directory" })),
        )
            .into_response();
    }

    if let Err(e) = state.db.set_setting(SETTING_KEY_SHARED_FOLDER, &payload.path).await {
        tracing::error!("Failed to set shared folder path setting: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let config = SharedFolderConfig {
        path: Some(payload.path),
        is_active: true,
    };

    let event = WsEvent::SharedFolderConfigUpdated {
        config: config.clone(),
    };
    if let Ok(json) = serde_json::to_string(&event) {
        state.ws_hub.broadcast(json);
    }
    let event_changed = WsEvent::SharedFolderChanged;
    if let Ok(json) = serde_json::to_string(&event_changed) {
        state.ws_hub.broadcast(json);
    }

    Json(config).into_response()
}

pub async fn list_files(State(state): State<AppState>) -> impl IntoResponse {
    let folder_path = match get_default_or_configured_path(&state).await {
        Some(p) => p,
        None => return Json(Vec::<SharedFolderItem>::new()).into_response(),
    };

    let mut items = Vec::new();
    let mut dir = match fs::read_dir(&folder_path).await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to read shared folder {folder_path:?}: {e}");
            return Json(Vec::<SharedFolderItem>::new()).into_response();
        }
    };

    while let Ok(Some(entry)) = dir.next_entry().await {
        let file_name = entry.file_name().to_string_lossy().to_string();
        // ponytail: skip hidden files, temporary uploads, and OS copy temp files
        if file_name.starts_with('.') || file_name.ends_with(".uploading") || file_name.ends_with(".tmp") {
            continue;
        }

        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        let file_size = metadata.len();
        let modified_at = metadata
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mime_type = if !is_dir {
            mime_guess::from_path(entry.path())
                .first()
                .map(|m| m.to_string())
        } else {
            None
        };

        items.push(SharedFolderItem {
            name: file_name.clone(),
            relative_path: file_name,
            file_size,
            mime_type,
            is_dir,
            modified_at,
        });
    }

    // Sort by modified_at descending
    items.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Json(items).into_response()
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let folder_path = match get_default_or_configured_path(&state).await {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Shared folder is not configured on server host" })),
            )
                .into_response();
        }
    };

    let mut saved_files = Vec::new();

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let original_name = match field.file_name() {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Sanitize filename to prevent directory traversal
        let file_name = StdPath::new(&original_name)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if file_name.is_empty() || file_name.starts_with('.') || file_name.ends_with(".uploading") {
            continue;
        }

        // ponytail: upload to temp filename first, then atomic rename on completion
        let temp_filename = format!(".{}.{}.uploading", file_name, uuid::Uuid::new_v4());
        let temp_path = folder_path.join(&temp_filename);
        let final_target_path = folder_path.join(&file_name);

        let mut out_file = match fs::File::create(&temp_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("Failed to create temp upload file: {e}");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        use tokio::io::AsyncWriteExt;
        let mut upload_success = true;
        while let Ok(Some(chunk)) = field.chunk().await {
            if let Err(e) = out_file.write_all(&chunk).await {
                tracing::error!("Failed to write chunk: {e}");
                upload_success = false;
                break;
            }
        }
        let _ = out_file.flush().await;
        drop(out_file);

        if upload_success {
            // Atomic rename from temp file to final destination once 100% finished
            if let Err(e) = fs::rename(&temp_path, &final_target_path).await {
                tracing::error!("Failed atomic rename from {:?} to {:?}: {e}", temp_path, final_target_path);
                let _ = fs::remove_file(&temp_path).await;
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            saved_files.push(file_name);
        } else {
            let _ = fs::remove_file(&temp_path).await;
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }

    if !saved_files.is_empty() {
        let event = WsEvent::SharedFolderChanged;
        if let Ok(json) = serde_json::to_string(&event) {
            state.ws_hub.broadcast(json);
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "success": true, "files": saved_files })),
    )
        .into_response()
}

pub async fn get_file(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let folder_path = match get_default_or_configured_path(&state).await {
        Some(p) => p,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let safe_filename = StdPath::new(&path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let target_path = folder_path.join(&safe_filename);
    if !target_path.exists() || !target_path.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let file = match fs::File::open(&target_path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let metadata = match target_path.metadata() {
        Ok(m) => m,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let mime_type = mime_guess::from_path(&target_path)
        .first_or_octet_stream()
        .to_string();

    let stream = ReaderStream::new(file);

    Response::builder()
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", safe_filename),
        )
        .body(Body::from_stream(stream))
        .unwrap()
}

pub async fn delete_file(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let folder_path = match get_default_or_configured_path(&state).await {
        Some(p) => p,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let safe_filename = StdPath::new(&path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let target_path = folder_path.join(&safe_filename);
    if !target_path.exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Err(e) = fs::remove_file(&target_path).await {
        tracing::error!("Failed to remove file {target_path:?}: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let event = WsEvent::SharedFolderChanged;
    if let Ok(json) = serde_json::to_string(&event) {
        state.ws_hub.broadcast(json);
    }

    StatusCode::NO_CONTENT.into_response()
}

// Track file sizes over time to detect active OS file copies (e.g. 4GB file copy in progress)
static FILE_SIZE_CACHE: LazyLock<StdMutex<HashMap<String, u64>>> = LazyLock::new(|| StdMutex::new(HashMap::new()));

use std::sync::LazyLock;

pub fn spawn_shared_folder_watcher(state: AppState) {
    tokio::spawn(async move {
        let mut last_signature = String::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(1500));

        loop {
            interval.tick().await;

            if let Some(folder_path) = get_default_or_configured_path(&state).await {
                let current_sig = compute_folder_signature(&folder_path).await;
                if !last_signature.is_empty() && current_sig != last_signature {
                    let event = WsEvent::SharedFolderChanged;
                    if let Ok(json) = serde_json::to_string(&event) {
                        state.ws_hub.broadcast(json);
                    }
                }
                last_signature = current_sig;
            }
        }
    });
}

async fn compute_folder_signature(folder_path: &StdPath) -> String {
    let mut dir = match fs::read_dir(folder_path).await {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    let mut parts = Vec::new();
    let mut current_sizes = HashMap::new();

    while let Ok(Some(entry)) = dir.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.ends_with(".uploading") || name.ends_with(".tmp") {
            continue;
        }
        if let Ok(meta) = entry.metadata().await {
            let size = meta.len();
            let mtime = meta
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);

            // Check if file is currently growing (active OS file copy)
            let is_growing = {
                let cache = FILE_SIZE_CACHE.lock().unwrap();
                if let Some(&prev_size) = cache.get(&name) {
                    prev_size != size
                } else {
                    false
                }
            };

            current_sizes.insert(name.clone(), size);

            // Only add to signature if size has stabilized (not actively changing mid-copy)
            if !is_growing {
                parts.push(format!("{}:{}:{}", name, size, mtime));
            }
        }
    }

    // Update size cache
    if let Ok(mut cache) = FILE_SIZE_CACHE.lock() {
        *cache = current_sizes;
    }

    parts.sort();
    parts.join("|")
}

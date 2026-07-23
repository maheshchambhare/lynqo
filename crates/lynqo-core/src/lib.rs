use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Core error type
#[derive(Debug, Error)]
pub enum LynqoError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// A shared file record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFile {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub download_count: u64,
    pub revoked: bool,
}

impl SharedFile {
    pub fn new(file_path: &PathBuf, id: &str) -> Self {
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
        let mime_type = mime_guess::from_path(file_path)
            .first()
            .map(|m| m.to_string());

        Self {
            id: id.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            file_name,
            file_size,
            mime_type,
            created_at: Utc::now().timestamp(),
            expires_at: None,
            download_count: 0,
            revoked: false,
        }
    }
}

/// A clipboard history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub source: String,
    pub created_at: i64,
    pub is_favorite: bool,
    pub category: String,
    pub ocr_text: Option<String>,
    pub metadata_json: Option<String>,
    pub hash: String,
}

impl ClipboardEntry {
    pub fn new(content: String, content_type: String, source: String) -> Self {
        let hash = Self::calculate_hash(&content);
        let category = Self::determine_category(&content, &content_type);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            content_type,
            source,
            created_at: chrono::Utc::now().timestamp(),
            is_favorite: false,
            category,
            ocr_text: None,
            metadata_json: None,
            hash,
        }
    }

    pub fn calculate_hash(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    pub fn determine_category(content: &str, content_type: &str) -> String {
        if content_type.starts_with("image/") {
            "image".to_string()
        } else if content.starts_with("http://") || content.starts_with("https://") {
            "link".to_string()
        } else if content.contains('{') && content.contains('}') && (content.contains("fn ") || content.contains("let ") || content.contains("const ") || content.contains("import ") || content.contains("def ") || content.contains("class ")) {
            "code".to_string()
        } else {
            "text".to_string()
        }
    }
}

/// A connected device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub last_seen: i64,
    pub is_trusted: bool,
    pub created_at: i64,
    pub battery_level: Option<u8>,
    pub storage_remaining_bytes: Option<u64>,
    pub connection_quality: Option<u8>,
    pub latency_ms: Option<u32>,
    pub color_theme: Option<String>,
    pub avatar_url: Option<String>,
    pub group_name: Option<String>,
    pub room_name: Option<String>,
}

/// A transfer log or active transfer task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTask {
    pub id: String,
    pub file_id: Option<String>,
    pub file_name: Option<String>,
    pub device_id: String,
    pub action: String, // "upload" | "download"
    pub status: String, // "pending" | "transferring" | "paused" | "completed" | "failed"
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub created_at: i64,
}

/// App configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub hostname: String,
    pub db_path: PathBuf,
    pub web_dir: PathBuf,
    pub clipboard_history_limit: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lynqo");

        Self {
            port: 7432,
            hostname: "lynqo".to_string(),
            db_path: data_dir.join("lynqo.db"),
            web_dir: PathBuf::from("web/browser-ui/dist"),
            clipboard_history_limit: 500,
        }
    }
}

/// WebSocket events broadcast to all clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    ClipboardUpdated { items: Vec<ClipboardEntry> },
    FileShared { file: SharedFile },
    FileRevoked { id: String },
    DeviceJoined { device: Device },
    DeviceLeft { device_id: String },
    DeviceUpdated { device: Device },
    TransferStarted { task: TransferTask },
    TransferProgress { task: TransferTask },
    TransferCompleted { task: TransferTask },
    TransferFailed { task: TransferTask },
    Pong,
}


use axum::{
    extract::{State, WebSocketUpgrade, ConnectInfo},
    extract::ws::{Message, WebSocket},
    response::IntoResponse,
    http::HeaderMap,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use std::net::SocketAddr;
use crate::state::AppState;

#[derive(Clone)]
pub struct WsHub {
    tx: broadcast::Sender<String>,
}

impl WsHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, msg: String) {
        let _ = self.tx.send(msg);
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let addr = connect_info
        .map(|ConnectInfo(a)| a)
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0)));

    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    ws.on_upgrade(move |socket| handle_socket(socket, state, addr, user_agent))
}

async fn handle_socket(socket: WebSocket, state: AppState, addr: SocketAddr, user_agent: Option<String>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.ws_hub.subscribe();

    // Create stable device ID using default hasher
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    addr.ip().to_string().hash(&mut hasher);
    if let Some(ref ua) = user_agent {
        ua.hash(&mut hasher);
    }
    let device_id = format!("web-{:x}", hasher.finish());

    // Determine device name based on User-Agent
    let ua_lower = user_agent.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
    let device_name = if ua_lower.contains("iphone") {
        "iPhone Browser".to_string()
    } else if ua_lower.contains("ipad") {
        "iPad Browser".to_string()
    } else if ua_lower.contains("android") {
        "Android Browser".to_string()
    } else if ua_lower.contains("macintosh") || ua_lower.contains("mac os x") {
        "Mac Browser".to_string()
    } else if ua_lower.contains("windows") {
        "Windows Browser".to_string()
    } else {
        "Web Browser".to_string()
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let device = lynqo_core::Device {
        id: device_id.clone(),
        name: device_name,
        user_agent: user_agent.clone(),
        ip_address: Some(addr.ip().to_string()),
        last_seen: now,
        is_trusted: true,
        created_at: now,
        battery_level: None,
        storage_remaining_bytes: None,
        connection_quality: Some(100),
        latency_ms: Some(5),
        color_theme: None,
        avatar_url: None,
        group_name: Some("Web Mesh".to_string()),
        room_name: None,
    };

    // Register device in DB
    let _ = state.db.upsert_device(&device).await;

    // Broadcast device joined event
    let join_event = lynqo_core::WsEvent::DeviceJoined { device: device.clone() };
    if let Ok(json) = serde_json::to_string(&join_event) {
        state.ws_hub.broadcast(json);
    }

    // Send initial state to new client
    send_initial_state(&mut sender, &state).await;

    // Spawn task to forward broadcast events → this client
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if sender.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Handle incoming messages from this client
    let state_clone = state.clone();
    let device_id_clone = device_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => handle_incoming(&text, &state_clone, &device_id_clone).await,
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // If either task exits, abort the other
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    // Clean up: delete device on disconnect
    let _ = state.db.delete_device(&device_id).await;

    // Broadcast device left event
    let leave_event = lynqo_core::WsEvent::DeviceLeft { device_id };
    if let Ok(json) = serde_json::to_string(&leave_event) {
        state.ws_hub.broadcast(json);
    }
}

async fn send_initial_state(
    sender: &mut (impl SinkExt<Message, Error = axum::Error> + Unpin),
    state: &AppState,
) {
    if let Ok(items) = state.db.get_clipboard_history(50).await {
        let event = lynqo_core::WsEvent::ClipboardUpdated { items };
        if let Ok(json) = serde_json::to_string(&event) {
            let _ = sender.send(Message::Text(json)).await;
        }
    }
    if let Ok(files) = state.db.list_shared_files().await {
        for file in files {
            let event = lynqo_core::WsEvent::FileShared { file };
            if let Ok(json) = serde_json::to_string(&event) {
                let _ = sender.send(Message::Text(json)).await;
            }
        }
    }
}

async fn handle_incoming(text: &str, state: &AppState, device_id: &str) {
    #[derive(serde::Deserialize)]
    struct Incoming {
        #[serde(rename = "type")]
        kind: String,
        text: Option<String>,
        battery: Option<u8>,
        storage: Option<u64>,
    }

    let Ok(msg) = serde_json::from_str::<Incoming>(text) else {
        return;
    };

    match msg.kind.as_str() {
        "clipboard_push" => {
            let Some(content) = msg.text else { return };
            let entry = lynqo_core::ClipboardEntry::new(content.clone(), "text/plain".to_string(), "browser".to_string());
            if state.db.add_clipboard_entry(&entry).await.is_ok() {
                // Also set system clipboard
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(&content);
                }
                broadcast_clipboard(state).await;
            }
        }
        "device_details" => {
            if let Ok(devices) = state.db.list_devices().await {
                if let Some(mut dev) = devices.into_iter().find(|d| d.id == device_id) {
                    dev.battery_level = msg.battery;
                    dev.storage_remaining_bytes = msg.storage;
                    if state.db.upsert_device(&dev).await.is_ok() {
                        let event = lynqo_core::WsEvent::DeviceUpdated { device: dev };
                        if let Ok(json) = serde_json::to_string(&event) {
                            state.ws_hub.broadcast(json);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

pub async fn broadcast_clipboard(state: &AppState) {
    if let Ok(items) = state.db.get_clipboard_history(50).await {
        let event = lynqo_core::WsEvent::ClipboardUpdated { items };
        if let Ok(json) = serde_json::to_string(&event) {
            state.ws_hub.broadcast(json);
        }
    }
}

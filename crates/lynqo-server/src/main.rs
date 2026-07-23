use anyhow::Result;
use axum::{
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod routes;
mod state;
mod ws;

use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lynqo_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = lynqo_core::AppConfig::default();

    // Initialise database
    let db = lynqo_db::Database::new(&config.db_path).await?;

    // WebSocket hub
    let ws_hub = ws::WsHub::new();

    // Clipboard watcher → Tokio channel bridge
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<lynqo_clipboard::ClipboardPayload>();
    let (tok_tx, mut tok_rx) = tokio::sync::mpsc::channel::<lynqo_clipboard::ClipboardPayload>(64);
    lynqo_clipboard::start_watcher(raw_tx);

    // Bridge OS thread → Tokio
    std::thread::spawn(move || {
        while let Ok(payload) = raw_rx.recv() {
            let _ = tok_tx.blocking_send(payload);
        }
    });

    // Process clipboard events
    {
        let db = db.clone();
        let ws_hub = ws_hub.clone();
        tokio::spawn(async move {
            while let Some(payload) = tok_rx.recv().await {
                let entry = match payload {
                    lynqo_clipboard::ClipboardPayload::Text(text) => {
                        lynqo_core::ClipboardEntry::new(text, "text/plain".to_string(), "desktop".to_string())
                    }
                    lynqo_clipboard::ClipboardPayload::Image { width, height, rgba } => {
                        let bmp_bytes = encode_bmp(width as u32, height as u32, &rgba);
                        let base64_str = base64_encode(&bmp_bytes);
                        let data_url = format!("data:image/bmp;base64,{}", base64_str);
                        lynqo_core::ClipboardEntry::new(data_url, "image/bmp".to_string(), "desktop".to_string())
                    }
                };
                if let Err(e) = db.add_clipboard_entry(&entry).await {
                    tracing::error!("clipboard save: {e}");
                    continue;
                }
                if let Ok(items) = db.get_clipboard_history(50).await {
                    let event = lynqo_core::WsEvent::ClipboardUpdated { items };
                    if let Ok(json) = serde_json::to_string(&event) {
                        ws_hub.broadcast(json);
                    }
                }
            }
        });
    }

    // mDNS advertisement
    let discovery = lynqo_discovery::Discovery::advertise(&config.hostname, config.port)?;
    
    // Start browsing for other devices
    let discovery_rx = discovery.start_browser()?;
    
    let handle = tokio::runtime::Handle::current();
    // Spawn task to process discovery events
    {
        let db = db.clone();
        let ws_hub = ws_hub.clone();
        std::thread::spawn(move || {
            while let Ok(event) = discovery_rx.recv() {
                let db = db.clone();
                let ws_hub = ws_hub.clone();
                handle.spawn(async move {
                    match event {
                        lynqo_discovery::DiscoveryEvent::DeviceDiscovered { id, name, ip, port: _, platform, version } => {
                            let device = lynqo_core::Device {
                                id: id.clone(),
                                name,
                                user_agent: Some(format!("Lynqo/{} ({})", version, platform)),
                                ip_address: Some(ip),
                                last_seen: chrono::Utc::now().timestamp(),
                                is_trusted: false,
                                created_at: chrono::Utc::now().timestamp(),
                                battery_level: None,
                                storage_remaining_bytes: None,
                                connection_quality: Some(100),
                                latency_ms: Some(0),
                                color_theme: None,
                                avatar_url: None,
                                group_name: None,
                                room_name: None,
                            };
                            
                            if let Err(e) = db.upsert_device(&device).await {
                                tracing::error!("Failed to upsert discovered device: {e}");
                            } else {
                                tracing::info!("Discovered device: {}", device.id);
                                let event = lynqo_core::WsEvent::DeviceJoined { device };
                                if let Ok(json) = serde_json::to_string(&event) {
                                    ws_hub.broadcast(json);
                                }
                            }
                        }
                        lynqo_discovery::DiscoveryEvent::DeviceLost { id } => {
                            tracing::info!("Lost device: {}", id);
                            let event = lynqo_core::WsEvent::DeviceLeft { device_id: id };
                            if let Ok(json) = serde_json::to_string(&event) {
                                ws_hub.broadcast(json);
                            }
                        }
                    }
                });
            }
        });
    }

    let web_dir = config.web_dir.clone();
    let state = AppState {
        db,
        ws_hub,
        config: config.clone(),
    };

    let app = Router::new()
        .route("/api/status", get(routes::status::get_status))
        .route("/api/files", get(routes::files::list_files))
        .route("/api/files/share", post(routes::files::share_file))
        .route("/api/files/upload", post(routes::files::upload_file))
        .route("/api/files/:id", get(routes::files::stream_file))
        .route("/api/files/:id/thumbnail", get(routes::files::get_thumbnail))
        .route("/api/files/:id", delete(routes::files::revoke_file))
        .route("/api/clipboard", get(routes::clipboard::get_history))
        .route("/api/clipboard", post(routes::clipboard::push_clipboard))
        .route("/api/devices", get(routes::devices::list_devices))
        .route("/ws", get(ws::ws_handler))
        // Serve browser UI last (catch-all)
        .nest_service("/", tower_http::services::ServeDir::new(web_dir))
        .layer(CorsLayer::permissive())
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(axum::middleware::from_fn(no_cache_middleware))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("lynqo-server listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn no_cache_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    if let Some(content_type) = response.headers().get(axum::http::header::CONTENT_TYPE) {
        if content_type.to_str().unwrap_or("").contains("text/html") {
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
            );
        }
    }
    response
}

fn encode_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut bmp = Vec::new();
    
    // File header (14 bytes)
    bmp.extend_from_slice(b"BM"); // Signature
    let file_size = 54 + (width * height * 4);
    bmp.extend_from_slice(&file_size.to_le_bytes()); // File size
    bmp.extend_from_slice(&[0, 0, 0, 0]); // Reserved
    bmp.extend_from_slice(&54u32.to_le_bytes()); // Offset to pixel data
    
    // DIB header (40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes()); // Header size
    bmp.extend_from_slice(&(width as i32).to_le_bytes()); // Width
    bmp.extend_from_slice(&(-(height as i32)).to_le_bytes()); // Height
    bmp.extend_from_slice(&1u16.to_le_bytes()); // Color planes
    bmp.extend_from_slice(&32u16.to_le_bytes()); // Bits per pixel (32-bit for RGBA)
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Compression (0 = BI_RGB)
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Image size
    bmp.extend_from_slice(&0i32.to_le_bytes()); // X pixels per meter
    bmp.extend_from_slice(&0i32.to_le_bytes()); // Y pixels per meter
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Colors in color table
    bmp.extend_from_slice(&0u32.to_le_bytes()); // Important colors
    
    // Pixel data. BMP 32-bit BGRA layout. Swap R and B from RGBA.
    for chunk in rgba.chunks_exact(4) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let a = chunk[3];
        bmp.push(b);
        bmp.push(g);
        bmp.push(r);
        bmp.push(a);
    }
    
    bmp
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut it = input.iter().copied();
    loop {
        match (it.next(), it.next(), it.next()) {
            (Some(a), Some(b), Some(c)) => {
                out.push(ALPHABET[(a >> 2) as usize] as char);
                out.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
                out.push(ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize] as char);
                out.push(ALPHABET[(c & 63) as usize] as char);
            }
            (Some(a), Some(b), None) => {
                out.push(ALPHABET[(a >> 2) as usize] as char);
                out.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
                out.push(ALPHABET[((b & 15) << 2) as usize] as char);
                out.push('=');
                break;
            }
            (Some(a), None, _) => {
                out.push(ALPHABET[(a >> 2) as usize] as char);
                out.push(ALPHABET[((a & 3) << 4) as usize] as char);
                out.push('=');
                out.push('=');
                break;
            }
            _ => break,
        }
    }
    out
}

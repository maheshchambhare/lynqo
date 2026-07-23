use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum ClipboardPayload {
    Text(String),
    Image {
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
}

/// Start a background OS thread that polls the system clipboard every 500ms.
/// Sends new text or image content to `tx` whenever the clipboard changes.
pub fn start_watcher(tx: Sender<ClipboardPayload>) {
    thread::Builder::new()
        .name("lynqo-clipboard-watcher".into())
        .spawn(move || {
            let mut clipboard = match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("clipboard init failed: {e}");
                    return;
                }
            };
            let mut last_text = String::new();
            let mut last_img_hash = 0u64;

            loop {
                // Try text first
                if let Ok(text) = clipboard.get_text() {
                    if !text.is_empty() && text != last_text {
                        last_text = text.clone();
                        // Reset image tracking to prevent confusion if user copies a text version of same image
                        last_img_hash = 0;
                        if tx.send(ClipboardPayload::Text(text)).is_err() {
                            break; // receiver dropped
                        }
                    }
                } else if let Ok(img) = clipboard.get_image() {
                    // Try image
                    let rgba = img.bytes.to_vec();
                    let hash = hash_bytes(&rgba);
                    if hash != last_img_hash {
                        last_img_hash = hash;
                        // Reset text tracking
                        last_text.clear();
                        let payload = ClipboardPayload::Image {
                            width: img.width,
                            height: img.height,
                            rgba,
                        };
                        if tx.send(payload).is_err() {
                            break; // receiver dropped
                        }
                    }
                }
                thread::sleep(Duration::from_millis(500));
            }
            tracing::info!("clipboard watcher stopped");
        })
        .expect("failed to spawn clipboard watcher");
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

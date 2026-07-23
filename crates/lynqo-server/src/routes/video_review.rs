use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use lynqo_core::VideoComment;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ReviewQuery {
    pub pwd: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentPayload {
    pub timestamp_sec: Option<f64>,
    pub author_name: Option<String>,
    pub comment_text: String,
}

/// Serve interactive HTML5 video review web page
pub async fn serve_video_review_page_handler(
    State(state): State<AppState>,
    AxumPath(token): AxumPath<String>,
    axum::extract::Query(query): axum::extract::Query<ReviewQuery>,
) -> impl IntoResponse {
    let share = match state.db.get_public_share(&token).await {
        Ok(Some(s)) => s,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Html("<h1>404 - Review Link Expired or Invalid</h1>".to_string()),
            )
                .into_response();
        }
    };

    if share.revoked {
        return (
            StatusCode::GONE,
            Html("<h1>410 - This review link has been revoked</h1>".to_string()),
        )
            .into_response();
    }

    let file = match state.db.get_shared_file(&share.file_id).await {
        Ok(Some(f)) => f,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Html("<h1>404 - Original File Not Found</h1>".to_string()),
            )
                .into_response();
        }
    };

    // Password verification check if password is set
    if let Some(expected_hash) = &share.password_hash {
        let provided_pwd = query.pwd.as_deref().unwrap_or("");
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        provided_pwd.hash(&mut hasher);
        let provided_hash = format!("{:x}", hasher.finish());

        if provided_hash != *expected_hash {
            let has_error = !provided_pwd.is_empty();
            let html_content = render_password_page(&file.file_name, has_error);
            return (StatusCode::OK, Html(html_content)).into_response();
        }
    }

    let pwd_param = query.pwd.as_deref().unwrap_or("");
    let html_content = generate_review_player_html(&token, &file.file_name, file.file_size, pwd_param);
    (StatusCode::OK, Html(html_content)).into_response()
}

/// Serve video file bytes with full HTTP Range header support for 0-second instant seeking
pub async fn stream_video_review_handler(
    State(state): State<AppState>,
    AxumPath(token): AxumPath<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let share = match state.db.get_public_share(&token).await {
        Ok(Some(s)) => s,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    if share.revoked {
        return StatusCode::GONE.into_response();
    }

    let file = match state.db.get_shared_file(&share.file_id).await {
        Ok(Some(f)) => f,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let path = std::path::Path::new(&file.file_path);
    if !path.exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let file_size = file.file_size;
    let mime_type = file
        .mime_type
        .unwrap_or_else(|| "video/mp4".to_string());

    let mut tokio_file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if let Some(range_header) = headers.get(header::RANGE) {
        if let Ok(range_str) = range_header.to_str() {
            if let Some(bytes_str) = range_str.strip_prefix("bytes=") {
                let parts: Vec<&str> = bytes_str.split('-').collect();
                let start: u64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                let mut end: u64 = parts
                    .get(1)
                    .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
                    .unwrap_or(file_size.saturating_sub(1));

                if end >= file_size {
                    end = file_size.saturating_sub(1);
                }

                if start <= end {
                    let length = end - start + 1;
                    if tokio_file.seek(std::io::SeekFrom::Start(start)).await.is_ok() {
                        let mut chunk = vec![0u8; length as usize];
                        if tokio_file.read_exact(&mut chunk).await.is_ok() {
                            return Response::builder()
                                .status(StatusCode::PARTIAL_CONTENT)
                                .header(header::CONTENT_TYPE, mime_type)
                                .header(header::ACCEPT_RANGES, "bytes")
                                .header(
                                    header::CONTENT_RANGE,
                                    format!("bytes {}-{}/{}", start, end, file_size),
                                )
                                .header(header::CONTENT_LENGTH, length)
                                .body(Body::from(chunk))
                                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
                        }
                    }
                }
            }
        }
    }

    // Fallback: full stream
    let mut contents = Vec::new();
    if tokio_file.read_to_end(&mut contents).await.is_ok() {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_LENGTH, file_size)
            .body(Body::from(contents))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    } else {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

/// List all timeline comments for this share token
pub async fn list_video_comments_handler(
    State(state): State<AppState>,
    AxumPath(token): AxumPath<String>,
) -> impl IntoResponse {
    match state.db.list_video_comments(&token).await {
        Ok(comments) => Json(comments).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Add timestamped comment to timeline
pub async fn add_video_comment_handler(
    State(state): State<AppState>,
    AxumPath(token): AxumPath<String>,
    Json(payload): Json<CreateCommentPayload>,
) -> impl IntoResponse {
    let author = match payload.author_name {
        Some(ref a) if !a.trim().is_empty() => a.trim().to_string(),
        _ => "Client Reviewer".to_string(),
    };

    let timestamp_sec = payload.timestamp_sec.unwrap_or(0.0);

    let comment = VideoComment {
        id: uuid::Uuid::new_v4().to_string(),
        share_token: token.clone(),
        timestamp_sec,
        author_name: author,
        comment_text: payload.comment_text,
        created_at: chrono::Utc::now().timestamp(),
    };

    if let Err(e) = state.db.add_video_comment(&comment).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }

    // Broadcast live event to all connected desktop clients & web clients
    let event = lynqo_core::WsEvent::VideoCommentAdded {
        comment: comment.clone(),
    };
    if let Ok(json) = serde_json::to_string(&event) {
        state.ws_hub.broadcast(json);
    }

    (StatusCode::CREATED, Json(comment)).into_response()
}

/// Render modern password page with error state
pub fn render_password_page(file_name: &str, has_error: bool) -> String {
    let error_html = if has_error {
        r#"<div class="error-banner">
            <span class="error-icon">⚠️</span>
            <span>Incorrect password. Please check and try again.</span>
          </div>"#
    } else {
        ""
    };

    let error_class = if has_error { "shake error-input" } else { "" };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Protected Shared Link - Lynqo Studio</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg: #080C14;
      --card: #121826;
      --primary: #00F0FF;
      --accent: #7000FF;
      --text: #F8FAFC;
      --text-sub: #94A3B8;
      --border: rgba(255, 255, 255, 0.08);
      --danger: #EF4444;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: 'Plus Jakarta Sans', sans-serif; }}
    body {{
      background: var(--bg);
      color: var(--text);
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 1.5rem;
      background-image: 
        radial-gradient(circle at 50% 20%, rgba(112, 0, 255, 0.15), transparent 40%),
        radial-gradient(circle at 80% 80%, rgba(0, 240, 255, 0.1), transparent 35%);
    }}
    .card {{
      background: rgba(18, 24, 38, 0.85);
      backdrop-filter: blur(20px);
      border: 1px solid var(--border);
      border-radius: 24px;
      padding: 2.5rem;
      width: 100%;
      max-width: 420px;
      box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.6);
      text-align: center;
      position: relative;
    }}
    .card.shake {{
      animation: shake 0.4s cubic-bezier(.36,.07,.19,.97) both;
    }}
    @keyframes shake {{
      10%, 90% {{ transform: translate3d(-2px, 0, 0); }}
      20%, 80% {{ transform: translate3d(4px, 0, 0); }}
      30%, 50%, 70% {{ transform: translate3d(-6px, 0, 0); }}
      40%, 60% {{ transform: translate3d(6px, 0, 0); }}
    }}
    .icon-wrapper {{
      width: 64px;
      height: 64px;
      margin: 0 auto 1.25rem;
      background: linear-gradient(135deg, rgba(0, 240, 255, 0.1), rgba(112, 0, 255, 0.15));
      border: 1px solid rgba(0, 240, 255, 0.3);
      border-radius: 20px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 1.75rem;
      box-shadow: 0 0 20px rgba(0, 240, 255, 0.15);
    }}
    h2 {{ font-size: 1.35rem; font-weight: 800; margin-bottom: 0.35rem; letter-spacing: -0.3px; }}
    .filename-tag {{ font-size: 0.85rem; color: var(--primary); font-weight: 600; margin-bottom: 1.25rem; word-break: break-word; }}
    .subtitle {{ font-size: 0.875rem; color: var(--text-sub); margin-bottom: 1.5rem; line-height: 1.5; }}
    
    .error-banner {{
      background: rgba(239, 68, 68, 0.12);
      border: 1px solid rgba(239, 68, 68, 0.3);
      color: #FCA5A5;
      padding: 0.75rem 1rem;
      border-radius: 12px;
      font-size: 0.825rem;
      font-weight: 600;
      display: flex;
      align-items: center;
      gap: 0.5rem;
      margin-bottom: 1.25rem;
      text-align: left;
    }}
    
    input[type="password"] {{
      width: 100%;
      padding: 0.85rem 1.1rem;
      background: rgba(8, 12, 20, 0.7);
      border: 1px solid var(--border);
      border-radius: 14px;
      color: var(--text);
      font-size: 1rem;
      outline: none;
      transition: all 0.2s;
      margin-bottom: 1.25rem;
    }}
    input[type="password"]:focus {{
      border-color: var(--primary);
      box-shadow: 0 0 15px rgba(0, 240, 255, 0.25);
    }}
    input.error-input {{
      border-color: var(--danger);
      box-shadow: 0 0 15px rgba(239, 68, 68, 0.25);
    }}
    
    .btn-unlock {{
      width: 100%;
      padding: 0.85rem;
      background: linear-gradient(135deg, var(--primary), #00A3FF);
      color: #000;
      border: none;
      border-radius: 14px;
      font-weight: 800;
      font-size: 0.95rem;
      cursor: pointer;
      transition: all 0.2s;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 0.5rem;
      box-shadow: 0 4px 20px rgba(0, 240, 255, 0.3);
    }}
    .btn-unlock:hover {{
      transform: translateY(-1px);
      filter: brightness(1.1);
      box-shadow: 0 6px 24px rgba(0, 240, 255, 0.4);
    }}
  </style>
</head>
<body>
  <div class="card {error_class}">
    <div class="icon-wrapper">🔒</div>
    <h2>Protected Shared File</h2>
    <div class="filename-tag">{file_name}</div>
    <p class="subtitle">This review link is password protected. Enter password to gain full access.</p>
    {error_html}
    <form method="GET">
      <input type="password" name="pwd" class="{error_class}" placeholder="Enter password..." required autofocus />
      <button type="submit" class="btn-unlock">
        <span>🔓 Unlock & Access Media</span>
      </button>
    </form>
  </div>
</body>
</html>"#
    )
}

/// HTML Template generator for the Frame.io grade interactive video review web page
fn generate_review_player_html(token: &str, file_name: &str, file_size: u64, pwd: &str) -> String {
    let size_mb = format!("{:.1} MB", file_size as f64 / 1024.0 / 1024.0);
    let pwd_qs = if pwd.is_empty() { String::new() } else { format!("?pwd={}", pwd) };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
  <title>Lynqo Pro Review - {file_name}</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link href="https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg-dark: #080C14;
      --bg-card: #121826;
      --bg-elevated: #1A2234;
      --primary: #00F0FF;
      --accent-purple: #8B5CF6;
      --emerald: #10B981;
      --text-main: #F8FAFC;
      --text-sub: #94A3B8;
      --border: rgba(255, 255, 255, 0.08);
      --border-glow: rgba(0, 240, 255, 0.3);
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: 'Plus Jakarta Sans', sans-serif; -webkit-tap-highlight-color: transparent; }}
    body {{
      background: var(--bg-dark);
      color: var(--text-main);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      overflow-x: hidden;
      background-image: 
        radial-gradient(circle at 20% 15%, rgba(0, 240, 255, 0.04), transparent 40%),
        radial-gradient(circle at 80% 85%, rgba(139, 92, 246, 0.05), transparent 40%);
    }}

    /* Header Bar with Studio Doodles */
    header {{
      background: rgba(18, 24, 38, 0.85);
      backdrop-filter: blur(20px);
      -webkit-backdrop-filter: blur(20px);
      border-bottom: 1px solid var(--border);
      padding: 0.85rem 1.5rem;
      display: flex;
      justify-content: space-between;
      align-items: center;
      position: sticky;
      top: 0;
      z-index: 100;
      box-shadow: 0 4px 30px rgba(0, 0, 0, 0.3);
    }}
    .brand {{ display: flex; align-items: center; gap: 0.85rem; }}
    .logo-mark {{
      width: 40px;
      height: 40px;
      background: linear-gradient(135deg, var(--primary), var(--accent-purple));
      border-radius: 12px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 800;
      color: #000;
      font-size: 1.25rem;
      box-shadow: 0 0 20px rgba(0, 240, 255, 0.4);
      position: relative;
    }}
    .title-block h3 {{ font-size: 1rem; font-weight: 800; color: var(--text-main); letter-spacing: -0.3px; max-width: 400px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
    .title-block .meta {{ font-size: 0.775rem; color: var(--text-sub); display: flex; align-items: center; gap: 0.5rem; margin-top: 2px; flex-wrap: wrap; }}
    
    .badge-live {{ background: rgba(16, 185, 129, 0.12); color: var(--emerald); padding: 0.2rem 0.6rem; border-radius: 20px; font-weight: 700; font-size: 0.7rem; border: 1px solid rgba(16, 185, 129, 0.3); display: flex; align-items: center; gap: 5px; }}
    .badge-client {{ background: rgba(0, 240, 255, 0.12); color: var(--primary); padding: 0.2rem 0.6rem; border-radius: 20px; font-weight: 700; font-size: 0.7rem; border: 1px solid rgba(0, 240, 255, 0.3); }}

    .header-actions {{ display: flex; align-items: center; gap: 0.6rem; }}

    /* Layout Grid */
    .studio-grid {{
      display: grid;
      grid-template-columns: 1fr 380px;
      gap: 1.5rem;
      padding: 1.5rem;
      max-width: 1750px;
      margin: 0 auto;
      width: 100%;
      flex: 1;
    }}

    /* Video Viewport Container */
    .player-card {{
      background: var(--bg-card);
      border-radius: 24px;
      border: 1px solid var(--border);
      padding: 1.25rem;
      display: flex;
      flex-direction: column;
      gap: 1rem;
      box-shadow: 0 20px 50px rgba(0,0,0,0.5);
      position: relative;
    }}

    /* Studio Reticle Corner Doodles */
    .viewfinder {{
      position: relative;
      width: 100%;
      background: #000;
      border-radius: 16px;
      overflow: hidden;
      display: flex;
      justify-content: center;
      align-items: center;
      border: 1px solid rgba(255,255,255,0.06);
      box-shadow: inset 0 0 30px rgba(0,0,0,0.8);
    }}
    .reticle-corner {{
      position: absolute;
      width: 16px;
      height: 16px;
      border: 2px solid var(--primary);
      opacity: 0.6;
      pointer-events: none;
      z-index: 10;
    }}
    .reticle-tl {{ top: 12px; left: 12px; border-right: none; border-bottom: none; }}
    .reticle-tr {{ top: 12px; right: 12px; border-left: none; border-bottom: none; }}
    .reticle-bl {{ bottom: 12px; left: 12px; border-right: none; border-top: none; }}
    .reticle-br {{ bottom: 12px; right: 12px; border-left: none; border-top: none; }}

    video {{ width: 100%; max-height: 68vh; outline: none; border-radius: 12px; object-fit: contain; }}

    /* Timeline & Scrubber Bar */
    .timeline-container {{ position: relative; width: 100%; padding: 10px 0; cursor: pointer; }}
    .timeline-track {{ position: relative; height: 10px; background: rgba(255,255,255,0.08); border-radius: 8px; border: 1px solid rgba(255,255,255,0.05); }}
    .timeline-fill {{ height: 100%; background: linear-gradient(90deg, var(--primary), var(--accent-purple)); border-radius: 8px; width: 0%; pointer-events: none; transition: width 0.08s linear; box-shadow: 0 0 12px rgba(0,240,255,0.4); }}
    .timeline-marker {{
      position: absolute;
      top: -6px;
      width: 10px;
      height: 22px;
      background: var(--emerald);
      border-radius: 5px;
      transform: translateX(-50%);
      border: 2px solid #080C14;
      cursor: pointer;
      z-index: 15;
      box-shadow: 0 0 12px var(--emerald);
      transition: transform 0.15s, background-color 0.15s;
    }}
    .timeline-marker:hover {{ transform: translateX(-50%) scale(1.4); z-index: 25; background: #fff; }}

    /* Controls Bar */
    .controls-row {{
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.85rem;
      background: var(--bg-elevated);
      padding: 0.75rem 1.25rem;
      border-radius: 16px;
      border: 1px solid var(--border);
      flex-wrap: wrap;
    }}
    .timecode-badge {{
      font-family: 'JetBrains Mono', monospace;
      font-size: 0.95rem;
      color: var(--primary);
      font-weight: 700;
      background: rgba(0,240,255,0.08);
      padding: 6px 12px;
      border-radius: 10px;
      border: 1px solid rgba(0,240,255,0.25);
      letter-spacing: -0.5px;
    }}

    .btn {{
      background: linear-gradient(135deg, var(--primary), #00A3FF);
      color: #000;
      border: none;
      padding: 0.65rem 1.15rem;
      border-radius: 12px;
      font-weight: 800;
      cursor: pointer;
      transition: all 0.2s;
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      font-size: 0.85rem;
      box-shadow: 0 4px 16px rgba(0, 240, 255, 0.25);
      min-height: 40px;
    }}
    .btn:hover {{ transform: translateY(-1px); filter: brightness(1.1); box-shadow: 0 6px 20px rgba(0,240,255,0.35); }}
    .btn:active {{ transform: translateY(0); }}
    .btn-secondary {{
      background: rgba(255,255,255,0.06);
      color: var(--text-main);
      border: 1px solid var(--border);
      box-shadow: none;
    }}
    .btn-secondary:hover {{ background: rgba(255,255,255,0.12); border-color: rgba(255,255,255,0.2); }}

    .select-speed {{
      background: rgba(255,255,255,0.06);
      color: var(--text-main);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 0.4rem 0.6rem;
      font-size: 0.8rem;
      font-weight: 700;
      outline: none;
      cursor: pointer;
    }}

    /* Sidebar Feedback Panel */
    .sidebar-panel {{
      background: var(--bg-card);
      border-radius: 24px;
      border: 1px solid var(--border);
      padding: 1.25rem;
      display: flex;
      flex-direction: column;
      gap: 1rem;
      max-height: calc(100vh - 120px);
      box-shadow: 0 20px 50px rgba(0,0,0,0.5);
    }}
    .panel-header {{ display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid var(--border); padding-bottom: 0.85rem; }}
    .panel-header h4 {{ font-size: 1.05rem; font-weight: 800; color: var(--text-main); display: flex; align-items: center; gap: 0.5rem; }}
    .notes-counter {{ font-size: 0.775rem; color: var(--emerald); font-weight: 700; background: rgba(16,185,129,0.12); padding: 3px 10px; border-radius: 14px; border: 1px solid rgba(16,185,129,0.25); }}

    .comments-scroll {{ display: flex; flex-direction: column; gap: 0.85rem; overflow-y: auto; flex: 1; padding-right: 0.3rem; min-height: 200px; }}
    .comment-card {{
      background: var(--bg-elevated);
      border-radius: 14px;
      padding: 0.95rem 1.1rem;
      border: 1px solid var(--border);
      cursor: pointer;
      transition: all 0.2s;
      position: relative;
    }}
    .comment-card:hover {{ border-color: var(--primary); transform: translateX(3px); background: #202A3F; }}
    .comment-top {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; }}
    .comment-time {{ font-family: 'JetBrains Mono', monospace; font-size: 0.775rem; color: var(--emerald); font-weight: 700; background: rgba(16,185,129,0.12); padding: 3px 8px; border-radius: 6px; border: 1px solid rgba(16,185,129,0.2); }}
    .comment-author {{ font-size: 0.775rem; font-weight: 700; color: var(--primary); display: flex; align-items: center; gap: 6px; }}
    .comment-body {{ font-size: 0.875rem; color: var(--text-main); line-height: 1.45; word-break: break-word; }}

    /* Form Panel */
    .form-panel {{ background: var(--bg-elevated); padding: 1.1rem; border-radius: 16px; border: 1px solid var(--border); margin-top: auto; display: flex; flex-direction: column; gap: 0.75rem; }}
    .form-title {{ font-size: 0.85rem; font-weight: 800; color: var(--primary); display: flex; align-items: center; justify-content: space-between; }}
    input[type="text"], textarea {{ background: var(--bg-dark); border: 1px solid var(--border); border-radius: 10px; padding: 0.7rem 0.9rem; color: #fff; font-size: 0.875rem; outline: none; width: 100%; transition: border-color 0.2s; }}
    input[type="text"]:focus, textarea:focus {{ border-color: var(--primary); box-shadow: 0 0 12px rgba(0,240,255,0.25); }}

    /* Mobile Navigation Tabs (Hidden on Desktop) */
    .mobile-tab-bar {{ display: none; margin-bottom: 1rem; gap: 0.5rem; border-bottom: 1px solid var(--border); padding-bottom: 0.75rem; }}
    .mobile-tab-btn {{ flex: 1; padding: 0.65rem 0.5rem; background: var(--bg-elevated); border: 1px solid var(--border); color: var(--text-sub); border-radius: 12px; font-size: 0.8rem; font-weight: 700; cursor: pointer; text-align: center; }}
    .mobile-tab-btn.active {{ background: rgba(0, 240, 255, 0.12); color: var(--primary); border-color: rgba(0, 240, 255, 0.3); }}

    /* Mobile Responsive Breakpoint */
    @media (max-width: 900px) {{
      .studio-grid {{ grid-template-columns: 1fr; padding: 1rem; gap: 1rem; }}
      .title-block h3 {{ max-width: 200px; }}
      .mobile-tab-bar {{ display: flex; }}
      .player-card {{ border-radius: 18px; padding: 0.85rem; }}
      .sidebar-panel {{ max-height: none; border-radius: 18px; padding: 1rem; }}
      .btn {{ min-height: 44px; padding: 0.65rem 0.9rem; font-size: 0.825rem; }}
      header {{ padding: 0.75rem 1rem; }}
    }}
  </style>
</head>
<body>

  <!-- Header Bar -->
  <header>
    <div class="brand">
      <div class="logo-mark">
        <span>L</span>
      </div>
      <div class="title-block">
        <h3>{file_name}</h3>
        <div class="meta">
          <span>{size_mb}</span> · 
          <span class="badge-client">Client Review Mode</span> · 
          <span class="badge-live"><span style="width:6px;height:6px;background:var(--emerald);border-radius:50%;display:inline-block;animation:pulse 1.5s infinite;"></span> Live Sync</span>
        </div>
      </div>
    </div>
    <div class="header-actions">
      <button class="btn btn-secondary" onclick="copyShareLink()">
        <span>🔗 Copy Link</span>
      </button>
      <a href="/public/v/{token}/stream{pwd_qs}" download="{file_name}" class="btn btn-secondary" style="text-decoration:none;">
        <span>⬇️ Download</span>
      </a>
    </div>
  </header>

  <!-- Main Studio Grid -->
  <div class="studio-grid">
    
    <!-- Video Player Panel -->
    <div class="player-card" id="playerPanel">
      
      <!-- Viewport with Viewfinder Reticle Doodles -->
      <div class="viewfinder">
        <div class="reticle-corner reticle-tl"></div>
        <div class="reticle-corner reticle-tr"></div>
        <div class="reticle-corner reticle-bl"></div>
        <div class="reticle-corner reticle-br"></div>
        
        <video id="videoPlayer" preload="metadata" playsinline controls>
          <source src="/public/v/{token}/stream{pwd_qs}" type="video/mp4">
          Your browser does not support HTML5 video playback.
        </video>
      </div>

      <!-- Timeline & Scrubber Bar -->
      <div class="timeline-container" id="timelineBar">
        <div class="timeline-track">
          <div class="timeline-fill" id="timelineFill"></div>
          <div id="pinsContainer"></div>
        </div>
      </div>

      <!-- Controls Row -->
      <div class="controls-row">
        <div class="timecode-badge" id="timeDisplay">00:00:00.0</div>
        
        <div style="display: flex; gap: 0.5rem; align-items: center;">
          <button class="btn btn-secondary" onclick="stepFrame(-0.04)" title="Step 1 Frame Back (Left Arrow)">◄ Step Back</button>
          <button class="btn btn-secondary" onclick="stepFrame(0.04)" title="Step 1 Frame Forward (Right Arrow)">Step Fwd ►</button>
          
          <select class="select-speed" onchange="video.playbackRate = parseFloat(this.value)">
            <option value="0.5">0.5x</option>
            <option value="1.0" selected>1.0x</option>
            <option value="1.25">1.25x</option>
            <option value="1.5">1.5x</option>
            <option value="2.0">2.0x</option>
          </select>
        </div>

        <button class="btn" onclick="focusCommentInput()">
          <span>💬 Add Note at</span>
          <span id="btnTimeCode" style="font-family:'JetBrains Mono'; color:#000;">00:00:00</span>
        </button>
      </div>
    </div>

    <!-- Mobile Navigation Tab Bar (Shown on Mobile) -->
    <div class="mobile-tab-bar">
      <button class="mobile-tab-btn active" id="tabPlayer" onclick="switchMobileTab('player')">🎬 Player</button>
      <button class="mobile-tab-btn" id="tabNotes" onclick="switchMobileTab('notes')">💬 Notes (<span id="mobileNoteCount">0</span>)</button>
      <button class="mobile-tab-btn" id="tabForm" onclick="switchMobileTab('form')">➕ Add Note</button>
    </div>

    <!-- Feedback Sidebar Panel -->
    <div class="sidebar-panel" id="sidebarPanel">
      <div class="panel-header">
        <h4>Review Feedback Notes</h4>
        <span class="notes-counter" id="commentCount">0 notes</span>
      </div>

      <!-- Notes Stream -->
      <div class="comments-scroll" id="commentsList">
        <div style="text-align: center; color: var(--text-sub); font-size: 0.85rem; margin-top: 3rem;">
          No review notes yet.<br>Click any frame on the video to leave timestamped feedback!
        </div>
      </div>

      <!-- WhatsApp Style Typing Indicator -->
      <div id="typingIndicator" style="display:none; align-items:center; gap:8px; font-size:0.8rem; color:var(--primary); font-weight:700; background:rgba(0,240,255,0.08); border:1px solid rgba(0,240,255,0.25); padding:6px 12px; border-radius:10px; margin-bottom:8px;">
        <span>💬</span>
        <span id="typingAuthor">👑 Editor is typing</span>
        <span style="display:inline-flex; gap:2px; font-weight:900;">
          <span style="animation: bounce 1.4s infinite 0.2s;">.</span>
          <span style="animation: bounce 1.4s infinite 0.4s;">.</span>
          <span style="animation: bounce 1.4s infinite 0.6s;">.</span>
        </span>
      </div>

      <!-- Feedback Form Panel -->
      <div class="form-panel" id="formPanel">
        <div class="form-title">
          <span>Leave Feedback Note</span>
          <span id="formTimestamp" style="font-family:'JetBrains Mono'; color:var(--emerald);">⏱ 00:00:00</span>
        </div>
        <input type="text" id="authorInput" placeholder="Your Name (e.g. Client / Director)" value="Client">
        <textarea id="commentInput" rows="3" placeholder="Type what needs to be changed at this exact timestamp..."></textarea>
        <button class="btn" style="width: 100%; justify-content: center;" onclick="submitComment()">
          <span>🚀 Submit Note to Editor</span>
        </button>
      </div>
    </div>

  </div>

  <style>
    @keyframes bounce {{
      0%, 80%, 100% {{ transform: translateY(0); }}
      40% {{ transform: translateY(-4px); }}
    }}
  </style>

  <script>
    const token = "{token}";
    const pwdParam = "{pwd_qs}";
    const video = document.getElementById('videoPlayer');
    const timeDisplay = document.getElementById('timeDisplay');
    const btnTimeCode = document.getElementById('btnTimeCode');
    const formTimestamp = document.getElementById('formTimestamp');
    const timelineFill = document.getElementById('timelineFill');
    const timelineBar = document.getElementById('timelineBar');
    const pinsContainer = document.getElementById('pinsContainer');
    const commentsList = document.getElementById('commentsList');

    let commentsData = [];
    let ws = null;
    let typingTimer = null;

    const commentInput = document.getElementById('commentInput');
    const authorInput = document.getElementById('authorInput');

    commentInput.addEventListener('input', () => {{
      if (ws && ws.readyState === WebSocket.OPEN) {{
        ws.send(JSON.stringify({{
          type: 'video_typing',
          share_token: token,
          author_name: authorInput.value.trim() || 'Client',
          is_typing: true
        }}));
        clearTimeout(typingTimer);
        typingTimer = setTimeout(() => {{
          ws.send(JSON.stringify({{
            type: 'video_typing',
            share_token: token,
            author_name: authorInput.value.trim() || 'Client',
            is_typing: false
          }}));
        }}, 2500);
      }}
    }});

    function formatTimeCode(sec) {{
      sec = Math.max(0, sec || 0);
      const h = Math.floor(sec / 3600);
      const m = Math.floor((sec % 3600) / 60);
      const s = Math.floor(sec % 60);
      const ms = Math.floor((sec % 1) * 10);
      const pad = (n) => n.toString().padStart(2, '0');
      return `${{pad(h)}}:${{pad(m)}}:${{pad(s)}}.${{ms}}`;
    }}

    video.addEventListener('timeupdate', () => {{
      const cur = video.currentTime || 0;
      const dur = video.duration || 1;
      timeDisplay.innerText = formatTimeCode(cur);
      btnTimeCode.innerText = formatTimeCode(cur).slice(0, 8);
      formTimestamp.innerText = `⏱ ${{formatTimeCode(cur).slice(0, 8)}}`;
      timelineFill.style.width = `${{(cur / dur) * 100}}%`;
    }});

    timelineBar.addEventListener('click', (e) => {{
      const rect = timelineBar.getBoundingClientRect();
      const pos = (e.clientX - rect.left) / rect.width;
      if (video.duration) {{
        video.currentTime = pos * video.duration;
      }}
    }});

    function stepFrame(delta) {{
      video.pause();
      video.currentTime = Math.max(0, Math.min(video.duration || 0, video.currentTime + delta));
    }}

    function focusCommentInput() {{
      video.pause();
      if (window.innerWidth <= 900) {{
        switchMobileTab('form');
      }}
      document.getElementById('commentInput').focus();
    }}

    function copyShareLink() {{
      navigator.clipboard.writeText(location.href);
      alert('✨ Review link copied to clipboard!');
    }}

    async function fetchComments() {{
      try {{
        const res = await fetch(`/public/v/${{token}}/comments${{pwdParam}}`);
        if (res.ok) {{
          commentsData = await res.json();
          renderComments();
        }}
      }} catch(e) {{ console.error("fetchComments err:", e); }}
    }}

    function renderComments() {{
      const countStr = `${{commentsData.length}} notes`;
      document.getElementById('commentCount').innerText = countStr;
      const mobileCounter = document.getElementById('mobileNoteCount');
      if (mobileCounter) mobileCounter.innerText = commentsData.length;

      pinsContainer.innerHTML = '';

      if (commentsData.length === 0) {{
        commentsList.innerHTML = `<div style="text-align: center; color: var(--text-sub); font-size: 0.85rem; margin-top: 3rem;">No review notes yet.<br>Click any frame on the video to leave timestamped feedback!</div>`;
        return;
      }}

      const dur = video.duration || 1;
      commentsList.innerHTML = commentsData.map(c => {{
        if (dur > 0) {{
          const pct = Math.min(100, Math.max(0, (c.timestamp_sec / dur) * 100));
          const pin = document.createElement('div');
          pin.className = 'timeline-marker';
          pin.style.left = `${{pct}}%`;
          pin.title = `${{c.author_name}}: ${{c.comment_text}}`;
          pin.onclick = (e) => {{
            e.stopPropagation();
            video.currentTime = c.timestamp_sec;
          }};
          pinsContainer.appendChild(pin);
        }}

        const isEditor = c.author_name.toLowerCase().includes('editor');
        const badgeIcon = isEditor ? '👑' : '👤';

        return `
          <div class="comment-card" onclick="seekToTime(${{c.timestamp_sec}})">
            <div class="comment-top">
              <span class="comment-author">${{badgeIcon}} ${{escapeHtml(c.author_name)}}</span>
              <span class="comment-time">⏱ ${{formatTimeCode(c.timestamp_sec).slice(0, 8)}}</span>
            </div>
            <div class="comment-body">${{escapeHtml(c.comment_text)}}</div>
          </div>
        `;
      }}).join('');
    }}

    function seekToTime(t) {{
      video.currentTime = t;
      if (window.innerWidth <= 900) {{
        switchMobileTab('player');
      }}
    }}

    async function submitComment() {{
      const text = document.getElementById('commentInput').value.trim();
      const author = document.getElementById('authorInput').value.trim() || 'Client Reviewer';
      if (!text) return alert('Please enter your feedback note');

      const payload = {{
        timestamp_sec: video.currentTime || 0,
        author_name: author,
        comment_text: text
      }};

      try {{
        const res = await fetch(`/public/v/${{token}}/comments${{pwdParam}}`, {{
          method: 'POST',
          headers: {{ 'Content-Type': 'application/json' }},
          body: JSON.stringify(payload)
        }});

        if (res.ok) {{
          document.getElementById('commentInput').value = '';
          if (ws && ws.readyState === WebSocket.OPEN) {{
            ws.send(JSON.stringify({{
              type: 'video_typing',
              share_token: token,
              author_name: author,
              is_typing: false
            }}));
          }}
          fetchComments();
          if (window.innerWidth <= 900) {{
            switchMobileTab('notes');
          }}
        }} else {{
          const errData = await res.json().catch(() => ({{}}));
          alert('Failed to submit comment: ' + (errData.error || res.statusText));
        }}
      }} catch(e) {{
        alert('Network error submitting comment: ' + e);
      }}
    }}

    function switchMobileTab(tab) {{
      const playerPanel = document.getElementById('playerPanel');
      const sidebarPanel = document.getElementById('sidebarPanel');
      const commentsList = document.getElementById('commentsList');
      const formPanel = document.getElementById('formPanel');
      
      const tabPlayer = document.getElementById('tabPlayer');
      const tabNotes = document.getElementById('tabNotes');
      const tabForm = document.getElementById('tabForm');

      [tabPlayer, tabNotes, tabForm].forEach(b => b && b.classList.remove('active'));

      if (tab === 'player') {{
        playerPanel.style.display = 'flex';
        sidebarPanel.style.display = 'flex';
        commentsList.style.display = 'flex';
        formPanel.style.display = 'flex';
        if (tabPlayer) tabPlayer.classList.add('active');
      }} else if (tab === 'notes') {{
        playerPanel.style.display = 'flex';
        sidebarPanel.style.display = 'flex';
        commentsList.style.display = 'flex';
        formPanel.style.display = 'none';
        if (tabNotes) tabNotes.classList.add('active');
      }} else if (tab === 'form') {{
        playerPanel.style.display = 'flex';
        sidebarPanel.style.display = 'flex';
        commentsList.style.display = 'none';
        formPanel.style.display = 'flex';
        if (tabForm) tabForm.classList.add('active');
      }}
    }}

    function escapeHtml(str) {{
      return (str || '').replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }}

    // Keyboard Shortcuts (Space: Play/Pause, Left/Right: Frame step)
    window.addEventListener('keydown', (e) => {{
      if (document.activeElement.tagName === 'INPUT' || document.activeElement.tagName === 'TEXTAREA') return;
      if (e.key === ' ') {{
        e.preventDefault();
        video.paused ? video.play() : video.pause();
      }} else if (e.key === 'ArrowLeft') {{
        e.preventDefault();
        stepFrame(-0.04);
      }} else if (e.key === 'ArrowRight') {{
        e.preventDefault();
        stepFrame(0.04);
      }}
    }});

    // Real-time WebSocket connection for instant zero-polling sync
    try {{
      const wsProtocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
      ws = new WebSocket(`${{wsProtocol}}//${{location.host}}/ws`);
      ws.onmessage = (event) => {{
        try {{
          const data = JSON.parse(event.data);
          if (data.type === 'video_comment_added' && data.comment.share_token === token) {{
            commentsData.push(data.comment);
            renderComments();
          }} else if (data.type === 'video_typing' && data.share_token === token) {{
            const author = data.author_name || '';
            const myName = authorInput.value.trim() || 'Client';
            const isSelf = author.toLowerCase() === myName.toLowerCase();
            const typingIndicator = document.getElementById('typingIndicator');
            const typingAuthor = document.getElementById('typingAuthor');

            if (!isSelf && data.is_typing) {{
              typingAuthor.innerText = `${{author}} is typing`;
              typingIndicator.style.display = 'flex';
            }} else if (!isSelf && !data.is_typing) {{
              typingIndicator.style.display = 'none';
            }}
          }}
        }} catch(e) {{}}
      }};
    }} catch(e) {{}}

    // Initial load
    fetchComments();
  </script>
</body>
</html>"#
    )
}

/// List all comments for a given file ID (across all public shares of that file)
pub async fn list_file_video_comments_handler(
    State(state): State<AppState>,
    AxumPath(file_id): AxumPath<String>,
) -> impl IntoResponse {
    match state.db.list_video_comments_for_file(&file_id).await {
        Ok(comments) => Json(comments).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

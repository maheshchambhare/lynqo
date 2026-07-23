use anyhow::{anyhow, Result};
use deadpool_sqlite::{Config, Pool, Runtime};
use lynqo_core::{ClipboardEntry, Device, SharedFile, TransferTask};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

#[derive(Clone)]
pub struct Database {
    pool: Pool,
}

impl Database {
    pub async fn new(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = Config::new(db_path);
        let pool = config.create_pool(Runtime::Tokio1)?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<()> {
        let conn = self.pool.get().await?;
        conn.interact(|conn| {
            conn.execute_batch(MIGRATIONS)?;
            // Dynamically alter tables to add new columns if they don't exist
            let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0", []);
            let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN category TEXT NOT NULL DEFAULT 'text'", []);
            let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN ocr_text TEXT", []);
            let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN metadata_json TEXT", []);
            let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN hash TEXT NOT NULL DEFAULT ''", []);

            let _ = conn.execute("ALTER TABLE devices ADD COLUMN battery_level INTEGER", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN storage_remaining_bytes INTEGER", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN connection_quality INTEGER", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN latency_ms INTEGER", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN color_theme TEXT", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN avatar_url TEXT", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN group_name TEXT", []);
            let _ = conn.execute("ALTER TABLE devices ADD COLUMN room_name TEXT", []);

            let _ = conn.execute("ALTER TABLE transfer_history ADD COLUMN file_name TEXT", []);
            let _ = conn.execute("ALTER TABLE transfer_history ADD COLUMN status TEXT NOT NULL DEFAULT 'completed'", []);
            let _ = conn.execute("ALTER TABLE transfer_history ADD COLUMN transferred_bytes INTEGER NOT NULL DEFAULT 0", []);
            let _ = conn.execute("ALTER TABLE transfer_history ADD COLUMN total_bytes INTEGER NOT NULL DEFAULT 0", []);

            conn.execute(
                "CREATE TABLE IF NOT EXISTS video_comments (
                    id            TEXT PRIMARY KEY,
                    share_token   TEXT NOT NULL,
                    timestamp_sec REAL NOT NULL DEFAULT 0.0,
                    author_name   TEXT NOT NULL DEFAULT 'Reviewer',
                    comment_text  TEXT NOT NULL,
                    created_at    INTEGER NOT NULL
                );",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_video_comments_token ON video_comments(share_token, timestamp_sec);",
                [],
            )?;
            Ok(())
        })
        .await
        .map_err(|e| anyhow!("interact error: {e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("migration SQL error: {e}"))?;
        Ok(())
    }

    // ── Clipboard ───────────────────────────────────────────────────────────

    pub async fn add_clipboard_entry(&self, entry: &ClipboardEntry) -> Result<()> {
        let entry = entry.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT OR IGNORE INTO clipboard_history \
                 (id, content, content_type, source, created_at, is_favorite, category, ocr_text, metadata_json, hash) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    entry.id,
                    entry.content,
                    entry.content_type,
                    entry.source,
                    entry.created_at,
                    entry.is_favorite as i64,
                    entry.category,
                    entry.ocr_text,
                    entry.metadata_json,
                    entry.hash
                ],
            )?;
            // Trim to 500 entries
            conn.execute(
                "DELETE FROM clipboard_history WHERE id NOT IN \
                 (SELECT id FROM clipboard_history ORDER BY created_at DESC LIMIT 500)",
                [],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn get_clipboard_history(&self, limit: usize) -> Result<Vec<ClipboardEntry>> {
        let conn = self.pool.get().await?;
        let entries = conn
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, content, content_type, source, created_at, is_favorite, category, ocr_text, metadata_json, hash \
                     FROM clipboard_history \
                     ORDER BY created_at DESC LIMIT ?1",
                )?;
                let rows = stmt.query_map(params![limit as i64], |row| {
                    Ok(ClipboardEntry {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        content_type: row.get(2)?,
                        source: row.get(3)?,
                        created_at: row.get(4)?,
                        is_favorite: row.get::<_, i64>(5)? != 0,
                        category: row.get(6)?,
                        ocr_text: row.get(7)?,
                        metadata_json: row.get(8)?,
                        hash: row.get(9)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(entries)
    }

    // ── Shared Files ────────────────────────────────────────────────────────

    pub async fn save_shared_file(&self, file: &SharedFile) -> Result<()> {
        let file = file.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT INTO shared_files \
                 (id, file_path, file_name, file_size, mime_type, \
                  created_at, expires_at, download_count, revoked) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    file.id,
                    file.file_path,
                    file.file_name,
                    file.file_size as i64,
                    file.mime_type,
                    file.created_at,
                    file.expires_at,
                    file.download_count as i64,
                    file.revoked as i64
                ],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn get_shared_file(&self, id: &str) -> Result<Option<SharedFile>> {
        let id = id.to_string();
        let conn = self.pool.get().await?;
        let result = conn
            .interact(move |conn| {
                conn.query_row(
                    "SELECT id, file_path, file_name, file_size, mime_type, \
                     created_at, expires_at, download_count, revoked \
                     FROM shared_files WHERE id=?1 AND revoked=0",
                    params![id],
                    |row| {
                        Ok(SharedFile {
                            id: row.get(0)?,
                            file_path: row.get(1)?,
                            file_name: row.get(2)?,
                            file_size: row.get::<_, i64>(3)? as u64,
                            mime_type: row.get(4)?,
                            created_at: row.get(5)?,
                            expires_at: row.get(6)?,
                            download_count: row.get::<_, i64>(7)? as u64,
                            revoked: row.get::<_, i64>(8)? != 0,
                        })
                    },
                )
                .optional()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(result)
    }

    pub async fn list_shared_files(&self) -> Result<Vec<SharedFile>> {
        let conn = self.pool.get().await?;
        let files = conn
            .interact(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, file_path, file_name, file_size, mime_type, \
                     created_at, expires_at, download_count, revoked \
                     FROM shared_files WHERE revoked=0 ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(SharedFile {
                        id: row.get(0)?,
                        file_path: row.get(1)?,
                        file_name: row.get(2)?,
                        file_size: row.get::<_, i64>(3)? as u64,
                        mime_type: row.get(4)?,
                        created_at: row.get(5)?,
                        expires_at: row.get(6)?,
                        download_count: row.get::<_, i64>(7)? as u64,
                        revoked: row.get::<_, i64>(8)? != 0,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(files)
    }

    pub async fn revoke_shared_file(&self, id: &str) -> Result<bool> {
        let id = id.to_string();
        let conn = self.pool.get().await?;
        let n = conn
            .interact(move |conn| {
                conn.execute(
                    "UPDATE shared_files SET revoked=1 WHERE id=?1",
                    params![id],
                )
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(n > 0)
    }

    pub async fn increment_download_count(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "UPDATE shared_files SET download_count=download_count+1 WHERE id=?1",
                params![id],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    // ── Public Shares ─────────────────────────────────────────────────────────

    pub async fn create_public_share(&self, share: &lynqo_core::PublicShare) -> Result<()> {
        let share = share.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT INTO public_shares (token, file_id, password_hash, max_downloads, download_count, expires_at, created_at, revoked) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    share.token,
                    share.file_id,
                    share.password_hash,
                    share.max_downloads,
                    share.download_count as i64,
                    share.expires_at,
                    share.created_at,
                    share.revoked as i64
                ],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn get_public_share(&self, token: &str) -> Result<Option<lynqo_core::PublicShare>> {
        let token = token.to_string();
        let conn = self.pool.get().await?;
        let res = conn
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT token, file_id, password_hash, max_downloads, download_count, expires_at, created_at, revoked \
                     FROM public_shares WHERE token=?1 AND revoked=0",
                )?;
                let share = stmt
                    .query_row(params![token], |row| {
                        let download_count_i: i64 = row.get(4)?;
                        let max_downloads_i: Option<i64> = row.get(3)?;
                        let revoked_i: i64 = row.get(7)?;
                        Ok(lynqo_core::PublicShare {
                            token: row.get(0)?,
                            file_id: row.get(1)?,
                            password_hash: row.get(2)?,
                            max_downloads: max_downloads_i.map(|v| v as u64),
                            download_count: download_count_i as u64,
                            expires_at: row.get(5)?,
                            created_at: row.get(6)?,
                            revoked: revoked_i != 0,
                        })
                    })
                    .optional()?;
                Ok::<_, rusqlite::Error>(share)
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(res)
    }

    pub async fn increment_public_share_download(&self, token: &str) -> Result<()> {
        let token = token.to_string();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "UPDATE public_shares SET download_count=download_count+1 WHERE token=?1",
                params![token],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn revoke_public_share(&self, token: &str) -> Result<bool> {
        let token = token.to_string();
        let conn = self.pool.get().await?;
        let n = conn
            .interact(move |conn| {
                conn.execute(
                    "UPDATE public_shares SET revoked=1 WHERE token=?1",
                    params![token],
                )
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(n > 0)
    }

    // ── Video Comments ───────────────────────────────────────────────────────

    pub async fn add_video_comment(&self, comment: &lynqo_core::VideoComment) -> Result<()> {
        let comment = comment.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT INTO video_comments (id, share_token, timestamp_sec, author_name, comment_text, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    comment.id,
                    comment.share_token,
                    comment.timestamp_sec,
                    comment.author_name,
                    comment.comment_text,
                    comment.created_at,
                ],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn list_video_comments(&self, share_token: &str) -> Result<Vec<lynqo_core::VideoComment>> {
        let share_token = share_token.to_string();
        let conn = self.pool.get().await?;
        let comments = conn
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, share_token, timestamp_sec, author_name, comment_text, created_at \
                     FROM video_comments WHERE share_token = ?1 ORDER BY timestamp_sec ASC",
                )?;
                let rows = stmt.query_map(params![share_token], |row| {
                    Ok(lynqo_core::VideoComment {
                        id: row.get(0)?,
                        share_token: row.get(1)?,
                        timestamp_sec: row.get(2)?,
                        author_name: row.get(3)?,
                        comment_text: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(comments)
    }

    pub async fn list_video_comments_for_file(&self, file_id: &str) -> Result<Vec<lynqo_core::VideoComment>> {
        let file_id = file_id.to_string();
        let conn = self.pool.get().await?;
        let comments = conn
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT vc.id, vc.share_token, vc.timestamp_sec, vc.author_name, vc.comment_text, vc.created_at \
                     FROM video_comments vc \
                     JOIN public_shares ps ON vc.share_token = ps.token \
                     WHERE ps.file_id = ?1 ORDER BY vc.created_at ASC",
                )?;
                let rows = stmt.query_map(params![file_id], |row| {
                    Ok(lynqo_core::VideoComment {
                        id: row.get(0)?,
                        share_token: row.get(1)?,
                        timestamp_sec: row.get(2)?,
                        author_name: row.get(3)?,
                        comment_text: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(comments)
    }

    pub async fn list_public_shares_for_file(&self, file_id: &str) -> Result<Vec<lynqo_core::PublicShare>> {
        let file_id = file_id.to_string();
        let conn = self.pool.get().await?;
        let shares = conn
            .interact(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT token, file_id, password_hash, max_downloads, download_count, expires_at, created_at, revoked \
                     FROM public_shares WHERE file_id = ?1 AND revoked = 0 ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map(params![file_id], |row| {
                    Ok(lynqo_core::PublicShare {
                        token: row.get(0)?,
                        file_id: row.get(1)?,
                        password_hash: row.get(2)?,
                        max_downloads: row.get(3)?,
                        download_count: row.get(4)?,
                        expires_at: row.get(5)?,
                        created_at: row.get(6)?,
                        revoked: row.get::<_, i64>(7)? != 0,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(shares)
    }

    // ── Devices ─────────────────────────────────────────────────────────────

    pub async fn upsert_device(&self, device: &Device) -> Result<()> {
        let device = device.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT INTO devices (id,name,user_agent,ip_address,last_seen,is_trusted,created_at,\
                 battery_level,storage_remaining_bytes,connection_quality,latency_ms,color_theme,avatar_url,group_name,room_name) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) \
                 ON CONFLICT(id) DO UPDATE SET \
                   name=excluded.name, \
                   user_agent=excluded.user_agent, \
                   ip_address=excluded.ip_address, \
                   last_seen=excluded.last_seen, \
                   battery_level=excluded.battery_level, \
                   storage_remaining_bytes=excluded.storage_remaining_bytes, \
                   connection_quality=excluded.connection_quality, \
                   latency_ms=excluded.latency_ms, \
                   color_theme=excluded.color_theme, \
                   avatar_url=excluded.avatar_url, \
                   group_name=excluded.group_name, \
                   room_name=excluded.room_name",
                params![
                    device.id,
                    device.name,
                    device.user_agent,
                    device.ip_address,
                    device.last_seen,
                    device.is_trusted as i64,
                    device.created_at,
                    device.battery_level,
                    device.storage_remaining_bytes,
                    device.connection_quality,
                    device.latency_ms,
                    device.color_theme,
                    device.avatar_url,
                    device.group_name,
                    device.room_name
                ],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn delete_device(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute("DELETE FROM devices WHERE id = ?1", params![id])?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn list_devices(&self) -> Result<Vec<Device>> {
        let conn = self.pool.get().await?;
        let devices = conn
            .interact(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id,name,user_agent,ip_address,last_seen,is_trusted,created_at,\
                     battery_level,storage_remaining_bytes,connection_quality,latency_ms,color_theme,avatar_url,group_name,room_name \
                     FROM devices ORDER BY last_seen DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(Device {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        user_agent: row.get(2)?,
                        ip_address: row.get(3)?,
                        last_seen: row.get(4)?,
                        is_trusted: row.get::<_, i64>(5)? != 0,
                        created_at: row.get(6)?,
                        battery_level: row.get(7)?,
                        storage_remaining_bytes: row.get(8)?,
                        connection_quality: row.get(9)?,
                        latency_ms: row.get(10)?,
                        color_theme: row.get(11)?,
                        avatar_url: row.get(12)?,
                        group_name: row.get(13)?,
                        room_name: row.get(14)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(devices)
    }

    // ── Transfer Tasks ──────────────────────────────────────────────────────

    pub async fn add_transfer_task(&self, task: &TransferTask) -> Result<()> {
        let task = task.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT INTO transfer_history \
                 (id, file_id, file_name, device_id, action, status, transferred_bytes, total_bytes, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    task.id,
                    task.file_id,
                    task.file_name,
                    task.device_id,
                    task.action,
                    task.status,
                    task.transferred_bytes as i64,
                    task.total_bytes as i64,
                    task.created_at
                ],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn update_transfer_task(&self, task: &TransferTask) -> Result<()> {
        let task = task.clone();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "UPDATE transfer_history SET \
                 status=?2, transferred_bytes=?3 \
                 WHERE id=?1",
                params![
                    task.id,
                    task.status,
                    task.transferred_bytes as i64
                ],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn list_transfer_tasks(&self) -> Result<Vec<TransferTask>> {
        let conn = self.pool.get().await?;
        let tasks = conn
            .interact(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, file_id, file_name, device_id, action, status, transferred_bytes, total_bytes, created_at \
                     FROM transfer_history ORDER BY created_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(TransferTask {
                        id: row.get(0)?,
                        file_id: row.get(1)?,
                        file_name: row.get(2)?,
                        device_id: row.get(3)?,
                        action: row.get(4)?,
                        status: row.get(5)?,
                        transferred_bytes: row.get::<_, i64>(6)? as u64,
                        total_bytes: row.get::<_, i64>(7)? as u64,
                        created_at: row.get(8)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(tasks)
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let key = key.to_string();
        let conn = self.pool.get().await?;
        let value = conn
            .interact(move |conn| {
                let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
                let mut rows = stmt.query(params![key])?;
                if let Some(row) = rows.next()? {
                    let val: String = row.get(0)?;
                    Ok(Some(val))
                } else {
                    Ok(None)
                }
            })
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(value)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let key = key.to_string();
        let value = value.to_string();
        let conn = self.pool.get().await?;
        conn.interact(move |conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .await
        .map_err(|e| anyhow!("{e}"))?
        .map_err(|e: rusqlite::Error| anyhow!("{e}"))?;
        Ok(())
    }
}

const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS clipboard_history (
    id            TEXT PRIMARY KEY,
    content       TEXT NOT NULL,
    content_type  TEXT NOT NULL DEFAULT 'text/plain',
    source        TEXT NOT NULL DEFAULT 'desktop',
    created_at    INTEGER NOT NULL,
    is_favorite   INTEGER NOT NULL DEFAULT 0,
    category      TEXT NOT NULL DEFAULT 'text',
    ocr_text      TEXT,
    metadata_json TEXT,
    hash          TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_clipboard_created
    ON clipboard_history(created_at DESC);

CREATE TABLE IF NOT EXISTS shared_files (
    id             TEXT PRIMARY KEY,
    file_path      TEXT NOT NULL,
    file_name      TEXT NOT NULL,
    file_size      INTEGER NOT NULL,
    mime_type      TEXT,
    created_at     INTEGER NOT NULL,
    expires_at     INTEGER,
    download_count INTEGER NOT NULL DEFAULT 0,
    revoked        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS devices (
    id                      TEXT PRIMARY KEY,
    name                    TEXT NOT NULL,
    user_agent              TEXT,
    ip_address              TEXT,
    last_seen               INTEGER NOT NULL,
    is_trusted              INTEGER NOT NULL DEFAULT 0,
    created_at              INTEGER NOT NULL,
    battery_level           INTEGER,
    storage_remaining_bytes INTEGER,
    connection_quality      INTEGER,
    latency_ms              INTEGER,
    color_theme             TEXT,
    avatar_url              TEXT,
    group_name              TEXT,
    room_name               TEXT
);

CREATE TABLE IF NOT EXISTS transfer_history (
    id                TEXT PRIMARY KEY,
    file_id           TEXT,
    file_name         TEXT,
    device_id         TEXT,
    action            TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'completed',
    transferred_bytes INTEGER NOT NULL DEFAULT 0,
    total_bytes       INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS public_shares (
    token              TEXT PRIMARY KEY,
    file_id            TEXT NOT NULL,
    password_hash      TEXT,
    max_downloads      INTEGER,
    download_count     INTEGER NOT NULL DEFAULT 0,
    expires_at         INTEGER,
    created_at         INTEGER NOT NULL,
    revoked            INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS video_comments (
    id            TEXT PRIMARY KEY,
    share_token   TEXT NOT NULL,
    timestamp_sec REAL NOT NULL DEFAULT 0.0,
    author_name   TEXT NOT NULL DEFAULT 'Reviewer',
    comment_text  TEXT NOT NULL,
    created_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_video_comments_token
    ON video_comments(share_token, timestamp_sec);
"#;


use anyhow::Result;
use lynqo_core::SharedFile;
use lynqo_db::Database;
use std::path::PathBuf;
use uuid::Uuid;

/// Create a share token for a file and persist it.
pub async fn share_file(path: PathBuf, db: &Database) -> Result<SharedFile> {
    let id = Uuid::new_v4().to_string();
    let file = SharedFile::new(&path, &id);
    db.save_shared_file(&file).await?;
    Ok(file)
}

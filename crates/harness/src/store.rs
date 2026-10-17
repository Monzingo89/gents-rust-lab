//! Single-process SQLite journal. Persistence does not imply automatic replay.
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
pub struct Store {
    connection: Arc<Mutex<Connection>>,
    _lock: Option<std::fs::File>,
}
impl Store {
    pub fn open(path: &str) -> Result<Self> {
        let lock = if path == ":memory:" {
            None
        } else {
            let parent = Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            std::fs::create_dir_all(parent)?;
            let mut options = std::fs::OpenOptions::new();
            options.create(true).truncate(false).read(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let lock = options.open(format!("{path}.lock"))?;
            lock.try_lock()
                .context("another harness owns this journal; use a different RUN_STORE")?;
            let db = options.open(path)?;
            drop(db);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            }
            Some(lock)
        };
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            bail!("journal schema newer than this harness; do not downgrade");
        }
        connection.execute_batch("PRAGMA secure_delete=ON; PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, payload TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0); PRAGMA user_version=1;")?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            _lock: lock,
        })
    }
    pub async fn save(&self, id: String, payload: String) -> Result<()> {
        if payload.len() > 8 * 1024 * 1024 {
            bail!("run journal snapshot exceeds 8 MiB limit");
        }
        let revision = serde_json::from_str::<serde_json::Value>(&payload)
            .ok()
            .and_then(|v| v["events"].as_array().map(|a| a.len() as i64))
            .unwrap_or(0);
        let db = self.connection.clone();
        tokio::task::spawn_blocking(move ||->Result<()> {db.lock().map_err(|_|anyhow::anyhow!("journal mutex poisoned"))?.execute("INSERT INTO runs(id,payload,revision) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,revision=excluded.revision WHERE runs.deleted=0 AND excluded.revision>=runs.revision",params![id,payload,revision])?;Ok(())}).await??;
        Ok(())
    }
    pub async fn load(&self) -> Result<Vec<(String, String)>> {
        let db = self.connection.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, String)>> {
            let db = db
                .lock()
                .map_err(|_| anyhow::anyhow!("journal mutex poisoned"))?;
            let mut q = db.prepare("SELECT id,payload FROM runs WHERE deleted=0")?;
            let rows = q
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await?
    }
    pub async fn delete(&self, id: String) -> Result<()> {
        let db = self.connection.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            db.lock()
                .map_err(|_| anyhow::anyhow!("journal mutex poisoned"))?
                .execute("UPDATE runs SET deleted=1,payload='' WHERE id=?1", [id])?;
            Ok(())
        })
        .await??;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn snapshots_replace_atomically_and_delete() {
        let s = Store::open(":memory:").unwrap();
        s.save("one".into(), "first".into()).await.unwrap();
        s.save("one".into(), "second".into()).await.unwrap();
        assert_eq!(
            s.load().await.unwrap(),
            vec![("one".into(), "second".into())]
        );
        s.delete("one".into()).await.unwrap();
        assert!(s.load().await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn stale_writes_cannot_replace_newer_state_or_resurrect_deleted_runs() {
        let s = Store::open(":memory:").unwrap();
        let newer = r#"{"events":[1,2],"status":"completed"}"#;
        s.save("run".into(), newer.into()).await.unwrap();
        s.save("run".into(), r#"{"events":[1],"status":"running"}"#.into())
            .await
            .unwrap();
        assert_eq!(s.load().await.unwrap()[0].1, newer);
        s.delete("run".into()).await.unwrap();
        s.save("run".into(), newer.into()).await.unwrap();
        assert!(s.load().await.unwrap().is_empty());
    }
}

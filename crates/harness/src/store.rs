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
        if version > 2 {
            bail!("journal schema newer than this harness; do not downgrade");
        }
        connection.execute_batch("PRAGMA secure_delete=ON; PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, payload TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS run_events(run_id TEXT NOT NULL,seq INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(run_id,seq)); PRAGMA user_version=2;")?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            _lock: lock,
        })
    }
    pub async fn save(&self, id: String, payload: String) -> Result<()> {
        // Bound individual records, not the lifetime of a multi-day run. Keep
        // the API's snapshot contract while storing immutable events separately.
        // Existing v1 snapshots migrate transactionally on their next save.
        let mut value = serde_json::from_str::<serde_json::Value>(&payload).ok();
        let events = value
            .as_mut()
            .and_then(|v| v.as_object_mut())
            .and_then(|v| v.get_mut("events"))
            .and_then(|v| v.as_array_mut())
            .map(std::mem::take);
        let revision = events.as_ref().map_or(0, |events| events.len() as i64);
        let metadata = if events.is_some() {
            serde_json::to_string(&value)?
        } else {
            payload
        };
        if metadata.len() > 8 * 1024 * 1024 {
            bail!("run journal metadata exceeds 8 MiB limit");
        }
        let events = events
            .map(|events| {
                events
                    .into_iter()
                    .map(|event| {
                        let payload = serde_json::to_string(&event)?;
                        if payload.len() > 2 * 1024 * 1024 {
                            bail!("run journal event exceeds 2 MiB limit");
                        }
                        Ok(payload)
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let db = self.connection.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut connection = db.lock().map_err(|_| anyhow::anyhow!("journal mutex poisoned"))?;
            let transaction = connection.transaction()?;
            let changed = transaction.execute("INSERT INTO runs(id,payload,revision) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,revision=excluded.revision WHERE runs.deleted=0 AND excluded.revision>=runs.revision",params![id,metadata,revision])?;
            if changed > 0 {
                if let Some(events) = events {
                    let mut insert = transaction.prepare("INSERT OR IGNORE INTO run_events(run_id,seq,payload) VALUES(?1,?2,?3)")?;
                    for (seq, event) in events.into_iter().enumerate() {
                        insert.execute(params![id, seq as i64, event])?;
                    }
                }
            }
            transaction.commit()?;
            Ok(())
        }).await??;
        Ok(())
    }
    pub async fn load(&self) -> Result<Vec<(String, String)>> {
        let db = self.connection.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<(String, String)>> {
            let db = db
                .lock()
                .map_err(|_| anyhow::anyhow!("journal mutex poisoned"))?;
            let mut q = db.prepare("SELECT id,payload FROM runs WHERE deleted=0")?;
            let mut rows: Vec<(String, String)> = q
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (id, payload) in &mut rows {
                let mut query =
                    db.prepare("SELECT payload FROM run_events WHERE run_id=?1 ORDER BY seq")?;
                let events = query
                    .query_map([id.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if !events.is_empty() {
                    let mut value: serde_json::Value = serde_json::from_str(payload)?;
                    value["events"] = serde_json::Value::Array(
                        events
                            .iter()
                            .map(|event| serde_json::from_str(event))
                            .collect::<std::result::Result<Vec<_>, _>>()?,
                    );
                    *payload = serde_json::to_string(&value)?;
                }
            }
            Ok(rows)
        })
        .await?
    }
    pub async fn delete(&self, id: String) -> Result<()> {
        let db = self.connection.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut connection = db
                .lock()
                .map_err(|_| anyhow::anyhow!("journal mutex poisoned"))?;
            let transaction = connection.transaction()?;
            transaction.execute("UPDATE runs SET deleted=1,payload='' WHERE id=?1", [&id])?;
            transaction.execute("DELETE FROM run_events WHERE run_id=?1", [&id])?;
            transaction.commit()?;
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
    async fn multi_day_event_history_survives_old_snapshot_limit_and_reopen() {
        let root = std::env::temp_dir().join(format!("gents-store-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("runs.sqlite3");
        let events = (0..10)
            .map(|seq| serde_json::json!({"seq":seq,"message":"x".repeat(900_000)}))
            .collect::<Vec<_>>();
        let snapshot = serde_json::json!({"events":events,"status":"running"}).to_string();
        assert!(snapshot.len() > 8 * 1024 * 1024);
        {
            let store = Store::open(path.to_str().unwrap()).unwrap();
            store.save("long".into(), snapshot.clone()).await.unwrap();
            assert_eq!(store.load().await.unwrap()[0].1, snapshot);
            let mut updated: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
            updated["events"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"seq":10,"message":"done"}));
            updated["status"] = serde_json::json!("completed");
            store
                .save("long".into(), updated.to_string())
                .await
                .unwrap();
            store.save("long".into(), snapshot).await.unwrap();
        }
        {
            let store = Store::open(path.to_str().unwrap()).unwrap();
            let loaded: serde_json::Value =
                serde_json::from_str(&store.load().await.unwrap()[0].1).unwrap();
            assert_eq!(loaded["status"], "completed");
            assert_eq!(loaded["events"].as_array().unwrap().len(), 11);
            store.delete("long".into()).await.unwrap();
            assert!(store.load().await.unwrap().is_empty());
            assert_eq!(
                store
                    .connection
                    .lock()
                    .unwrap()
                    .query_row("SELECT count(*) FROM run_events", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn legacy_snapshot_migrates_without_losing_events() {
        let store = Store::open(":memory:").unwrap();
        let legacy = r#"{"events":[{"seq":0,"message":"retained"}],"status":"interrupted"}"#;
        store
            .connection
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO runs(id,payload,revision) VALUES('legacy',?1,1)",
                [legacy],
            )
            .unwrap();
        assert_eq!(store.load().await.unwrap()[0].1, legacy);
        let next = r#"{"events":[{"seq":0,"message":"retained"},{"seq":1,"message":"resumed"}],"status":"running"}"#;
        store.save("legacy".into(), next.into()).await.unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&store.load().await.unwrap()[0].1).unwrap(),
            serde_json::from_str::<serde_json::Value>(next).unwrap()
        );
    }
    #[tokio::test]
    async fn oversized_event_rejection_preserves_previous_transaction() {
        let store = Store::open(":memory:").unwrap();
        let original = r#"{"events":[1],"status":"running"}"#;
        store.save("run".into(), original.into()).await.unwrap();
        let large = serde_json::json!({"events":[1,"x".repeat(2 * 1024 * 1024)]}).to_string();
        assert!(store.save("run".into(), large).await.is_err());
        assert_eq!(store.load().await.unwrap()[0].1, original);
    }
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

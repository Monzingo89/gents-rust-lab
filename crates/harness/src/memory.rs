//! Private, versioned memory indexes. Canonical context stays in the company
//! store; snapshots contain source references/hashes, never copied model text.
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

pub const QUIET_MS: u64 = 30_000;
pub const MAX_DIRTY_MS: u64 = 120_000;
static CHANGED: Notify = Notify::const_new();
pub fn wake() {
    CHANGED.notify_one();
}

#[derive(Debug)]
pub struct Clock {
    pub generation: i64,
    pub first_dirty: Option<u64>,
    pub last_dirty: Option<u64>,
}
impl Clock {
    pub fn deadline(&self) -> Option<u64> {
        Some(
            self.first_dirty?
                .saturating_add(MAX_DIRTY_MS)
                .min(self.last_dirty?.saturating_add(QUIET_MS)),
        )
    }
    pub fn due(&self, now: u64) -> bool {
        // A backward wall adjustment must not postpone durable dirty work
        // indefinitely. Scheduling early grants no extra source authority.
        self.first_dirty.is_some_and(|first| now < first)
            || self.last_dirty.is_some_and(|last| now < last)
            || self.deadline().is_some_and(|deadline| now >= deadline)
    }
}

pub fn install(db: &Connection) -> Result<()> {
    crate::semantic_memory::install(db)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS memory_clock(id INTEGER PRIMARY KEY CHECK(id=1),generation INTEGER NOT NULL,first_dirty INTEGER,last_dirty INTEGER);
        INSERT OR IGNORE INTO memory_clock VALUES(1,0,NULL,NULL);
        CREATE TABLE IF NOT EXISTS memory_snapshots(run_id TEXT NOT NULL,agent TEXT NOT NULL,revision INTEGER NOT NULL,generation INTEGER NOT NULL,policy_revision INTEGER NOT NULL,manifest TEXT NOT NULL,sha256 TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(run_id,agent,revision));")?;
    // SQLite triggers cover every existing write path, including operator
    // recalls and permission changes. Raw data and dirty state commit together.
    for table in ["context", "jobs", "run_governance", "operator_recalls"] {
        for operation in ["INSERT", "UPDATE", "DELETE"] {
            db.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS memory_{table}_{operation} AFTER {operation} ON {table} BEGIN
                UPDATE memory_clock SET generation=generation+1,
                    first_dirty=COALESCE(first_dirty,CAST(unixepoch('subsec')*1000 AS INTEGER)),
                    last_dirty=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE id=1; END;"))?;
        }
    }
    Ok(())
}

pub fn clock(db: &Connection) -> Result<Clock> {
    Ok(db.query_row(
        "SELECT generation,first_dirty,last_dirty FROM memory_clock WHERE id=1",
        [],
        |r| {
            Ok(Clock {
                generation: r.get(0)?,
                first_dirty: r.get::<_, Option<i64>>(1)?.map(|v| v as u64),
                last_dirty: r.get::<_, Option<i64>>(2)?.map(|v| v as u64),
            })
        },
    )?)
}

/// The access-controlled company adapter supplies the projection. This store
/// cannot query another agent's context or grant memory access itself.
pub struct SqliteSnapshots<'a> {
    pub db: &'a Connection,
}
impl SqliteSnapshots<'_> {
    pub fn latest(&self, run: &str, agent: &str) -> Result<Option<Value>> {
        let row:Option<(String,String,i64,i64,i64,i64)>=self.db.query_row(
            "SELECT manifest,sha256,revision,generation,policy_revision,created FROM memory_snapshots WHERE run_id=?1 AND agent=?2 ORDER BY revision DESC LIMIT 1",
            params![run,agent],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
        row.map(|(body, hash, revision, generation, policy, created)| {
            if format!("{:x}", Sha256::digest(body.as_bytes())) != hash {
                bail!("memory snapshot integrity check failed");
            }
            let manifest: Value = serde_json::from_str(&body)?;
            if manifest["revision"] != revision
                || manifest["source_generation"] != generation
                || manifest["policy_revision"] != policy
                || manifest["created_at_ms"] != created
                || manifest["run_id"] != run
                || manifest["recipient"] != agent
            {
                bail!("memory snapshot metadata binding failed");
            }
            Ok(
                json!({"manifest":manifest,"sha256":hash,"revision":revision,
                "source_generation":generation,"policy_revision":policy,"created_at_ms":created}),
            )
        })
        .transpose()
    }
    pub fn append(
        &self,
        run: &str,
        agent: &str,
        generation: i64,
        policy: u64,
        manifest: &Value,
        now: u64,
    ) -> Result<Value> {
        let revision: i64 = self.db.query_row(
            "SELECT COALESCE(MAX(revision),0)+1 FROM memory_snapshots WHERE run_id=?1 AND agent=?2",
            params![run, agent],
            |r| r.get(0),
        )?;
        if manifest["run_id"] != run || manifest["recipient"] != agent {
            bail!("memory projection scope mismatch");
        }
        // Scan source-derived metadata before adding trusted runtime counters
        // and epoch timestamps (which are not payment-card input).
        crate::security::require_persistable(&manifest.to_string())?;
        let mut manifest = manifest.clone();
        manifest["revision"] = json!(revision);
        manifest["source_generation"] = json!(generation);
        manifest["policy_revision"] = json!(policy);
        manifest["created_at_ms"] = json!(now);
        let body = serde_json::to_string(&manifest)?;
        if body.len() > 64_000 {
            bail!("memory manifest exceeds bounded index size");
        }
        let hash = format!("{:x}", Sha256::digest(body.as_bytes()));
        self.db.execute(
            "INSERT INTO memory_snapshots VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                run,
                agent,
                revision,
                generation,
                policy as i64,
                body,
                hash,
                now as i64
            ],
        )?;
        // Bound derived cache versions; canonical context and audit records are
        // retained under their existing lifecycle policy, not deleted here.
        self.db.execute(
            "DELETE FROM memory_snapshots WHERE run_id=?1 AND agent=?2 AND revision<=?3",
            params![run, agent, revision - 64],
        )?;
        self.latest(run, agent)?.context("new snapshot missing")
    }
}

pub fn project(
    company: &str,
    run: &str,
    agent: &str,
    contract: &Value,
    live: &Value,
) -> Result<Value> {
    let records = live["records"]
        .as_array()
        .context("authorized memory records missing")?;
    let refs=records.iter().map(|row|{
        let text=row["text"].as_str().unwrap_or_default();
        json!({"id":row["id"],"source":row["source"],"tier":row["tier"],"sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"trust":if row["source"]=="model_output"{"advisory_not_verified"}else{"source_reference_not_authority"}})
    }).collect::<Vec<_>>();
    Ok(
        json!({"version":1,"company_id":company,"run_id":run,"recipient":agent,
        "contract_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(contract)?)),
        "source_refs":refs,"model_text_copied":false,"authority":"index only; live authorization and original source records remain authoritative",
        "coverage":"bounded relevance/recent index; complete canonical history remains in authorized company retrieval"}),
    )
}

pub fn descriptor(snapshot: &Value, current: i64) -> Value {
    json!({"revision":snapshot["revision"],"sha256":snapshot["sha256"],"policy_revision":snapshot["policy_revision"],
        "source_generation":snapshot["source_generation"],"current_generation":current,
        "pending_updates":snapshot["source_generation"].as_i64()!=Some(current),
        "contract_sha256":snapshot["manifest"]["contract_sha256"],"created_at_ms":snapshot["created_at_ms"],
        "model_text_copied":false})
}

pub async fn maintain(app: crate::Shared) {
    loop {
        if app.shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        let active = {
            let runs = app.runs.lock().await;
            let mut companies =
                std::collections::BTreeMap::<(String, String), Vec<uuid::Uuid>>::new();
            for (id, run) in runs.iter().filter(|(_, r)| r.status == "running") {
                if let Some(company) = &run.company_id {
                    companies
                        .entry((company.clone(), run.owner.clone()))
                        .or_default()
                        .push(*id);
                }
            }
            companies
        };
        let mut wait_ms = 30_000;
        for ((company, owner), runs) in active {
            let company_clone = company.clone();
            let owner_clone = owner.clone();
            let observed_runs = runs.clone();
            let work = tokio::task::spawn_blocking(move || {
                crate::company::flush_memory(
                    &std::env::current_dir()?,
                    &company_clone,
                    &owner_clone,
                    &runs,
                    crate::now_ms(),
                )
            })
            .await;
            match work {
                Ok(Ok((deadline, events))) => {
                    if let Some(deadline) = deadline {
                        wait_ms = wait_ms.min(deadline.saturating_sub(crate::now_ms()).max(1));
                    }
                    for (run, agent, metadata) in events {
                        let _ = crate::emit(
                            &app,
                            run,
                            (&agent, "memory"),
                            ("observability", "memory_snapshot_created"),
                            &metadata.to_string(),
                            None,
                            None,
                        )
                        .await;
                    }
                    if app.config.semantic_memory.is_some() {
                        for run in &observed_runs {
                            for actor in &app.config.agents {
                                match crate::semantic_memory::summarize(
                                    &app, *run, &actor.id, false,
                                )
                                .await
                                {
                                    Ok(true) => break,
                                    Ok(false) => {}
                                    Err(_) => {
                                        let _=crate::emit(&app,*run,(&actor.id,"memory_summary"),("observability","semantic_memory_unavailable"),"Derived memory unavailable; canonical source records retained.",None,None).await;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                failure => {
                    let reason = match failure {
                        Ok(Err(error)) => error.to_string(),
                        Err(error) => error.to_string(),
                        _ => "unknown memory worker failure".into(),
                    };
                    let reason = if crate::security::assess(&reason).persisted {
                        reason.chars().take(600).collect::<String>()
                    } else {
                        "Sensitive diagnostic omitted".into()
                    };
                    for run in observed_runs {
                        let _=crate::emit(&app,run,("Memory Service","memory"),("observability","memory_snapshot_failed"),
                            &json!({"reason":reason,"access_fails_closed":true,"payload_logged":false}).to_string(),None,None).await;
                    }
                }
            }
        }
        tokio::select! {_ = CHANGED.notified()=>{},_ = tokio::time::sleep(std::time::Duration::from_millis(wait_ms))=>{}}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versioned_indexes_are_bounded_and_bound_their_scope_and_metadata() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE context(id TEXT);CREATE TABLE jobs(id TEXT);CREATE TABLE run_governance(id TEXT);CREATE TABLE operator_recalls(id TEXT);").unwrap();
        install(&db).unwrap();
        let store = SqliteSnapshots { db: &db };
        let manifest = project(
            "one",
            "run",
            "Reader",
            &json!({"idea":"Frozen task"}),
            &json!({"records":[]}),
        )
        .unwrap();
        for generation in 1..=70 {
            store
                .append("run", "Reader", generation, 1, &manifest, 4111111111111111)
                .unwrap();
        }
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM memory_snapshots", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            64
        );
        assert_eq!(
            store.latest("run", "Reader").unwrap().unwrap()["revision"],
            70
        );
        assert!(store
            .append("other-run", "Reader", 71, 1, &manifest, 0)
            .is_err());
        db.execute(
            "UPDATE memory_snapshots SET policy_revision=2 WHERE revision=70",
            [],
        )
        .unwrap();
        assert!(store.latest("run", "Reader").is_err());
    }
    #[test]
    fn quiet_debounce_has_a_maximum_dirty_window() {
        let clock = Clock {
            generation: 1,
            first_dirty: Some(1000),
            last_dirty: Some(1000),
        };
        assert!(!clock.due(30_999));
        assert!(clock.due(31_000));
        let busy = Clock {
            last_dirty: Some(120_999),
            ..clock
        };
        assert_eq!(busy.deadline(), Some(121_000));
        assert!(busy.due(121_000));
    }
    #[test]
    fn backward_wall_adjustment_cannot_starve_dirty_memory() {
        let dirty = Clock {
            generation: 1,
            first_dirty: Some(100_000),
            last_dirty: Some(110_000),
        };
        assert!(dirty.due(90_000));
        assert!(!dirty.due(120_000));
        assert!(dirty.due(140_000));
        assert!(!Clock {
            generation: 0,
            first_dirty: None,
            last_dirty: None
        }
        .due(90_000));
    }

    #[test]
    fn projection_keeps_source_hashes_without_copying_private_model_text() {
        let live = json!({"records":[{"id":"r1","source":"model_output","tier":"ASTC","text":"PRIVATE-DUMMY-MARKER"}]});
        let value = project("one", "run", "Reader", &json!({"idea":"original"}), &live).unwrap();
        assert!(!value.to_string().contains("PRIVATE-DUMMY-MARKER"));
        assert_eq!(
            value["source_refs"][0]["sha256"],
            format!("{:x}", Sha256::digest(b"PRIVATE-DUMMY-MARKER"))
        );
    }
}

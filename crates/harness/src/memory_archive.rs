//! Local archive storage and metadata checkpoints. Callers own authorization and
//! a single SQLite transaction; these records never approve or recall content.
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub enum ContextLifecycle {
    Completed,
    Inactive,
    CandidateSubmitted,
}
impl ContextLifecycle {
    fn key(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Inactive => "inactive",
            Self::CandidateSubmitted => "candidate_submitted",
        }
    }
    fn tiers(self) -> (&'static str, &'static str) {
        match self {
            Self::Completed => ("ALTC", "EVIDENCE_ALTC"),
            Self::Inactive | Self::CandidateSubmitted => ("PLTC", "EVIDENCE_PLTC"),
        }
    }
}

pub fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS memory_archive_checkpoints(
        run_id TEXT NOT NULL,agent TEXT NOT NULL,lifecycle TEXT NOT NULL
            CHECK(lifecycle IN ('completed','inactive','candidate_submitted')),
        policy_revision INTEGER,contract_sha256 TEXT,record_count INTEGER NOT NULL CHECK(record_count>0),
        records_sha256 TEXT NOT NULL CHECK(length(records_sha256)=64),updated_ms INTEGER NOT NULL,
        PRIMARY KEY(run_id,agent,lifecycle));")?;
    Ok(())
}

pub struct SqliteContextArchive<'a> {
    pub db: &'a Connection,
}

fn hash_fields(hash: &mut Sha256, fields: &[&str]) {
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field.as_bytes());
    }
}
impl SqliteContextArchive<'_> {
    fn checkpoint(&self, run: Option<&str>, lifecycle: ContextLifecycle, now: u64) -> Result<()> {
        let mut groups = BTreeMap::<(String, String), (Sha256, i64)>::new();
        let mut query = self.db.prepare("SELECT run_id,agent,id,tier,source,body,created FROM context_archive WHERE (?1 IS NULL OR run_id=?1) ORDER BY run_id,agent,id")?;
        let mut rows = query.query([run])?;
        while let Some(row) = rows.next()? {
            let run: String = row.get(0)?;
            let actor: String = row.get(1)?;
            let (hash, count) = groups.entry((run.clone(), actor.clone())).or_default();
            let body: String = row.get(5)?;
            hash_fields(
                hash,
                &[
                    &run,
                    &actor,
                    &row.get::<_, String>(2)?,
                    &row.get::<_, String>(3)?,
                    &row.get::<_, String>(4)?,
                    &format!("{:x}", Sha256::digest(body.as_bytes())),
                    &row.get::<_, i64>(6)?.to_string(),
                ],
            );
            *count += 1;
        }
        drop(rows);
        drop(query);
        for ((run, actor), (hash, count)) in groups {
            let revision: Option<i64> = self
                .db
                .query_row(
                    "SELECT revision FROM run_governance WHERE run_id=?1",
                    [&run],
                    |r| r.get(0),
                )
                .optional()?;
            let contract: Option<String> = self
                .db
                .query_row("SELECT digest FROM jobs WHERE run_id=?1", [&run], |r| {
                    r.get(0)
                })
                .optional()?;
            self.db.execute("INSERT INTO memory_archive_checkpoints VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
                ON CONFLICT(run_id,agent,lifecycle) DO UPDATE SET policy_revision=excluded.policy_revision,contract_sha256=excluded.contract_sha256,record_count=excluded.record_count,records_sha256=excluded.records_sha256,updated_ms=excluded.updated_ms
                WHERE policy_revision IS NOT excluded.policy_revision OR contract_sha256 IS NOT excluded.contract_sha256 OR record_count<>excluded.record_count OR records_sha256<>excluded.records_sha256",
                params![run,actor,lifecycle.key(),revision,contract,count,format!("{:x}",hash.finalize()),i64::try_from(now)?])?;
        }
        Ok(())
    }
    pub fn archive(
        &self,
        run: Option<&str>,
        lifecycle: ContextLifecycle,
        now: u64,
    ) -> Result<usize> {
        ensure!(
            !self.db.is_autocommit(),
            "archive requires the caller's atomic transaction"
        );
        let (tier, evidence) = lifecycle.tiers();
        let moved = self.db.execute("INSERT OR REPLACE INTO context_archive SELECT id,run_id,agent,CASE WHEN tier GLOB 'EVIDENCE*' THEN ?3 ELSE ?2 END,body,source,created FROM context WHERE (?1 IS NULL OR run_id=?1)",params![run,tier,evidence])?;
        self.db.execute("UPDATE context_archive SET tier=CASE WHEN tier GLOB 'EVIDENCE*' THEN ?3 ELSE ?2 END WHERE (?1 IS NULL OR run_id=?1)",params![run,tier,evidence])?;
        self.db
            .execute("DELETE FROM context WHERE (?1 IS NULL OR run_id=?1)", [run])?;
        self.checkpoint(run, lifecycle, now)?;
        Ok(moved)
    }
    pub fn recall(&self, run: &str) -> Result<usize> {
        ensure!(
            !self.db.is_autocommit(),
            "recall requires the caller's atomic transaction"
        );
        let moved = self.db.execute("INSERT OR REPLACE INTO context SELECT id,run_id,agent,CASE WHEN tier GLOB 'EVIDENCE*' THEN 'EVIDENCE' ELSE 'PSTC' END,body,source,created FROM context_archive WHERE run_id=?1", [run])?;
        self.db
            .execute("DELETE FROM context_archive WHERE run_id=?1", [run])?;
        Ok(moved)
    }
}

pub fn move_to_cold(
    db: &Connection,
    run: Option<&str>,
    lifecycle: ContextLifecycle,
) -> Result<usize> {
    SqliteContextArchive { db }.archive(run, lifecycle, crate::now_ms())
}
pub fn recall_from_cold(db: &Connection, run: &str) -> Result<usize> {
    SqliteContextArchive { db }.recall(run)
}
pub fn checkpoints(db: &Connection, run: &str) -> Result<Value> {
    let mut query = db.prepare("SELECT agent,lifecycle,policy_revision,contract_sha256,record_count,records_sha256,updated_ms FROM memory_archive_checkpoints WHERE run_id=?1 ORDER BY agent,lifecycle")?;
    let records = query.query_map([run], |r| Ok(json!({"agent":r.get::<_,String>(0)?,"lifecycle":r.get::<_,String>(1)?,"policy_revision":r.get::<_,Option<i64>>(2)?,"contract_sha256":r.get::<_,Option<String>>(3)?,"record_count":r.get::<_,i64>(4)?,"records_sha256":r.get::<_,String>(5)?,"updated_ms":r.get::<_,i64>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(
        json!({"run_id":run,"backend":"owned_local_sqlite","checkpoints":records,"scope":"Historical per-agent archive checkpoints only; no acceptance, qualification, recall or current-access authority","payload_logged":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn database() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE context(id TEXT PRIMARY KEY,run_id TEXT,agent TEXT,tier TEXT,body TEXT,source TEXT,created INTEGER);
            CREATE TABLE context_archive AS SELECT * FROM context WHERE 0;
            CREATE UNIQUE INDEX cold_ids ON context_archive(id);
            CREATE TABLE run_governance(run_id TEXT,revision INTEGER);CREATE TABLE jobs(run_id TEXT,digest TEXT,state TEXT);
            INSERT INTO context VALUES('a','run','Author','ASTC','private-source','model_output',1),('b','run','Other','EVIDENCE','private-evidence','tool',2);
            INSERT INTO run_governance VALUES('run',3);INSERT INTO jobs VALUES('run','frozen-contract','running');").unwrap();
        install(&db).unwrap();
        db
    }
    #[test]
    fn per_actor_upserts_are_idempotent_bound_and_never_copy_payloads() {
        let mut db = database();
        {
            let tx = db.transaction().unwrap();
            let repo = SqliteContextArchive { db: &tx };
            assert_eq!(
                repo.archive(Some("run"), ContextLifecycle::Completed, 10)
                    .unwrap(),
                2
            );
            let first = checkpoints(&tx, "run").unwrap();
            assert_eq!(first["checkpoints"].as_array().unwrap().len(), 2);
            assert!(!first.to_string().contains("private-"));
            assert_eq!(first["checkpoints"][0]["policy_revision"], 3);
            assert_eq!(
                repo.archive(Some("run"), ContextLifecycle::Completed, 20)
                    .unwrap(),
                0
            );
            assert_eq!(checkpoints(&tx, "run").unwrap(), first);
            tx.execute(
                "UPDATE context_archive SET body='private-updated' WHERE id='a'",
                [],
            )
            .unwrap();
            repo.archive(Some("run"), ContextLifecycle::Completed, 30)
                .unwrap();
            let changed = checkpoints(&tx, "run").unwrap();
            assert_ne!(
                changed["checkpoints"][0]["records_sha256"],
                first["checkpoints"][0]["records_sha256"]
            );
            assert_eq!(changed["checkpoints"][0]["updated_ms"], 30);
            assert_eq!(changed["checkpoints"][1], first["checkpoints"][1]);
            assert_eq!(changed["checkpoints"].as_array().unwrap().len(), 2);
            tx.commit().unwrap();
        }
        assert!(SqliteContextArchive { db: &db }.recall("run").is_err());
        let tx = db.transaction().unwrap();
        assert_eq!(SqliteContextArchive { db: &tx }.recall("run").unwrap(), 2);
        assert_eq!(
            tx.query_row(
                "SELECT COUNT(*) FROM context WHERE tier IN ('PSTC','EVIDENCE')",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert!(checkpoints(&tx, "other-run").unwrap()["checkpoints"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn checkpoint_failure_rolls_back_lifecycle_data_and_job_state_together() {
        let mut db = database();
        db.execute_batch("CREATE TRIGGER fail_checkpoint BEFORE INSERT ON memory_archive_checkpoints BEGIN SELECT RAISE(ABORT,'simulated checkpoint failure');END;").unwrap();
        {
            let tx = db.transaction().unwrap();
            tx.execute("UPDATE jobs SET state='completed'", []).unwrap();
            assert!(SqliteContextArchive { db: &tx }
                .archive(Some("run"), ContextLifecycle::Completed, 10)
                .is_err());
        }
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM context", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM context_archive", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT state FROM jobs", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "running"
        );
    }
    #[test]
    fn unqualified_candidates_remain_passive_including_prior_cold_rows() {
        let mut db = database();
        let tx = db.transaction().unwrap();
        let repo = SqliteContextArchive { db: &tx };
        repo.archive(Some("run"), ContextLifecycle::Completed, 10)
            .unwrap();
        repo.archive(Some("run"), ContextLifecycle::CandidateSubmitted, 20)
            .unwrap();
        assert_eq!(
            tx.query_row(
                "SELECT COUNT(*) FROM context_archive WHERE tier IN ('PLTC','EVIDENCE_PLTC')",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            checkpoints(&tx, "run").unwrap()["checkpoints"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["lifecycle"] == "candidate_submitted")
                .count(),
            2
        );
    }
}

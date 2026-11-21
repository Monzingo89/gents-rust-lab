//! Governed original-record handoffs. Retrieval is bound to an accepted
//! predecessor record; model text cannot choose identities or grant access.
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn record_hash(record: &Value) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(record)?)))
}

pub struct Query<'a> {
    pub sender: &'a str,
    pub correlation_key: &'a str,
    pub correlation_value: &'a str,
    pub accepted_record_sha256: &'a str,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub run_id: Uuid,
    pub recipient: String,
    pub sender: String,
    pub correlation_key: String,
    pub correlation_value: String,
    pub source_id: String,
    pub source_sha256: String,
    pub accepted_record_sha256: String,
    pub policy_revision: u64,
}

pub struct Record {
    pub binding: Binding,
    pub value: Value,
}
impl Record {
    /// Only the retrieved original and provenance, never a downstream answer.
    pub fn packet(&self) -> Value {
        json!({"record":self.value,"source_sha256":self.binding.source_sha256,"record_sha256":self.binding.accepted_record_sha256,"sender":self.binding.sender,"policy_revision":self.binding.policy_revision})
    }
}

/// An authenticated company connection and runtime-bound recipient are required.
pub struct SqliteHandoffs<'a> {
    pub db: &'a Connection,
    pub run: Uuid,
    pub recipient: &'a str,
}
impl SqliteHandoffs<'_> {
    fn revision(&self, sender: &str) -> Result<u64> {
        let policy = crate::governance::load(self.db, &self.run.to_string())?
            .context("handoff requires installed governance")?;
        let grant = policy
            .grants
            .get(self.recipient)
            .context("handoff recipient missing")?;
        anyhow::ensure!(
            sender == self.recipient || grant.memory_from.contains(sender),
            "handoff sender not authorized"
        );
        Ok(policy.revision)
    }
    fn source(&self, id: &str, sender: &str, revision: u64) -> Result<Value> {
        let local: bool = self.db.query_row("SELECT COUNT(*)=1 FROM context WHERE id=?1 AND run_id=?2 AND agent=?3 AND source='model_output'", params![id,self.run.to_string(),sender], |r|r.get(0))?;
        anyhow::ensure!(local, "handoff must originate in this run and sender");
        crate::company::semantic_source(self.db, self.run, self.recipient, id, revision)?
            .context("handoff source not currently authorized")
    }
    pub fn find(&self, query: &Query<'_>) -> Result<Record> {
        anyhow::ensure!(
            !query.correlation_key.is_empty()
                && query.correlation_key.len() <= 64
                && !query.correlation_value.is_empty()
                && query.correlation_value.len() <= 200,
            "handoff correlation is not bounded"
        );
        let revision = self.revision(query.sender)?;
        // Inspect only identifiers before passing through the shared point-read
        // boundary. No unauthorized record bodies are returned to the caller.
        let mut stmt=self.db.prepare("SELECT id FROM context WHERE run_id=?1 AND agent=?2 AND source='model_output' ORDER BY created DESC,id LIMIT 128")?;
        let ids = stmt
            .query_map(params![self.run.to_string(), query.sender], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for id in ids {
            let Ok(source) = self.source(&id, query.sender, revision) else {
                continue;
            };
            let Some(text) = source["text"].as_str().filter(|t| t.len() <= 14500) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(text) else {
                continue;
            };
            if value[query.correlation_key] != query.correlation_value
                || record_hash(&value)? != query.accepted_record_sha256
            {
                continue;
            }
            let binding = Binding {
                run_id: self.run,
                recipient: self.recipient.into(),
                sender: query.sender.into(),
                correlation_key: query.correlation_key.into(),
                correlation_value: query.correlation_value.into(),
                source_id: id,
                source_sha256: source["sha256"]
                    .as_str()
                    .context("source hash missing")?
                    .into(),
                accepted_record_sha256: query.accepted_record_sha256.into(),
                policy_revision: revision,
            };
            return self.revalidate(&binding);
        }
        anyhow::bail!("accepted predecessor record unavailable through governed memory")
    }
    pub fn revalidate(&self, binding: &Binding) -> Result<Record> {
        anyhow::ensure!(
            binding.run_id == self.run
                && binding.recipient == self.recipient
                && self.revision(&binding.sender)? == binding.policy_revision,
            "handoff recipient/run/policy changed"
        );
        let source = self.source(&binding.source_id, &binding.sender, binding.policy_revision)?;
        let value: Value = serde_json::from_str(
            source["text"]
                .as_str()
                .context("handoff original missing")?,
        )?;
        anyhow::ensure!(
            source["sha256"] == binding.source_sha256
                && record_hash(&value)? == binding.accepted_record_sha256
                && value[&binding.correlation_key] == binding.correlation_value,
            "handoff original or accepted record changed"
        );
        Ok(Record {
            binding: binding.clone(),
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepted_record_handoffs_reject_forgery_revocation_regrant_and_other_owners() {
        let root = std::env::temp_dir().join(format!("gents-handoff-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let run = Uuid::new_v4();
        let op = |v: Value| crate::company::operation(&root, "one", "alice", &v).unwrap();
        op(json!({"op":"register","name":"Synthetic memory handoff"}));
        let job = op(
            json!({"op":"intake","run_id":run,"intake":{"idea":"Route synthetic exceptions","acceptance":"Authorized originals only","exclusions":"External effects and personal data","timeline":"Bounded test","company_signer":"A","provider_signer":"B","budget_usd":0}}),
        );
        op(json!({"op":"approve","run_id":run,"digest":job["jobs"][0]["digest"]}));
        let master = json!({"tools":["access_delegate"],"skills":[],"memory_from":[]});
        let writer = json!({"tools":[],"skills":[],"memory_from":[]});
        let reader = json!({"tools":[],"skills":[],"memory_from":["Writer"]});
        let grants = json!({"Master":master,"Writer":writer,"Reader":reader});
        op(
            json!({"op":"governance_install","run_id":run,"expected_revision":0,"policy":{"revision":1,"master":"Master","ceilings":grants,"grants":grants}}),
        );
        crate::company::admit(&root, "one", "alice", run, "Route synthetic exceptions").unwrap();
        let original = json!({"case_id":"case-7","status":"mismatch","difference_cents":-20});
        crate::company::stage_at_policy(
            &root,
            "one",
            "alice",
            run,
            "Writer",
            &json!({"run_id":run,"policy_revision":1,"output":original.to_string()}).to_string(),
            1,
        )
        .unwrap();
        let db = crate::company::connect(&root, "one", "alice", false).unwrap();
        let repo = SqliteHandoffs {
            db: &db,
            run,
            recipient: "Reader",
        };
        let hash = record_hash(&original).unwrap();
        let query = Query {
            sender: "Writer",
            correlation_key: "case_id",
            correlation_value: "case-7",
            accepted_record_sha256: &hash,
        };
        assert!(
            repo.find(&query).is_err(),
            "unacknowledged ASTC must stay private"
        );
        crate::company::acknowledge(&root, "one", "alice", run, "Reader").unwrap();
        let record = repo.find(&query).unwrap();
        assert_eq!(record.packet()["record"], original);
        assert!(record.packet().get("queue").is_none());
        // Recovery carries the typed locator, not a trusted copy of the
        // original. Reopen the company and resolve it through current policy.
        let packet = record.packet();
        let checkpoint = root.join("handoff-binding.json");
        std::fs::write(&checkpoint, serde_json::to_vec(&record.binding).unwrap()).unwrap();
        drop(db);
        let binding: Binding = serde_json::from_slice(&std::fs::read(checkpoint).unwrap()).unwrap();
        let db = crate::company::connect(&root, "one", "alice", false).unwrap();
        let repo = SqliteHandoffs {
            db: &db,
            run,
            recipient: "Reader",
        };
        let record = repo.revalidate(&binding).unwrap();
        assert_eq!(record.packet(), packet);
        let other = SqliteHandoffs {
            db: &db,
            run,
            recipient: "Master",
        };
        assert!(other.find(&query).is_err());
        assert!(other.revalidate(&record.binding).is_err());
        assert!(crate::company::connect(&root, "one", "mallory", false).is_err());
        let wrong_run = SqliteHandoffs {
            db: &db,
            run: Uuid::new_v4(),
            recipient: "Reader",
        };
        assert!(wrong_run.revalidate(&record.binding).is_err());
        let mut forged = record.binding.clone();
        forged.source_sha256 = "0".repeat(64);
        assert!(repo.revalidate(&forged).is_err());
        let wrong = Query {
            accepted_record_sha256: &"f".repeat(64),
            ..query
        };
        assert!(repo.find(&wrong).is_err());
        let body: String = db
            .query_row(
                "SELECT body FROM context WHERE id=?1",
                [&record.binding.source_id],
                |r| r.get(0),
            )
            .unwrap();
        db.execute(
            "UPDATE context SET body=replace(body,'mismatch','matched') WHERE id=?1",
            [&record.binding.source_id],
        )
        .unwrap();
        assert!(repo.revalidate(&record.binding).is_err());
        db.execute(
            "UPDATE context SET body=?1 WHERE id=?2",
            params![body, record.binding.source_id],
        )
        .unwrap();
        for (revision, sources) in [(1, json!([])), (2, json!(["Writer"]))] {
            crate::company::delegate_access(&root,"one","alice",run,"Master",&json!({"recipient":"Reader","expected_revision":revision,"grant":{"tools":[],"skills":[],"memory_from":sources}})).unwrap();
            assert!(repo.revalidate(&record.binding).is_err());
            assert!(
                repo.find(&query).is_err(),
                "old model epoch cannot revive after regrant"
            );
        }
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }
}

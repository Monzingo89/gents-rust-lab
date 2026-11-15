//! Owner-configured, local-only planning model selection. Never tool authority.
use crate::provider::Model;
use anyhow::{ensure, Result};
use gents_local_mcp::{digest, host_memory::Snapshot};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub model: Model,
    pub sha256: String,
    /// Owner-reviewed capacity floor, not measured GPU memory or fit assurance.
    pub minimum_memory_mib: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub failures_before_next: usize,
    pub candidates: Vec<Candidate>,
}

impl Policy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=3).contains(&self.failures_before_next)
                && (1..=4).contains(&self.candidates.len()),
            "planning policy needs 1..4 candidates and a 1..3 failure threshold"
        );
        let mut ids = BTreeSet::new();
        for candidate in &self.candidates {
            ensure!(
                !candidate.id.is_empty()
                    && candidate.id.len() <= 64
                    && candidate.id.bytes().all(|b| b.is_ascii_lowercase()
                        || b.is_ascii_digit()
                        || b == b'_'
                        || b == b'-')
                    && ids.insert(&candidate.id),
                "planning candidate ids must be unique bounded lowercase identifiers"
            );
            candidate.model.validate()?;
            ensure!(
                candidate.model.provider == "local"
                    && candidate
                        .model
                        .path
                        .as_deref()
                        .is_some_and(crate::model_artifact::relative_path),
                "planning candidates require local GGUF paths under models without traversal"
            );
            ensure!(
                candidate.sha256.len() == 64
                    && candidate
                        .sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "planning candidate needs an exact SHA-256 pin"
            );
            ensure!(
                (512..=2 * 1024 * 1024).contains(&candidate.minimum_memory_mib),
                "planning capacity floor must be 512 MiB..2 TiB"
            );
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<String> {
        Ok(digest(serde_json::to_value(self)?.to_string().as_bytes()))
    }
    pub fn public_view(&self) -> Value {
        json!({"mode":"owner_local_policy","failures_before_next":self.failures_before_next,"candidates":self.candidates.iter().map(|c|json!({"id":c.id,"context_tokens":c.model.context_tokens.unwrap_or(16384),"reasoning_tokens":c.model.local_reasoning_tokens.unwrap_or(0),"minimum_memory_mib":c.minimum_memory_mib})).collect::<Vec<_>>(),"scope":"Owner-configured local planning choices only; no model paths, credentials, task data or tool grants."})
    }
    pub fn failures(&self, db: &Connection, session: &str) -> Result<(usize, Value)> {
        // Only the authenticated owner's native adoption transaction starts a
        // new phase. Model text, a protocol label or elapsed time cannot do so.
        let mut query = db.prepare(
            "SELECT body FROM audit WHERE run_id=?1 AND kind='ceo_owned_selection_adopted' LIMIT 2",
        )?;
        let boundaries = query
            .query_map([session], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            boundaries.len() <= 1,
            "ambiguous native planning phase boundary"
        );
        let phase: Value = match boundaries.first() {
            Some(body) => serde_json::from_str(body)?,
            None => json!({"kind":"session","start_revision":1}),
        };
        let start = phase["start_revision"]
            .as_u64()
            .filter(|v| (1..=9).contains(v))
            .ok_or_else(|| anyhow::anyhow!("invalid native planning phase boundary"))?;
        if !boundaries.is_empty() {
            ensure!(
                start > 1
                    && phase["kind"] == "owned_selection_adopted"
                    && ["proposal_sha256", "data_sha256", "catalog_sha256"]
                        .iter()
                        .all(|k| phase[k].as_str().is_some_and(|s| s.len() == 64
                            && s.bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))),
                "invalid native selection adoption boundary"
            );
        }
        // Only completed native, non-cutoff invalid proposals under this exact
        // owner policy count. Operator text and unreceived calls do not count.
        let count:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE run_id=?1 AND kind='ceo_planning_completed' AND json_extract(body,'$.planning_model_selection.policy_sha256')=?2 AND json_extract(body,'$.status')='failed' AND json_type(body,'$.validation_error')='text' AND json_extract(body,'$.local_execution.finish_reason')='stop' AND json_extract(body,'$.local_execution.backend_pid')>0 AND (?3=1 OR json_extract(body,'$.revision')>=?3)",rusqlite::params![session,self.digest()?,i64::try_from(start)?],|row|row.get(0))?;
        Ok((usize::try_from(count)?, phase))
    }
    pub fn select(
        &self,
        root: &Path,
        failures: usize,
        memory: &Snapshot,
    ) -> Result<(Model, Value)> {
        self.validate()?;
        let capacity = match memory.effective_limit_bytes {
            Some(value)=>value,
            None=>return Err(Unavailable{proof:json!({"policy_sha256":self.digest()?,"reason":"process_capacity_unavailable","host_memory":memory,"inference_dispatched":false})}.into()),
        };
        let requested = (failures / self.failures_before_next).min(self.candidates.len() - 1);
        let mut skipped = vec![];
        for (index, candidate) in self.candidates.iter().enumerate().skip(requested) {
            let reason = if candidate.minimum_memory_mib * 1024 * 1024 > capacity {
                Some("host_capacity_below_owner_floor")
            } else {
                match pinned_file(root, candidate) {
                    Ok(bytes) => {
                        return Ok((
                            candidate.model.clone(),
                            json!({"policy_sha256":self.digest()?,"candidate_id":candidate.id,"candidate_index":index,"requested_index":requested,"structural_failures":failures,"weight_sha256":candidate.sha256,"weight_bytes":bytes,"host_memory":memory,"skipped":skipped,"scope":"Owner-listed local planning model only; capacity floor is not GPU fit, model competence or tool authority. Native runtime still fits actual device/context."}),
                        ))
                    }
                    Err(error) => Some(error),
                }
            };
            skipped.push(json!({"candidate_id":candidate.id,"reason":reason}));
        }
        Err(Unavailable{proof:json!({"policy_sha256":self.digest()?,"structural_failures":failures,"requested_index":requested,"host_memory":memory,"skipped":skipped,"inference_dispatched":false})}.into())
    }
}

#[derive(Debug)]
pub struct Unavailable {
    pub proof: Value,
}
impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("No eligible installed model remains in the owner's local planning policy")
    }
}
impl std::error::Error for Unavailable {}

fn pinned_file(root: &Path, candidate: &Candidate) -> std::result::Result<u64, &'static str> {
    crate::model_artifact::verify(
        root,
        candidate
            .model
            .path
            .as_deref()
            .ok_or("model_path_missing")?,
        &candidate.sha256,
    )
}

pub fn failure(error: &anyhow::Error) -> Value {
    let proof = error
        .downcast_ref::<Unavailable>()
        .map(|e| e.proof.clone())
        .unwrap_or(Value::Null);
    json!({"code":"local_planning_policy_unavailable","inference_dispatched":false,"retryable_without_host_change":false,"selection":proof})
}

pub fn cache_scope(base: &str, selection: Option<&Value>) -> String {
    match selection {
        None => base.to_owned(),
        Some(proof) => format!(
            "{base}:model-{}",
            digest(
                json!([
                    proof["policy_sha256"],
                    proof["candidate_id"],
                    proof["weight_sha256"]
                ])
                .to_string()
                .as_bytes()
            )
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gents_local_mcp::host_memory::{MemoryProbe, ProcessLimit, Reading};
    use std::fs;
    struct Capacity(u64);
    impl MemoryProbe for Capacity {
        fn read(&self) -> Result<Reading> {
            Ok(Reading {
                total_bytes: self.0,
                available_bytes: self.0 / 2,
                process_limit: ProcessLimit::NotObservedOnPlatform,
            })
        }
    }
    fn memory(mib: u64) -> Snapshot {
        Snapshot::capture(&Capacity(mib * 1024 * 1024)).unwrap()
    }
    fn policy() -> Policy {
        serde_json::from_value(json!({"failures_before_next":2,"candidates":[{"id":"primary","model":{"provider":"local","model":"primary","path":"models/a.gguf","context_tokens":8192},"sha256":digest(b"primary"),"minimum_memory_mib":2048},{"id":"reasoner","model":{"provider":"local","model":"reasoner","path":"models/b.gguf","context_tokens":8192,"local_reasoning_tokens":512},"sha256":digest(b"reasoner"),"minimum_memory_mib":1024}]})).unwrap()
    }
    fn files() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("model-policy-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("models")).unwrap();
        fs::write(root.join("models/a.gguf"), b"primary").unwrap();
        fs::write(root.join("models/b.gguf"), b"reasoner").unwrap();
        root
    }
    #[test]
    fn cache_identity_tracks_policy_candidate_and_weight_pin_but_not_device_timestamps() {
        let first = json!({"policy_sha256":"owner-policy","candidate_id":"a","weight_sha256":"pin-a","host_memory":{"observed_at_ms":1}});
        let scope = cache_scope("company:session", Some(&first));
        let mut time = first.clone();
        time["host_memory"]["observed_at_ms"] = json!(999);
        assert_eq!(scope, cache_scope("company:session", Some(&time)));
        for key in ["policy_sha256", "candidate_id", "weight_sha256"] {
            let mut changed = first.clone();
            changed[key] = json!("changed");
            assert_ne!(scope, cache_scope("company:session", Some(&changed)));
        }
        assert_eq!(cache_scope("company:session", None), "company:session");
    }
    #[test]
    fn native_failure_evidence_is_session_policy_and_receipt_bound() {
        let p = policy();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE audit(run_id TEXT,kind TEXT,body TEXT)")
            .unwrap();
        let valid = json!({"status":"failed","validation_error":"invalid graph","planning_model_selection":{"policy_sha256":p.digest().unwrap()},"local_execution":{"finish_reason":"stop","backend_pid":123}});
        for (session, kind, body) in [
            ("session", "ceo_planning_completed", valid.clone()),
            ("other", "ceo_planning_completed", valid.clone()),
            ("session", "model_claim", valid.clone()),
            (
                "session",
                "ceo_planning_completed",
                json!({"status":"failed","validation_error":"model says escalate"}),
            ),
        ] {
            db.execute(
                "INSERT INTO audit VALUES(?1,?2,?3)",
                rusqlite::params![session, kind, body.to_string()],
            )
            .unwrap();
        }
        for (field, value) in [
            ("status", json!("ready")),
            ("planning_model_selection", json!({"policy_sha256":"other"})),
            (
                "local_execution",
                json!({"finish_reason":"length","backend_pid":123}),
            ),
            (
                "local_execution",
                json!({"finish_reason":"stop","backend_pid":0}),
            ),
            ("validation_error", Value::Null),
        ] {
            let mut bad = valid.clone();
            bad[field] = value;
            db.execute(
                "INSERT INTO audit VALUES('session','ceo_planning_completed',?1)",
                [bad.to_string()],
            )
            .unwrap();
        }
        assert_eq!(p.failures(&db, "session").unwrap().0, 1);
    }
    #[test]
    fn only_native_owner_adoption_starts_a_new_failure_count() {
        let p = policy();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE audit(run_id TEXT,kind TEXT,body TEXT)")
            .unwrap();
        let failed = |revision| json!({"revision":revision,"status":"failed","validation_error":"invalid graph","planning_model_selection":{"policy_sha256":p.digest().unwrap()},"local_execution":{"finish_reason":"stop","backend_pid":123}});
        for revision in [1, 2, 4] {
            db.execute(
                "INSERT INTO audit VALUES('session','ceo_planning_completed',?1)",
                [failed(revision).to_string()],
            )
            .unwrap();
        }
        let boundary = json!({"kind":"owned_selection_adopted","start_revision":4,"proposal_sha256":"a".repeat(64),"data_sha256":"b".repeat(64),"catalog_sha256":"c".repeat(64)});
        db.execute(
            "INSERT INTO audit VALUES('session','model_claim',?1)",
            [boundary.to_string()],
        )
        .unwrap();
        assert_eq!(
            p.failures(&db, "session").unwrap(),
            (3, json!({"kind":"session","start_revision":1}))
        );
        db.execute(
            "INSERT INTO audit VALUES('other','ceo_owned_selection_adopted',?1)",
            [boundary.to_string()],
        )
        .unwrap();
        assert_eq!(p.failures(&db, "session").unwrap().0, 3);
        db.execute(
            "INSERT INTO audit VALUES('session','ceo_owned_selection_adopted',?1)",
            [boundary.to_string()],
        )
        .unwrap();
        assert_eq!(p.failures(&db, "session").unwrap(), (1, boundary.clone()));
        db.execute(
            "INSERT INTO audit VALUES('session','ceo_owned_selection_adopted',?1)",
            [boundary.to_string()],
        )
        .unwrap();
        assert!(p.failures(&db, "session").is_err());
    }
    #[test]
    fn escalation_and_hardware_eligibility_do_not_change_the_models_or_grant_authority() {
        let root = files();
        let p = policy();
        for (failures, mib, index) in [
            (0, 4096, 0),
            (1, 4096, 0),
            (2, 4096, 1),
            (100, 4096, 1),
            (0, 1536, 1),
        ] {
            let (model, proof) = p.select(&root, failures, &memory(mib)).unwrap();
            assert_eq!(proof["candidate_index"], index);
            assert_eq!(model.model, p.candidates[index].model.model);
            assert!(proof["host_memory"]["gpu_memory_bytes"].is_null());
        }
        let error = p.select(&root, 0, &memory(512)).err().unwrap();
        assert_eq!(failure(&error)["inference_dispatched"], false);
        let mut unknown = memory(4096);
        unknown.effective_limit_bytes = None;
        assert!(p.select(&root, 0, &unknown).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn no_remote_paths_unbounded_settings_or_unpinned_candidates_enter_the_policy() {
        let public = policy().public_view().to_string();
        assert!(
            !public.contains("a.gguf") && !public.contains("sha256") && !public.contains("key_env")
        );
        let original = serde_json::to_value(policy()).unwrap();
        for value in [json!(0), json!(4)] {
            let mut bad = original.clone();
            bad["failures_before_next"] = value;
            assert!(serde_json::from_value::<Policy>(bad)
                .unwrap()
                .validate()
                .is_err());
        }
        for (field, value) in [
            ("sha256", json!("bad")),
            ("id", json!("Secret candidate")),
            ("minimum_memory_mib", json!(0)),
        ] {
            let mut bad = original.clone();
            bad["candidates"][0][field] = value;
            assert!(serde_json::from_value::<Policy>(bad)
                .unwrap()
                .validate()
                .is_err());
        }
        for path in [
            "../private.gguf",
            "models/../private.gguf",
            "https://remote/model.gguf",
            "models\\private.gguf",
        ] {
            let mut p = policy();
            p.candidates[0].model.path = Some(path.into());
            assert!(p.validate().is_err());
        }
        let mut p = policy();
        p.candidates[0].model.provider = "openrouter".into();
        assert!(p.validate().is_err());
        let mut p = policy();
        p.candidates[1].id = p.candidates[0].id.clone();
        assert!(p.validate().is_err());
        let mut p = policy();
        p.candidates.clear();
        assert!(p.validate().is_err());
    }
    #[test]
    fn altered_or_missing_weights_are_skipped_with_explicit_receipts_and_never_loaded() {
        let root = files();
        let p = policy();
        fs::write(root.join("models/a.gguf"), b"tampered").unwrap();
        let (_, proof) = p.select(&root, 0, &memory(4096)).unwrap();
        assert_eq!(proof["candidate_id"], "reasoner");
        assert_eq!(proof["skipped"][0]["reason"], "model_hash_pin_mismatch");
        fs::remove_file(root.join("models/b.gguf")).unwrap();
        let error = p.select(&root, 0, &memory(4096)).err().unwrap();
        assert_eq!(
            failure(&error)["selection"]["skipped"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn aliases_and_writable_model_directories_are_rejected() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = files();
        let p = policy();
        fs::remove_file(root.join("models/a.gguf")).unwrap();
        symlink("b.gguf", root.join("models/a.gguf")).unwrap();
        assert_eq!(
            pinned_file(&root, &p.candidates[0]),
            Err("model_symlink_rejected")
        );
        fs::remove_file(root.join("models/a.gguf")).unwrap();
        fs::hard_link(root.join("models/b.gguf"), root.join("models/a.gguf")).unwrap();
        assert_eq!(
            pinned_file(&root, &p.candidates[0]),
            Err("model_links_or_permissions_rejected")
        );
        fs::remove_file(root.join("models/a.gguf")).unwrap();
        fs::set_permissions(root.join("models"), fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            pinned_file(&root, &p.candidates[1]),
            Err("model_owner_or_directory_permissions_rejected")
        );
        fs::remove_dir_all(root).unwrap();
    }
}

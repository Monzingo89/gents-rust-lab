//! Stable identities for verified native-stage submissions, never acceptance.
//! The caller supplies fresh canonical gates; model text and receipt counts
//! cannot make unchanged work eligible for another automatic submission.
use serde_json::{json, Value};

pub struct Submission {
    pub fingerprint: String,
    pub proposal: Value,
    pub observation: Value,
}

pub fn compile(
    contract: &str,
    actor: &str,
    stage: usize,
    epoch: Option<u64>,
    gate: &Value,
) -> Option<Submission> {
    Some(Submission {
        fingerprint: fingerprint(contract, actor, stage, epoch, gate)?,
        proposal: json!({"action":"finish","arguments":{"deliverable":"The owner-approved native source/artifact contract is verified. Submit this stage to independent review; this summary adds no unverified domain facts.","evidence":"Canonical native source projection, actor-scoped receipts and artifact_bindings checks in the host review packet."},"summary":"Host-compiled stage submission after verified native contracts; independent QA still checks every stage and goal criterion"}),
        observation: json!({"origin":"host_compiled_native_artifact_submission","model_generated_summary":false,"independent_qa_required":true}),
    })
}

pub fn fingerprint(
    contract: &str,
    actor: &str,
    stage: usize,
    epoch: Option<u64>,
    gate: &Value,
) -> Option<String> {
    let epoch = epoch.filter(|revision| *revision > 0)?;
    if contract.len() != 64
        || !contract.bytes().all(|b| b.is_ascii_hexdigit())
        || gate["passed"] != true
        || gate["native_receipts"]["passed"] != true
        || gate["artifact_bindings"]["passed"] != true
        || gate["native_receipts"]["policy_revision"] != epoch
        || gate["artifact_bindings"]["policy_revision"] != epoch
    {
        return None;
    }
    let calls = gate["native_receipts"]["checks"].as_array()?;
    let artifacts = gate["artifact_bindings"]["checks"].as_array()?;
    if calls.is_empty()
        || artifacts.is_empty()
        || !calls
            .iter()
            .chain(artifacts)
            .all(|row| row["passed"] == true)
    {
        return None;
    }
    let calls = calls
        .iter()
        .map(|row| json!({"actor":row["actor"],"tool":row["tool"],"passed":true}))
        .collect::<Vec<_>>();
    let artifacts=artifacts.iter().map(|row|{let mut value=json!({"path":row["path"],"source_actor":row["source_actor"],"writer":row["writer"],"source_result_sha256":row["source_result_sha256"],"source_arguments_sha256":row["source_arguments_sha256"],"source_projection":row["source_projection"],"expected_sha256":row["expected_sha256"],"observed_sha256":row["observed_sha256"],"writer_native_operations":row["writer_native_operations"],"phase":row["phase"]}); if row["receipt_only"]==true {value["receipt_only"]=json!(true);} value}).collect::<Vec<_>>();
    let identity = json!({"version":1,"evidence_schema_version":crate::evidence::SCHEMA_VERSION,"contract":contract,"actor":actor,"stage":stage,"epoch":epoch,"native_calls":calls,"artifacts":artifacts,"generic_required":gate["required"],"generic_artifact_sha256":gate["artifact_sha256"],"named_artifact_source":gate["named_artifact_source"]});
    Some(gents_local_mcp::digest(identity.to_string().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gate() -> Value {
        json!({"passed":true,"required":{},"native_receipts":{"passed":true,"policy_revision":1,"checks":[{"actor":"Analyst","tool":"audio_inspect","passed":true,"successful_current_epoch_receipts":3}]},"artifact_bindings":{"passed":true,"policy_revision":1,"checks":[{"path":"frames.txt","passed":true,"source_result_sha256":"a".repeat(64),"expected_sha256":"b".repeat(64),"observed_sha256":"b".repeat(64)}]}})
    }
    #[test]
    fn submission_requires_all_current_gates_and_changes_only_with_verified_identity() {
        let digest = "c".repeat(64);
        let original = gate();
        let first = fingerprint(&digest, "Writer", 1, Some(1), &original).unwrap();
        for pointer in [
            "/passed",
            "/native_receipts/passed",
            "/artifact_bindings/passed",
            "/artifact_bindings/checks/0/passed",
        ] {
            let mut failed = original.clone();
            *failed.pointer_mut(pointer).unwrap() = json!(false);
            assert!(fingerprint(&digest, "Writer", 1, Some(1), &failed).is_none());
        }
        assert!(fingerprint(&digest, "Writer", 1, Some(2), &original).is_none());
        assert!(fingerprint(&digest, "Writer", 1, None, &original).is_none());
        let mut counts = original.clone();
        counts["native_receipts"]["checks"][0]["successful_current_epoch_receipts"] = json!(99);
        counts["model_claim"] = json!("all done");
        assert_eq!(
            fingerprint(&digest, "Writer", 1, Some(1), &counts),
            Some(first.clone())
        );
        let mut changed = original.clone();
        changed["artifact_bindings"]["checks"][0]["source_result_sha256"] = json!("d".repeat(64));
        assert_ne!(
            fingerprint(&digest, "Writer", 1, Some(1), &changed),
            Some(first.clone())
        );
        let mut stored = std::collections::BTreeMap::new();
        stored.insert(1, first);
        let restored: std::collections::BTreeMap<usize, String> =
            serde_json::from_str(&serde_json::to_string(&stored).unwrap()).unwrap();
        assert_eq!(
            restored.get(&1),
            fingerprint(&digest, "Writer", 1, Some(1), &original).as_ref()
        );
    }
}

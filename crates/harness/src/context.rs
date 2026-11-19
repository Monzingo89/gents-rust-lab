//! Portable chat representation and loss-aware compaction. Required task,
//! criteria, exact requests and verified handoffs are never string-truncated.
use anyhow::{bail, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub struct ContextProfile {
    pub max_bytes: usize,
    pub include_history: bool,
    pub include_guidance: bool,
}

/// Only trusted, governed execution protocols omit repeated global advice.
/// Explicit task fields, assigned procedures and permission envelopes remain.
pub fn assigned_operator(system: &str, input: &str, governed: bool) -> bool {
    if !governed {
        return false;
    }
    let Ok(packet) = task_input(input) else {
        return false;
    };
    system.starts_with("COMPANY_OPERATOR:") && packet["ceo_stage"].is_object()
        || system.starts_with("EXECUTIVE_RECOVERY:") && packet["assignment"].is_object()
}
pub fn profile(phase: &str) -> ContextProfile {
    match phase {
        // Final QA already receives the full frozen task, criteria and current
        // native artifact/receipt gates. Extra company guidance and handoff
        // copies are neither evidence nor authority and can exhaust local KV.
        "judge" => ContextProfile {
            max_bytes: 40_000,
            include_history: false,
            include_guidance: false,
        },
        "memory_summary" | "memory_extract" => ContextProfile {
            max_bytes: 16000,
            include_history: false,
            include_guidance: false,
        },
        "scoped_worker" | "memory_select" => ContextProfile {
            max_bytes: 6000,
            include_history: false,
            include_guidance: false,
        },
        "observer" => ContextProfile {
            max_bytes: 6000,
            include_history: false,
            include_guidance: true,
        },
        "admin" | "oversight" => ContextProfile {
            max_bytes: 40_000,
            include_history: false,
            include_guidance: true,
        },
        _ => ContextProfile {
            max_bytes: 40_000,
            include_history: true,
            include_guidance: true,
        },
    }
}

/// A trusted source-only protocol can omit unrelated advisory context while
/// retaining the worker identity and deterministic permission checks. A task
/// field or reference document cannot select this profile.
pub fn protocol_profile(phase: &str, system: &str) -> ContextProfile {
    if phase == "worker" && system.starts_with("LOCAL_CANDIDATE_AUTHOR_FOCUSED:") {
        ContextProfile {
            max_bytes: 32768,
            include_history: false,
            include_guidance: false,
        }
    } else {
        profile(phase)
    }
}

/// The policy epoch is part of inference-cache identity. A permission change
/// must not reuse an agent's old backend KV state, even within the same run.
pub fn session_key(company: &str, run: &str, actor: &str, policy_revision: u64) -> String {
    format!(
        "{company}:{run}:{:x}:policy-{policy_revision}",
        Sha256::digest(actor.as_bytes())
    )
}

/// Hierarchical cancellation must respect segment boundaries: cancelling
/// policy-1 must never terminate policy-10 or an unrelated run prefix.
pub fn session_in_scope(session: &str, scope: &str) -> bool {
    !scope.is_empty()
        && (session == scope
            || session
                .strip_prefix(scope)
                .is_some_and(|suffix| suffix.starts_with(':')))
}

pub fn messages(system: &str, input: &str) -> Value {
    json!([{"role":"system","content":system},{"role":"user","content":input}])
}

/// A field-oriented role protocol keeps its task fields at the documented
/// root. Trusted runtime metadata lives in a reserved, non-authoritative field.
pub fn scoped_packet(task: Value, runtime: Value) -> Result<Value> {
    let mut fields = task
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("scoped role requires an object task"))?
        .clone();
    if fields.contains_key("_runtime") {
        bail!("task cannot shadow reserved runtime metadata");
    }
    fields.insert("_runtime".into(), runtime);
    Ok(Value::Object(fields))
}

/// Compatibility for adapters/fixtures accepting either legacy string payloads
/// or version-two structured envelopes.
pub fn task_input(input: &str) -> Result<Value> {
    let outer: Value = serde_json::from_str(input)?;
    Ok(match &outer["task_input"] {
        Value::String(nested) => serde_json::from_str(nested)?,
        Value::Object(_) => outer["task_input"].clone(),
        _ => outer,
    })
}

/// Optional adapter-supplied binding prevents a source read under an old grant
/// from being sent under a newly admitted inference epoch. It can only deny;
/// it does not authorize a source or replace repository revalidation.
pub fn validate_evidence_epoch(input: &str, current: u64) -> Result<()> {
    let Ok(packet) = task_input(input) else {
        return Ok(());
    };
    if let Some(revision) = packet.get("evidence_policy_revision") {
        anyhow::ensure!(
            revision.as_u64() == Some(current) && current > 0,
            "retrieved evidence policy changed before inference"
        );
    }
    Ok(())
}

/// Fingerprint the actual task after native prompt compaction, independent of
/// the runtime envelope. Source fields remain part of this binding.
pub fn task_fingerprint(input: &str) -> String {
    let mut task = task_input(input).unwrap_or_else(|_| json!(input));
    if let Some(fields) = task.as_object_mut() {
        fields.remove("_runtime");
    }
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&task).expect("JSON value encodes"))
    )
}

/// Lossless inference-only encoding of repeated SHA-256 strings in native
/// proof fields. Canonical receipts and completion gates are never rewritten.
pub fn intern_native_proof_hashes(mut packet: Value) -> Result<Value> {
    use std::collections::BTreeMap;
    const KEY: &str = "$h";
    fn hash(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
    fn count(value: &Value, counts: &mut BTreeMap<String, usize>) {
        match value {
            Value::String(text) if hash(text) => {
                *counts.entry(text.clone()).or_default() += 1;
            }
            Value::Array(rows) => {
                for row in rows {
                    count(row, counts)
                }
            }
            Value::Object(fields) => {
                for row in fields.values() {
                    count(row, counts)
                }
            }
            _ => {}
        }
    }
    fn replace(value: &mut Value, ids: &BTreeMap<String, usize>) {
        match value {
            Value::String(text) => {
                if let Some(id) = ids.get(text) {
                    *value = json!({KEY:id});
                }
            }
            Value::Array(rows) => {
                for row in rows {
                    replace(row, ids)
                }
            }
            Value::Object(fields) => {
                for row in fields.values_mut() {
                    replace(row, ids)
                }
            }
            _ => {}
        }
    }
    anyhow::ensure!(
        packet.is_object()
            && packet.get("_native_proof_hashes").is_none()
            && packet.get("_native_proof_encoding").is_none(),
        "native proof encoding metadata is reserved"
    );
    fn has_reference(value: &Value) -> bool {
        match value {
            Value::Object(fields) => fields.contains_key(KEY) || fields.values().any(has_reference),
            Value::Array(rows) => rows.iter().any(has_reference),
            _ => false,
        }
    }
    for field in ["artifact_gate", "native_receipts"] {
        anyhow::ensure!(
            !packet.get(field).is_some_and(has_reference),
            "reserved native proof hash reference in source packet"
        );
    }
    let original = packet.clone();
    let mut counts = BTreeMap::new();
    for field in ["artifact_gate", "native_receipts"] {
        if let Some(value) = packet.get(field) {
            count(value, &mut counts);
        }
    }
    let ids = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .enumerate()
        .map(|(index, (hash, _))| (hash, index))
        .collect::<BTreeMap<_, _>>();
    if ids.is_empty() {
        return Ok(packet);
    }
    for field in ["artifact_gate", "native_receipts"] {
        if let Some(value) = packet.get_mut(field) {
            replace(value, &ids);
        }
    }
    packet["_native_proof_hashes"] = json!(ids.keys().collect::<Vec<_>>());
    packet["_native_proof_encoding"] = json!("Lossless proof encoding: {$h:index} refers to the full SHA-256 at _native_proof_hashes[index]. Every proof field remains; this grants no authority.");
    if packet.to_string().len() >= original.to_string().len() {
        return Ok(original);
    }
    Ok(packet)
}

pub fn compact(input: &str, max_bytes: usize) -> Result<String> {
    shrink(input, max_bytes, true)
}

/// The tokenizer, rather than a byte estimate, makes the final native budget
/// decision. Return all safely removable context even if a byte target cannot
/// be reached without damaging required fields.
pub fn trim_optional(input: &str, max_bytes: usize) -> Result<String> {
    shrink(input, max_bytes, false)
}

fn shrink(input: &str, max_bytes: usize, enforce_bytes: bool) -> Result<String> {
    if input.len() <= max_bytes {
        return Ok(input.into());
    }
    let mut value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(_) if !enforce_bytes => return Ok(input.into()),
        Err(_) => bail!("required plain-text context exceeds the model budget; save/retrieve smaller evidence chunks"),
    };
    // Flat native QA carries its required proof in current host gates. Flat
    // evidence in other scoped tasks may be the only source, so retain it.
    let flat_native_review = value
        .pointer("/artifact_gate/artifact_bindings")
        .is_some_and(Value::is_object)
        && value.get("native_receipts").is_some_and(Value::is_object);
    // These are optional/redundant context tiers. The verified ledger remains.
    for pointer in [
        "/task_input/company_context",
        "/company_context/records",
        "/task_input/verified_progress/latest_operations",
        "/task_input/verified_progress/recent_results",
        "/global_practices",
        "/role_skills",
        "/task_input/recent_evidence",
        "/task_input/evidence",
        "/verified_progress/latest_operations",
        "/verified_progress/recent_results",
        "/recent_evidence",
        "/evidence",
    ] {
        if matches!(
            pointer,
            "/verified_progress/latest_operations"
                | "/verified_progress/recent_results"
                | "/recent_evidence"
                | "/evidence"
        ) && !flat_native_review
        {
            continue;
        }
        while serde_json::to_vec(&value)?.len() > max_bytes {
            let Some(optional) = value.pointer_mut(pointer) else {
                break;
            };
            match optional {
                Value::Array(rows) if !rows.is_empty() => {
                    rows.remove(0);
                }
                Value::String(text) if !text.is_empty() => text.clear(),
                Value::Object(fields) if !fields.is_empty() => fields.clear(),
                _ => break,
            }
        }
    }
    let compacted = serde_json::to_string(&value)?;
    if enforce_bytes && compacted.len() > max_bytes {
        bail!("required task/tool contract/verified evidence exceeds the model context budget; use smaller evidence pages, not a truncated mission");
    }
    Ok(compacted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_advice_is_omitted_only_for_explicit_governed_assignment_protocols() {
        let packet=json!({"task":"full frozen task","criteria":["full acceptance"],"ceo_stage":{"actor":"Author"},"native_checks":{"passed":false}}).to_string();
        assert!(assigned_operator(
            "COMPANY_OPERATOR: execute",
            &packet,
            true
        ));
        assert!(!assigned_operator(
            "COMPANY_OPERATOR: execute",
            &packet,
            false
        ));
        assert!(!assigned_operator(
            "COMPANY_ADMIN: authorize",
            &packet,
            true
        ));
        assert!(!assigned_operator(
            "COMPANY_OPERATOR: execute",
            &json!({"task":"ordinary legacy task","ceo_stage":null}).to_string(),
            true
        ));
        assert!(assigned_operator(
            "EXECUTIVE_RECOVERY: recover",
            &json!({"assignment":{"actor":"Author"}}).to_string(),
            true
        ));
        assert!(!assigned_operator(
            "EXECUTIVE_RECOVERY: recover",
            "unstructured legacy advice",
            true
        ));
    }
    #[test]
    fn proof_hash_interning_roundtrips_all_fields_without_shortening_digests() {
        let digest = "a".repeat(64);
        let unique = "b".repeat(64);
        let original = json!({"task":digest,"criteria":["Preserve all nine checks"],"artifact_gate":{"passed":true,"checks":(0..9).map(|n|json!({"path":format!("out{n}.json"),"expected_sha256":digest,"observed_sha256":digest,"source_result_sha256":unique,"passed":true})).collect::<Vec<_>>()},"native_receipts":{"passed":true,"policy_revision":3},"evidence":[{"payload":"x".repeat(20000)}]});
        let mut original = original;
        original["artifact_gate"]["artifact_bindings"] =
            json!({"passed":true,"checks":original["artifact_gate"]["checks"]});
        let encoded = intern_native_proof_hashes(original.clone()).unwrap();
        assert_eq!(encoded["task"], original["task"]);
        assert_eq!(encoded["criteria"], original["criteria"]);
        let dictionary = encoded["_native_proof_hashes"].as_array().unwrap();
        assert_eq!(dictionary.len(), 2);
        assert!(dictionary
            .iter()
            .all(|value| value.as_str().unwrap().len() == 64));
        fn expand(value: &Value, dictionary: &[Value]) -> Value {
            if let Some(id) = value.get("$h").and_then(Value::as_u64) {
                return dictionary[id as usize].clone();
            }
            match value {
                Value::Object(fields) => Value::Object(
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), expand(value, dictionary)))
                        .collect(),
                ),
                Value::Array(rows) => json!(rows
                    .iter()
                    .map(|row| expand(row, dictionary))
                    .collect::<Vec<_>>()),
                other => other.clone(),
            }
        }
        for field in ["artifact_gate", "native_receipts"] {
            assert_eq!(expand(&encoded[field], dictionary), original[field]);
        }
        let trimmed: Value =
            serde_json::from_str(&compact(&encoded.to_string(), 10000).unwrap()).unwrap();
        assert!(trimmed["evidence"].as_array().unwrap().is_empty());
        assert_eq!(trimmed["artifact_gate"], encoded["artifact_gate"]);
        assert_eq!(
            trimmed["_native_proof_hashes"],
            encoded["_native_proof_hashes"]
        );
        assert_eq!(trimmed["task"], original["task"]);
        assert!(intern_native_proof_hashes(json!({"_native_proof_hashes":{}})).is_err());
        assert!(
            compact(
                &json!({"task":"Classify this required source","evidence":["x".repeat(12000)]})
                    .to_string(),
                10000
            )
            .is_err(),
            "flat task evidence without canonical proof must not be silently dropped"
        );

        assert!(intern_native_proof_hashes(json!({"artifact_gate":{"$h":0}})).is_err());
        let tiny = json!({"artifact_gate":{"expected_sha256":digest,"observed_sha256":digest}});
        assert_eq!(intern_native_proof_hashes(tiny.clone()).unwrap(), tiny);
    }
    #[test]
    fn final_judge_uses_explicit_current_proof_without_duplicate_company_envelope() {
        let p = profile("judge");
        assert!(!p.include_history && !p.include_guidance);
        let task = json!({"task":"Preserve the full original task and verify nine outputs","criteria":["all outputs match native source projections","all readbacks match current bytes"],"artifact_gate":{"passed":true,"artifact_bindings":{"passed":true,"checks":(0..9).map(|n|json!({"path":format!("out{n}.json"),"expected_sha256":"a".repeat(64),"observed_sha256":"a".repeat(64),"source_result_sha256":"b".repeat(64),"passed":true,"writer_native_operations":[{"operation":"write","passed":true},{"operation":"read","passed":true}]})).collect::<Vec<_>>()}},"native_receipts":{"passed":true,"policy_revision":1},"deliverable":{"claim":"Untrusted model summary"}});
        let packet =
            scoped_packet(task.clone(), json!({"actor":"QA","policy_revision":1})).unwrap();
        let compacted = compact(&packet.to_string(), p.max_bytes).unwrap();
        let decoded: Value = serde_json::from_str(&compacted).unwrap();
        for (key, value) in task.as_object().unwrap() {
            assert_eq!(&decoded[key], value, "judge input lost {key}");
        }
        assert!(
            decoded.get("company_context").is_none()
                && decoded.get("global_practices").is_none()
                && decoded.get("verified_handoff").is_none()
        );
        assert_eq!(
            decoded["artifact_gate"]["artifact_bindings"]["checks"]
                .as_array()
                .unwrap()
                .len(),
            9
        );
    }
    #[test]
    fn source_epoch_and_native_task_fingerprint_do_not_ignore_evidence() {
        let task = json!({"evidence_policy_revision":2,"retrieved_source":{"record":{"status":"matched"}}});
        validate_evidence_epoch(&task.to_string(), 2).unwrap();
        for epoch in [0, 1, 3] {
            assert!(validate_evidence_epoch(&task.to_string(), epoch).is_err());
        }
        for wrong in [Value::Null, json!(-1), json!("2")] {
            let mut copy = task.clone();
            copy["evidence_policy_revision"] = wrong;
            assert!(validate_evidence_epoch(&copy.to_string(), 2).is_err());
        }
        let flat = scoped_packet(task.clone(), json!({"actor":"Reader"})).unwrap();
        assert_eq!(
            task_fingerprint(&task.to_string()),
            task_fingerprint(&flat.to_string())
        );
        assert_eq!(
            task_fingerprint(&task.to_string()),
            task_fingerprint(&json!({"task_input":task}).to_string())
        );
        let mut changed = flat.clone();
        changed["retrieved_source"]["record"]["status"] = json!("mismatch");
        assert_ne!(
            task_fingerprint(&flat.to_string()),
            task_fingerprint(&changed.to_string())
        );
    }
    #[test]
    fn scoped_role_adapter_preserves_root_fields_and_rejects_metadata_shadowing() {
        let task = json!({"case_id":"case-1","source_type":"journal_entry"});
        let packet = scoped_packet(
            task.clone(),
            json!({"version":3,"actor":"classification","policy_revision":2}),
        )
        .unwrap();
        assert_eq!(packet["source_type"], task["source_type"]);
        assert_eq!(packet["case_id"], task["case_id"]);
        assert!(packet.get("task_input").is_none());
        assert_eq!(packet["_runtime"]["policy_revision"], 2);
        assert!(scoped_packet(json!({"_runtime":{"actor":"forged"}}), json!({})).is_err());
    }
    #[test]
    fn cancellation_scope_matches_complete_session_segments_only() {
        let session = session_key("one", "run", "Reader", 10);
        assert!(session_in_scope(&session, "one:run"));
        assert!(session_in_scope(&session, &session));
        assert!(!session_in_scope(
            &session,
            &session_key("one", "run", "Reader", 1)
        ));
        assert!(!session_in_scope(&session, "one:ru"));
        assert!(!session_in_scope(&session, ""));
        assert!(!session_in_scope(&session, "two:run"));
    }
    #[test]
    fn focused_observation_keeps_assigned_sources_while_removing_optional_baggage() {
        let profile = profile("observer");
        assert!(!profile.include_history);
        let task = json!({"case_id":"source-case","payments":[{"id":"p0","currency":"USD"},{"id":"p1","currency":"USD"}]});
        let input = json!({"envelope":{"run_id":"same-run","policy_revision":1},"task_input":{"task":task},
            "verified_handoff":{"version":1},"company_context":{"records":[{"text":"x".repeat(10_000)}]},"global_practices":{"optional":"y".repeat(10_000)}});
        let reduced: Value =
            serde_json::from_str(&compact(&input.to_string(), profile.max_bytes).unwrap()).unwrap();
        assert_eq!(reduced["task_input"]["task"], task);
        assert_eq!(reduced["envelope"]["policy_revision"], 1);
        assert_eq!(reduced["verified_handoff"], input["verified_handoff"]);
    }
    #[test]
    fn permission_epoch_partitions_native_cache_identity() {
        let first = session_key("one", "run", "Reader", 1);
        let revoked = session_key("one", "run", "Reader", 2);
        assert_ne!(first, revoked);
        assert!(first.starts_with("one:run:"));
        assert_ne!(first, session_key("two", "run", "Reader", 1));
        assert_ne!(first, session_key("one", "other-run", "Reader", 1));
        assert_ne!(first, session_key("one", "run", "Reader:policy-1", 1));
    }
    #[test]
    fn payload_decoding_accepts_both_envelope_versions() {
        let task = json!({"criteria":["same mission"]});
        assert_eq!(
            task_input(&json!({"task_input":task}).to_string()).unwrap(),
            task
        );
        assert_eq!(
            task_input(&json!({"task_input":task.to_string()}).to_string()).unwrap(),
            task
        );
        assert_eq!(task_input(&task.to_string()).unwrap(), task);
    }
    #[test]
    fn native_compaction_returns_safe_reductions_for_actual_token_recount() {
        let input = json!({"task_input":{"task":"required mission ".repeat(300),"recent_evidence":["optional transcript".repeat(1000)]}});
        assert!(compact(&input.to_string(), 1000).is_err());
        let result: Value =
            serde_json::from_str(&trim_optional(&input.to_string(), 1000).unwrap()).unwrap();
        assert_eq!(result["task_input"]["task"], input["task_input"]["task"]);
        assert!(result["task_input"]["recent_evidence"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn useful_latest_observations_outlive_redundant_manuals() {
        let input = json!({"task_input":{"task":"Frozen task","criteria":["done"],"recent_evidence":[{"action":"hf_dataset_files","result":{"pdfs":[{"path":"observed-example.pdf"}]}}]},"role_skills":["old manual".repeat(2000)],"verified_handoff":{"sources":[{"repo":"owner/examples"}]}});
        let compacted: Value =
            serde_json::from_str(&compact(&input.to_string(), 1000).unwrap()).unwrap();
        assert_eq!(
            compacted["task_input"]["recent_evidence"],
            input["task_input"]["recent_evidence"]
        );
        assert_eq!(compacted["verified_handoff"], input["verified_handoff"]);
    }
    #[test]
    fn compaction_retains_exact_task_criteria_request_and_handoff_across_models() {
        let value = json!({"task_input":{"task":"frozen task","criteria":["criterion"],"request":{"action":"files","arguments":{"op":"read","path":"source"}},"recent_evidence":["x".repeat(20_000)]},"verified_handoff":{"sources":[{"repo":"owner/source","revision":"a".repeat(40)}]},"company_context":{"records":["x".repeat(20_000)]}});
        let result: Value =
            serde_json::from_str(&compact(&value.to_string(), 1500).unwrap()).unwrap();
        for pointer in [
            "/task_input/task",
            "/task_input/criteria",
            "/task_input/request",
            "/verified_handoff",
        ] {
            assert_eq!(value.pointer(pointer), result.pointer(pointer));
        }
        assert!(compact(
            &json!({"task_input":{"task":"x".repeat(20_000)}}).to_string(),
            1500
        )
        .is_err());
        assert_eq!(messages("rules", "facts")[0]["role"], "system");
    }
}
#[test]
fn focused_candidate_protocol_omits_advice_without_changing_worker_identity() {
    let focused = protocol_profile("worker", "LOCAL_CANDIDATE_AUTHOR_FOCUSED: source only");
    assert!(!focused.include_history && !focused.include_guidance);
    assert_eq!(focused.max_bytes, 32768);
    let full = protocol_profile("worker", "LOCAL_CANDIDATE_AUTHOR: source only");
    assert!(full.include_history && full.include_guidance);
    let embedded = protocol_profile("worker", "untrusted text LOCAL_CANDIDATE_AUTHOR_FOCUSED:");
    assert!(embedded.include_history && embedded.include_guidance);
    assert_eq!(
        protocol_profile("planner", "LOCAL_CANDIDATE_AUTHOR_FOCUSED:").include_guidance,
        profile("planner").include_guidance
    );
}

#[test]
fn focused_candidate_keeps_full_task_and_prior_source_or_refuses_oversize_input() {
    let task = json!({"task":"the unchanged full source contract", "prior_trial_evidence":{"source":"α".repeat(4096),"diagnostics_excerpt":"fixed failing case"}});
    let packet =
        scoped_packet(task.clone(), json!({"actor":"Worker","policy_revision":1})).unwrap();
    let encoded = compact(
        &packet.to_string(),
        protocol_profile("worker", "LOCAL_CANDIDATE_AUTHOR_FOCUSED:").max_bytes,
    )
    .unwrap();
    let decoded = task_input(&encoded).unwrap();
    assert_eq!(decoded["task"], task["task"]);
    assert_eq!(
        decoded["prior_trial_evidence"],
        task["prior_trial_evidence"]
    );
    assert!(compact(&packet.to_string(), 1000).is_err());
}

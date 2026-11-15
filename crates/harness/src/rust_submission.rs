//! Compile a review request from fresh native source gates, never acceptance.
//! The caller validates owner approval/canonical gate custody. No source,
//! tool execution, permissions, task answer or promotion is created here.
use crate::{governance::Policy, native_submission::Submission};
use gents_local_mcp::{Phase, Scope};
use serde_json::{json, Value};

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn compile(
    intake: &Value,
    scope: &Scope,
    policy: &Policy,
    contract: &str,
    stage: usize,
    gate: &Value,
) -> Option<Submission> {
    scope.validate().ok()?;
    let epoch = scope.expected_revision.filter(|epoch| *epoch > 0)?;
    let native = &gate["native_receipts"];
    let artifacts = &gate["artifact_bindings"];
    let assignment = intake["ceo_plan"]["assignments"].as_array()?.get(stage)?;
    if scope.phase != Phase::Worker
        || !hash(contract)
        || policy.revision != epoch
        || assignment["actor"] != scope.actor
        || gate["passed"] != true
        || native["passed"] != true
        || native["policy_revision"] != epoch
        || artifacts["passed"] != true
        || artifacts["policy_revision"] != epoch
    {
        return None;
    }
    for tool in ["files:write", "files:read", "rust_verify"] {
        if !assignment["tools"].as_array()?.contains(&json!(tool)) {
            return None;
        }
    }
    policy.authorize(&scope.actor, "finish", &json!({})).ok()?;
    let calls = native["checks"].as_array()?;
    if !calls.iter().all(|row| row["passed"] == true) {
        return None;
    }
    let checks = native["rust_checks"].as_array()?;
    if checks.is_empty() || !checks.iter().all(|row| row["passed"] == true) {
        return None;
    }
    let specs = intake["required_rust_checks"].as_array()?;
    let mut sources = Vec::new();
    for spec in specs.iter().filter(|spec| spec["actor"] == scope.actor) {
        let path = spec["source_path"].as_str()?;
        let tests = spec["tests_sha256"].as_str()?;
        if !hash(tests) || !gents_local_mcp::workspace_path::is_safe_relative_path(path) {
            return None;
        }
        for op in ["read", "write"] {
            policy
                .authorize(&scope.actor, "files", &json!({"op":op,"path":path}))
                .ok()?;
        }
        policy
            .authorize(&scope.actor, "rust_verify", &json!({"source_path":path}))
            .ok()?;
        let check = checks.iter().find(|row| {
            row["actor"] == scope.actor
                && row["source_path"] == path
                && row["tests_sha256"] == tests
        })?;
        let source = check["current_source_sha256"].as_str()?;
        if !hash(source)
            || check["successful_current_source_checks"].as_u64()? == 0
            || check["successful_current_source_readbacks"].as_u64()? == 0
            || check["latest_check"]["canonical_receipt_verified"] != true
            || check["latest_check"]["passed"] != true
            || check["latest_check"]["stage"] != "acceptance"
        {
            return None;
        }
        sources.push(json!({"actor":scope.actor,"source_path":path,"source_sha256":source,"tests_sha256":tests}));
    }
    if sources.is_empty() {
        return None;
    }
    let artifact_identity = if artifacts["checks"].as_array()?.is_empty() {
        None
    } else {
        Some(crate::native_submission::fingerprint(
            contract,
            &scope.actor,
            stage,
            Some(epoch),
            gate,
        )?)
    };
    let identity = json!({"version":1,"kind":"frozen_rust_submission","evidence_schema_version":crate::evidence::SCHEMA_VERSION,
        "contract":contract,"actor":scope.actor,"stage":stage,"epoch":epoch,"sources":sources,
        "artifacts":artifact_identity,"generic_required":gate["required"],"generic_artifact_sha256":gate["artifact_sha256"],"named_artifact_source":gate["named_artifact_source"]});
    Some(Submission {
        fingerprint: gents_local_mcp::digest(identity.to_string().as_bytes()),
        proposal: json!({"action":"finish","arguments":{"deliverable":"Submit the current source-bound native verification evidence for independent review.","evidence":json!({"policy_revision":epoch,"rust_sources":sources,"scope":"Canonical native acceptance and full readback only; independent QA must still review every frozen criterion."}).to_string()},"summary":"Host-compiled Rust review request; no new source, tool authority, acceptance or promotion"}),
        observation: json!({"origin":"host_compiled_frozen_rust_submission","model_generated_summary":false,"independent_qa_required":true,"authority_granted":false,"rust_sources":sources}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Value, Scope, Policy, Value) {
        let scope = Scope {
            company: "fixture".into(),
            owner: "owner".into(),
            actor: "Author".into(),
            run: uuid::Uuid::new_v4(),
            phase: Phase::Worker,
            expected_revision: Some(2),
        };
        let access = json!({"tools":["files:write","files:read","rust_verify","finish"],"skills":[],"memory_from":[]});
        let policy = serde_json::from_value(json!({"revision":2,"master":"Author","ceilings":{"Author":access},"grants":{"Author":access}})).unwrap();
        let intake = json!({"ceo_plan":{"assignments":[{"actor":"Author","tools":["files:write","files:read","rust_verify"]}]},"required_rust_checks":[{"actor":"Author","source_path":"candidate.rs","tests_path":"acceptance.rs","tests_sha256":"b".repeat(64)}]});
        let gate = json!({"passed":true,"required":{},"artifact_bindings":{"passed":true,"policy_revision":2,"checks":[]},"native_receipts":{"passed":true,"policy_revision":2,"checks":[{"actor":"Author","tool":"files","passed":true},{"actor":"Author","tool":"rust_verify","passed":true}],"rust_checks":[{"actor":"Author","source_path":"candidate.rs","tests_sha256":"b".repeat(64),"current_source_sha256":"a".repeat(64),"successful_current_source_checks":1,"successful_current_source_readbacks":1,"passed":true,"latest_check":{"canonical_receipt_verified":true,"passed":true,"stage":"acceptance"}}]}});
        (intake, scope, policy, gate)
    }
    #[test]
    fn identity_tracks_bytes_epoch_contract_and_stage_not_counts_or_model_claims() {
        let (intake, scope, policy, gate) = fixture();
        let before = serde_json::to_value(&policy).unwrap();
        let first = compile(&intake, &scope, &policy, &"c".repeat(64), 0, &gate).unwrap();
        assert_eq!(first.proposal["action"], "finish");
        assert_eq!(first.observation["authority_granted"], false);
        assert_eq!(first.observation["independent_qa_required"], true);
        assert_eq!(serde_json::to_value(&policy).unwrap(), before);
        let mut more = gate.clone();
        more["native_receipts"]["rust_checks"][0]["successful_current_source_checks"] = json!(99);
        more["model_claim"] = json!("REPAIR_SUCCEEDED_MARKER");
        let same = compile(&intake, &scope, &policy, &"c".repeat(64), 0, &more).unwrap();
        assert_eq!(first.fingerprint, same.fingerprint);
        // A source-only contract may use typed Rust checks without an extra
        // generic required_mcp_calls list. The canonical source gate still
        // requires both actual acceptance and full readback.
        more["native_receipts"]["checks"] = json!([]);
        assert!(compile(&intake, &scope, &policy, &"c".repeat(64), 0, &more).is_some());
        assert!(!same
            .proposal
            .to_string()
            .contains("REPAIR_SUCCEEDED_MARKER"));
        let mut changed = gate.clone();
        changed["native_receipts"]["rust_checks"][0]["current_source_sha256"] =
            json!("d".repeat(64));
        assert_ne!(
            first.fingerprint,
            compile(&intake, &scope, &policy, &"c".repeat(64), 0, &changed)
                .unwrap()
                .fingerprint
        );
        assert_ne!(
            first.fingerprint,
            compile(&intake, &scope, &policy, &"d".repeat(64), 0, &gate)
                .unwrap()
                .fingerprint
        );
    }
    #[test]
    fn missing_stale_unverified_or_unassigned_facts_and_revocation_cannot_submit() {
        let (intake, scope, policy, gate) = fixture();
        for (pointer, value) in [
            ("/passed", json!(false)),
            ("/native_receipts/passed", json!(false)),
            ("/artifact_bindings/passed", json!(false)),
            ("/artifact_bindings/policy_revision", json!(3)),
            ("/native_receipts/policy_revision", json!(3)),
            ("/native_receipts/checks/0/passed", json!(false)),
            ("/native_receipts/rust_checks/0/actor", json!("Other")),
            ("/native_receipts/rust_checks/0/passed", json!(false)),
            (
                "/native_receipts/rust_checks/0/current_source_sha256",
                json!("unknown"),
            ),
            (
                "/native_receipts/rust_checks/0/tests_sha256",
                json!("d".repeat(64)),
            ),
            (
                "/native_receipts/rust_checks/0/successful_current_source_checks",
                json!(0),
            ),
            (
                "/native_receipts/rust_checks/0/successful_current_source_readbacks",
                json!(0),
            ),
            (
                "/native_receipts/rust_checks/0/latest_check/canonical_receipt_verified",
                json!(false),
            ),
            (
                "/native_receipts/rust_checks/0/latest_check/passed",
                json!(false),
            ),
            (
                "/native_receipts/rust_checks/0/latest_check/stage",
                json!("compile_candidate"),
            ),
        ] {
            let mut changed = gate.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(
                compile(&intake, &scope, &policy, &"c".repeat(64), 0, &changed).is_none(),
                "{pointer}"
            );
        }
        for tool in ["files:write", "files:read", "rust_verify", "finish"] {
            let mut revoked = policy.clone();
            revoked.grants.get_mut("Author").unwrap().tools.remove(tool);
            assert!(compile(&intake, &scope, &revoked, &"c".repeat(64), 0, &gate).is_none());
        }
        let mut stale = scope.clone();
        stale.expected_revision = Some(1);
        assert!(compile(&intake, &stale, &policy, &"c".repeat(64), 0, &gate).is_none());
        stale = scope.clone();
        stale.phase = Phase::Planner;
        assert!(compile(&intake, &stale, &policy, &"c".repeat(64), 0, &gate).is_none());
        assert!(compile(&intake, &scope, &policy, &"c".repeat(64), 1, &gate).is_none());
        assert!(compile(&intake, &scope, &policy, "model approval", 0, &gate).is_none());
        let mut wrong = intake.clone();
        wrong["ceo_plan"]["assignments"][0]["actor"] = json!("Other");
        assert!(compile(&wrong, &scope, &policy, &"c".repeat(64), 0, &gate).is_none());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn real_native_readback_and_acceptance_enable_only_a_review_proposal() {
        use gents_local_mcp::authority::Authority;
        let mut native = crate::native_fixture::Fixture::new();
        let (mut intake, _, policy, _) = fixture();
        native.scope.actor = "Author".into();
        native.scope.expected_revision = Some(2);
        let tests = "#[test]fn protocol(){std::assert_eq!(candidate::f(),1);}";
        intake["required_rust_checks"][0]["tests_sha256"] =
            json!(gents_local_mcp::digest(tests.as_bytes()));
        let contract = gents_local_mcp::digest(intake.to_string().as_bytes());
        let db = native.db();
        gents_local_mcp::governance::save(&db, &native.scope.run.to_string(), &policy, 1).unwrap();
        db.execute("UPDATE jobs SET intake=?1,digest=?2,approval=?3",rusqlite::params![intake.to_string(),contract,json!({"owner":native.scope.owner,"sha256":contract,"type":"local_operator_approval"}).to_string()]).unwrap();
        let root = native.authority.workspace(&native.scope).unwrap();
        gents_local_mcp::workspace::open(&root, true)
            .unwrap()
            .write("acceptance.rs", tests)
            .unwrap();
        let registry = gents_local_mcp::Registry::new();
        let gate = || {
            let native_gate =
                gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap();
            json!({"passed":native_gate["passed"],"native_receipts":native_gate,"artifact_bindings":gents_local_mcp::artifact_bindings::check(&native.authority,&native.scope,true).unwrap()})
        };
        for source in ["pub fn f()->u64{0}", "pub fn f()->u64{1}"] {
            registry
                .call(
                    &native.authority,
                    &native.scope,
                    "files",
                    json!({"op":"write","path":"candidate.rs","content":source}),
                )
                .unwrap();
            assert!(compile(&intake, &native.scope, &policy, &contract, 0, &gate()).is_none());
            registry
                .call(
                    &native.authority,
                    &native.scope,
                    "files",
                    json!({"op":"read","path":"candidate.rs"}),
                )
                .unwrap();
            registry
                .call(
                    &native.authority,
                    &native.scope,
                    "rust_verify",
                    json!({"source_path":"candidate.rs"}),
                )
                .unwrap();
            assert_eq!(
                compile(&intake, &native.scope, &policy, &contract, 0, &gate()).is_some(),
                source.ends_with("{1}")
            );
        }
        let first = compile(&intake, &native.scope, &policy, &contract, 0, &gate()).unwrap();
        registry
            .call(
                &native.authority,
                &native.scope,
                "rust_verify",
                json!({"source_path":"candidate.rs"}),
            )
            .unwrap();
        assert_eq!(
            compile(&intake, &native.scope, &policy, &contract, 0, &gate())
                .unwrap()
                .fingerprint,
            first.fingerprint
        );
        assert_eq!(first.proposal["action"], "finish");
        assert_eq!(first.observation["authority_granted"], false);
    }
}

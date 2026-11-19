//! Explicit owner-approved review submission. Diagnostic success is never
//! promoted into independent acceptance; normal admin and QA still run.
use crate::{governance::Policy, native_submission::Submission};
use anyhow::{ensure, Result};
use gents_local_mcp::{Phase, Scope};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    version: u8,
    actor: String,
    contract_id: String,
}
fn spec(intake: &Value) -> Result<Option<Spec>> {
    intake
        .get("python_review_submission")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}
pub fn validate(intake: &Value) -> Result<()> {
    let Some(s) = spec(intake)? else {
        return Ok(());
    };
    ensure!(s.version==1 && intake["deliverable_type"]=="python_review_candidate"
        && intake["max_model_calls"].as_u64().is_some_and(|n|(1..=1024).contains(&n))
        && intake["ceo_plan"].is_null() && intake["candidate_workflow"].is_null()
        && intake["python_check_contracts"].as_array().is_some_and(|contracts|contracts.iter()
            .any(|c|c["actor"]==s.actor && c["id"]==s.contract_id
                && c["source_paths"].as_array().is_some_and(|paths|paths.iter().any(|p|
                    p.as_str().is_some_and(|name|c["frozen_inputs"].get(name).is_none()))))),
        "Python review submission requires its explicit candidate type, frozen allowance and diagnostic actor/contract; existing workflow modes cannot be combined");
    Ok(())
}
pub fn compile(
    intake: &Value,
    scope: &Scope,
    policy: &Policy,
    digest: &str,
    observations: &Value,
) -> Option<Submission> {
    scope.validate().ok()?;
    validate(intake).ok()?;
    let s = spec(intake).ok()??;
    let epoch = scope.expected_revision.filter(|r| *r > 0)?;
    if scope.phase != Phase::Worker
        || scope.actor != s.actor
        || policy.revision != epoch
        || digest.len() != 64
        || !digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let contract = intake["python_check_contracts"]
        .as_array()?
        .iter()
        .find(|c| c["actor"] == s.actor && c["id"] == s.contract_id)?;
    let view = observations
        .as_array()?
        .iter()
        .find(|v| v["contract_id"] == s.contract_id)?;
    if view["policy_revision"] != epoch
        || view["current_source_available"] != true
        || view["latest_attempt_uncertain"] != false
        || view["independent_acceptance"] != false
        || view["authority_granted"] != false
        || view["latest_current_diagnostic"]["status"] != "reported_pass"
        || view["latest_current_diagnostic"]["canonical_receipt_verified"] != true
    {
        return None;
    }
    let identity = view["current_source_identity"].as_str()?;
    if identity.len() != 64 || !identity.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let paths = contract["source_paths"].as_array()?;
    if paths.is_empty() || paths.len() > 31 {
        return None;
    }
    let mut sources = vec![];
    for path in paths {
        let path = path.as_str()?;
        if contract["frozen_inputs"].get(path).is_some() {
            continue;
        }
        let custody = &view["current_source_custody"][path];
        let hash = custody["source_sha256"].as_str()?;
        if custody["written"] != true
            || custody["read_back"] != true
            || hash.len() != 64
            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return None;
        }
        for op in ["read", "write"] {
            policy
                .authorize(&scope.actor, "files", &json!({"op":op,"path":path}))
                .ok()?;
        }
        sources.push(json!({"path":path,"source_sha256":hash}));
    }
    if sources.is_empty() {
        return None;
    }
    policy
        .authorize(
            &scope.actor,
            "python_check",
            &json!({"contract_id":s.contract_id}),
        )
        .ok()?;
    policy.authorize(&scope.actor, "finish", &json!({})).ok()?;
    let fingerprint=gents_local_mcp::digest(json!({"version":1,"job_digest":digest,
        "run":scope.run,"actor":scope.actor,"policy_revision":epoch,"contract_id":s.contract_id,
        "source_identity":identity,"sources":sources,"controller":gents_local_mcp::digest(include_str!("python_review_submission.rs").as_bytes())}).to_string().as_bytes());
    Some(Submission {
        fingerprint,
        proposal: json!({"action":"finish","arguments":{
        "deliverable":"Submit the current Python candidate sources for separate host review. Current native write/readback receipts and diagnostic results are retained. Independent correctness and source promotion are not established.",
        "evidence":"Current source-bound python_diagnostics and canonical file write/readback receipts; original task criteria still require independent QA review."},
        "summary":"Host-compiled owner-approved review-stage submission; no new authority, independent acceptance or promotion"}),
        observation: json!({"origin":"owner_frozen_python_review_submission","sources":sources,
            "source_identity":identity,"policy_revision":epoch,"independent_qa_required":true,
            "independent_acceptance":false,"authority_granted":false,"promoted":false}),
    })
}

/// Compile required read/test steps only after an exact confirmed native write.
/// Never replay a write or manufacture its receipt; each followup still passes
/// ordinary current-policy and independent action review.
pub fn after_write(
    intake: &Value,
    scope: &Scope,
    policy: &Policy,
    request: &Value,
    result: &Value,
) -> Option<Value> {
    validate(intake).ok()?;
    let s = spec(intake).ok()??;
    let epoch = scope.expected_revision.filter(|r| *r > 0)?;
    let args = &request["arguments"];
    let path = args["path"].as_str()?;
    let content = args["content"].as_str()?;
    let value = &result["result"];
    let receipt = &result["mcp_receipt"];
    if scope.phase != Phase::Worker
        || scope.actor != s.actor
        || policy.revision != epoch
        || request["action"] != "files"
        || args["op"] != "write"
        || result["ok"] != true
        || content.len() > 32768
        || value["path"] != path
        || value["bytes"] != content.len()
        || value["sha256"] != gents_local_mcp::digest(content.as_bytes())
        || receipt["status"] != "completed"
        || receipt["tool"] != "files"
        || receipt["actor"] != scope.actor
        || receipt["phase"] != "worker"
        || receipt["policy_revision"] != epoch
        || receipt["provider_cost_usd"] != 0
        || receipt["arguments_sha256"]
            != gents_local_mcp::digest(serde_json::to_vec(args).ok()?.as_slice())
        || receipt["result_sha256"]
            != gents_local_mcp::digest(serde_json::to_vec(value).ok()?.as_slice())
    {
        return None;
    }
    let contract = intake["python_check_contracts"]
        .as_array()?
        .iter()
        .find(|c| c["actor"] == s.actor && c["id"] == s.contract_id)?;
    if !contract["source_paths"].as_array()?.contains(&json!(path))
        || contract["frozen_inputs"].get(path).is_some()
    {
        return None;
    }
    let mut calls = vec![];
    let mut offset = 0;
    loop {
        let args = json!({"op":"read","path":path,"offset_bytes":offset,"max_bytes":8192});
        policy.authorize(&scope.actor, "files", &args).ok()?;
        calls.push(json!({"action":"files","arguments":args,"summary":"Host-compiled current-source readback after confirmed write; normal authorization required"}));
        let mut end = content.len().min(offset + 8192);
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        if end == content.len() {
            break;
        }
        if end <= offset {
            return None;
        }
        offset = end;
    }
    let args = json!({"contract_id":s.contract_id});
    policy.authorize(&scope.actor, "python_check", &args).ok()?;
    calls.push(json!({"action":"python_check","arguments":args,"summary":"Host-compiled frozen diagnostic check after source readback; normal authorization required"}));
    Some(
        json!({"calls":calls,"source_path":path,"source_sha256":value["sha256"],"policy_revision":epoch,
        "origin":"owner_frozen_python_write_followups","model_generated":false,"authority_granted":false,
        "independent_acceptance":false,"promoted":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmed_write_requires_exact_receipt_current_grants_and_complete_utf8_readback() {
        let intake = json!({"deliverable_type":"python_review_candidate","max_model_calls":16,
            "python_review_submission":{"version":1,"actor":"Author","contract_id":"tests"},
            "python_check_contracts":[{"id":"tests","actor":"Author","source_paths":["a.py","base.py"],"frozen_inputs":{"base.py":"d".repeat(64)}}]});
        let access = json!({"tools":["files:read","files:write","python_check","finish"],"skills":[],"memory_from":[]});
        let policy:Policy=serde_json::from_value(json!({"revision":1,"master":"Author","ceilings":{"Author":access},"grants":{"Author":access}})).unwrap();
        let scope = Scope {
            company: "fixture".into(),
            owner: "owner".into(),
            run: uuid::Uuid::new_v4(),
            actor: "Author".into(),
            phase: Phase::Worker,
            expected_revision: Some(1),
        };
        let content = format!("{}αtail", "x".repeat(8191));
        let args = json!({"op":"write","path":"a.py","content":content});
        let value = json!({"path":"a.py","bytes":content.len(),"sha256":gents_local_mcp::digest(content.as_bytes())});
        let request = json!({"action":"files","arguments":args});
        let result = json!({"ok":true,"result":value,"mcp_receipt":{"status":"completed","tool":"files","actor":"Author","phase":"worker","policy_revision":1,"provider_cost_usd":0,
            "arguments_sha256":gents_local_mcp::digest(&serde_json::to_vec(&args).unwrap()),"result_sha256":gents_local_mcp::digest(&serde_json::to_vec(&value).unwrap())}});
        let plan = after_write(&intake, &scope, &policy, &request, &result).unwrap();
        assert_eq!(plan["calls"].as_array().unwrap().len(), 3);
        assert_eq!(plan["calls"][0]["arguments"]["offset_bytes"], 0);
        assert_eq!(plan["calls"][1]["arguments"]["offset_bytes"], 8191);
        assert_eq!(plan["calls"][2]["action"], "python_check");
        assert_eq!(plan["promoted"], false);
        for (pointer, value) in [
            ("/ok", json!(false)),
            ("/mcp_receipt/status", json!("effect_uncertain")),
            ("/mcp_receipt/actor", json!("Other")),
            ("/mcp_receipt/policy_revision", json!(2)),
            ("/mcp_receipt/arguments_sha256", json!("forged")),
            ("/result/sha256", json!("forged")),
        ] {
            let mut changed = result.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(
                after_write(&intake, &scope, &policy, &request, &changed).is_none(),
                "{pointer}"
            );
        }
        let mut frozen = intake.clone();
        frozen["python_check_contracts"][0]["frozen_inputs"]["a.py"] = value["sha256"].clone();
        assert!(after_write(&frozen, &scope, &policy, &request, &result).is_none());
        for tool in ["files:read", "python_check"] {
            let mut revoked = policy.clone();
            revoked.grants.get_mut("Author").unwrap().tools.remove(tool);
            assert!(after_write(&intake, &scope, &revoked, &request, &result).is_none());
        }
        let mut changed = request.clone();
        changed["arguments"]["content"] = json!("different");
        assert!(after_write(&intake, &scope, &policy, &changed, &result).is_none());
    }
    #[test]
    fn only_explicit_current_receipt_linked_candidate_can_be_submitted() {
        let intake = json!({"deliverable_type":"python_review_candidate","max_model_calls":16,
            "python_review_submission":{"version":1,"actor":"Author","contract_id":"tests"},
            "python_check_contracts":[{"id":"tests","actor":"Author","source_paths":["a.py"]}]});
        let access = json!({"tools":["files:read","files:write","python_check","finish"],"skills":[],"memory_from":[]});
        let policy:Policy=serde_json::from_value(json!({"revision":1,"master":"Author","ceilings":{"Author":access},"grants":{"Author":access}})).unwrap();
        let scope = Scope {
            company: "fixture".into(),
            owner: "owner".into(),
            run: uuid::Uuid::new_v4(),
            actor: "Author".into(),
            phase: Phase::Worker,
            expected_revision: Some(1),
        };
        let view = json!([{"contract_id":"tests","policy_revision":1,"current_source_available":true,
            "current_source_identity":"a".repeat(64),"latest_attempt_uncertain":false,
            "independent_acceptance":false,"authority_granted":false,
            "latest_current_diagnostic":{"status":"reported_pass","canonical_receipt_verified":true},
            "current_source_custody":{"a.py":{"written":true,"read_back":true,"source_sha256":"b".repeat(64)}}}]);
        let compiled = compile(&intake, &scope, &policy, &"c".repeat(64), &view).unwrap();
        assert_eq!(compiled.proposal["action"], "finish");
        assert_eq!(compiled.observation["independent_acceptance"], false);
        let mut with_dependency = intake.clone();
        with_dependency["python_check_contracts"][0]["source_paths"] = json!(["a.py", "base.py"]);
        with_dependency["python_check_contracts"][0]["frozen_inputs"] =
            json!({"base.py":"d".repeat(64)});
        assert!(compile(&with_dependency, &scope, &policy, &"c".repeat(64), &view).is_some());
        with_dependency["python_check_contracts"][0]["frozen_inputs"]["a.py"] =
            json!("b".repeat(64));
        assert!(validate(&with_dependency).is_err());
        for (pointer, value) in [
            ("/0/current_source_custody/a.py/written", json!(false)),
            ("/0/current_source_custody/a.py/read_back", json!(false)),
            ("/0/latest_attempt_uncertain", json!(true)),
            (
                "/0/latest_current_diagnostic/status",
                json!("reported_fail"),
            ),
            (
                "/0/latest_current_diagnostic/canonical_receipt_verified",
                json!(false),
            ),
            ("/0/policy_revision", json!(2)),
        ] {
            let mut invalid = view.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                compile(&intake, &scope, &policy, &"c".repeat(64), &invalid).is_none(),
                "{pointer}"
            );
        }
        let mut changed = intake.clone();
        changed["deliverable_type"] = json!("agent");
        assert!(validate(&changed).is_err());
        let mut revoked = policy.clone();
        revoked
            .grants
            .get_mut("Author")
            .unwrap()
            .tools
            .remove("finish");
        assert!(compile(&intake, &scope, &revoked, &"c".repeat(64), &view).is_none());
        assert!(compile(&json!({}), &scope, &policy, &"c".repeat(64), &view).is_none());
    }
}

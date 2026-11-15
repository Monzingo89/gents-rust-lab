//! Compile owner-required readback/verification after an actual native source write.
//! No authoring, permissions, acceptance, promotion or write replay occurs here.
use crate::governance::Policy;
use gents_local_mcp::{Phase, Scope};
use serde_json::{json, Value};

pub fn prepend(plan: &Value, queued: Vec<Value>) -> Vec<Value> {
    let mut required = plan["calls"].as_array().unwrap().clone();
    let remaining = queued
        .into_iter()
        .filter(|call| {
            !required.iter().any(|next| {
                call["action"] == next["action"]
                    && crate::contracts::arguments(call).ok().as_ref() == Some(&next["arguments"])
            })
        })
        .collect::<Vec<_>>();
    required.extend(remaining);
    required
}

pub fn after_write(
    intake: &Value,
    scope: &Scope,
    policy: &Policy,
    request: &Value,
    result: &Value,
) -> Option<Value> {
    let args = &request["arguments"];
    let path = args["path"].as_str()?;
    let content = args["content"].as_str()?;
    let receipt = &result["mcp_receipt"];
    let value = &result["result"];
    if scope.phase != Phase::Worker
        || scope.expected_revision != Some(policy.revision)
        || request["action"] != "files"
        || args["op"] != "write"
        || result["ok"] != true
        || content.len() > 8192
        || receipt["status"] != "completed"
        || receipt["phase"] != "worker"
        || receipt["actor"] != scope.actor
        || receipt["tool"] != "files"
        || receipt["policy_revision"] != policy.revision
        || receipt["provider_cost_usd"] != 0
        || receipt["arguments_sha256"] != gents_local_mcp::digest(&serde_json::to_vec(args).ok()?)
        || receipt["result_sha256"] != gents_local_mcp::digest(&serde_json::to_vec(value).ok()?)
        || value["path"] != path
        || value["bytes"] != content.len()
        || value["sha256"] != gents_local_mcp::digest(content.as_bytes())
    {
        return None;
    }
    if !intake["required_rust_checks"]
        .as_array()?
        .iter()
        .any(|spec| spec["actor"] == scope.actor && spec["source_path"] == path)
        || !intake["ceo_plan"]["assignments"]
            .as_array()?
            .iter()
            .any(|stage| {
                stage["actor"] == scope.actor
                    && ["files:write", "files:read", "rust_verify"]
                        .iter()
                        .all(|tool| {
                            stage["tools"]
                                .as_array()
                                .is_some_and(|tools| tools.contains(&json!(tool)))
                        })
            })
    {
        return None;
    }
    let read = json!({"op":"read","path":path});
    let verify = json!({"source_path":path});
    policy.authorize(&scope.actor, "files", &read).ok()?;
    policy
        .authorize(&scope.actor, "rust_verify", &verify)
        .ok()?;
    Some(
        json!({"actor":scope.actor,"source_path":path,"source_sha256":value["sha256"],"policy_revision":policy.revision,
        "origin":"host_compiled_frozen_rust_workflow","authority_granted":false,"model_generated":false,
        "calls":[{"action":"files","arguments":read,"summary":"Host-compiled mandatory full source readback after native author write; independent authorization required"},
            {"action":"rust_verify","arguments":verify,"summary":"Host-compiled owner-frozen Rust acceptance after source readback; independent authorization required"}]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Value, Scope, Policy, Value, Value) {
        let intake = json!({"required_rust_checks":[{"actor":"Author","source_path":"candidate.rs"}],"ceo_plan":{"assignments":[{"actor":"Author","tools":["files:write","files:read","rust_verify"]}]}});
        let scope = Scope {
            company: "company".into(),
            owner: "owner".into(),
            run: uuid::Uuid::new_v4(),
            actor: "Author".into(),
            phase: Phase::Worker,
            expected_revision: Some(1),
        };
        let access = json!({"tools":["files:write","files:read","rust_verify"],"skills":[],"memory_from":[]});
        let policy=serde_json::from_value(json!({"revision":1,"master":"Author","ceilings":{"Author":access},"grants":{"Author":access}})).unwrap();
        let args = json!({"op":"write","path":"candidate.rs","content":"pub fn f(){}"});
        let value = json!({"path":"candidate.rs","bytes":12,"sha256":gents_local_mcp::digest(b"pub fn f(){}")});
        let receipt = json!({"status":"completed","phase":"worker","actor":"Author","tool":"files","policy_revision":1,"provider_cost_usd":0,"arguments_sha256":gents_local_mcp::digest(&serde_json::to_vec(&args).unwrap()),"result_sha256":gents_local_mcp::digest(&serde_json::to_vec(&value).unwrap())});
        (
            intake,
            scope,
            policy,
            json!({"action":"files","arguments":args}),
            json!({"ok":true,"result":value,"mcp_receipt":receipt}),
        )
    }
    #[test]
    fn actual_frozen_write_compiles_only_read_then_verify_without_mutating_authority() {
        let (intake, scope, policy, request, result) = fixture();
        let before = serde_json::to_value(&policy).unwrap();
        let plan = after_write(&intake, &scope, &policy, &request, &result).unwrap();
        assert_eq!(
            plan["calls"][0]["arguments"],
            json!({"op":"read","path":"candidate.rs"})
        );
        assert_eq!(plan["calls"][1]["action"], "rust_verify");
        assert_eq!(plan["authority_granted"], false);
        assert_eq!(serde_json::to_value(&policy).unwrap(), before);
        let later_write = json!({"action":"files","arguments":{"op":"write","path":"candidate.rs","content":"pub fn later(){}"}});
        let queue = prepend(
            &plan,
            vec![
                plan["calls"][0].clone(),
                later_write.clone(),
                plan["calls"][1].clone(),
            ],
        );
        assert_eq!(queue.len(), 3);
        assert_eq!(queue[0]["arguments"]["op"], "read");
        assert_eq!(queue[1]["action"], "rust_verify");
        assert_eq!(queue[2], later_write);
    }
    #[test]
    fn failed_uncertain_forged_unassigned_and_revoked_writes_produce_no_plan() {
        let (intake, scope, policy, request, result) = fixture();
        for (pointer, value) in [
            ("/ok", json!(false)),
            ("/mcp_receipt/status", json!("effect_uncertain")),
            ("/mcp_receipt/actor", json!("Other")),
            ("/mcp_receipt/policy_revision", json!(2)),
            ("/result/sha256", json!("forged")),
        ] {
            let mut changed = result.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(after_write(&intake, &scope, &policy, &request, &changed).is_none());
        }
        let mut revoked = policy.clone();
        revoked
            .grants
            .get_mut("Author")
            .unwrap()
            .tools
            .remove("files:read");
        assert!(after_write(&intake, &scope, &revoked, &request, &result).is_none());
        let mut different = intake.clone();
        different["required_rust_checks"][0]["actor"] = json!("Other");
        assert!(after_write(&different, &scope, &policy, &request, &result).is_none());
        let mut different = request.clone();
        different["arguments"]["content"] = json!("pub fn changed(){}");
        assert!(after_write(&intake, &scope, &policy, &different, &result).is_none());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn real_native_write_followups_reach_source_bound_acceptance() {
        use gents_local_mcp::authority::Authority;
        let mut native = crate::native_fixture::Fixture::new();
        native.scope.actor = "Author".into();
        native.scope.expected_revision = Some(2);
        let (mut intake, _, mut policy, request, _) = fixture();
        policy.revision = 2;
        let tests = "#[test]fn smoke(){candidate::f();}";
        intake["required_rust_checks"][0]["tests_path"] = json!("acceptance.rs");
        intake["required_rust_checks"][0]["tests_sha256"] =
            json!(gents_local_mcp::digest(tests.as_bytes()));
        let db = native.db();
        gents_local_mcp::governance::save(&db, &native.scope.run.to_string(), &policy, 1).unwrap();
        db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
            .unwrap();
        let root = native.authority.workspace(&native.scope).unwrap();
        gents_local_mcp::workspace::open(&root, true)
            .unwrap()
            .write("acceptance.rs", tests)
            .unwrap();
        let registry = gents_local_mcp::Registry::new();
        let write = registry
            .call(
                &native.authority,
                &native.scope,
                "files",
                request["arguments"].clone(),
            )
            .unwrap();
        let result = json!({"ok":true,"result":write["value"],"mcp_receipt":write["receipt"]});
        let plan = after_write(&intake, &native.scope, &policy, &request, &result).unwrap();
        assert_eq!(
            gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap()["passed"],
            false
        );
        for call in plan["calls"].as_array().unwrap() {
            registry
                .call(
                    &native.authority,
                    &native.scope,
                    call["action"].as_str().unwrap(),
                    call["arguments"].clone(),
                )
                .unwrap();
        }
        let gate = gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap();
        assert_eq!(gate["passed"], true);
        assert_eq!(
            gate["rust_checks"][0]["successful_current_source_readbacks"],
            1
        );
        let receipts: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(receipts, 3);
    }
}

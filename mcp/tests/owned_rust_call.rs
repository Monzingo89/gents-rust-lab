#![cfg(target_os = "macos")]
mod support;
use gents_local_mcp::{authority::Authority, completion, governance, Phase, Registry};
use serde_json::{json, Value};
use support::Fixture;
const SOURCE: &str = "pub fn protocol_value(x:u64)->u64{x}";
fn prepare(source: &str) -> Fixture {
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        for name in ["Worker", "Reader"] {
            access
                .get_mut(name)
                .unwrap()
                .tools
                .insert("owned_rust_call".into());
        }
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    let description = gents_local_mcp::component_api::describe(source).unwrap();
    let intake = json!({"required_mcp_calls":[{"actor":"Worker","tool":"owned_rust_call"}],"required_owned_rust_calls":[{"id":"protocol","actor":"Worker","source_path":"owned/source.rs","source_sha256":description["source_sha256"],"api_sha256":description["api_sha256"],"input_sha256":gents_local_mcp::digest(b"[\"18446744073709551615\"]"),"function":"protocol_value","owner_declared_review_package_id":"a".repeat(64)}]});
    completion::validate(&intake).unwrap();
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), true).unwrap();
    work.write("owned/source.rs", source).unwrap();
    f
}
fn request() -> Value {
    json!({"binding_id":"protocol","arguments":["18446744073709551615"]})
}
fn call(f: &Fixture) -> Value {
    Registry::new()
        .call(&f.authority, &f.scope, "owned_rust_call", request())
        .unwrap()
}
fn gate(f: &Fixture) -> Value {
    completion::check(&f.authority, &f.scope).unwrap()
}
#[test]
fn exact_owned_invocation_has_canonical_source_input_and_epoch_bound_evidence() {
    let f = prepare(SOURCE);
    assert_eq!(gate(&f)["passed"], false);
    let r = call(&f);
    assert_eq!(r["value"]["passed"], true, "{r}");
    assert_eq!(r["value"]["output"], "18446744073709551615");
    assert_eq!(
        r["receipt"]["result_sha256"],
        gents_local_mcp::digest(&serde_json::to_vec(&r["value"]).unwrap())
    );
    assert_eq!(r["receipt"]["provider_cost_usd"], 0);
    assert_eq!(r["value"]["promotion_authorized"], false);
    assert_eq!(
        gate(&f)["owned_rust_calls"][0]["successful_current_epoch_invocations"],
        1
    );
    assert_eq!(gate(&f)["passed"], true);
    f.db()
        .execute("DELETE FROM audit WHERE kind='mcp_tool_call'", [])
        .unwrap();
    assert_eq!(
        gate(&f)["passed"],
        false,
        "internal observation is not a canonical receipt"
    );
}
#[test]
fn actor_phase_input_epoch_grant_and_terminal_state_are_independent_denials() {
    let f = prepare(SOURCE);
    let registry = Registry::new();
    assert!(registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "owned_rust_call",
            request()
        )
        .is_err());
    let mut planner = f.scope.clone();
    planner.phase = Phase::Planner;
    assert!(registry
        .call(&f.authority, &planner, "owned_rust_call", request())
        .is_err());
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "owned_rust_call",
            json!({"binding_id":"protocol","arguments":["1"]})
        )
        .is_err());
    let mut forged = request();
    forged["source_path"] = json!("elsewhere.rs");
    assert!(registry
        .call(&f.authority, &f.scope, "owned_rust_call", forged)
        .is_err());
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let mut stale = f.scope.clone();
    stale.expected_revision = Some(policy.revision);
    let prior = policy.revision;
    policy.revision += 1;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("owned_rust_call");
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    assert!(registry
        .call(&f.authority, &stale, "owned_rust_call", request())
        .is_err());
    assert!(registry
        .call(&f.authority, &f.scope, "owned_rust_call", request())
        .is_err());
    let other = prepare(SOURCE);
    other
        .db()
        .execute("UPDATE jobs SET state='completed'", [])
        .unwrap();
    assert!(registry
        .call(&other.authority, &other.scope, "owned_rust_call", request())
        .is_err());
}
#[test]
fn frozen_source_is_not_agent_writable_and_changed_source_or_api_cannot_reuse_receipts() {
    let f = prepare(SOURCE);
    call(&f);
    assert_eq!(gate(&f)["passed"], true);
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"write","path":"owned/source.rs","content":"pub fn replacement(){}"})
        )
        .is_err());
    let root = f.authority.workspace(&f.scope).unwrap();
    let work = gents_local_mcp::workspace::open(&root, false).unwrap();
    work.write("owned/source.rs", "pub fn protocol_value(x:u64)->u64{x+1}")
        .unwrap();
    assert_eq!(gate(&f)["passed"], false);
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "owned_rust_call", request())
        .is_err());
    work.write("owned/source.rs", SOURCE).unwrap();
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    intake["required_owned_rust_calls"][0]["api_sha256"] = json!("b".repeat(64));
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    assert_eq!(gate(&f)["passed"], false);
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "owned_rust_call", request())
        .is_err());
}
#[test]
fn a_failed_runtime_receipt_cannot_satisfy_the_frozen_invocation() {
    let f = prepare("pub fn protocol_value(x:u64)->u64{let _=x;loop{}}");
    let r = call(&f);
    assert_eq!(r["value"]["passed"], false);
    assert_eq!(r["receipt"]["status"], "completed");
    let g = gate(&f);
    assert_eq!(g["checks"][0]["passed"], true);
    assert_eq!(g["owned_rust_calls"][0]["passed"], false);
    assert_eq!(g["passed"], false);
}
#[test]
fn revoked_epoch_never_borrows_the_previous_success() {
    let f = prepare(SOURCE);
    call(&f);
    assert_eq!(gate(&f)["passed"], true);
    let db = f.db();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let old = p.revision;
    p.revision += 1;
    governance::save(&db, &f.scope.run.to_string(), &p, old).unwrap();
    assert_eq!(gate(&f)["passed"], false);
    call(&f);
    assert_eq!(gate(&f)["passed"], true);
}
#[tokio::test]
async fn actual_stdio_and_sse_owned_calls_keep_process_scope_and_compiler_requirements() {
    let f = prepare(SOURCE);
    let root = f.root.join("target/debug");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        root.join(format!("gents-local-mcp{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
    let previous = std::env::var_os("GENTS_MCP_TRANSPORT");
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let r = gents_local_mcp::client::invoke(
            &f.root,
            &f.scope,
            "owned_rust_call",
            &request(),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .await
        .unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["result"]["passed"], true, "{r}");
        assert_eq!(r["result"]["output"], "18446744073709551615");
        assert_eq!(r["mcp_receipt"]["actor"], f.scope.actor);
        assert_eq!(r["mcp_receipt"]["provider_cost_usd"], 0);
    }
    if let Some(v) = previous {
        std::env::set_var("GENTS_MCP_TRANSPORT", v);
    } else {
        std::env::remove_var("GENTS_MCP_TRANSPORT");
    }
    assert_eq!(
        gate(&f)["owned_rust_calls"][0]["successful_current_epoch_invocations"],
        2
    );
    let r = Registry::new()
        .runtime_requirements("owned_rust_call")
        .unwrap();
    assert!(r.installed_rustc);
    assert_eq!(r.call_timeout_seconds, 110);
    assert!(Registry::new()
        .runtime_requirements("unregistered")
        .is_err());
}
#[test]
fn another_run_with_identical_source_and_input_cannot_borrow_a_receipt() {
    let f = prepare(SOURCE);
    call(&f);
    let db = f.db();
    let run = uuid::Uuid::new_v4();
    db.execute(
        "INSERT INTO jobs SELECT ?1,digest,approval,state,intake FROM jobs WHERE run_id=?2",
        rusqlite::params![run.to_string(), f.scope.run.to_string()],
    )
    .unwrap();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    p.revision = 1;
    governance::save(&db, &run.to_string(), &p, 0).unwrap();
    let mut scope = f.scope.clone();
    scope.run = run;
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&scope).unwrap(), true).unwrap();
    work.write("owned/source.rs", SOURCE).unwrap();
    assert_eq!(
        completion::check(&f.authority, &scope).unwrap()["passed"],
        false
    );
    Registry::new()
        .call(&f.authority, &scope, "owned_rust_call", request())
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &scope).unwrap()["passed"],
        true
    );
}
#[test]
fn approved_workflow_admits_only_exact_owner_bound_library_and_rechecks_it_for_the_writer() {
    let f = prepare(SOURCE);
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    intake["artifact_bindings"] = json!([{"source_actor":"Worker","writer":"Reader","source_tool":"owned_rust_call","arguments":request(),"path":"output.json","pointer":"/output","encoding":"json"}]);
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let old = p.revision;
    p.revision += 1;
    for access in [&mut p.ceilings, &mut p.grants] {
        for actor in ["Worker", "Reader"] {
            access
                .get_mut(actor)
                .unwrap()
                .contract_execution_phases
                .insert("worker".into());
        }
        access
            .get_mut("Reader")
            .unwrap()
            .tools
            .insert("files:write".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &p, old).unwrap();
    let mut source_scope = f.scope.clone();
    source_scope.expected_revision = Some(p.revision);
    let mut writer_scope = f.actor("Reader");
    writer_scope.expected_revision = Some(p.revision);
    let registry = Registry::new();
    let meta = registry.catalog();
    let tool = meta["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "owned_rust_call")
        .unwrap();
    assert_eq!(tool["deterministic"], false);
    assert_eq!(tool["file_input_fields"], json!([]));
    let admission = gents_local_mcp::contract_execution::authorize_call(
        &f.authority,
        &source_scope,
        "owned_rust_call",
        &request(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(admission["execution_established"], false);
    assert_eq!(
        db.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
    assert!(gents_local_mcp::contract_execution::authorize_call(
        &f.authority,
        &writer_scope,
        "owned_rust_call",
        &request()
    )
    .is_err());
    let mut changed = request();
    changed["arguments"] = json!(["1"]);
    assert!(gents_local_mcp::contract_execution::authorize_call(
        &f.authority,
        &source_scope,
        "owned_rust_call",
        &changed
    )
    .is_err());
    let result = registry
        .call(&f.authority, &source_scope, "owned_rust_call", request())
        .unwrap();
    assert_eq!(result["receipt"]["contract_enforced"], true);
    let write =
        json!({"op":"write","path":"output.json","content":result["value"]["output"].to_string()});
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), false).unwrap();
    work.write("owned/source.rs", "pub fn protocol_value(x:u64)->u64{x+1}")
        .unwrap();
    assert!(
        registry
            .call(&f.authority, &writer_scope, "files", write.clone())
            .is_err(),
        "a prior source result cannot authorize copying after its library changes"
    );
    work.write("owned/source.rs", SOURCE).unwrap();
    let written = registry
        .call(&f.authority, &writer_scope, "files", write)
        .unwrap();
    assert_eq!(written["receipt"]["contract_enforced"], true);
    let read = registry
        .call(
            &f.authority,
            &writer_scope,
            "files",
            json!({"op":"read","path":"output.json"}),
        )
        .unwrap();
    assert_eq!(read["value"]["text"], result["value"]["output"].to_string());
    assert_eq!(read["receipt"]["contract_enforced"], true);
}

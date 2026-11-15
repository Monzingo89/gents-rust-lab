#![cfg(target_os = "macos")]
mod support;
use gents_local_mcp::{completion, governance, Phase, Registry, Scope};
use serde_json::{json, Value};
use support::Fixture;
fn prepare(source: &str, tests: &str) -> Fixture {
    let f = Fixture::new();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    for grants in [&mut policy.ceilings, &mut policy.grants] {
        grants
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("rust_verify".into());
        grants
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("files:read".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    let intake = json!({"required_rust_checks":[{"actor":"Worker","source_path":"candidate.rs","tests_path":"acceptance.rs","tests_sha256":gents_local_mcp::digest(tests.as_bytes())}]});
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    let root = f.authority.workspace(&f.scope).unwrap();
    let work = gents_local_mcp::workspace::open(&root, true).unwrap();
    work.write("candidate.rs", source).unwrap();
    work.write("acceptance.rs", tests).unwrap();
    f
}
use gents_local_mcp::authority::Authority;
#[test]
fn verification_identity_tracks_source_acceptance_and_policy_epoch() {
    let f = prepare(
        "pub fn f()->u32{1}",
        "#[test]fn check(){std::assert_eq!(candidate::f(),1);}",
    );
    let identity = || {
        gents_local_mcp::rust_verification::request_identity(&f.authority, &f.scope, "candidate.rs")
            .unwrap()
    };
    let first = identity();
    assert_eq!(first, identity());
    let root = f.authority.workspace(&f.scope).unwrap();
    let work = gents_local_mcp::workspace::open(&root, false).unwrap();
    work.write("candidate.rs", "pub fn f()->u32{2}").unwrap();
    let changed_source = identity();
    assert_ne!(first, changed_source);
    work.write("acceptance.rs", "#[test]fn changed(){}")
        .unwrap();
    let changed_tests = identity();
    assert_ne!(changed_source, changed_tests);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    assert_ne!(changed_tests, identity());
    let mut other = f.scope.clone();
    other.actor = "other".into();
    assert!(gents_local_mcp::rust_verification::request_identity(
        &f.authority,
        &other,
        "candidate.rs"
    )
    .is_err());
}
fn call(f: &Fixture) -> Value {
    Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"candidate.rs"}),
        )
        .unwrap();
    Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "rust_verify",
            json!({"source_path":"candidate.rs"}),
        )
        .unwrap()["value"]
        .clone()
}
#[test]
fn failed_acceptance_context_is_frozen_source_and_requires_current_read_permission() {
    let f = prepare(
        "pub fn f()->u32{0}",
        "#[test]\nfn check(){\nstd::assert_eq!(candidate::f(),1);\n}\n",
    );
    let failed = call(&f);
    assert_eq!(failed["passed"], false);
    assert_eq!(failed["failure_context"]["line"], 3);
    assert!(failed["failure_context"]["excerpt"]
        .as_str()
        .unwrap()
        .contains("candidate::f()"));
    let gate = completion::check(&f.authority, &f.scope).unwrap();
    let latest = &gate["rust_checks"][0]["latest_check"];
    assert_eq!(latest["failure_context"], failed["failure_context"]);
    assert!(
        latest["diagnostics_excerpt"]
            .as_str()
            .unwrap()
            .chars()
            .count()
            + latest["failure_context"].to_string().chars().count()
            <= 900
    );
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("files:read");
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    let denied_context = Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "rust_verify",
            json!({"source_path":"candidate.rs"}),
        )
        .unwrap();
    assert!(denied_context["value"].get("failure_context").is_none());
}
#[test]
fn acceptance_alone_cannot_replace_full_current_byte_readback() {
    let f = prepare(
        "pub fn f()->u32{1}",
        "#[test]fn check(){std::assert_eq!(candidate::f(),1);}",
    );
    let result = Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "rust_verify",
            json!({"source_path":"candidate.rs"}),
        )
        .unwrap();
    assert_eq!(result["value"]["passed"], true);
    let before = completion::check(&f.authority, &f.scope).unwrap();
    assert_eq!(
        before["rust_checks"][0]["successful_current_source_checks"],
        1
    );
    assert_eq!(before["passed"], false);
    Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"candidate.rs","max_bytes":1}),
        )
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"candidate.rs"}),
        )
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
    f.db()
        .execute(
            "DELETE FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='files'",
            [],
        )
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
}
#[test]
fn real_compilation_and_frozen_tests_are_bound_to_current_source_and_epoch() {
    let f = prepare(
        "pub fn length(s:&str)->usize{s.chars().count()}",
        "#[test]fn unicode(){std::assert_eq!(candidate::length(\"α🧪\"),2);}",
    );
    let result = call(&f);
    assert_eq!(result["passed"], true, "{result}");
    assert_eq!(result["tests_count"], 1);
    assert_eq!(result["promotion_authorized"], false);
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
    let root = f.authority.workspace(&f.scope).unwrap();
    gents_local_mcp::workspace::open(&root, false)
        .unwrap()
        .write("candidate.rs", "pub fn length(s:&str)->usize{s.len()}")
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    let failed = call(&f);
    assert_eq!(failed["passed"], false, "{failed}");
    assert_eq!(failed["stage"], "acceptance");
    let progress = completion::check(&f.authority, &f.scope).unwrap()["rust_checks"][0]
        ["execution_progress"]
        .clone();
    assert_eq!(progress["best_stage"], 4);
    assert_eq!(progress["current_stage"], 3);
    assert_eq!(progress["regressed"], true);
    assert_eq!(progress["distinct_sources"], 2);
    // Rows without their canonical receipt cannot influence recovery context.
    f.db().execute("DELETE FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='rust_verify'", []).unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["rust_checks"][0]["execution_progress"]
            ["distinct_sources"],
        0
    );
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    let mut planner = f.scope.clone();
    planner.phase = Phase::Planner;
    assert!(Registry::new()
        .call(
            &f.authority,
            &planner,
            "rust_verify",
            json!({"source_path":"candidate.rs"})
        )
        .is_err());
}
#[test]
fn forbidden_source_and_changed_tests_never_qualify() {
    let f = prepare(
        "pub fn f(){std::process::exit(0)}",
        "#[test]fn check(){candidate::f();}",
    );
    let result = call(&f);
    assert_eq!(result["passed"], false);
    assert_eq!(result["stage"], "admission");
    let evidence = completion::check(&f.authority, &f.scope).unwrap();
    assert_eq!(evidence["rust_checks"][0]["latest_check"]["passed"], false);
    assert_eq!(
        evidence["rust_checks"][0]["latest_check"]["stage"],
        "admission"
    );
    assert_eq!(
        evidence["rust_checks"][0]["latest_check"]["canonical_receipt_verified"],
        true
    );
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    let root = f.authority.workspace(&f.scope).unwrap();
    gents_local_mcp::workspace::open(&root, false)
        .unwrap()
        .write("acceptance.rs", "#[test]fn changed(){}")
        .unwrap();
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "rust_verify",
            json!({"source_path":"candidate.rs"})
        )
        .is_err());
    let other = Scope {
        actor: "Reader".into(),
        ..f.scope.clone()
    };
    assert!(Registry::new()
        .call(
            &f.authority,
            &other,
            "rust_verify",
            json!({"source_path":"candidate.rs"})
        )
        .is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn actual_stdio_and_sse_children_receive_only_explicit_compiler_configuration() {
    let f = prepare(
        "pub fn twice(x:u32)->u32{x*2}",
        "#[test]fn check(){std::assert_eq!(candidate::twice(3),6);}",
    );
    let target = f.root.join("target/debug");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        target.join("gents-local-mcp"),
    )
    .unwrap();
    let previous = std::env::var_os("GENTS_MCP_TRANSPORT");
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let result = gents_local_mcp::client::invoke(
            &f.root,
            &f.scope,
            "rust_verify",
            &json!({"source_path":"candidate.rs"}),
            &std::sync::atomic::AtomicBool::new(false),
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true, "{transport}: {result}");
        assert_eq!(result["result"]["passed"], true, "{transport}: {result}");
    }
    if let Some(previous) = previous {
        std::env::set_var("GENTS_MCP_TRANSPORT", previous);
    } else {
        std::env::remove_var("GENTS_MCP_TRANSPORT");
    }
}

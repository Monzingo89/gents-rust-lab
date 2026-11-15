mod support;
use gents_local_mcp::{
    artifact_bindings, authority::Authority, contract_execution, digest, governance, Registry,
};
use serde_json::{json, Value};
use support::{audio::wav, Fixture};

fn fixture() -> (Fixture, Value) {
    let mut f = Fixture::new();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 2;
    for map in [&mut policy.ceilings, &mut policy.grants] {
        let source = map.get_mut("Worker").unwrap();
        source.tools.insert("audio_inspect".into());
        source.contract_execution_phases.insert("worker".into());
        let writer = map.get_mut("Reader").unwrap();
        writer
            .tools
            .extend(["files:read".into(), "files:write".into()]);
        writer.contract_execution_phases.insert("worker".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    let root = f.authority.workspace(&f.scope).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let bytes = wav(1, 8000, &[-1, 0, 1]);
    std::fs::write(root.join("clip.wav"), &bytes).unwrap();
    let args = json!({"path":"clip.wav","expected_sha256":digest(&bytes)});
    let intake = json!({"ceo_source_inputs":[{"id":"audio","tool":"audio_inspect","arguments":args}],"artifact_bindings":[{"path":"frames.txt","source_actor":"Worker","writer":"Reader","source_tool":"audio_inspect","arguments":args,"pointer":"/frame_count","encoding":"integer_text"}]});
    let body = intake.to_string();
    let sha = digest(body.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            body,
            sha,
            json!({"owner":f.scope.owner,"sha256":sha,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    f.scope.expected_revision = Some(2);
    (f, args)
}
fn successes(f: &Fixture) -> i64 {
    f.db()
        .query_row(
            "SELECT count(*)FROM audit WHERE kind='mcp_tool_call'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn exact_owner_calls_execute_without_weakening_native_scope_or_artifact_checks() {
    let (f, args) = fixture();
    let registry = Registry::new();
    let context = contract_execution::scope_authority(&f.authority, &f.scope)
        .unwrap()
        .unwrap();
    assert_eq!(context["mode"], "approved_workflow");
    assert!(
        contract_execution::authorize_call(&f.authority, &f.scope, "audio_inspect", &args)
            .unwrap()
            .is_some()
    );
    assert_eq!(successes(&f), 0);
    let source = registry
        .call(&f.authority, &f.scope, "audio_inspect", args.clone())
        .unwrap();
    assert_eq!(source["receipt"]["contract_enforced"], true);
    let writer = f.actor("Reader");
    let root = f.authority.workspace(&f.scope).unwrap();
    for bad in [
        json!({"op":"write","path":"frames.txt","content":"wrong"}),
        json!({"op":"write","path":"frames.txt","content":"3","unexpected":true}),
        json!({"op":"write","path":"other.txt","content":"3"}),
        json!({"op":"read","path":"clip.wav"}),
        json!({"op":"read","path":"frames.txt","limit_bytes":1}),
    ] {
        assert!(registry.call(&f.authority, &writer, "files", bad).is_err());
    }
    assert!(!root.join("frames.txt").exists());
    assert!(!root.join("other.txt").exists());
    assert_eq!(successes(&f), 1);
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "integer_arithmetic",
            json!({"calculations":[{"id":"not-approved","values":[1,2],"subtract":0}]})
        )
        .is_err());
    std::fs::write(root.join("frames.txt"), "external owner file").unwrap();
    let collision = artifact_bindings::next_call(&f.authority, &writer, true)
        .unwrap()
        .unwrap();
    assert!(registry
        .call(
            &f.authority,
            &writer,
            "files",
            collision["arguments"].clone()
        )
        .is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("frames.txt")).unwrap(),
        "external owner file"
    );
    std::fs::remove_file(root.join("frames.txt")).unwrap();
    for _ in 0..2 {
        let next = artifact_bindings::next_call(&f.authority, &writer, true)
            .unwrap()
            .unwrap();
        if next["arguments"]["op"] == "write" {
            assert!(next["arguments"].get("expected_sha256").is_none());
        }
        assert!(contract_execution::authorize_call(
            &f.authority,
            &writer,
            "files",
            &next["arguments"]
        )
        .unwrap()
        .is_some());
        let reply = registry
            .call(&f.authority, &writer, "files", next["arguments"].clone())
            .unwrap();
        assert_eq!(reply["receipt"]["contract_enforced"], true);
    }
    assert_eq!(
        std::fs::read_to_string(root.join("frames.txt")).unwrap(),
        "3"
    );
    assert_eq!(successes(&f), 3);
    assert_eq!(
        artifact_bindings::check(&f.authority, &writer, true).unwrap()["passed"],
        true
    );
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 3;
    policy
        .grants
        .get_mut("Reader")
        .unwrap()
        .tools
        .remove("files:write");
    governance::save(&db, &f.scope.run.to_string(), &policy, 2).unwrap();
    let request =
        json!({"op":"write","path":"frames.txt","content":"3","expected_sha256":digest(b"3")});
    assert!(registry
        .call(&f.authority, &writer, "files", request.clone())
        .is_err());
    let mut current = writer;
    current.expected_revision = Some(3);
    assert!(registry
        .call(&f.authority, &current, "files", request)
        .is_err());
    assert_eq!(successes(&f), 3);
}

#[test]
fn delegation_preserves_owner_mode_and_cannot_enable_it_or_escape_the_ceiling() {
    let (f, _) = fixture();
    let mut policy = governance::load(&f.db(), &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    for map in [&mut policy.ceilings, &mut policy.grants] {
        map.get_mut("Worker")
            .unwrap()
            .tools
            .insert("access_delegate".into());
    }
    let empty = serde_json::from_value(json!({"tools":[],"skills":[],"memory_from":[]})).unwrap();
    policy.delegate("Worker", "Reader", empty, 2).unwrap();
    assert!(policy.grants["Reader"]
        .contract_execution_phases
        .contains("worker"));
    assert!(policy.grants["Reader"].tools.is_empty());
    policy
        .grants
        .get_mut("Reader")
        .unwrap()
        .contract_execution_phases
        .clear();
    let before = serde_json::to_value(&policy).unwrap();
    let escalation = serde_json::from_value(
        json!({"tools":[],"skills":[],"memory_from":[],"contract_execution_phases":["worker"]}),
    )
    .unwrap();
    assert!(policy.delegate("Worker", "Reader", escalation, 3).is_err());
    assert_eq!(serde_json::to_value(&policy).unwrap(), before);
    policy
        .ceilings
        .get_mut("Reader")
        .unwrap()
        .contract_execution_phases
        .insert("planner".into());
    assert!(policy.validate().is_err());
    let legacy: governance::Access =
        serde_json::from_value(json!({"tools":[],"skills":[],"memory_from":[]})).unwrap();
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("contract_execution_phases")
        .is_none());
}

#[test]
fn nondeterministic_file_sources_require_a_complete_hash_pin() {
    let (f, _) = fixture();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    for map in [&mut policy.ceilings, &mut policy.grants] {
        map.get_mut("Worker")
            .unwrap()
            .tools
            .insert("table_read_json".into());
    }
    // Private fixture setup only: retain the same epoch while selecting a
    // different approved source whose optional digest was omitted.
    db.execute(
        "UPDATE run_governance SET manifest=?1",
        [serde_json::to_string(&policy).unwrap()],
    )
    .unwrap();
    let args = json!({"path":"input.json"});
    let intake = json!({"artifact_bindings":[{"path":"rows.json","source_actor":"Worker","writer":"Reader","source_tool":"table_read_json","arguments":args,"pointer":"/rows","encoding":"json"}]});
    let body = intake.to_string();
    let sha = digest(body.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            body,
            sha,
            json!({"owner":f.scope.owner,"sha256":sha,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    std::fs::write(
        f.authority.workspace(&f.scope).unwrap().join("input.json"),
        "[]",
    )
    .unwrap();
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "table_read_json", args)
        .is_err());
    assert_eq!(successes(&f), 0);
}

#[tokio::test]
async fn real_stdio_and_sse_enforce_exact_contracts_and_return_native_enforcement_receipts() {
    let old = std::env::var_os("GENTS_MCP_TRANSPORT");
    for transport in ["stdio", "sse"] {
        let (f, args) = fixture();
        let bin = f.root.join("target/debug");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::copy(
            env!("CARGO_BIN_EXE_gents-local-mcp"),
            bin.join(format!("gents-local-mcp{}", std::env::consts::EXE_SUFFIX)),
        )
        .unwrap();
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut wrong = args.clone();
        wrong["path"] = json!("different.wav");
        let rejected =
            gents_local_mcp::client::invoke(&f.root, &f.scope, "audio_inspect", &wrong, &cancel)
                .await
                .unwrap();
        assert_eq!(rejected["ok"], false);
        let source =
            gents_local_mcp::client::invoke(&f.root, &f.scope, "audio_inspect", &args, &cancel)
                .await
                .unwrap();
        assert_eq!(source["mcp_receipt"]["contract_enforced"], true);
        let writer = f.actor("Reader");
        let unbound = gents_local_mcp::client::invoke(
            &f.root,
            &writer,
            "files",
            &json!({"op":"write","path":"unbound.txt","content":"not approved"}),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(unbound["ok"], false);
        for _ in 0..2 {
            let next = artifact_bindings::next_call(&f.authority, &writer, true)
                .unwrap()
                .unwrap();
            let result = gents_local_mcp::client::invoke(
                &f.root,
                &writer,
                "files",
                &next["arguments"],
                &cancel,
            )
            .await
            .unwrap();
            assert_eq!(result["ok"], true);
            assert_eq!(result["mcp_receipt"]["contract_enforced"], true);
        }
        assert_eq!(successes(&f), 3);
        assert!(!f
            .authority
            .workspace(&f.scope)
            .unwrap()
            .join("unbound.txt")
            .exists());
    }
    if let Some(old) = old {
        std::env::set_var("GENTS_MCP_TRANSPORT", old);
    } else {
        std::env::remove_var("GENTS_MCP_TRANSPORT");
    }
}

#[test]
fn owner_digest_epoch_active_state_and_missing_source_are_native_preconditions() {
    let (f, args) = fixture();
    let registry = Registry::new();
    let writer = f.actor("Reader");
    assert!(registry
        .call(
            &f.authority,
            &writer,
            "files",
            json!({"op":"write","path":"frames.txt","content":"3"})
        )
        .is_err());
    let mut stale = f.scope.clone();
    stale.expected_revision = None;
    assert!(registry
        .call(&f.authority, &stale, "audio_inspect", args.clone())
        .is_err());
    stale.expected_revision = Some(1);
    assert!(registry
        .call(&f.authority, &stale, "audio_inspect", args.clone())
        .is_err());
    let db = f.db();
    let original: String = db
        .query_row("SELECT intake FROM jobs", [], |r| r.get(0))
        .unwrap();
    db.execute("UPDATE jobs SET intake='{}'", []).unwrap();
    assert!(registry
        .call(&f.authority, &f.scope, "audio_inspect", args.clone())
        .is_err());
    db.execute("UPDATE jobs SET intake=?1", [original]).unwrap();
    db.execute("UPDATE jobs SET state='completed'", []).unwrap();
    assert!(registry
        .call(&f.authority, &f.scope, "audio_inspect", args)
        .is_err());
    assert_eq!(successes(&f), 0);
}

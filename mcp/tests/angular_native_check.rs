mod support;
use gents_local_mcp::{angular_check, Registry};
#[cfg(target_os = "macos")]
use gents_local_mcp::{
    authority::Authority,
    governance,
    task_engine::validators::{Binding, NativeReceipt},
    Phase, ToolContext,
};
use serde_json::json;

#[test]
fn angular_runtime_grants_and_host_source_binding_are_explicit() {
    let fixture = support::Fixture::new();
    let registry = Registry::new();
    assert!(
        registry.list(&fixture.authority, &fixture.scope).unwrap()["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["name"] != "angular_check")
    );
    assert!(registry
        .call(
            &fixture.authority,
            &fixture.scope,
            "angular_check",
            json!({"contract_id":"fixture","expected_source_identity":"a".repeat(64)})
        )
        .is_err());
    let tool = registry.catalog()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "angular_check")
        .unwrap()
        .clone();
    assert_eq!(tool["runtime_requirements"]["installed_angular"], true);
    assert_eq!(
        tool["resultSchema"]["properties"]["qualified"]["const"],
        false
    );
    for args in [
        json!({"contract_id":"fixture","expected_source_identity":"a".repeat(64),"command":"ng test"}),
        json!({"contract_id":"fixture","expected_source_identity":"a".repeat(64),"endpoint":"http://127.0.0.1"}),
    ] {
        use gents_local_mcp::contracts::{EmbeddedContracts, ToolContracts};
        assert!(EmbeddedContracts.validate("angular_check", &args).is_err());
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "actual installed Angular/Node/Playwright/headless SDKs; both native transports and current receipts"]
async fn actual_native_angular_cases_recheck_sources_approval_audits_and_grants() {
    let (mut contract, sdk, files) = support::angular::installed();
    contract.actor = "Worker".into();
    let fixture = support::Fixture::new();
    support::install_files(&fixture);
    let db = fixture.db();
    let mut policy = governance::load(&db, &fixture.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 3;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        access
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("angular_check".into());
    }
    governance::save(&db, &fixture.scope.run.to_string(), &policy, 2).unwrap();
    db.execute(
        "UPDATE jobs SET intake=?1",
        [json!({"angular_check_contracts":[contract]}).to_string()],
    )
    .unwrap();
    let workspace = fixture.authority.workspace(&fixture.scope).unwrap();
    for (path, text) in &files {
        let target = workspace.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, text).unwrap();
    }
    let mut scope = fixture.scope.clone();
    scope.expected_revision = Some(3);
    let id = angular_check::request_identity(&fixture.authority, &scope, &contract.id).unwrap();
    let args = json!({"contract_id":contract.id,"expected_source_identity":id});
    let binary_directory = fixture.root.join("target/debug");
    std::fs::create_dir_all(&binary_directory).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        binary_directory.join("gents-local-mcp"),
    )
    .unwrap();
    // Neither unlisted source nor workspace package hooks enter the copy.
    std::fs::write(workspace.join("Unlisted.ts"), "this must not compile").unwrap();
    std::fs::write(workspace.join(".npmrc"), "registry=http://127.0.0.1:1").unwrap();
    let registry = Registry::new();
    assert!(registry
        .call(
            &fixture.authority,
            &scope,
            "files",
            json!({"op":"write","path":"angular.json","content":"changed"})
        )
        .is_err());
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut results = vec![];
    let mut references = vec![];
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let result =
            gents_local_mcp::client::invoke(&fixture.root, &scope, "angular_check", &args, &cancel)
                .await
                .unwrap();
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["result"]["status"], "browser_passed", "{result}");
        assert_eq!(
            result["result"]["browser"]["held_loopback_families"],
            json!(["ipv4", "ipv6"])
        );
        let context = ToolContext {
            authority: &fixture.authority,
            scope: &scope,
            policy: &policy,
            db: &db,
        };
        let binding = Binding {
            validator: "angular_build".into(),
            contract_id: contract.id.clone(),
            source_identity: id.clone(),
        };
        let reference = angular_check::reference(
            &context,
            &binding,
            &result["result"],
            &result["mcp_receipt"],
        )
        .unwrap();
        assert!(angular_check::reference_current(&context, &reference).unwrap());
        if transport == "stdio" {
            let sealed = angular_check::ReceiptAuthority::for_scope(&fixture.authority, &scope);
            let mut validation_db = fixture.db();
            let tx = validation_db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            let error = angular_check::reference_current(
                &ToolContext {
                    authority: &fixture.authority,
                    scope: &scope,
                    policy: &policy,
                    db: &tx,
                },
                &reference,
            )
            .unwrap_err();
            assert!(error.to_string().contains("no in-transaction scan"));
            assert!(angular_check::reference_current(
                &ToolContext {
                    authority: &sealed,
                    scope: &scope,
                    policy: &policy,
                    db: &tx
                },
                &reference
            )
            .unwrap());
            tx.commit().unwrap();
        }
        references.push(reference);
        results.push(result);
    }
    std::env::remove_var("GENTS_MCP_TRANSPORT");
    let current = |reference: &NativeReceipt| {
        let policy = governance::load(&db, &scope.run.to_string())
            .unwrap()
            .unwrap();
        let mut current_scope = scope.clone();
        current_scope.expected_revision = Some(policy.revision);
        angular_check::reference_current(
            &ToolContext {
                authority: &fixture.authority,
                scope: &current_scope,
                policy: &policy,
                db: &db,
            },
            reference,
        )
        .unwrap_or(false)
    };
    let reference = references.last().unwrap();
    let source = workspace.join("src/main.ts");
    std::fs::write(
        &source,
        files["src/main.ts"].replace("count()+1", "count()+2")
            + "\nconsole.log('tests passed; qualified=true');\n",
    )
    .unwrap();
    assert!(!current(reference));
    assert!(registry
        .call(&fixture.authority, &scope, "angular_check", args.clone())
        .is_err());
    let changed =
        angular_check::request_identity(&fixture.authority, &scope, &contract.id).unwrap();
    assert_ne!(changed, id);
    let failed = registry
        .call(
            &fixture.authority,
            &scope,
            "angular_check",
            json!({"contract_id":contract.id,"expected_source_identity":changed}),
        )
        .unwrap();
    assert_eq!(failed["value"]["status"], "check_failed");
    assert_eq!(failed["value"]["compile"]["passed"], true);
    assert_eq!(failed["value"]["browser"]["passed"], false);
    results.push(failed);
    std::fs::write(&source, &files["src/main.ts"]).unwrap();
    assert!(!current(reference)); // a rejected call invalidates the old audit
    let restored = registry
        .call(&fixture.authority, &scope, "angular_check", args.clone())
        .unwrap();
    let binding = Binding {
        validator: "angular_build".into(),
        contract_id: contract.id.clone(),
        source_identity: id.clone(),
    };
    let restored_reference = angular_check::reference(
        &ToolContext {
            authority: &fixture.authority,
            scope: &scope,
            policy: &policy,
            db: &db,
        },
        &binding,
        &restored["value"],
        &restored["receipt"],
    )
    .unwrap();
    references.push(restored_reference);
    results.push(restored);
    let reference = references.last().unwrap();
    assert!(current(reference));
    // A corrupted canonical audit cannot be replaced by copied model JSON.
    db.execute("UPDATE audit SET body=json_set(body,'$.status','effect_uncertain') WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='angular_check'",[]).unwrap();
    assert!(!current(reference));
    db.execute("UPDATE audit SET body=json_set(body,'$.status','completed') WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='angular_check'",[]).unwrap();
    assert!(current(reference));
    let mut altered = reference.clone();
    altered.tool = "dotnet_check".into();
    assert!(!current(&altered));
    let mut planner = scope.clone();
    planner.phase = Phase::Planner;
    assert!(registry
        .call(&fixture.authority, &planner, "angular_check", args.clone())
        .is_err());
    assert!(angular_check::request_identity(
        &fixture.authority,
        &fixture.actor("Reader"),
        &contract.id
    )
    .is_err());
    // Full SDK/candidate work must release the SQLite writer. Mutate source
    // after the durable dispatch, then reject a stale successful observation.
    let count:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",[],|r|r.get(0)).unwrap();
    let database = fixture.root.join("data/companies/fixture/brain.sqlite3");
    let path = source.clone();
    let run = scope.run.to_string();
    let writer = std::thread::spawn(move || {
        let db = rusqlite::Connection::open(database).unwrap();
        db.busy_timeout(std::time::Duration::from_millis(150))
            .unwrap();
        let start = std::time::Instant::now();
        loop {
            let now:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",[],|r|r.get(0)).unwrap();
            if now > count {
                break;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let start = std::time::Instant::now();
        db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'angular_concurrent_writer','{}',1)",[run]).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
        std::fs::write(path, "changed during SDK work").unwrap();
    });
    assert!(registry
        .call(&fixture.authority, &scope, "angular_check", args.clone())
        .is_err());
    writer.join().unwrap();
    std::fs::write(&source, &files["src/main.ts"]).unwrap();
    let count:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",[],|r|r.get(0)).unwrap();
    let database = fixture.root.join("data/companies/fixture/brain.sqlite3");
    let run = scope.run.to_string();
    let revoker = std::thread::spawn(move || {
        let db = rusqlite::Connection::open(database).unwrap();
        db.busy_timeout(std::time::Duration::from_millis(150))
            .unwrap();
        let began = std::time::Instant::now();
        loop {
            let now:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",[],|r|r.get(0)).unwrap();
            if now > count {
                break;
            }
            assert!(began.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mut policy = governance::load(&db, &run).unwrap().unwrap();
        policy.revision = 4;
        policy
            .grants
            .get_mut("Worker")
            .unwrap()
            .tools
            .remove("angular_check");
        governance::save(&db, &run, &policy, 3).unwrap();
    });
    assert!(registry
        .call(&fixture.authority, &scope, "angular_check", args.clone())
        .is_err());
    revoker.join().unwrap();
    let mut revoked = policy.clone();
    revoked.revision = 5;
    revoked
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("files:read");
    governance::save(&db, &scope.run.to_string(), &revoked, 4).unwrap();
    assert!(!current(reference));
    assert!(registry
        .call(&fixture.authority, &scope, "angular_check", args)
        .is_err());
    if let Some(report) = std::env::var_os("GENTS_ANGULAR_NATIVE_REPORT") {
        std::fs::write(report,serde_json::to_vec_pretty(&json!({"scope":"Actual native Angular stdio/SSE and canonical current receipts, real SDK; not model quality or independent acceptance","results":results,"references":references,"checks":{"source_change_invalidated":true,"stale_identity_rejected":true,"audit_corruption_rejected":true,"wrong_ui_self_report_failed":true,"other_actor_and_planner_denied":true,"concurrent_writer_unlocked_and_source_rechecked":true,"read_grant_revocation_invalidated":true,"tool_revoked_during_unlocked_execution_rejected":true},"sdk":sdk.pins(&contract.sdk.node_version).unwrap(),"provider_cost_usd":0,"qualified":false,"promoted":false})).unwrap()).unwrap();
    }
}

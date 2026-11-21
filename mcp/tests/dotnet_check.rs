mod support;
#[cfg(target_os = "macos")]
use gents_local_mcp::{authority::Authority, governance, Phase};
use gents_local_mcp::{dotnet_check, Registry};
use serde_json::{json, Value};
use support::Fixture;

const PROJECT: &str = "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><UseAppHost>false</UseAppHost><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>";
fn intake() -> Value {
    json!({"dotnet_check_contracts":[{"id":"api-build","actor":"Worker","project_path":"api/Api.csproj","source_paths":["api/Program.cs"],"sdk_version":"10.0.401","dotnet_sha256":"a".repeat(64),"artifact_mode":"managed_dll","frozen_inputs":{"api/Api.csproj":gents_local_mcp::digest(PROJECT.as_bytes())}}]})
}
#[test]
fn contract_cannot_grant_access_or_weaken_project_identity() {
    assert!(dotnet_check::validate(&json!({})).is_ok());
    assert!(dotnet_check::validate(&intake()).is_ok());
    for (field, value) in [
        ("id", json!("../bad")),
        ("actor", json!("")),
        ("project_path", json!("../Api.csproj")),
        ("source_paths", json!(["outside.cs"])),
        ("source_paths", json!(["api/Program.cs", "api/Program.cs"])),
        ("source_paths", json!(["api/bin/Evil.cs"])),
        ("source_paths", json!(["api/.hidden/Evil.cs"])),
        ("source_paths", json!(["api/custom.targets"])),
        ("source_paths", json!([])),
        ("sdk_version", json!("10.0.*")),
        ("sdk_version", json!("../../sdk")),
        ("dotnet_sha256", json!("missing")),
        ("artifact_mode", json!("native_exe")),
        ("frozen_inputs", json!({})),
        ("frozen_inputs", json!({"other.cs":"a".repeat(64)})),
        ("network_allowed", json!(true)),
    ] {
        let mut value_contract = intake();
        value_contract["dotnet_check_contracts"][0][field] = value;
        assert!(dotnet_check::validate(&value_contract).is_err(), "{field}");
    }
    let f = Fixture::new();
    let registry = Registry::new();
    assert!(registry.list(&f.authority, &f.scope).unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["name"] != "dotnet_check"));
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "dotnet_check",
            json!({"contract_id":"api-build"})
        )
        .is_err());
    assert!(gents_local_mcp::frozen_input_path(
        &intake(),
        "api/Api.csproj"
    ));
    assert!(!gents_local_mcp::frozen_input_path(
        &intake(),
        "api/Program.cs"
    ));
    let descriptor = registry.catalog()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "dotnet_check")
        .unwrap()
        .clone();
    assert_eq!(descriptor["runtime_requirements"]["installed_dotnet"], true);
    assert!(descriptor["resultSchema"]["properties"]
        .get("artifact_sha256")
        .is_some());
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "bounded live check: requires owner-configured GENTS_DOTNET and installed SDK 10.0.401"]
async fn installed_sdk_builds_and_fails_exact_snapshot_over_both_mcp_transports() {
    let executable = dotnet_check::executable_path().unwrap();
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 3;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        access
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("dotnet_check".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, 2).unwrap();
    let mut contract = intake();
    contract["dotnet_check_contracts"][0]["dotnet_sha256"] = json!(gents_local_mcp::digest(
        &std::fs::read(&executable).unwrap()
    ));
    db.execute("UPDATE jobs SET intake=?1", [contract.to_string()])
        .unwrap();
    let workspace = f.authority.workspace(&f.scope).unwrap();
    std::fs::create_dir_all(workspace.join("api")).unwrap();
    std::fs::write(workspace.join("api/Api.csproj"), PROJECT).unwrap();
    let registry = Registry::new();
    let mut scope = f.scope.clone();
    scope.expected_revision = Some(3);
    let write = |text: &str| {
        registry
            .call(
                &f.authority,
                &scope,
                "files",
                json!({"op":"write","path":"api/Program.cs","content":text}),
            )
            .unwrap()
    };
    write("Console.WriteLine(UnknownSymbol);");
    let old = dotnet_check::request_identity(&f.authority, &scope, "api-build").unwrap();
    let args = |id: &str| json!({"contract_id":"api-build","expected_source_identity":id});
    assert_eq!(
        dotnet_check::source_read_identity(&f.authority, &scope, "api/Program.cs").unwrap(),
        old
    );
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "dotnet_check",
            json!({"contract_id":"api-build"})
        )
        .is_err());
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"api/Api.csproj","content":"changed"})
        )
        .is_err());
    let failed = registry
        .call(&f.authority, &scope, "dotnet_check", args(&old))
        .unwrap();
    assert_eq!(failed["value"]["status"], "build_failed", "{failed}");
    assert_eq!(failed["value"]["phases"][2]["phase"], "build");
    assert!(failed["value"]["phases"][2]["stdout"]
        .as_str()
        .unwrap()
        .contains("CS0103"));
    assert_eq!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_build"]
            ["status"],
        "build_failed"
    );
    write("Console.WriteLine(\"bounded managed build\");");
    let fresh = dotnet_check::request_identity(&f.authority, &scope, "api-build").unwrap();
    assert_ne!(old, fresh);
    assert!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_build"]
            .is_null()
    );
    assert!(registry
        .call(&f.authority, &scope, "dotnet_check", args(&old))
        .is_err());
    assert!(dotnet_check::request_identity(&f.authority, &f.actor("Reader"), "api-build").is_err());
    let mut planner = scope.clone();
    planner.phase = Phase::Planner;
    assert!(registry
        .call(&f.authority, &planner, "dotnet_check", args(&fresh))
        .is_err());
    let binary_directory = f.root.join("target/debug");
    std::fs::create_dir_all(&binary_directory).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        binary_directory.join("gents-local-mcp"),
    )
    .unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let mut receipts = vec![failed];
    // Sibling code/custom targets would fail or execute a custom action if
    // this accidentally compiled the original workspace instead of its copy.
    std::fs::write(
        workspace.join("api/Unlisted.cs"),
        "this would fail compilation",
    )
    .unwrap();
    std::fs::write(
        workspace.join("api/Directory.Build.targets"),
        "<Project><Target Name=\"Build\"><Exec Command=\"touch forbidden\"/></Target></Project>",
    )
    .unwrap();
    assert_eq!(
        dotnet_check::request_identity(&f.authority, &scope, "api-build").unwrap(),
        fresh
    );
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let result = gents_local_mcp::client::invoke(
            &f.root,
            &scope,
            "dotnet_check",
            &args(&fresh),
            &cancelled,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["result"]["status"], "build_passed", "{result}");
        assert_eq!(result["result"]["snapshot_unchanged"], true);
        assert_eq!(result["result"]["qualified"], false);
        assert_eq!(result["result"]["promoted"], false);
        assert_eq!(result["result"]["model_calls"], 0);
        assert_eq!(result["mcp_receipt"]["policy_revision"], 3);
        assert_eq!(
            result["result"]["artifact_sha256"].as_str().unwrap().len(),
            64
        );
        receipts.push(result);
    }
    std::env::remove_var("GENTS_MCP_TRANSPORT");
    assert_eq!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_build"]
            ["canonical_receipt_verified"],
        true
    );
    // A real SDK run must not hold SQLite's writer. Change source only after
    // its durable dispatch reservation, then prove the old snapshot cannot
    // commit a passing receipt against the new workspace bytes.
    let dispatches:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='dotnet_check'",[],|r|r.get(0)).unwrap();
    let database = f.root.join("data/companies/fixture/brain.sqlite3");
    let candidate = workspace.join("api/Program.cs");
    let run = scope.run.to_string();
    let writer = std::thread::spawn(move || {
        let db = rusqlite::Connection::open(database).unwrap();
        db.busy_timeout(std::time::Duration::from_millis(150))
            .unwrap();
        let start = std::time::Instant::now();
        loop {
            let count:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='dotnet_check'",[],|r|r.get(0)).unwrap();
            if count > dispatches {
                break;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let start = std::time::Instant::now();
        db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'dotnet_concurrent_writer','{}',1)",[run]).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
        std::fs::write(candidate, "Console.WriteLine(ChangedDuringBuild);").unwrap();
    });
    assert!(registry
        .call(&f.authority, &scope, "dotnet_check", args(&fresh))
        .is_err());
    writer.join().unwrap();
    let committed:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='dotnet_check'",[],|r|r.get(0)).unwrap();
    assert_eq!(committed, 3);
    assert!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_build"]
            .is_null()
    );
    write("Console.WriteLine(\"bounded managed build\");");
    db.execute("DELETE FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='dotnet_check'",[]).unwrap();
    assert!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_build"]
            .is_null()
    );
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 4;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("files:read");
    governance::save(&db, &f.scope.run.to_string(), &policy, 3).unwrap();
    assert!(dotnet_check::request_identity(&f.authority, &scope, "api-build").is_err());
    assert!(registry
        .call(&f.authority, &scope, "dotnet_check", args(&fresh))
        .is_err());
    scope.expected_revision = Some(4);
    assert!(
        dotnet_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_build"]
            .is_null()
    );
    // Current grants are rechecked after unlocked SDK execution, rather than
    // fencing an owner revocation behind a long compile transaction.
    let mut policy = governance::load(&db, &scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 5;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .insert("files:read".into());
    governance::save(&db, &scope.run.to_string(), &policy, 4).unwrap();
    scope.expected_revision = Some(5);
    let identity = dotnet_check::request_identity(&f.authority, &scope, "api-build").unwrap();
    let dispatches:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='dotnet_check'",[],|r|r.get(0)).unwrap();
    let database = f.root.join("data/companies/fixture/brain.sqlite3");
    let run = scope.run.to_string();
    let revoke = std::thread::spawn(move || {
        let db = rusqlite::Connection::open(database).unwrap();
        db.busy_timeout(std::time::Duration::from_millis(150))
            .unwrap();
        let start = std::time::Instant::now();
        loop {
            let count:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='dotnet_check'",[],|r|r.get(0)).unwrap();
            if count > dispatches {
                break;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let mut policy = governance::load(&db, &run).unwrap().unwrap();
        policy.revision = 6;
        policy
            .grants
            .get_mut("Worker")
            .unwrap()
            .tools
            .remove("dotnet_check");
        governance::save(&db, &run, &policy, 5).unwrap();
    });
    assert!(registry
        .call(&f.authority, &scope, "dotnet_check", args(&identity))
        .is_err());
    revoke.join().unwrap();
    assert_eq!(db.query_row::<i64,_,_>("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='dotnet_check'",[],|r|r.get(0)).unwrap(),0);
    // Optional durable, non-secret live report. Test scratch/workspace still
    // disappears; the report contains actual tool receipts and exact inputs.
    if let Some(path) = std::env::var_os("GENTS_DOTNET_TEST_REPORT") {
        let report = json!({"schema_version":1,"scope":"Installed SDK compile check; no app/API or specialist qualification","inputs":{"api/Api.csproj":PROJECT,"api/Program.cs":"Console.WriteLine(\"bounded managed build\");"},"receipts":receipts,"checks":{"negative_compile":true,"source_change_invalidates":true,"stale_identity_denied":true,"frozen_project_write_denied":true,"wrong_actor_denied":true,"planner_denied":true,"stdio_build":true,"sse_build":true,"unlisted_code_and_targets_excluded":true,"sqlite_writer_available_during_sdk":true,"midbuild_source_change_rejected":true,"owner_revocation_during_sdk_rejected_commit":true,"canonical_receipt_required":true,"current_access_required":true},"provider_cost_usd":0,"qualified":false,"promoted":false});
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}

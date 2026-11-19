mod support;
use gents_local_mcp::{authority::Authority, governance, python_check, Phase, Registry};
use serde_json::{json, Value};
use support::Fixture;

fn intake() -> Value {
    json!({"python_check_contracts":[{"id":"unit-tests","actor":"Worker",
        "source_paths":["candidate.py"],"tests_path":"test_candidate.py",
        "tests_sha256":"a".repeat(64),"interpreter_sha256":"b".repeat(64)}]})
}
#[test]
fn contracts_fail_closed_and_do_not_add_grants_or_completion_requirements() {
    assert!(python_check::validate(&json!({})).is_ok());
    assert!(python_check::validate(&intake()).is_ok());
    for (pointer, invalid) in [
        ("/python_check_contracts/0/id", json!("../bad")),
        ("/python_check_contracts/0/actor", json!("")),
        ("/python_check_contracts/0/source_paths", json!([])),
        (
            "/python_check_contracts/0/source_paths",
            json!(["../candidate.py"]),
        ),
        (
            "/python_check_contracts/0/source_paths",
            json!(["candidate.py", "candidate.py"]),
        ),
        (
            "/python_check_contracts/0/source_paths",
            json!(["test_candidate.py"]),
        ),
        ("/python_check_contracts/0/tests_path", json!("test*.py")),
        (
            "/python_check_contracts/0/tests_path",
            json!("nested/test.py"),
        ),
        ("/python_check_contracts/0/tests_sha256", json!("unknown")),
        (
            "/python_check_contracts/0/interpreter_sha256",
            json!("unknown"),
        ),
    ] {
        let mut value = intake();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(python_check::validate(&value).is_err(), "{pointer}");
    }
    for (key, value) in [
        ("scratch_bytes", json!(8388609)),
        ("scratch_bytes", json!(true)),
        ("frozen_inputs", json!({"other.py":"a".repeat(64)})),
    ] {
        let mut invalid = intake();
        invalid["python_check_contracts"][0][key] = value;
        assert!(python_check::validate(&invalid).is_err());
    }
    let f = Fixture::new();
    let registry = Registry::new();
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "python_check",
            json!({"contract_id":"unit-tests"})
        )
        .is_err());
    assert!(registry.list(&f.authority, &f.scope).unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["name"] != "python_check"));
    assert!(gents_local_mcp::frozen_input_path(
        &intake(),
        "test_candidate.py"
    ));
    assert!(!gents_local_mcp::frozen_input_path(
        &intake(),
        "candidate.py"
    ));
    let descriptor = registry.catalog()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "python_check")
        .unwrap()
        .clone();
    assert_eq!(
        descriptor["resultSchema"]["properties"]["qualified"]["const"],
        false
    );
    assert_eq!(descriptor["runtime_requirements"]["installed_python"], true);
}

#[cfg(target_os = "macos")]
fn installed_python() -> std::path::PathBuf {
    // Test-host discovery only; native callers must configure the absolute path.
    let output = std::process::Command::new("python3").args(["-I","-S","-c",
        "import sys,pathlib; p=pathlib.Path(sys.base_prefix)/'Resources/Python.app/Contents/MacOS/Python'; print(p if p.is_file() else pathlib.Path(sys.executable).resolve())"])
        .output().unwrap();
    assert!(output.status.success());
    std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn native_python_repair_receipts_revocation_and_both_transports() {
    let executable = installed_python();
    // This integration binary contains one env-mutating test; no concurrent
    // mutation, no credentials or unrelated runtime configuration is inherited.
    std::env::set_var("GENTS_PYTHON", &executable);
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 3;
    policy
        .ceilings
        .get_mut("Worker")
        .unwrap()
        .tools
        .insert("python_check".into());
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .insert("python_check".into());
    governance::save(&db, &f.scope.run.to_string(), &policy, 2).unwrap();
    let tests = "import unittest,candidate\nclass T(unittest.TestCase):\n def test_value(self): self.assertEqual(candidate.value,2)\n";
    let mut contract = intake();
    contract["python_check_contracts"][0]["tests_sha256"] =
        json!(gents_local_mcp::digest(tests.as_bytes()));
    contract["python_check_contracts"][0]["interpreter_sha256"] = json!(gents_local_mcp::digest(
        &std::fs::read(&executable).unwrap()
    ));
    db.execute("UPDATE jobs SET intake=?1", [contract.to_string()])
        .unwrap();
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), true).unwrap();
    work.write("test_candidate.py", tests).unwrap();
    let registry = Registry::new();
    registry
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"write","path":"candidate.py","content":"value=1"}),
        )
        .unwrap();
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"write","path":"test_candidate.py","content":"changed"})
        )
        .is_err());
    let mut scope = f.scope.clone();
    scope.expected_revision = Some(3);
    let old = python_check::request_identity(&f.authority, &scope, "unit-tests").unwrap();
    assert_eq!(
        python_check::source_read_identity(&f.authority, &scope, "candidate.py").unwrap(),
        old
    );
    assert!(python_check::source_read_identity(&f.authority, &scope, "other.py").is_err());
    let args =
        |identity: &str| json!({"contract_id":"unit-tests","expected_source_identity":identity});
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "python_check",
            json!({"contract_id":"unit-tests"})
        )
        .is_err());
    let failed = registry
        .call(&f.authority, &scope, "python_check", args(&old))
        .unwrap();
    assert_eq!(failed["value"]["status"], "reported_fail", "{failed}");
    assert_eq!(failed["receipt"]["tool"], "python_check");
    let observed = python_check::observations(&f.authority, &scope).unwrap();
    assert_eq!(
        observed[0]["latest_current_diagnostic"]["status"],
        "reported_fail"
    );
    assert_eq!(
        observed[0]["latest_current_diagnostic"]["canonical_receipt_verified"],
        true
    );
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"candidate.py","content":"value=2"}),
        )
        .unwrap();
    let changed = python_check::request_identity(&f.authority, &scope, "unit-tests").unwrap();
    assert_ne!(old, changed);
    assert_eq!(
        python_check::source_read_identity(&f.authority, &scope, "candidate.py").unwrap(),
        changed
    );
    assert!(python_check::observations(&f.authority, &scope).unwrap()[0]
        ["latest_current_diagnostic"]
        .is_null());
    assert!(registry
        .call(&f.authority, &scope, "python_check", args(&old))
        .is_err());
    let good = registry
        .call(&f.authority, &scope, "python_check", args(&changed))
        .unwrap();
    assert_eq!(good["value"]["status"], "reported_pass", "{good}");
    assert_eq!(good["value"]["qualified"], false);
    let custody = |scope: &gents_local_mcp::Scope| {
        python_check::observations(&f.authority, scope).unwrap()[0]["current_source_custody"]
            ["candidate.py"]
            .clone()
    };
    assert_eq!(custody(&scope)["written"], true);
    assert_eq!(custody(&scope)["read_back"], false);
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"read","path":"candidate.py","metadata_only":true}),
        )
        .unwrap();
    assert_eq!(custody(&scope)["read_back"], false);
    for args in [
        json!({"op":"read","path":"candidate.py","max_bytes":2}),
        json!({"op":"read","path":"candidate.py","offset_bytes":4}),
    ] {
        registry.call(&f.authority, &scope, "files", args).unwrap();
        assert_eq!(custody(&scope)["read_back"], false);
    }
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"read","path":"candidate.py","offset_bytes":2,"max_bytes":2}),
        )
        .unwrap();
    assert_eq!(custody(&scope)["read_back"], true);
    db.execute(
        "UPDATE python_source_custody SET result='{}' WHERE operation='read' AND offset_bytes=2",
        [],
    )
    .unwrap();
    assert_eq!(custody(&scope)["read_back"], false);
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"read","path":"candidate.py"}),
        )
        .unwrap();
    assert_eq!(custody(&scope)["read_back"], true);
    assert_eq!(
        python_check::observations(&f.authority, &scope).unwrap()[0]["latest_current_diagnostic"]
            ["status"],
        "reported_pass"
    );
    db.execute("DELETE FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='python_check'",[]).unwrap();
    db.execute(
        "INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'model_output',?2,1)",
        rusqlite::params![scope.run.to_string(), good["receipt"].to_string()],
    )
    .unwrap();
    assert!(python_check::observations(&f.authority, &scope).unwrap()[0]
        ["latest_current_diagnostic"]
        .is_null());
    assert_eq!(
        good["receipt"]["result_sha256"],
        gents_local_mcp::digest(&serde_json::to_vec(&good["value"]).unwrap())
    );
    let mut planner = scope.clone();
    planner.phase = Phase::Planner;
    assert!(registry
        .call(&f.authority, &planner, "python_check", args(&changed))
        .is_err());
    assert!(
        python_check::request_identity(&f.authority, &f.actor("Reader"), "unit-tests").is_err()
    );
    // Actual process-bound MCP clients, not an in-process transport simulation.
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let binary_directory = f.root.join("target/debug");
    std::fs::create_dir_all(&binary_directory).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        binary_directory.join("gents-local-mcp"),
    )
    .unwrap();
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let result = gents_local_mcp::client::invoke(
            &f.root,
            &scope,
            "python_check",
            &args(&changed),
            &cancelled,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(result["result"]["status"], "reported_pass", "{result}");
        assert_eq!(result["mcp_receipt"]["policy_revision"], 3);
    }
    std::env::remove_var("GENTS_MCP_TRANSPORT");
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"candidate.py",
        "content":"print('4'+'1'*15)\nvalue=2"}),
        )
        .unwrap();
    let private_output =
        python_check::request_identity(&f.authority, &scope, "unit-tests").unwrap();
    assert!(registry
        .call(&f.authority, &scope, "python_check", args(&private_output))
        .is_err());
    let journal: String = db
        .query_row("SELECT group_concat(body) FROM audit", [], |r| r.get(0))
        .unwrap();
    assert!(!journal.contains("4111111111111111"));
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"candidate.py","content":"value=2"}),
        )
        .unwrap();
    // A lost audit commit must not become a successful native observation.
    db.execute_batch("CREATE TRIGGER reject_python_receipt BEFORE INSERT ON audit WHEN NEW.kind='mcp_tool_call' AND json_extract(NEW.body,'$.tool')='python_check' BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;").unwrap();
    let error = registry
        .call(&f.authority, &scope, "python_check", args(&changed))
        .unwrap_err();
    assert!(error.to_string().contains("audit receipt failed"));
    db.execute_batch("DROP TRIGGER reject_python_receipt;")
        .unwrap();
    let uncertain:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_effect_uncertain' AND json_extract(body,'$.reason')='audit_commit_failed'",[],|r|r.get(0)).unwrap();
    assert_eq!(uncertain, 1);
    let view = python_check::observations(&f.authority, &scope).unwrap();
    assert_eq!(view[0]["latest_attempt_uncertain"], true);
    assert!(view[0]["latest_current_diagnostic"].is_null());
    // Owner-frozen helpers and scratch I/O are separate from candidate writes.
    let helper = "EXPECTED=2\n";
    work.write("fixed_helper.py", helper).unwrap();
    contract["python_check_contracts"][0]["source_paths"] =
        json!(["candidate.py", "fixed_helper.py"]);
    contract["python_check_contracts"][0]["frozen_inputs"] =
        json!({"fixed_helper.py":gents_local_mcp::digest(helper.as_bytes())});
    contract["python_check_contracts"][0]["scratch_bytes"] = json!(1048576);
    db.execute("UPDATE jobs SET intake=?1", [contract.to_string()])
        .unwrap();
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"fixed_helper.py","content":"EXPECTED=0"})
        )
        .is_err());
    registry.call(&f.authority,&scope,"files",json!({"op":"write","path":"candidate.py",
        "content":"import pathlib,tempfile\nfrom fixed_helper import EXPECTED\nwith tempfile.TemporaryDirectory() as d:\n p=pathlib.Path(d)/'v';p.write_text(str(EXPECTED));value=int(p.read_text())"})).unwrap();
    let scratch_identity =
        python_check::request_identity(&f.authority, &scope, "unit-tests").unwrap();
    let checked = registry
        .call(
            &f.authority,
            &scope,
            "python_check",
            args(&scratch_identity),
        )
        .unwrap();
    assert_eq!(checked["value"]["status"], "reported_pass", "{checked}");
    assert_eq!(checked["value"]["scratch_limit_bytes"], 1048576);
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let observed = gents_local_mcp::client::invoke(
            &f.root,
            &scope,
            "python_check",
            &args(&scratch_identity),
            &cancelled,
        )
        .await
        .unwrap();
        assert_eq!(observed["result"]["status"], "reported_pass", "{observed}");
    }
    std::env::remove_var("GENTS_MCP_TRANSPORT");
    work.write("fixed_helper.py", "EXPECTED=0").unwrap();
    assert!(python_check::request_identity(&f.authority, &scope, "unit-tests").is_err());
    work.write("fixed_helper.py", helper).unwrap();
    // Frozen test mutation and tool/read revocation cannot reuse old identity.
    work.write("test_candidate.py", "raise RuntimeError('not frozen')")
        .unwrap();
    assert!(python_check::request_identity(&f.authority, &scope, "unit-tests").is_err());
    work.write("test_candidate.py", tests).unwrap();
    policy.revision = 4;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("files:read");
    governance::save(&db, &scope.run.to_string(), &policy, 3).unwrap();
    assert!(registry
        .call(&f.authority, &scope, "python_check", args(&changed))
        .is_err());
    scope.expected_revision = Some(4);
    assert!(python_check::request_identity(&f.authority, &scope, "unit-tests").is_err());
    std::env::remove_var("GENTS_PYTHON");
}

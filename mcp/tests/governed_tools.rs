use gents_local_mcp::{
    authority::Authority,
    governance,
    protocol::{self, Session},
    Phase, Registry, Scope,
};
use rusqlite::params;
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};
mod support;
use support::Fixture;
#[test]
fn harness_dispatch_is_derived_from_the_reviewed_native_registry() {
    for tool in gents_local_mcp::functions::builtins() {
        assert!(gents_local_mcp::client::handles(tool.name()));
    }
    assert!(!gents_local_mcp::client::handles("unreviewed_function"));
}
#[test]
fn shared_governance_filters_catalog_and_rejects_impersonation() {
    let f = Fixture::new();
    let registry = Registry::new();
    let reader = f.actor("Reader");
    assert_eq!(
        registry.list(&f.authority, &reader).unwrap()["tools"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(registry
        .call(
            &f.authority,
            &reader,
            "memory_append",
            json!({"kind":"fact","body":"wrong role"})
        )
        .is_err());
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "memory_append",
            json!({"kind":"fact","body":"x","actor":"Reader"})
        )
        .is_err());
    let mut wrong = f.scope.clone();
    wrong.owner = "another owner".into();
    assert!(f.authority.open(&wrong).is_err());
    wrong = f.scope.clone();
    wrong.company = "../fixture".into();
    assert!(f.authority.open(&wrong).is_err());
    wrong = f.scope.clone();
    wrong.run = uuid::Uuid::new_v4();
    assert!(f.authority.open(&wrong).is_err());
    let logs: String = f
        .db()
        .query_row("SELECT group_concat(body) FROM audit", [], |r| r.get(0))
        .unwrap();
    assert!(!logs.contains("wrong role"));
    assert!(logs.contains("arguments_sha256"));
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "untrusted_name_must_not_be_logged",
            json!({})
        )
        .is_err());
    let logs: String = f
        .db()
        .query_row("SELECT group_concat(body) FROM audit", [], |r| r.get(0))
        .unwrap();
    assert!(!logs.contains("untrusted_name_must_not_be_logged"));
}

#[test]
fn corrupt_canonical_sources_cannot_be_laundered_into_a_derived_note() {
    let f = Fixture::new();
    let registry = Registry::new();
    let original = registry
        .call(
            &f.authority,
            &f.scope,
            "memory_append",
            json!({"kind":"advisory","body":"reviewed source observation"}),
        )
        .unwrap();
    f.db()
        .execute(
            "UPDATE mcp_memory SET body='altered source' WHERE id=?1",
            [original["value"]["id"].as_str().unwrap()],
        )
        .unwrap();
    assert!(registry.call(&f.authority,&f.scope,"memory_append",json!({"kind":"advisory","body":"derived note","source_refs":[{"id":original["value"]["id"],"sha256":original["value"]["sha256"]}]})).is_err());
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "memory_search",
            json!({"query":"source","limit":1})
        )
        .is_err());
}

#[test]
fn bounded_recovery_only_proposes_an_authorized_read_and_never_executes_it() {
    let f = Fixture::new();
    let registry = Registry::new();
    let db = f.db();
    db.execute(
        "UPDATE jobs SET intake=?1",
        [json!({"required_mcp_calls":[{"actor":"Worker","tool":"memory_append"}]}).to_string()],
    )
    .unwrap();
    assert!(
        registry.recovery(&f.authority, &f.scope).unwrap().is_none(),
        "never auto-write an invented plan"
    );
    db.execute(
        "UPDATE jobs SET intake=?1",
        [json!({"required_mcp_calls":[{"actor":"Worker","tool":"memory_search"}]}).to_string()],
    )
    .unwrap();
    let proposal = registry.recovery(&f.authority, &f.scope).unwrap().unwrap();
    assert_eq!(proposal["action"], "memory_search");
    assert_eq!(proposal["arguments"], json!({"query":"","limit":4}));
    assert!(
        registry
            .recovery(&f.authority, &f.actor("Reader"))
            .unwrap()
            .is_none(),
        "cannot fill another actor's requirement"
    );
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM audit", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "proposal is not execution");
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 2;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("memory_search");
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    assert!(
        registry.recovery(&f.authority, &f.scope).is_err(),
        "discovery cannot bypass revocation"
    );
}

#[test]
fn skills_are_shared_but_never_cross_the_assigned_phase_or_actor() {
    let f = Fixture::new();
    let registry = Registry::new();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 2;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        let worker = access.get_mut("Worker").unwrap();
        worker
            .tools
            .extend(["skill_list".into(), "skill_read".into()]);
        worker
            .skills
            .extend(["source-onboarding".into(), "local-code-repair".into()]);
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    let list = registry
        .call(&f.authority, &f.scope, "skill_list", json!({}))
        .unwrap();
    assert_eq!(list["value"]["skills"].as_array().unwrap().len(), 1);
    assert_eq!(list["value"]["skills"][0]["id"], "local-code-repair");
    assert!(list["value"]["skills"][0].get("instructions").is_none());
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "skill_read",
            json!({"id":"source-onboarding"})
        )
        .is_err());
    let planner = Scope {
        phase: Phase::Planner,
        ..f.scope.clone()
    };
    let read = registry
        .call(
            &f.authority,
            &planner,
            "skill_read",
            json!({"id":"source-onboarding"}),
        )
        .unwrap();
    assert_eq!(read["value"]["id"], "source-onboarding");
    assert!(read["value"]["instructions"].as_str().unwrap().len() > 100);
    assert!(registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "skill_read",
            json!({"id":"source-onboarding"})
        )
        .is_err());
    assert!(registry
        .call(
            &f.authority,
            &planner,
            "skill_read",
            json!({"id":"source-onboarding","phase":"worker"})
        )
        .is_err());
}

#[test]
fn completion_requires_real_receipts_from_the_frozen_actor_and_current_epoch() {
    use gents_local_mcp::completion;
    let f = Fixture::new();
    let registry = Registry::new();
    let db = f.db();
    let intake = json!({"required_mcp_calls":[{"actor":"Worker","tool":"memory_append"},{"actor":"Worker","tool":"memory_search"}]});
    completion::validate(&intake).unwrap();
    assert!(completion::validate(
        &json!({"required_mcp_calls":[{"actor":"Worker","tool":"shell"}]})
    )
    .is_err());
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'model_output',?2,1)",params![f.scope.run.to_string(),json!({"status":"completed","actor":"Worker","tool":"memory_append","policy_revision":1}).to_string()]).unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    registry
        .call(
            &f.authority,
            &f.actor("Outsider"),
            "memory_append",
            json!({"kind":"advisory","body":"other actor observation"}),
        )
        .unwrap();
    registry
        .call(
            &f.authority,
            &f.scope,
            "memory_search",
            json!({"query":"context","limit":1}),
        )
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    registry
        .call(
            &f.authority,
            &f.scope,
            "memory_append",
            json!({"kind":"advisory","body":"context sizing plan"}),
        )
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 2;
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
}

#[tokio::test(flavor = "current_thread")]
async fn harness_adapter_uses_actual_stdio_and_prelaunch_cancel_cannot_mutate() {
    use std::sync::atomic::AtomicBool;
    let f = Fixture::new();
    let folder = f.root.join("target/debug");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        folder.join("gents-local-mcp"),
    )
    .unwrap();
    let result = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "memory_append",
        &json!({"kind":"advisory","body":"native adapter observation"}),
        &AtomicBool::new(false),
    )
    .await
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["mcp_receipt"]["actor"], "Worker");
    assert!(gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "memory_append",
        &json!({"kind":"fact","body":"must not appear"}),
        &AtomicBool::new(true)
    )
    .await
    .is_err());
    let count: i64 = f
        .db()
        .query_row("SELECT COUNT(*) FROM mcp_memory", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
#[test]
fn private_memory_shares_only_allowed_current_epoch_sources() {
    let f = Fixture::new();
    let registry = Registry::new();
    let original = registry
        .call(
            &f.authority,
            &f.scope,
            "memory_append",
            json!({"kind":"experiment","body":"cache batch experiment"}),
        )
        .unwrap();
    registry
        .call(
            &f.authority,
            &f.actor("Outsider"),
            "memory_append",
            json!({"kind":"fact","body":"private hidden cache note"}),
        )
        .unwrap();
    let read = registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "memory_search",
            json!({"query":"cache","limit":1}),
        )
        .unwrap();
    assert_eq!(read["value"]["records"][0]["id"], original["value"]["id"]);
    let none = registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "memory_search",
            json!({"query":"hidden"}),
        )
        .unwrap();
    assert_eq!(none["value"]["records"], json!([]));
    let reference = json!({"id":original["value"]["id"],"sha256":original["value"]["sha256"]});
    assert!(registry
        .call(
            &f.authority,
            &f.actor("Outsider"),
            "memory_append",
            json!({"kind":"advisory","body":"derived record","source_refs":[reference.clone()]})
        )
        .is_err());
    assert!(registry.call(&f.authority,&f.scope,"memory_append",json!({"kind":"advisory","body":"authorized derived hypothesis","source_refs":[reference]})).is_ok());
    let db = f.db();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    p.revision = 2;
    p.grants.get_mut("Reader").unwrap().memory_from.clear();
    governance::save(&db, &f.scope.run.to_string(), &p, 1).unwrap();
    let none = registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "memory_search",
            json!({"query":"cache"}),
        )
        .unwrap();
    assert_eq!(none["value"]["records"], json!([]));
    p.revision = 3;
    p.grants
        .get_mut("Reader")
        .unwrap()
        .memory_from
        .insert("Worker".into());
    governance::save(&db, &f.scope.run.to_string(), &p, 2).unwrap();
    assert_eq!(
        registry
            .call(
                &f.authority,
                &f.actor("Reader"),
                "memory_search",
                json!({"query":"cache"})
            )
            .unwrap()["value"]["records"],
        json!([]),
        "regrant must not silently launder an old-epoch source"
    );
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "memory_append",
            json!({"kind":"fact","body":"person@example.invalid"})
        )
        .is_err());
}
#[test]
fn native_arithmetic_has_no_evaluation_or_boolean_coercion() {
    let f = Fixture::new();
    let registry = Registry::new();
    let args = json!({"calculations":[{"id":"case_1","values":[7,-3],"subtract":2}]});
    let value = registry
        .call(&f.authority, &f.scope, "integer_arithmetic", args)
        .unwrap();
    assert_eq!(value["value"]["calculations"][0]["difference"], 2);
    assert_eq!(value["value"]["calculations"][0]["id"], "case_1");
    for id in [
        "".to_string(),
        "bad/id".into(),
        "id\n".into(),
        "é".into(),
        "a".repeat(41),
    ] {
        assert!(registry
            .call(
                &f.authority,
                &f.scope,
                "integer_arithmetic",
                json!({"calculations":[{"id":id,"values":[1],"subtract":0}]})
            )
            .is_err());
    }
    let duplicate = json!({"id":"same_id","values":[1],"subtract":0});
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "integer_arithmetic",
            json!({"calculations":[duplicate,duplicate]})
        )
        .is_err());
    assert_eq!(value["receipt"]["provider_cost_usd"], 0);
    for values in [
        json!([true]),
        json!([1.5]),
        json!([10000001]),
        json!(["eval(1)"]),
    ] {
        assert!(registry
            .call(
                &f.authority,
                &f.scope,
                "integer_arithmetic",
                json!({"calculations":[{"id":"bad","values":values,"subtract":0}]})
            )
            .is_err());
    }
}
#[test]
fn protocol_requires_lifecycle_and_notifications_cannot_execute_tools() {
    let f = Fixture::new();
    let registry = Registry::new();
    let mut s = Session::new(&registry, &f.authority, &f.scope);
    assert_eq!(
        s.handle(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .unwrap()["error"]["code"],
        -32000
    );
    assert!(s.handle(json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"memory_append","arguments":{"kind":"fact","body":"not executed"}}})).is_none());
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM mcp_memory", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    s.handle(json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":protocol::VERSION}}));
    s.handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    assert_eq!(
        s.handle(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))
            .unwrap()["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(s.handle(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"integer_arithmetic","arguments":{},"actor":"Worker"}})).unwrap()["error"]["code"],-32602);
    assert!(protocol::read_frame(&mut std::io::Cursor::new(vec![b'x'; 65537])).is_err());
}
#[test]
fn actual_stdio_process_speaks_only_json_rpc() {
    let f = Fixture::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gents-local-mcp"))
        .env_clear()
        .args([
            "--root",
            f.root.to_str().unwrap(),
            "--company",
            &f.scope.company,
            "--owner",
            &f.scope.owner,
            "--run",
            &f.scope.run.to_string(),
            "--actor",
            &f.scope.actor,
            "--phase",
            "worker",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let frames = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":protocol::VERSION}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"integer_arithmetic","arguments":{"calculations":[{"id":"native","values":[8,4],"subtract":2}]}}}),
    ];
    {
        let mut stdin = child.stdin.take().unwrap();
        for frame in frames {
            writeln!(stdin, "{frame}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(
        responses[1]["result"]["structuredContent"]["value"]["calculations"][0]["difference"],
        10
    );
}

mod support;
use gents_local_mcp::{artifact_bindings, governance, Registry};
use serde_json::{json, Value};
use support::Fixture;
fn fixture() -> (Fixture, Value) {
    let f = Fixture::new();
    support::install_files(&f);
    let args = json!({"calculations":[{"id":"approved","values":[31,-8,19],"subtract":2}]});
    let binding = json!({"path":"total.txt","source_actor":"Worker","writer":"Worker","source_tool":"integer_arithmetic","arguments":args,"pointer":"/calculations/0/difference","encoding":"integer_text"});
    let intake = json!({"artifact_bindings":[binding]});
    artifact_bindings::parse(&intake).unwrap();
    f.db()
        .execute(
            "UPDATE jobs SET intake=?1 WHERE run_id=?2",
            rusqlite::params![intake.to_string(), f.scope.run.to_string()],
        )
        .unwrap();
    (f, args)
}
fn call(f: &Fixture, tool: &str, args: Value) -> Value {
    Registry::new()
        .call(&f.authority, &f.scope, tool, args)
        .unwrap()
}
#[test]
fn artifact_requires_actual_approved_source_and_writer_write_readback() {
    let (f, args) = fixture();
    let wrong = json!({"calculations":[{"id":"different","values":[31,-8,19],"subtract":2}]});
    call(&f, "integer_arithmetic", wrong);
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.scope, true).unwrap()["passed"],
        false
    );
    call(&f, "integer_arithmetic", args);
    call(
        &f,
        "files",
        json!({"op":"write","path":"total.txt","content":"42"}),
    );
    call(&f, "files", json!({"op":"read","path":"total.txt"}));
    let bad = artifact_bindings::check(&f.authority, &f.scope, true).unwrap();
    assert_eq!(bad["passed"], false);
    assert_eq!(bad["checks"][0]["expected_from_native_source"], "40");
    // An externally corrected file still lacks a matching native writer receipt.
    std::fs::write(
        f.root
            .join("data/companies/fixture/runs")
            .join(f.scope.run.to_string())
            .join("workspace/total.txt"),
        "40",
    )
    .unwrap();
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.scope, true).unwrap()["passed"],
        false
    );
    call(
        &f,
        "files",
        json!({"op":"write","path":"total.txt","content":"40"}),
    );
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.scope, true).unwrap()["passed"],
        false
    );
    call(&f, "files", json!({"op":"read","path":"total.txt"}));
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.scope, true).unwrap()["passed"],
        true
    );
    let db = f.db();
    let prior = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let mut policy = prior.clone();
    policy.revision += 1;
    governance::save(&db, &f.scope.run.to_string(), &policy, prior.revision).unwrap();
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.scope, true).unwrap()["passed"],
        false
    );
}
#[test]
fn projection_cannot_survive_failed_audit_or_launder_modified_data() {
    let (f, args) = fixture();
    f.db().execute_batch("CREATE TRIGGER reject_receipt BEFORE INSERT ON audit WHEN NEW.kind='mcp_tool_call' BEGIN SELECT RAISE(ABORT,'fixture audit failure'); END;").unwrap();
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "integer_arithmetic", args.clone())
        .is_err());
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM mcp_binding_sources_v1", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    f.db().execute_batch("DROP TRIGGER reject_receipt").unwrap();
    call(&f, "integer_arithmetic", args);
    f.db()
        .execute("UPDATE mcp_binding_sources_v1 SET value_json='42'", [])
        .unwrap();
    assert!(artifact_bindings::check(&f.authority, &f.scope, true).is_err());
}
#[test]
fn compiled_calls_use_approved_source_arguments_and_actual_projected_bytes_without_executing() {
    let (f, args) = fixture();
    let proposal = artifact_bindings::next_call(&f.authority, &f.scope, true)
        .unwrap()
        .unwrap();
    assert_eq!(proposal["action"], "integer_arithmetic");
    assert_eq!(proposal["arguments"], args);
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM audit", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    call(&f, "integer_arithmetic", args);
    let write = artifact_bindings::next_call(&f.authority, &f.scope, true)
        .unwrap()
        .unwrap();
    assert_eq!(
        write["arguments"],
        json!({"op":"write","path":"total.txt","content":"40"})
    );
    call(&f, "files", write["arguments"].clone());
    let read = artifact_bindings::next_call(&f.authority, &f.scope, true)
        .unwrap()
        .unwrap();
    assert_eq!(read["arguments"], json!({"op":"read","path":"total.txt"}));
    call(&f, "files", read["arguments"].clone());
    assert!(artifact_bindings::next_call(&f.authority, &f.scope, true)
        .unwrap()
        .is_none());
}
#[test]
fn source_access_and_binding_contract_fail_closed() {
    let (f, args) = fixture();
    call(&f, "integer_arithmetic", args);
    assert!(artifact_bindings::check(&f.authority, &f.actor("Outsider"), true).is_err());
    for change in [
        json!({"path":"../outside"}),
        json!({"encoding":"execute"}),
        json!({"source_tool":"shell"}),
        json!({"pointer":"not-a-pointer"}),
    ] {
        let encoded: String = f
            .db()
            .query_row("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap();
        let mut intake: Value = serde_json::from_str(&encoded).unwrap();
        for (key, value) in change.as_object().unwrap() {
            intake["artifact_bindings"][0][key] = value.clone();
        }
        assert!(artifact_bindings::parse(&intake).is_err());
    }
}

#[test]
fn selected_value_encodings_preserve_types_instead_of_coercing() {
    let (f, args) = fixture();
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row("SELECT intake FROM jobs", [], |r| r.get::<_, String>(0))
            .unwrap(),
    )
    .unwrap();
    intake["artifact_bindings"][0]["pointer"] = json!("/exact");
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    assert!(
        Registry::new()
            .call(&f.authority, &f.scope, "integer_arithmetic", args.clone())
            .is_err(),
        "integer_text must reject a boolean projection"
    );
    intake["artifact_bindings"][0]["pointer"] = json!("/calculations/0");
    intake["artifact_bindings"][0]["encoding"] = json!("json");
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    call(&f, "integer_arithmetic", args);
    let next = artifact_bindings::next_call(&f.authority, &f.scope, true)
        .unwrap()
        .unwrap();
    let object: Value =
        serde_json::from_str(next["arguments"]["content"].as_str().unwrap()).unwrap();
    assert_eq!(object, json!({"id":"approved","sum":42,"difference":40}));
}

#[test]
fn utf8_projection_is_copied_without_model_rewriting() {
    let f = Fixture::new();
    support::install_files(&f);
    let intake = json!({"artifact_bindings":[{"path":"copy.txt","source_actor":"Worker","writer":"Worker","source_tool":"files","arguments":{"op":"read","path":"source.txt"},"pointer":"/text","encoding":"utf8_text"}]});
    f.db()
        .execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    call(
        &f,
        "files",
        json!({"op":"write","path":"source.txt","content":"A🧪β"}),
    );
    call(&f, "files", json!({"op":"read","path":"source.txt"}));
    let next = artifact_bindings::next_call(&f.authority, &f.scope, true)
        .unwrap()
        .unwrap();
    assert_eq!(next["arguments"]["content"], "A🧪β");
    call(&f, "files", next["arguments"].clone());
    call(&f, "files", json!({"op":"read","path":"copy.txt"}));
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.scope, true).unwrap()["passed"],
        true
    );
}

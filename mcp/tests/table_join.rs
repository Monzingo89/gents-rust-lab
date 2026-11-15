mod support;
use gents_local_mcp::{artifact_bindings, authority::Authority, governance, Registry};
use serde_json::{json, Value};
use support::Fixture;
fn fixture() -> Fixture {
    let f = Fixture::new();
    let db = f.db();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = p.revision;
    p.revision += 1;
    for access in [&mut p.grants, &mut p.ceilings] {
        access.get_mut("Worker").unwrap().tools.extend([
            "table_join".into(),
            "table_read_json".into(),
            "table_aggregate".into(),
        ]);
        access
            .get_mut("Reader")
            .unwrap()
            .tools
            .extend(["files:write".into(), "files:read".into()]);
    }
    governance::save(&db, &f.scope.run.to_string(), &p, prior).unwrap();
    drop(db);
    f
}
fn args(left: Value, right: Value, kind: &str) -> Value {
    json!({"left_rows":left,"right_rows":right,"left_key":"k","right_key":"k","kind":kind,"columns":[{"side":"left","field":"k","as":"key"},{"side":"left","field":"n","as":"units"},{"side":"right","field":"label","as":"category"}]})
}
fn call(f: &Fixture, input: Value) -> anyhow::Result<Value> {
    Registry::new()
        .call(&f.authority, &f.scope, "table_join", input)
        .map(|r| r["value"].clone())
}
#[test]
fn join_keys_preserve_scalar_types_duplicates_nulls_and_source_order() {
    let f = fixture();
    let left = json!([{"k":"x","n":3},{"k":true,"n":4},{"k":1,"n":5},{"k":null,"n":6},{"k":"missing","n":7},{"k":"x","n":3}]);
    let right = json!([{"k":"x","label":"first"},{"k":1,"label":"number"},{"k":true,"label":"bool"},{"k":"x","label":"second"},{"k":null,"label":"null"},{"k":"1","label":"string"}]);
    let result = call(&f, args(left.clone(), right.clone(), "left")).unwrap();
    assert_eq!(
        result["rows"],
        json!([
            {"key":"x","units":3,"category":"first"},{"key":"x","units":3,"category":"second"},
            {"key":true,"units":4,"category":"bool"},{"key":1,"units":5,"category":"number"},
            {"key":null,"units":6,"category":"null"},{"key":"missing","units":7,"category":null},
            {"key":"x","units":3,"category":"first"},{"key":"x","units":3,"category":"second"}
        ])
    );
    assert_eq!(result["matched_left_rows"], 5);
    assert_eq!(result["unmatched_left_rows"], 1);
    let inner = call(&f, args(left, right, "inner")).unwrap();
    assert_eq!(inner["rows"].as_array().unwrap().len(), 7);
    let empty = call(&f, args(json!([]), json!([]), "left")).unwrap();
    assert_eq!(empty["rows"], json!([]));
    let unmatched = call(&f, args(json!([{"k":"a","n":0}]), json!([]), "left")).unwrap();
    assert_eq!(unmatched["rows"][0]["category"], Value::Null);
    assert_eq!(unmatched["unmatched_left_rows"], 1);
}
#[test]
fn malformed_unmatched_rows_and_ambiguous_projection_fail_closed() {
    let f = fixture();
    let base = args(
        json!([{"k":"a","n":1}]),
        json!([{"k":"a","label":"x"}]),
        "left",
    );
    for change in [
        "alias",
        "missing_left",
        "missing_right",
        "key",
        "nested",
        "float",
        "field",
        "kind",
    ] {
        let mut input = base.clone();
        match change {
            "alias" => input["columns"][1]["as"] = json!("key"),
            "missing_left" => input["left_rows"] = json!([{"k":"unmatched"}]),
            "missing_right" => input["right_rows"] = json!([{"k":"unmatched"}]),
            "key" => input["right_rows"] = json!([{"label":"unused"}]),
            "nested" => input["left_rows"][0]["n"] = json!({}),
            "float" => input["left_rows"][0]["n"] = json!(1.0),
            "field" => input["columns"][1]["as"] = json!("bad-key"),
            "kind" => input["kind"] = json!("outer"),
            _ => unreachable!(),
        };
        assert!(call(&f, input).is_err(), "accepted {change}");
    }
    assert!(Registry::new()
        .call(&f.authority, &f.actor("Reader"), "table_join", base.clone())
        .is_err());
    Registry::new()
        .validate_projection("table_join", &base, "/rows", "json")
        .unwrap();
    assert!(Registry::new()
        .validate_projection("table_join", &base, "/rows", "integer_text")
        .is_err());
}
#[test]
fn cardinality_and_byte_limits_reject_without_partial_success_receipts() {
    let f = fixture();
    let left = json!((0..8).map(|_| json!({"k":1,"n":1})).collect::<Vec<_>>());
    let right = json!((0..8)
        .map(|_| json!({"k":1,"label":"x"}))
        .collect::<Vec<_>>());
    assert_eq!(
        call(&f, args(left.clone(), right.clone(), "inner")).unwrap()["rows"]
            .as_array()
            .unwrap()
            .len(),
        64
    );
    let before: i64 = f
        .db()
        .query_row(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut more = left.clone();
    more.as_array_mut().unwrap().push(json!({"k":1,"n":1}));
    assert!(call(&f, args(more, right.clone(), "inner")).is_err());
    let mut bytes = args(left, right, "inner");
    bytes["columns"] = json!((0..16)
        .map(|i| json!({"side":"right","field":"label","as":format!("c{i}")}))
        .collect::<Vec<_>>());
    bytes["right_rows"] = json!((0..8)
        .map(|_| json!({"k":1,"label":"a".repeat(128)}))
        .collect::<Vec<_>>());
    assert!(call(&f, bytes)
        .unwrap_err()
        .to_string()
        .contains("join output exceeds 16384 bytes"));
    let after: i64 = f
        .db()
        .query_row(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
}
#[test]
fn fan_in_requires_both_actual_source_receipts_before_binding_join_results() {
    let f = fixture();
    let registry = Registry::new();
    let root = f.authority.workspace(&f.scope).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let left = json!([{"k":"a","n":7},{"k":"b","n":-2}]);
    let right = json!([{"k":"a","label":"yes"},{"k":"b","label":"yes"}]);
    std::fs::write(root.join("left.json"), left.to_string()).unwrap();
    std::fs::write(root.join("right.json"), right.to_string()).unwrap();
    let join = args(json!([]), json!([]), "left");
    let aggregate = json!({"rows":[],"filter":null,"group_by":"category","value_field":"units"});
    let intake = json!({"artifact_bindings":[
        {"path":"left_rows.json","source_actor":"Worker","writer":"Reader","source_tool":"table_read_json","arguments":{"path":"left.json"},"pointer":"/rows","encoding":"json"},
        {"path":"right_rows.json","source_actor":"Worker","writer":"Reader","source_tool":"table_read_json","arguments":{"path":"right.json"},"pointer":"/rows","encoding":"json"},
        {"path":"joined.json","source_actor":"Worker","writer":"Reader","source_tool":"table_join","arguments":join,"argument_sources":[{"field":"left_rows","path":"left_rows.json"},{"field":"right_rows","path":"right_rows.json"}],"pointer":"/rows","encoding":"json"},
        {"path":"total.txt","source_actor":"Worker","writer":"Reader","source_tool":"table_aggregate","arguments":aggregate,"argument_sources":[{"field":"rows","path":"joined.json"}],"pointer":"/total","encoding":"integer_text"}
    ]});
    artifact_bindings::parse(&intake).unwrap();
    f.db()
        .execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    registry
        .call(
            &f.authority,
            &f.scope,
            "table_join",
            args(left.clone(), right.clone(), "left"),
        )
        .unwrap();
    for expected in [
        "table_read_json",
        "table_read_json",
        "table_join",
        "table_aggregate",
    ] {
        let next = artifact_bindings::next_call(&f.authority, &f.scope, false)
            .unwrap()
            .unwrap();
        assert_eq!(next["action"], expected);
        if expected == "table_join" {
            assert_eq!(next["arguments"]["left_rows"], left);
            assert_eq!(next["arguments"]["right_rows"], right);
        }
        registry
            .call(&f.authority, &f.scope, expected, next["arguments"].clone())
            .unwrap();
    }
    for _ in 0..8 {
        let next = artifact_bindings::next_call(&f.authority, &f.actor("Reader"), true)
            .unwrap()
            .unwrap();
        registry
            .call(
                &f.authority,
                &f.actor("Reader"),
                "files",
                next["arguments"].clone(),
            )
            .unwrap();
    }
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()["passed"],
        true
    );
    assert_eq!(
        std::fs::read_to_string(root.join("total.txt")).unwrap(),
        "5"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn actual_stdio_join_preserves_typed_keys_and_explicit_aliases() {
    let f = fixture();
    let binary = f.root.join("target/debug");
    std::fs::create_dir_all(&binary).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        binary.join("gents-local-mcp"),
    )
    .unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let result = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "table_join",
        &args(
            json!([{"k":1,"n":-4},{"k":true,"n":9}]),
            json!([{"k":"1","label":"string"},{"k":true,"label":"bool"}]),
            "left",
        ),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["mcp_receipt"]["tool"], "table_join");
    assert_eq!(
        result["result"]["rows"],
        json!([{"key":1,"units":-4,"category":null},{"key":true,"units":9,"category":"bool"}])
    );
}

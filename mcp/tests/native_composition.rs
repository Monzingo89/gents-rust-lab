mod support;
use gents_local_mcp::{artifact_bindings, governance, Registry};
use serde_json::{json, Value};
use support::Fixture;
fn fixture() -> (Fixture, Value, Value) {
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    for access in [&mut policy.grants, &mut policy.ceilings] {
        access
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("table_aggregate".into());
        access
            .get_mut("Reader")
            .unwrap()
            .tools
            .insert("files:write".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    let source = json!({"rows":[{"warehouse":"a","units":7},{"warehouse":"b","units":3},{"warehouse":"a","units":-2}],"filter":null,"group_by":"warehouse","value_field":"units"});
    let consumer = json!({"rows":[],"filter":null,"group_by":"rows","value_field":"total"});
    let base = |path: &str, args: &Value, pointer: &str, encoding: &str| json!({"path":path,"source_actor":"Worker","writer":"Reader","source_tool":"table_aggregate","arguments":args,"pointer":pointer,"encoding":encoding});
    let mut child = base("buckets.json", &consumer, "/groups", "json");
    child["argument_sources"] = json!([{"field":"rows","path":"warehouses.json"}]);
    let mut total = base("total.txt", &consumer, "/total", "integer_text");
    total["argument_sources"] = child["argument_sources"].clone();
    let intake =
        json!({"artifact_bindings":[base("warehouses.json",&source,"/groups","json"),child,total]});
    artifact_bindings::parse(&intake).unwrap();
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    drop(db);
    (f, source, consumer)
}
#[test]
fn chained_arguments_require_real_upstream_receipts_and_copy_native_values_exactly() {
    let (f, source, mut consumer) = fixture();
    let registry = Registry::new();
    let early = registry
        .call(&f.authority, &f.scope, "table_aggregate", consumer.clone())
        .unwrap();
    assert_eq!(early["value"]["total"], 0);
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM mcp_binding_sources_v1", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0,
        "a tool result without upstream provenance cannot satisfy the dependent binding"
    );
    let first = registry
        .call(&f.authority, &f.scope, "table_aggregate", source)
        .unwrap();
    consumer["rows"] = first["value"]["groups"].clone();
    let next = artifact_bindings::next_call(&f.authority, &f.scope, false)
        .unwrap()
        .unwrap();
    assert_eq!(next["arguments"], consumer);
    registry
        .call(
            &f.authority,
            &f.scope,
            "table_aggregate",
            next["arguments"].clone(),
        )
        .unwrap();
    assert!(artifact_bindings::next_call(&f.authority, &f.scope, false)
        .unwrap()
        .is_none());
    let writer = f.actor("Reader");
    for _ in 0..6 {
        let proposal = artifact_bindings::next_call(&f.authority, &writer, true)
            .unwrap()
            .unwrap();
        registry
            .call(
                &f.authority,
                &writer,
                "files",
                proposal["arguments"].clone(),
            )
            .unwrap();
    }
    assert_eq!(
        artifact_bindings::check(&f.authority, &writer, true).unwrap()["passed"],
        true
    );
    let text = registry
        .call(
            &f.authority,
            &writer,
            "files",
            json!({"op":"read","path":"total.txt"}),
        )
        .unwrap();
    assert_eq!(text["value"]["text"], "8");
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    assert_eq!(
        artifact_bindings::check(&f.authority, &writer, true).unwrap()["passed"],
        false,
        "previous-epoch source chains cannot satisfy current completion"
    );
}
#[test]
fn dependencies_reject_cycles_literal_overrides_and_unauthorized_recipients() {
    let (f, source, _) = fixture();
    let db = f.db();
    let raw: String = db
        .query_row("SELECT intake FROM jobs", [], |r| r.get(0))
        .unwrap();
    let original: Value = serde_json::from_str(&raw).unwrap();
    for change in ["forward", "cycle", "literal", "duplicate", "mutating"] {
        let mut intake = original.clone();
        match change {
            "forward" => {
                intake["artifact_bindings"][0]["argument_sources"] =
                    json!([{"field":"rows","path":"buckets.json"}])
            }
            "cycle" => {
                intake["artifact_bindings"][1]["argument_sources"][0]["path"] =
                    json!("buckets.json")
            }
            "literal" => {
                intake["artifact_bindings"][1]["arguments"]["rows"] =
                    json!([{"rows":1,"total":999}])
            }
            "duplicate" => {
                let r = intake["artifact_bindings"][1]["argument_sources"][0].clone();
                intake["artifact_bindings"][1]["argument_sources"]
                    .as_array_mut()
                    .unwrap()
                    .push(r);
            }
            "mutating" => {
                intake["artifact_bindings"][1]["source_tool"] = json!("files");
                intake["artifact_bindings"][1]["arguments"] =
                    json!({"op":"write","path":"x.txt","content":""});
                intake["artifact_bindings"][1]["argument_sources"] =
                    json!([{"field":"content","path":"warehouses.json"}]);
            }
            _ => unreachable!(),
        };
        assert!(
            artifact_bindings::parse(&intake).is_err(),
            "accepted {change}"
        );
    }
    drop(db);
    Registry::new()
        .call(&f.authority, &f.scope, "table_aggregate", source)
        .unwrap();
    assert!(artifact_bindings::check(&f.authority, &f.actor("Outsider"), true).is_err());
    f.db()
        .execute("UPDATE mcp_binding_sources_v1 SET value_json='[]'", [])
        .unwrap();
    assert!(artifact_bindings::check(&f.authority, &f.scope, false).is_err());
}

#[test]
fn changed_native_inputs_in_the_same_epoch_require_and_accept_fresh_consumer_evidence() {
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let intake = json!({"artifact_bindings":[
 {"path":"copy.txt","source_actor":"Worker","writer":"Worker","source_tool":"files","arguments":{"op":"read","path":"source.txt"},"pointer":"/text","encoding":"utf8_text"},
 {"path":"matches.json","source_actor":"Worker","writer":"Worker","source_tool":"memory_search","arguments":{"query":"","limit":1},"argument_sources":[{"field":"query","path":"copy.txt"}],"pointer":"/records","encoding":"json"}]});
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    drop(db);
    let registry = Registry::new();
    for text in ["first-needle", "second-needle"] {
        registry
            .call(
                &f.authority,
                &f.scope,
                "files",
                json!({"op":"write","path":"source.txt","content":text}),
            )
            .unwrap();
        registry
            .call(
                &f.authority,
                &f.scope,
                "files",
                json!({"op":"read","path":"source.txt"}),
            )
            .unwrap();
        let gate = artifact_bindings::check(&f.authority, &f.scope, true).unwrap();
        let child = gate["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["path"] == "matches.json")
            .unwrap();
        assert_eq!(
            child["required_source_arguments"]["query"], text,
            "an old consumer receipt cannot satisfy changed native inputs"
        );
        registry
            .call(
                &f.authority,
                &f.scope,
                "memory_search",
                json!({"query":text,"limit":1}),
            )
            .unwrap();
        let gate = artifact_bindings::check(&f.authority, &f.scope, true).unwrap();
        let child = gate["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["path"] == "matches.json")
            .unwrap();
        assert!(
            child["source_result_sha256"].is_string(),
            "fresh consumer projection must replace its concrete argument hash"
        );
    }
}

#[test]
fn repeated_pure_source_detection_requires_current_approved_canonical_evidence() {
    let (f, source, _) = fixture();
    let registry = Registry::new();
    assert!(registry.deterministic("table_aggregate"));
    assert!(registry.deterministic("table_join"));
    assert!(!registry.deterministic("table_read_json"));
    assert!(!registry.deterministic("memory_search"));
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &f.scope,
        "table_aggregate",
        &source
    )
    .unwrap()
    .is_none());
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row("SELECT intake FROM jobs", [], |r| r.get::<_, String>(0))
            .unwrap(),
    )
    .unwrap();
    intake["artifact_bindings"].as_array_mut().unwrap().push(json!({"path":"raw-total.txt","source_actor":"Worker","writer":"Reader","source_tool":"table_aggregate","arguments":source,"pointer":"/total","encoding":"integer_text"}));
    db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    drop(db);
    registry
        .call(&f.authority, &f.scope, "table_aggregate", source.clone())
        .unwrap();
    let hint =
        artifact_bindings::observed_pure_call(&f.authority, &f.scope, "table_aggregate", &source)
            .unwrap()
            .unwrap();
    assert!(hint["source_result_sha256"].is_string());
    assert!(hint.get("value").is_none());
    f.db()
        .execute(
            "DELETE FROM mcp_binding_sources_v1 WHERE path='raw-total.txt'",
            [],
        )
        .unwrap();
    assert!(
        artifact_bindings::observed_pure_call(&f.authority, &f.scope, "table_aggregate", &source)
            .unwrap()
            .is_none(),
        "one missing matching projection still requires a real native call"
    );
    registry
        .call(&f.authority, &f.scope, "table_aggregate", source.clone())
        .unwrap();
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &f.scope,
        "table_aggregate",
        &source
    )
    .unwrap()
    .is_some());

    let mut changed = source.clone();
    changed["rows"][0]["units"] = json!(8);
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &f.scope,
        "table_aggregate",
        &changed
    )
    .unwrap()
    .is_none());
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &f.actor("Reader"),
        "table_aggregate",
        &source
    )
    .is_err());
    f.db()
        .execute("UPDATE mcp_binding_sources_v1 SET value_json='[]'", [])
        .unwrap();
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &f.scope,
        "table_aggregate",
        &source
    )
    .is_err());
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &f.scope,
        "table_aggregate",
        &source
    )
    .unwrap()
    .is_none());
    let mut stale = f.scope.clone();
    stale.expected_revision = Some(prior);
    assert!(artifact_bindings::observed_pure_call(
        &f.authority,
        &stale,
        "table_aggregate",
        &source
    )
    .is_err());
}

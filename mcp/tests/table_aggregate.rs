mod support;
use gents_local_mcp::{
    contracts::{EmbeddedContracts, ToolContracts},
    governance, Registry,
};
use serde_json::{json, Value};
use support::Fixture;
fn fixture() -> Fixture {
    let f = Fixture::new();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    for grants in [&mut policy.grants, &mut policy.ceilings] {
        grants
            .get_mut("Worker")
            .unwrap()
            .tools
            .insert("table_aggregate".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    drop(db);
    f
}
fn args(rows: Value) -> Value {
    json!({"rows":rows,"filter":{"field":"accepted","equals":true},"group_by":"warehouse","value_field":"units"})
}
#[test]
fn typed_filter_grouping_zero_totals_and_duplicates_are_exact() {
    let f = fixture();
    let input = args(json!([
        {"warehouse":"south","accepted":true,"units":8},
        {"warehouse":"north","accepted":true,"units":7},
        {"warehouse":"south","accepted":true,"units":-8},
        {"warehouse":"north","accepted":true,"units":7},
        {"warehouse":"west","accepted":"true","units":999},
        {"warehouse":"west","accepted":1,"units":999}
    ]));
    let result = Registry::new()
        .call(&f.authority, &f.scope, "table_aggregate", input)
        .unwrap();
    assert_eq!(
        result["value"],
        json!({"input_rows":6,"matched_rows":4,"excluded_rows":2,"groups":[{"key":"north","rows":2,"total":14},{"key":"south","rows":2,"total":0}],"total":14,"exact":true})
    );
    assert_eq!(result["receipt"]["actor"], "Worker");
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.actor("Reader"),
            "table_aggregate",
            args(json!([]))
        )
        .is_err());
}
#[test]
fn scalar_keys_keep_types_and_have_deterministic_json_order() {
    let f = fixture();
    let input = json!({"rows":[{"k":true,"n":1},{"k":null,"n":2},{"k":1,"n":3},{"k":"1","n":4}],"filter":null,"group_by":"k","value_field":"n"});
    let result = Registry::new()
        .call(&f.authority, &f.scope, "table_aggregate", input)
        .unwrap()["value"]
        .clone();
    assert_eq!(
        result["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["key"].clone())
            .collect::<Vec<_>>(),
        vec![json!("1"), json!(1), Value::Null, json!(true)]
    );
    assert_eq!(result["total"], 10);
    let empty = Registry::new()
        .call(&f.authority, &f.scope, "table_aggregate", args(json!([])))
        .unwrap();
    assert_eq!(empty["value"]["groups"], json!([]));
    assert_eq!(empty["value"]["total"], 0);
}
#[test]
fn excluded_rows_cannot_hide_schema_errors_or_coercible_values() {
    let f = fixture();
    let registry = Registry::new();
    for rows in [
        json!([{"warehouse":"a","accepted":false}]),
        json!([{"warehouse":"a","accepted":false,"units":"4"}]),
        json!([{"warehouse":"a","accepted":false,"units":true}]),
        json!([{"warehouse":"a","accepted":false,"units":1.0}]),
        json!([{"warehouse":"a","units":1}]),
        json!([{"accepted":false,"units":1}]),
        json!([{"warehouse":{"nested":true},"accepted":true,"units":1}]),
        json!([{"warehouse":"a","accepted":true,"units":1000000000001i64}]),
    ] {
        assert!(registry
            .call(&f.authority, &f.scope, "table_aggregate", args(rows))
            .is_err());
    }
    let oversized = args(json!((0..65)
        .map(|_| json!({"warehouse":"a","accepted":true,"units":1}))
        .collect::<Vec<_>>()));
    assert!(EmbeddedContracts
        .validate("table_aggregate", &oversized)
        .is_err());
    let mut fields = serde_json::Map::new();
    for index in 0..17 {
        fields.insert(format!("field{index}"), json!(0));
    }
    assert!(EmbeddedContracts
        .validate("table_aggregate", &args(json!([fields])))
        .is_err());
}
#[test]
fn native_result_projection_types_are_declared_by_tools() {
    let registry = Registry::new();
    let input = args(json!([]));
    registry
        .validate_projection("table_aggregate", &input, "/groups", "json")
        .unwrap();
    registry
        .validate_projection("table_aggregate", &input, "/total", "integer_text")
        .unwrap();
    for (pointer, encoding) in [
        ("/groups", "integer_text"),
        ("/total", "utf8_text"),
        ("/invented", "json"),
        ("/groups/00/total", "integer_text"),
        ("/groups/64/total", "integer_text"),
        ("/exact", "integer_text"),
        ("/~2bad", "json"),
    ] {
        assert!(registry
            .validate_projection("table_aggregate", &input, pointer, encoding)
            .is_err());
    }
    let catalog = registry.catalog();
    let tool = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "table_aggregate")
        .unwrap();
    assert_eq!(
        tool["resultSchema"]["properties"]["total"]["type"],
        "integer"
    );
}

#[test]
fn projection_diagnostics_name_actual_tool_and_available_fields_without_source_data() {
    let registry = gents_local_mcp::Registry::new();
    let args = serde_json::json!({"left_rows":[],"right_rows":[],"left_key":"item","right_key":"code","kind":"left","columns":[{"side":"left","field":"item","as":"item"}]});
    let error = registry
        .validate_projection("table_join", &args, "/groups", "json")
        .unwrap_err()
        .to_string();
    assert!(error.contains("native table_join"));
    assert!(error.contains("rows"));
    assert!(error.contains("unmatched_left_rows"));
    assert!(!error.contains("exposes groups"));
}

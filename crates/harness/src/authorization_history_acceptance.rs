use super::compact_legacy_file_write_history;
use serde_json::{json, Value};
fn row() -> Value {
    json!({"actor":"Engineer Gent","step":17,"action":"files","arguments":{"op":"write","path":"code/module.py","content":"α".repeat(4096),"expected_sha256":"keep-me"},"result":{"ok":true},"result_excerpt":"old quoted body ".repeat(400),"full_result":"tool-000017.json","capability_revision":"unchanged","other":{"retain":true}})
}
#[test]
fn shrinks_only_old_body_and_excerpt_preserving_identity_and_unknown_status() {
    let original = row();
    let mut rows = vec![original.clone()];
    compact_legacy_file_write_history(&mut rows);
    let out = &rows[0];
    assert!(out.to_string().len() < original.to_string().len());
    assert!(out["arguments"].get("content").is_none());
    assert!(out.get("result_excerpt").is_none());
    for key in [
        "actor",
        "step",
        "action",
        "result",
        "full_result",
        "capability_revision",
        "other",
    ] {
        assert_eq!(out[key], original[key]);
    }
    assert_eq!(out["arguments"]["path"], original["arguments"]["path"]);
    assert_eq!(out["arguments"]["expected_sha256"], "keep-me");
    let view = &out["legacy_history_view"];
    assert_eq!(view["kind"], "legacy_write_body_omitted");
    assert_eq!(view["content_bytes"], 8192);
    assert_eq!(
        view["content_sha256"],
        gents_local_mcp::digest(
            original["arguments"]["content"]
                .as_str()
                .unwrap()
                .as_bytes()
        )
    );
    assert_eq!(
        view["excerpt_sha256"],
        gents_local_mcp::digest(original["result_excerpt"].as_str().unwrap().as_bytes())
    );
    assert_eq!(
        view["excerpt_bytes"],
        original["result_excerpt"].as_str().unwrap().len()
    );
    assert_eq!(
        view["original_record_sha256"],
        gents_local_mcp::digest(original.to_string().as_bytes())
    );
    assert_eq!(view["authority_granted"], false);
    assert_eq!(view["receipt_verified"], false);
    assert!(out["result"].get("mcp_receipt").is_none());
    assert!(view["scope"].is_string());
    let first = rows.clone();
    compact_legacy_file_write_history(&mut rows);
    assert_eq!(rows, first);
}
#[test]
fn malformed_unrecognized_and_receipt_bearing_rows_stay_unchanged() {
    let cases = [
        ("/action", json!("shell")),
        ("/arguments/op", json!("read")),
        ("/arguments/content", Value::Null),
        ("/actor", json!("")),
        ("/actor", Value::Null),
        ("/step", json!(-1)),
        ("/step", json!("17")),
        ("/full_result", json!("../tool-000017.json")),
        ("/full_result", json!("tool-000018.json")),
        ("/result_excerpt", json!({})),
        ("/result", json!({"ok":"true"})),
        ("/result", json!({"ok":true,"mcp_receipt":{}})),
        ("/result", Value::Null),
    ];
    for (path, value) in cases {
        let mut item = row();
        *item.pointer_mut(path).unwrap() = value;
        let mut rows = vec![item.clone()];
        compact_legacy_file_write_history(&mut rows);
        assert_eq!(rows, vec![item], "{path}");
    }
    let mut item = row();
    item["legacy_history_view"] = json!({"do_not_overwrite":true});
    let mut rows = vec![item.clone()];
    compact_legacy_file_write_history(&mut rows);
    assert_eq!(rows, vec![item]);
}
#[test]
fn failed_historical_result_is_not_changed_into_success() {
    let mut item = row();
    item["result"]["ok"] = json!(false);
    let mut rows = vec![item];
    compact_legacy_file_write_history(&mut rows);
    assert_eq!(rows[0]["result"], json!({"ok":false}));
    assert_eq!(rows[0]["legacy_history_view"]["receipt_verified"], false);
}
#[test]
fn small_records_are_not_enlarged() {
    let mut item = row();
    item["arguments"]["content"] = json!("x");
    item["result_excerpt"] = json!("y");
    let mut rows = vec![item.clone()];
    compact_legacy_file_write_history(&mut rows);
    assert_eq!(rows, vec![item]);
}

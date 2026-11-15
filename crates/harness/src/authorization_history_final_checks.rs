use super::compact_legacy_file_write_history;
use serde_json::json;
#[test]
fn varied_steps_unicode_and_nested_preconditions_survive_and_second_pass_is_exact() {
    for step in [0u64, 8, 41, 999999, 1000001, u64::MAX] {
        for ok in [true, false] {
            let input = json!({"actor":"作者 Gent","step":step,"action":"files","arguments":{"op":"write","path":"nested/φ.rs","content":"🧪 café λ".repeat(1000),"preconditions":{"tags":[1,true,null],"mode":"retain"}},"result":{"ok":ok},"result_excerpt":"","full_result":format!("tool-{step:06}.json"),"caller_extra":["preserve",17]});
            let mut rows = vec![input.clone()];
            compact_legacy_file_write_history(&mut rows);
            assert!(rows[0].to_string().len() < input.to_string().len());
            let mut expected_args = input["arguments"].clone();
            expected_args.as_object_mut().unwrap().remove("content");
            assert_eq!(rows[0]["arguments"], expected_args);
            for key in [
                "actor",
                "step",
                "action",
                "result",
                "full_result",
                "caller_extra",
            ] {
                assert_eq!(rows[0][key], input[key]);
            }
            assert_eq!(rows[0]["legacy_history_view"]["receipt_verified"], false);
            assert_eq!(rows[0]["legacy_history_view"]["authority_granted"], false);
            let prior = rows.clone();
            compact_legacy_file_write_history(&mut rows);
            assert_eq!(rows, prior);
        }
    }
}
#[test]
fn absent_or_non_string_excerpt_and_any_existing_view_are_not_reinterpreted() {
    let base = json!({"actor":"Engineer","step":2,"action":"files","arguments":{"op":"write","path":"x.rs","content":"data".repeat(4000)},"result":{"ok":true},"result_excerpt":"excerpt".repeat(1000),"full_result":"tool-000002.json"});
    let mut items = vec![];
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("result_excerpt");
    items.push(missing);
    for wrong in [json!(false), json!([]), json!(null), json!(12)] {
        let mut item = base.clone();
        item["result_excerpt"] = wrong;
        items.push(item);
    }
    for marker in [
        json!(null),
        json!(false),
        json!("already a view"),
        json!({}),
    ] {
        let mut item = base.clone();
        item["legacy_history_view"] = marker;
        items.push(item);
    }
    let original = items.clone();
    compact_legacy_file_write_history(&mut items);
    assert_eq!(items, original);
}

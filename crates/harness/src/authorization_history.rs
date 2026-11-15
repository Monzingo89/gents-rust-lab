//! Compact legacy prompt history without creating receipt or permission authority.
use serde_json::{json, Value};

pub fn compact_legacy_file_write_history(rows: &mut [Value]) {
    for row in rows.iter_mut() {
        let original = row.clone();

        let eligible = original
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|action| action == "files")
            && original
                .get("arguments")
                .and_then(Value::as_object)
                .is_some_and(|arguments| {
                    arguments
                        .get("op")
                        .and_then(Value::as_str)
                        .is_some_and(|op| op == "write")
                        && arguments.get("content").and_then(Value::as_str).is_some()
                })
            && original
                .get("actor")
                .and_then(Value::as_str)
                .is_some_and(|actor| !actor.is_empty())
            && original.get("step").and_then(Value::as_u64).is_some()
            && original
                .get("full_result")
                .and_then(Value::as_str)
                .is_some_and(|full_result| {
                    original
                        .get("step")
                        .and_then(Value::as_u64)
                        .is_some_and(|step| full_result == format!("tool-{:06}.json", step))
                })
            && original
                .get("result")
                .and_then(Value::as_object)
                .is_some_and(|result| {
                    result.len() == 1 && result.get("ok").and_then(Value::as_bool).is_some()
                })
            && original
                .get("result_excerpt")
                .and_then(Value::as_str)
                .is_some()
            && original.get("legacy_history_view").is_none();

        if !eligible {
            continue;
        }

        let mut compacted = original.clone();

        let content = compacted["arguments"]["content"]
            .as_str()
            .expect("eligibility verified content is a string")
            .to_string();
        let excerpt = compacted["result_excerpt"]
            .as_str()
            .expect("eligibility verified excerpt is a string")
            .to_string();

        let content_bytes = content.len();
        let excerpt_bytes = excerpt.len();
        let content_sha256 = gents_local_mcp::digest(content.as_bytes());
        let excerpt_sha256 = gents_local_mcp::digest(excerpt.as_bytes());
        let original_record_sha256 = gents_local_mcp::digest(original.to_string().as_bytes());

        if let Some(arguments) = compacted
            .get_mut("arguments")
            .and_then(Value::as_object_mut)
        {
            arguments.remove("content");
        }
        compacted.as_object_mut().unwrap().remove("result_excerpt");

        compacted["legacy_history_view"] = json!({
            "kind": "legacy_write_body_omitted",
            "content_sha256": content_sha256,
            "content_bytes": content_bytes,
            "excerpt_sha256": excerpt_sha256,
            "excerpt_bytes": excerpt_bytes,
            "original_record_sha256": original_record_sha256,
            "authority_granted": false,
            "receipt_verified": false,
            "scope": "historical unverified compact checkpoint view only; full journal retained; no current file state, execution/readback/test/authorization claim"
        });

        if compacted.to_string().len() < original.to_string().len() {
            *row = compacted;
        }
    }
}
#[cfg(test)]
#[path = "authorization_history_acceptance.rs"]
mod acceptance;
#[cfg(test)]
#[path = "authorization_history_final_checks.rs"]
mod final_checks;

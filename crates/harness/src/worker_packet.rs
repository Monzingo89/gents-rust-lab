//! Bounded public context views. Durable requests and frozen requirements stay intact.
use serde_json::{json, Value};

pub fn deduplicate_goals(packet: &mut Value) {
    for (duplicate, primary) in [
        ("original_goal", "task"),
        ("original_definition_of_done", "criteria"),
    ] {
        if packet.get(duplicate).is_some()
            && packet.get(primary).is_some()
            && packet[duplicate] == packet[primary]
        {
            packet.as_object_mut().unwrap().remove(duplicate);
            packet["goal_aliases"][duplicate] = json!(format!("/{primary}"));
        }
    }
}

pub fn blocked_request(proposal: &Value) -> Value {
    let mut args = proposal["arguments"].clone();
    let mut redacted = false;
    if let Some(content) = args["content"].as_str().filter(|text| text.len() > 256) {
        let hash = gents_local_mcp::digest(content.as_bytes());
        let bytes = content.len();
        let fields = args.as_object_mut().unwrap();
        fields.remove("content");
        fields.insert("content_sha256".into(), json!(hash));
        fields.insert("content_bytes".into(), json!(bytes));
        redacted = true;
    }
    let mut view = json!({"action":proposal["action"],"arguments":args});
    if redacted {
        view["arguments_sha256"] = json!(gents_local_mcp::digest(
            proposal["arguments"].to_string().as_bytes()
        ));
        view["non_executable_summary"] = json!(true);
    }
    view
}

/// Scoped recovery already receives granted capabilities in the runtime envelope
/// and native completion facts. Do not duplicate legacy catalogs/activity indexes.
pub fn compact_scoped_recovery(packet: &mut Value) {
    let fields = packet.as_object_mut().unwrap();
    fields.remove("tool_catalog");
    fields.remove("verified_progress");
    if fields.get("operator_supplied_facts") == Some(&json!([])) {
        fields.remove("operator_supplied_facts");
    }
    fields.remove("delivery_watch");
    packet["capability_reference"]=json!("Use role_capabilities in the runtime envelope; native_checks are observations, not authority.");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_goal_duplicates_are_aliased_without_truncating_or_changing_requirements() {
        let task = "complete requirement ".repeat(1000);
        let mut packet = json!({"task":task,"original_goal":task,"criteria":["all checks"],"original_definition_of_done":["all checks"]});
        deduplicate_goals(&mut packet);
        assert_eq!(packet["task"], task);
        assert_eq!(packet["criteria"], json!(["all checks"]));
        assert_eq!(packet["goal_aliases"]["original_goal"], "/task");
        assert!(packet.get("original_goal").is_none());
        let mut stage = json!({"task":"stage","original_goal":"whole goal","criteria":["stage"],"original_definition_of_done":["whole"]});
        let before = stage.clone();
        deduplicate_goals(&mut stage);
        assert_eq!(stage, before);
    }
    #[test]
    fn blocked_source_bodies_are_hashed_only_in_the_prompt_view() {
        let source = "old source body ".repeat(100);
        let proposal = json!({"action":"files","arguments":{"op":"write","path":"candidate.rs","content":source}});
        let view = blocked_request(&proposal);
        assert!(view["arguments"].get("content").is_none());
        assert_eq!(
            view["arguments"]["content_sha256"],
            gents_local_mcp::digest(source.as_bytes())
        );
        assert_eq!(view["arguments"]["path"], "candidate.rs");
        assert_eq!(proposal["arguments"]["content"], source);
        assert!(!view.to_string().contains("old source body"));
        assert_eq!(
            blocked_request(&json!({"action":"resources","arguments":{}}))["arguments"],
            json!({})
        );
    }
    #[test]
    fn scoped_recovery_keeps_requirements_and_canonical_failures_without_duplicate_indexes() {
        let mut packet = json!({"task":"frozen task","native_checks":{"passed":false,"diagnostics":"compiler failure"},"recent_evidence":["actual result"],"tool_catalog":{"files":"duplicate"},"verified_progress":{"old":"index"},"operator_supplied_facts":[],"delivery_watch":{},"operator_answers":["frozen answer"]});
        packet["proposal_feedback"] = json!([{"executed":false,"proposed_source_policy":{"passed":false,"diagnostic":"syntax rejection"}}]);
        let feedback = packet["proposal_feedback"].clone();
        compact_scoped_recovery(&mut packet);
        assert_eq!(packet["proposal_feedback"], feedback);
        assert_eq!(packet["task"], "frozen task");
        assert_eq!(packet["native_checks"]["passed"], false);
        assert_eq!(packet["recent_evidence"], json!(["actual result"]));
        assert_eq!(packet["operator_answers"], json!(["frozen answer"]));
        assert!(packet.get("tool_catalog").is_none());
        assert!(packet.get("verified_progress").is_none());
    }
}

//! Bounded public context views. Durable requests and frozen requirements stay intact.
use serde_json::{json, Value};

/// Preserve complete bounded native file pages as structured evidence. A raw
/// JSON string prefix can cut the source mid-line and hide its next offset.
/// Other large tool payloads retain the existing journal/excerpt behavior.
pub fn journal_view(record: &Value, journal: &str) -> Value {
    let result = &record["result"]["result"];
    let complete_page = record["action"] == "files"
        && record["arguments"]["op"] == "read"
        && record["result"]["ok"] == true
        && result["text"].as_str().is_some_and(|text| {
            text.len() <= 8192 && result["bytes"].as_u64() == Some(text.len() as u64)
        });
    let summary = record.to_string();
    if complete_page || summary.len() <= 4000 {
        let mut view = record.clone();
        view["full_result"] = json!(journal);
        return view;
    }
    json!({"actor":record["actor"],"ceo_stage":record["ceo_stage"],"step":record["step"],
        "action":record["action"],"arguments":record["arguments"],"result":{"ok":record["result"]["ok"]},
        "capability_revision":record["capability_revision"],"verification_identity":record["verification_identity"],
        "result_excerpt":summary.chars().take(3500).collect::<String>(),"full_result":journal})
}

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

/// Rejected source-bound calls are feedback for their exact prior identity,
/// never a path-wide blacklist that would prevent checking repaired bytes.
pub fn stateful_rejections(history: &[Value], stage: Option<u64>) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    history.iter().rev().filter_map(|item| {
        let proposal=item.get("rejected_proposal")?;
        let identity=proposal["verification_identity"].as_str()?;
        if stage.is_some_and(|s|proposal["ceo_stage"].as_u64()!=Some(s)) {return None;}
        let mut view=blocked_request(proposal);
        view["verification_identity"]=json!(identity);
        view["actor"]=proposal["actor"].clone();
        view["scope"]=json!("Rejected for this source/program identity and these arguments only. Changed source bytes or a distinct read page must be evaluated afresh; grants and native guards still apply.");
        seen.insert(view.to_string()).then_some(view)
    }).take(8).collect()
}

/// Current, explicitly read Python source pages are required task material,
/// rather than optional conversation history. Do not read new files here.
pub fn source_pages(history: &[Value], diagnostics: &Value, actor: &str) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut bytes = 0;
    history
        .iter()
        .rev()
        .filter_map(|item| {
            let result = &item["result"]["result"];
            let path = item["arguments"]["path"].as_str()?;
            let text = result["text"].as_str()?;
            if item["actor"] != actor
                || item["action"] != "files"
                || item["arguments"]["op"] != "read"
                || item["result"]["ok"] != true
                || text.len() > 8192
                || result["bytes"].as_u64() != Some(text.len() as u64)
                || !diagnostics.as_array()?.iter().any(|view| {
                    view["current_source_available"] == true
                        && view["policy_revision"]
                            == item["result"]["mcp_receipt"]["policy_revision"]
                        && view["current_source_custody"][path]["source_sha256"].is_string()
                        && view["current_source_custody"][path]["source_sha256"] == result["sha256"]
                })
                || !seen.insert(json!([path, result["offset_bytes"], result["sha256"]]).to_string())
                || bytes + text.len() > 16384
            {
                return None;
            }
            bytes += text.len();
            Some(item.clone())
        })
        .take(8)
        .collect()
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
    #[test]
    fn current_source_pages_survive_optional_history_compaction_and_exclude_stale_bytes() {
        let page = json!({"actor":"Author","action":"files","arguments":{"op":"read","path":"a.py"},"result":{"ok":true,"mcp_receipt":{"policy_revision":2},"result":{"text":"α".repeat(2500),"bytes":5000,"sha256":"current","offset_bytes":0,"next_offset_bytes":5000}}});
        let diagnostic = json!([{"current_source_available":true,"policy_revision":2,"current_source_custody":{"a.py":{"source_sha256":"current"}}}]);
        let mut old = page.clone();
        old["result"]["result"]["sha256"] = json!("old");
        let pages = super::source_pages(&[old, page.clone(), page.clone()], &diagnostic, "Author");
        assert_eq!(pages, vec![page.clone()]);
        assert!(super::source_pages(&[page], &diagnostic, "Other").is_empty());
        let packet = json!({"task_input":{"task":"Repair exactly this source","source_pages":pages,"recent_evidence":["optional".repeat(1000)]}});
        let trimmed: Value = serde_json::from_str(
            &crate::context::trim_optional(&packet.to_string(), 1000).unwrap(),
        )
        .unwrap();
        assert_eq!(
            trimmed["task_input"]["source_pages"],
            packet["task_input"]["source_pages"]
        );
    }
    #[test]
    fn stateful_rejections_keep_changed_identity_and_page_separate_without_cross_stage_feedback() {
        let a = serde_json::json!({"rejected_proposal":{"actor":"Author","ceo_stage":1,"action":"files","arguments":{"op":"read","path":"a.py","offset_bytes":0},"verification_identity":"old"}});
        let mut b = a.clone();
        b["rejected_proposal"]["verification_identity"] = serde_json::json!("new");
        let mut c = b.clone();
        c["rejected_proposal"]["arguments"]["offset_bytes"] = serde_json::json!(8192);
        let history = vec![
            a.clone(),
            a,
            b,
            c,
            serde_json::json!({"rejected_proposal":{"action":"shell","arguments":{"command":"python"}}}),
        ];
        let feedback = super::stateful_rejections(&history, Some(1));
        assert_eq!(feedback.len(), 3);
        assert_eq!(feedback[0]["arguments"]["offset_bytes"], 8192);
        assert!(super::stateful_rejections(&history, Some(2)).is_empty());
    }
    #[test]
    fn native_file_pages_keep_exact_source_offsets_and_receipts_without_string_clipping() {
        let text = format!("{}\nassert final_line", "α".repeat(3500));
        let record = serde_json::json!({"actor":"Author","ceo_stage":2,"action":"files","arguments":{"op":"read","path":"tests.py"},
            "result":{"ok":true,"result":{"text":text,"bytes":text.len(),"offset_bytes":8191,"next_offset_bytes":8191+text.len(),"truncated":true,"sha256":"a".repeat(64)},"mcp_receipt":{"status":"completed","result_sha256":"b".repeat(64)}}});
        let original = record.clone();
        let view = super::journal_view(&record, "tool-000003.json");
        assert_eq!(view["result"], record["result"]);
        assert_eq!(view["ceo_stage"], 2);
        assert!(view.get("result_excerpt").is_none());
        assert_eq!(view["full_result"], "tool-000003.json");
        assert_eq!(record, original);
        for (action, ok) in [("python_check", true), ("files", false)] {
            let mut other = record.clone();
            other["action"] = serde_json::json!(action);
            other["result"]["ok"] = serde_json::json!(ok);
            let bounded = super::journal_view(&other, "tool-000003.json");
            assert!(bounded["result_excerpt"].as_str().unwrap().chars().count() <= 3500);
            assert!(bounded["result"].get("result").is_none());
        }
    }
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

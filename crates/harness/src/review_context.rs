//! Completion reviewers receive execution observations, never earlier verdicts
//! as fresh evidence. Canonical current-byte gates remain separate authority.
use serde_json::Value;

fn superseded_rust_check(item: &Value, native: Option<&Value>) -> bool {
    if item["action"] != "rust_verify" {
        return false;
    }
    let Some(native) = native else { return false };
    let result = &item["result"]["result"];
    let receipt = &item["result"]["mcp_receipt"];
    let hash = |v: &Value| {
        v.as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    };
    native["rust_checks"].as_array().is_some_and(|checks| {
        checks.iter().any(|check| {
            check["passed"] == true
                && hash(&check["current_source_sha256"])
                && hash(&result["source_sha256"])
                && check["current_source_sha256"] != result["source_sha256"]
                && check["actor"].is_string()
                && check["actor"] == item["actor"]
                && receipt["actor"] == item["actor"]
                && receipt["status"] == "completed"
                && receipt["tool"] == "rust_verify"
                && native["policy_revision"].is_u64()
                && receipt["policy_revision"] == native["policy_revision"]
                && check["source_path"].is_string()
                && check["source_path"] == result["source_path"]
                && hash(&check["tests_sha256"])
                && check["tests_sha256"] == result["tests_sha256"]
        })
    })
}

pub fn tool_evidence(history: &[Value], native: Option<&Value>) -> Vec<Value> {
    let mut evidence = history
        .iter()
        .rev()
        .filter(|item| {
            (item.get("result").is_some() || item.get("result_excerpt").is_some())
                && item.get("qa_revision_required").is_none()
                && !superseded_rust_check(item, native)
        })
        .take(4)
        .cloned()
        .collect::<Vec<_>>();
    evidence.reverse();
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn check(source: char, passed: bool) -> Value {
        json!({"action":"rust_verify","actor":"Author","result":{"ok":true,"mcp_receipt":{"actor":"Author","tool":"rust_verify","status":"completed","policy_revision":4},"result":{"source_path":"candidate.rs","source_sha256":source.to_string().repeat(64),"tests_sha256":"c".repeat(64),"passed":passed}}})
    }
    fn native() -> Value {
        json!({"policy_revision":4,"rust_checks":[{"actor":"Author","source_path":"candidate.rs","tests_sha256":"c".repeat(64),"current_source_sha256":"b".repeat(64),"passed":true}]})
    }
    #[test]
    fn current_pass_replaces_other_source_failure_in_review_packet_without_editing_history() {
        let history = vec![
            check('a', false),
            check('b', true),
            json!({"qa_revision_required":{"summary":"old test failure"}}),
            json!({"worker_assessment":{"claim":"all done"}}),
        ];
        let original = history.clone();
        assert_eq!(
            tool_evidence(&history, Some(&native())),
            vec![check('b', true)]
        );
        assert_eq!(history, original);
    }
    #[test]
    fn missing_mismatched_or_failed_canonical_gate_never_suppresses_execution_failure() {
        let history = vec![check('a', false), check('b', true)];
        let mut gates = vec![Value::Null, native(), native(), native(), native()];
        gates[1]["rust_checks"][0]["passed"] = json!(false);
        gates[2]["rust_checks"][0]["tests_sha256"] = json!("d".repeat(64));
        gates[3]["rust_checks"][0]["actor"] = json!("Other");
        gates[4]["policy_revision"] = json!(5);
        for gate in gates {
            assert_eq!(tool_evidence(&history, Some(&gate)), history);
        }
        assert_eq!(tool_evidence(&history, None), history);
        let same_bytes = vec![check('b', false), check('b', true)];
        assert_eq!(tool_evidence(&same_bytes, Some(&native())), same_bytes);
    }
    #[test]
    fn newest_actual_tool_failures_and_bounded_order_are_preserved() {
        let history = (0..7)
            .map(|i| json!({"action":"files","result_excerpt":{"ok":false,"id":i}}))
            .collect::<Vec<_>>();
        assert_eq!(tool_evidence(&history, Some(&native())), history[3..]);
    }
}

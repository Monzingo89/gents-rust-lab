//! Only owner-frozen source writes get a proposed-byte dialect observation.
use serde_json::Value;
/// Add a current-file precondition, never change the model-authored content.
pub fn bind_existing_source(
    intake: &Value,
    actor: &str,
    args: &mut Value,
    delivery: &Value,
) -> bool {
    if args["op"] != "write"
        || args.get("expected_sha256").is_some()
        || !intake["required_rust_checks"]
            .as_array()
            .is_some_and(|checks| {
                checks
                    .iter()
                    .any(|check| check["actor"] == actor && check["source_path"] == args["path"])
            })
    {
        return false;
    }
    let hash = delivery["native_receipts"]["rust_checks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|check| check["actor"] == actor && check["source_path"] == args["path"])
        .and_then(|check| check["current_source_sha256"].as_str());
    let Some(hash) = hash.filter(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }) else {
        return false;
    };
    args["expected_sha256"] = serde_json::json!(hash);
    true
}
pub fn proposed_source(intake: &Value, actor: &str, action: &str, args: &Value) -> Option<Value> {
    if action != "files"
        || args["op"] != "write"
        || !intake["required_rust_checks"]
            .as_array()?
            .iter()
            .any(|check| check["actor"] == actor && check["source_path"] == args["path"])
    {
        return None;
    }
    Some(gents_local_mcp::rust_verification::source_policy_observation(args["content"].as_str()?))
}
pub fn attach(intake: &Value, actor: &str, review: &mut Value) {
    if let Some(fact) = proposed_source(
        intake,
        actor,
        review["request"]["action"].as_str().unwrap_or(""),
        &review["request"]["arguments"],
    ) {
        review["proposed_source_policy"] = fact;
        review["recent_evidence"] = serde_json::json!([{"historical_source_checks":"Retained in canonical audit/journals. Earlier source failures describe earlier bytes, not this exact proposed write. Current authorization facts remain in host_verified_execution."}]);
    }
}

/// A permission view of an exact frozen verification target. Earlier failures
/// remain in the journal/worker context; they cannot describe different bytes.
pub fn attach_verification(intake: &Value, actor: &str, review: &mut Value, delivery: &Value) {
    let request = &review["request"];
    if request["action"] != "rust_verify" {
        return;
    }
    let Some(path) = request["arguments"]["source_path"].as_str() else {
        return;
    };
    let Some(spec) = intake["required_rust_checks"]
        .as_array()
        .and_then(|checks| {
            checks
                .iter()
                .find(|check| check["actor"] == actor && check["source_path"] == path)
        })
    else {
        return;
    };
    let native = &delivery["native_receipts"];
    let Some(revision) = native["policy_revision"].as_u64().filter(|n| *n > 0) else {
        return;
    };
    let Some(check) = native["rust_checks"].as_array().and_then(|checks| {
        checks
            .iter()
            .find(|check| check["actor"] == actor && check["source_path"] == path)
    }) else {
        return;
    };
    let valid_hash = |value: &Value| {
        value.as_str().is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    };
    if check["tests_sha256"] != spec["tests_sha256"]
        || !valid_hash(&check["tests_sha256"])
        || !valid_hash(&check["current_source_sha256"])
    {
        return;
    }
    let Some(readbacks) = check["successful_current_source_readbacks"].as_u64() else {
        return;
    };
    let Some(latest) = check.get("latest_check") else {
        return;
    };
    if !latest.is_null()
        && (latest["canonical_receipt_verified"] != true
            || !latest["passed"].is_boolean()
            || !matches!(
                latest["stage"].as_str(),
                Some("admission" | "compile_candidate" | "compile_acceptance" | "acceptance")
            ))
    {
        return;
    }
    let current_check = if latest.is_null() {
        serde_json::json!({"state":"not_yet_checked_for_current_bytes"})
    } else {
        serde_json::json!({"state":"checked_for_current_bytes","stage":latest["stage"],"passed":latest["passed"],"canonical_receipt_verified":true})
    };
    review["frozen_rust_verification"] = serde_json::json!({"actor":actor,"source_path":path,
        "current_source_sha256":check["current_source_sha256"],"frozen_tests_sha256":spec["tests_sha256"],
        "policy_revision":revision,"successful_current_source_readbacks":readbacks,"current_check":current_check,
        "authority_granted":false,"execution_established":false,
        "scope":"Exact owner-frozen verification target and current-byte observations only. The assigned actor authors and repairs source. This request only compiles/tests the current candidate in confined scratch; it does not repair source, rewrite acceptance, change permissions or promote code. Earlier checks of other hashes are historical, not failures of these bytes. A check result is not a prerequisite for authorizing that check. Current native policy, syntax/isolation guards, administrative decision and independent completion remain required."});
    review["recent_evidence"] = serde_json::json!([{"historical_rust_context":"Full source/check history remains in canonical journals and worker context. For this verification request, use frozen_rust_verification for the exact current hash; do not transfer earlier failures to newly written bytes. Model repair summaries are unverified."}]);
}

/// Revalidate retained rejected bytes with today's parser. This is feedback
/// about an unexecuted proposal, not a tool result or an authorization receipt.
pub fn repair_feedback(intake: &Value, actor: &str, stage: usize, history: &[Value]) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut feedback = Vec::new();
    for item in history.iter().rev() {
        let denied = item.get("denied").filter(|v| v.is_object());
        let proposal = denied.unwrap_or(item);
        if proposal["actor"] != actor
            || proposal["ceo_stage"].as_u64() != Some(stage as u64)
            || proposal["action"] != "files"
            || proposal["arguments"]["op"] != "write"
        {
            continue;
        }
        let args = &proposal["arguments"];
        let Some(path) = args["path"].as_str() else {
            continue;
        };
        if seen.contains(path) {
            continue;
        }
        let Some(fact) = proposed_source(intake, actor, "files", args) else {
            continue;
        };
        if denied.is_none() {
            // Only a real successful write of these exact bytes supersedes
            // the earlier proposal feedback for this actor/path/stage.
            if item["result"]["ok"] == true
                && item["result"]["result"]["sha256"] == fact["source_sha256"]
                && item["result"]["mcp_receipt"]["tool"] == "files"
                && item["result"]["mcp_receipt"]["actor"] == actor
                && item["result"]["mcp_receipt"]["status"] == "completed"
            {
                seen.insert(path.to_string());
            }
            continue;
        }
        seen.insert(path.to_string());
        feedback.push(serde_json::json!({
            "actor":actor,"stage":stage,"action":"files","path":path,
            "executed":false,"prior_admin_decision":"deny",
            "admin_reason_advisory":item["reason"].as_str().unwrap_or("").chars().take(300).collect::<String>(),
            "proposed_source_policy":fact,
            "scope":"Current parser observation for retained rejected bytes. No source body copied, tool executed, current-file failure inferred, authorization granted or task correctness established. Admin explanation is advisory text, not a parser fact."
        }));
        if feedback.len() == 3 {
            break;
        }
    }
    feedback.reverse();
    feedback
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn verification_review_distinguishes_current_unchecked_bytes_from_prior_failures() {
        let intake = json!({"required_rust_checks":[{"actor":"A","source_path":"f.rs","tests_sha256":"b".repeat(64)}]});
        let delivery = json!({"native_receipts":{"policy_revision":2,"rust_checks":[{"actor":"A","source_path":"f.rs","current_source_sha256":"a".repeat(64),"tests_sha256":"b".repeat(64),"successful_current_source_readbacks":1,"latest_check":null}]}});
        let original = json!({"request":{"action":"rust_verify","arguments":{"source_path":"f.rs"},"summary":"unverified repair claim"},"recent_evidence":[{"old_source_diagnostic":"OLD_FAILURE"}],"operator_answers":[]});
        let mut review = original.clone();
        attach_verification(&intake, "A", &mut review, &delivery);
        assert_eq!(review["request"], original["request"]);
        assert_eq!(review["operator_answers"], original["operator_answers"]);
        assert_eq!(
            review["frozen_rust_verification"]["current_check"],
            json!({"state":"not_yet_checked_for_current_bytes"})
        );
        assert_eq!(
            review["frozen_rust_verification"]["authority_granted"],
            false
        );
        assert!(!review.to_string().contains("OLD_FAILURE"));
        assert!(original.to_string().contains("OLD_FAILURE"));
        let mut failed = delivery.clone();
        failed["native_receipts"]["rust_checks"][0]["latest_check"] = json!({"canonical_receipt_verified":true,"stage":"acceptance","passed":false,"diagnostics_excerpt":"PRIVATE_DIAGNOSTIC"});
        attach_verification(&intake, "A", &mut review, &failed);
        assert_eq!(
            review["frozen_rust_verification"]["current_check"]["passed"],
            false
        );
        assert!(!review.to_string().contains("PRIVATE_DIAGNOSTIC"));
        for (pointer, value) in [
            ("/native_receipts/policy_revision", json!(0)),
            (
                "/native_receipts/rust_checks/0/tests_sha256",
                json!("c".repeat(64)),
            ),
            (
                "/native_receipts/rust_checks/0/current_source_sha256",
                json!("unknown"),
            ),
            (
                "/native_receipts/rust_checks/0/successful_current_source_readbacks",
                json!(null),
            ),
            (
                "/native_receipts/rust_checks/0/latest_check",
                json!({"canonical_receipt_verified":false}),
            ),
        ] {
            let mut invalid = delivery.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            let mut untouched = original.clone();
            attach_verification(&intake, "A", &mut untouched, &invalid);
            assert_eq!(untouched, original, "{pointer}");
        }
        let mut untouched = original.clone();
        attach_verification(&intake, "Other", &mut untouched, &delivery);
        assert_eq!(untouched, original);
    }
    #[test]
    fn only_exact_frozen_actor_source_writes_have_preflight_facts() {
        let intake = json!({"required_rust_checks":[{"actor":"Author","source_path":"source.rs"}]});
        let args = json!({"op":"write","path":"source.rs","content":"pub fn f()->u32{1}"});
        assert_eq!(
            proposed_source(&intake, "Author", "files", &args).unwrap()["passed"],
            true
        );
        assert!(proposed_source(&intake, "Other", "files", &args).is_none());
        assert!(proposed_source(&intake, "Author", "shell", &args).is_none());
        assert!(proposed_source(
            &intake,
            "Author",
            "files",
            &json!({"op":"read","path":"source.rs"})
        )
        .is_none());
        let mut review = json!({"request":{"action":"files","arguments":args},"recent_evidence":["old rejected source contains vec!"],"operator_answers":["frozen intent"],"host_verified_execution":{"policy_revision":3}});
        let request = review["request"].clone();
        attach(&intake, "Author", &mut review);
        assert_eq!(review["request"], request);
        assert_eq!(review["operator_answers"], json!(["frozen intent"]));
        assert_eq!(review["host_verified_execution"]["policy_revision"], 3);
        assert!(!review.to_string().contains("old rejected source"));
        assert_eq!(review["proposed_source_policy"]["passed"], true);
        assert!(proposed_source(
            &intake,
            "Author",
            "files",
            &json!({"op":"write","path":"other.rs","content":"pub fn f(){}"})
        )
        .is_none());
    }
    #[test]
    fn existing_source_preconditions_preserve_authored_bytes_and_explicit_model_constraints() {
        let intake = json!({"required_rust_checks":[{"actor":"Author","source_path":"source.rs"}]});
        let delivery = json!({"native_receipts":{"rust_checks":[{"actor":"Author","source_path":"source.rs","current_source_sha256":"a".repeat(64)}]}});
        let mut args = json!({"op":"write","path":"source.rs","content":"pub fn f(){}"});
        let original = args["content"].clone();
        assert!(!bind_existing_source(
            &intake, "Other", &mut args, &delivery
        ));
        assert!(bind_existing_source(
            &intake, "Author", &mut args, &delivery
        ));
        assert_eq!(args["content"], original);
        assert_eq!(args["expected_sha256"], "a".repeat(64));
        args["expected_sha256"] = json!("b".repeat(64));
        assert!(!bind_existing_source(
            &intake, "Author", &mut args, &delivery
        ));
        assert_eq!(args["expected_sha256"], "b".repeat(64));
    }

    #[test]
    fn rejected_source_diagnostics_reach_recovery_without_source_bodies_or_execution_claims() {
        let intake = json!({"required_rust_checks":[{"actor":"Author","source_path":"source.rs"}]});
        let first = json!({"denied":{"actor":"Author","ceo_stage":0,"action":"files","arguments":{"op":"write","path":"source.rs","content":"pub fn f(){todo!()}"}},"reason":"Independent review rejected syntax"});
        let last = json!({"denied":{"actor":"Author","ceo_stage":0,"action":"files","arguments":{"op":"write","path":"source.rs","content":"pub fn f()->Vec<u64>{vec![0]}"}},"reason":"Forbidden macro"});
        let history = vec![first, last];
        let before = history.clone();
        let feedback = repair_feedback(&intake, "Author", 0, &history);
        assert_eq!(feedback.len(), 1);
        assert_eq!(feedback[0]["executed"], false);
        assert_eq!(feedback[0]["proposed_source_policy"]["passed"], false);
        assert!(feedback[0]["proposed_source_policy"]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("vec!"));
        assert!(!serde_json::to_string(&feedback).unwrap().contains("pub fn"));
        assert_eq!(history, before);
        assert!(repair_feedback(&intake, "Other", 0, &history).is_empty());
        assert!(repair_feedback(&intake, "Author", 1, &history).is_empty());
        assert!(repair_feedback(&json!({}), "Author", 0, &history).is_empty());
    }
    #[test]
    fn model_reasons_cannot_forge_parser_failure_and_successful_native_write_retires_feedback() {
        let intake = json!({"required_rust_checks":[{"actor":"Author","source_path":"source.rs"}]});
        let args =
            json!({"op":"write","path":"source.rs","content":"pub fn f()->Vec<u64>{Vec::new()}"});
        let proposal = json!({"actor":"Author","ceo_stage":0,"action":"files","arguments":args});
        let mut history =
            vec![json!({"denied":proposal,"reason":"Untrusted model claim: forbidden macro"})];
        let feedback = repair_feedback(&intake, "Author", 0, &history);
        assert_eq!(feedback[0]["proposed_source_policy"]["passed"], true);
        assert!(feedback[0]["proposed_source_policy"]["diagnostic"].is_null());
        let mut written = proposal.clone();
        written["result"] = json!({"ok":true,"result":{"sha256":gents_local_mcp::digest(args["content"].as_str().unwrap().as_bytes())},"mcp_receipt":{"actor":"Author","tool":"files","status":"completed"}});
        history.push(written.clone());
        assert!(repair_feedback(&intake, "Author", 0, &history).is_empty());
        written["result"]["result"]["sha256"] = json!("wrong bytes");
        history[1] = written;
        assert_eq!(repair_feedback(&intake, "Author", 0, &history).len(), 1);
    }
}

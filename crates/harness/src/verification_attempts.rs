//! Retry accounting for stateful verification, independent from write replay.
use serde_json::{json, Value};

/// Caller supplies a freshly computed native identity and canonical delivery
/// view. History is only an executed-identity witness, never acceptance proof.
/// Return a rejection observation, never permission or an alternative action.
pub fn unchanged_acceptance_failure(
    history: &[Value],
    actor: &str,
    identity: &str,
    path: &str,
    delivery: &Value,
) -> Option<Value> {
    let native = &delivery["native_receipts"];
    let revision = native["policy_revision"].as_u64().filter(|n| *n > 0)?;
    let check = native["rust_checks"]
        .as_array()?
        .iter()
        .find(|check| check["actor"] == actor && check["source_path"] == path)?;
    if check["latest_check"]["canonical_receipt_verified"] != true
        || check["latest_check"]["stage"] != "acceptance"
        || check["latest_check"]["passed"] != false
        || !history.iter().any(|row| {
            row["action"] == "rust_verify"
                && row["actor"] == actor
                && row["arguments"]["source_path"] == path
                && row["verification_identity"] == identity
                && row["result"]["ok"] == true
        })
    {
        return None;
    }
    for field in ["current_source_sha256", "tests_sha256"] {
        let hash = check[field].as_str()?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
    }
    Some(
        json!({"actor":actor,"source_path":path,"source_sha256":check["current_source_sha256"],
        "tests_sha256":check["tests_sha256"],"policy_revision":revision,"verification_identity":identity,
        "prior_native_acceptance_passed":false,"executed":false,"authority_granted":false,
        "scope":"Current source and frozen acceptance bytes match a canonical failed acceptance under this verification identity. A model repair summary is not a source write. No tool, owner answer, permission change or acceptance is established."}),
    )
}

pub fn count(history: &[Value], action: &str, actor: &str, identity: &str) -> usize {
    history
        .iter()
        .filter_map(|item| {
            if item.get("action").is_some() {
                Some(item)
            } else {
                item.get("rejected_proposal")
            }
        })
        .filter(|item| {
            item["action"] == action
                && item["actor"] == actor
                && item["verification_identity"] == identity
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn failed_acceptance_requires_materialized_change_not_a_repair_claim() {
        let history = vec![
            json!({"action":"rust_verify","actor":"A","arguments":{"source_path":"f.rs"},"verification_identity":"current","result":{"ok":true}}),
        ];
        let delivery = json!({"native_receipts":{"policy_revision":2,"rust_checks":[{"actor":"A","source_path":"f.rs","current_source_sha256":"a".repeat(64),"tests_sha256":"b".repeat(64),"latest_check":{"canonical_receipt_verified":true,"stage":"acceptance","passed":false}}]}});
        let observed =
            unchanged_acceptance_failure(&history, "A", "current", "f.rs", &delivery).unwrap();
        assert_eq!(observed["executed"], false);
        assert_eq!(observed["authority_granted"], false);
        assert_eq!(observed["source_sha256"], "a".repeat(64));
        // A new source/policy/verifier identity permits a fresh reviewed check.
        assert!(
            unchanged_acceptance_failure(&history, "A", "changed", "f.rs", &delivery).is_none()
        );
        assert!(
            unchanged_acceptance_failure(&history, "B", "current", "f.rs", &delivery).is_none()
        );
        assert!(
            unchanged_acceptance_failure(&history, "A", "current", "other.rs", &delivery).is_none()
        );
        for (pointer, value) in [
            ("/native_receipts/policy_revision", json!(0)),
            (
                "/native_receipts/rust_checks/0/latest_check/canonical_receipt_verified",
                json!(false),
            ),
            (
                "/native_receipts/rust_checks/0/latest_check/stage",
                json!("compile_candidate"),
            ),
            (
                "/native_receipts/rust_checks/0/latest_check/passed",
                json!(true),
            ),
            (
                "/native_receipts/rust_checks/0/current_source_sha256",
                json!("unknown"),
            ),
            ("/native_receipts/rust_checks/0/tests_sha256", json!(null)),
        ] {
            let mut changed = delivery.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(
                unchanged_acceptance_failure(&history, "A", "current", "f.rs", &changed).is_none(),
                "{pointer}"
            );
        }
        let denied = vec![json!({"rejected_proposal":history[0]})];
        assert!(unchanged_acceptance_failure(&denied, "A", "current", "f.rs", &delivery).is_none());
        let mut unexecuted = history.clone();
        unexecuted[0]["result"]["ok"] = json!(false);
        assert!(
            unchanged_acceptance_failure(&unexecuted, "A", "current", "f.rs", &delivery).is_none()
        );
    }
    #[test]
    fn changed_bytes_policy_or_verifier_renew_checks_without_erasing_history() {
        let history = vec![
            json!({"action":"rust_verify","actor":"worker","result":{"ok":true}}),
            json!({"action":"rust_verify","actor":"worker","verification_identity":"old"}),
            json!({"action":"rust_verify","actor":"worker","verification_identity":"current"}),
            json!({"rejected_proposal":{"action":"rust_verify","actor":"worker","verification_identity":"current"}}),
            json!({"action":"files","actor":"worker","verification_identity":"current"}),
        ];
        assert_eq!(count(&history, "rust_verify", "worker", "current"), 2);
        assert_eq!(count(&history, "rust_verify", "worker", "new"), 0);
        assert_eq!(count(&history, "rust_verify", "other", "current"), 0);
        assert_eq!(count(&history, "files", "worker", "current"), 1);
        assert_eq!(history.len(), 5);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn real_native_failed_bytes_are_rejected_and_actual_changed_bytes_can_pass() {
        use gents_local_mcp::authority::Authority;
        let mut native = crate::native_fixture::Fixture::new();
        native.scope.expected_revision = Some(2);
        let db = native.db();
        let mut policy = gents_local_mcp::governance::load(&db, &native.scope.run.to_string())
            .unwrap()
            .unwrap();
        policy.revision = 2;
        for grants in [&mut policy.ceilings, &mut policy.grants] {
            grants.get_mut("Worker").unwrap().tools.extend([
                "files:read".into(),
                "files:write".into(),
                "rust_verify".into(),
            ]);
        }
        gents_local_mcp::governance::save(&db, &native.scope.run.to_string(), &policy, 1).unwrap();
        // Protocol fixture only, unrelated to the company's authoring benchmark.
        let tests = "#[test]fn check(){std::assert_eq!(candidate::f(),1);}";
        let intake = json!({"required_rust_checks":[{"actor":"Worker","source_path":"f.rs","tests_path":"acceptance.rs","tests_sha256":gents_local_mcp::digest(tests.as_bytes())}]});
        db.execute("UPDATE jobs SET intake=?1", [intake.to_string()])
            .unwrap();
        let workspace = native.authority.workspace(&native.scope).unwrap();
        gents_local_mcp::workspace::open(&workspace, true)
            .unwrap()
            .write("acceptance.rs", tests)
            .unwrap();
        let registry = gents_local_mcp::Registry::new();
        let written = registry
            .call(
                &native.authority,
                &native.scope,
                "files",
                json!({"op":"write","path":"f.rs","content":"pub fn f()->u32{0}"}),
            )
            .unwrap();
        registry
            .call(
                &native.authority,
                &native.scope,
                "files",
                json!({"op":"read","path":"f.rs"}),
            )
            .unwrap();
        let identity = gents_local_mcp::rust_verification::request_identity(
            &native.authority,
            &native.scope,
            "f.rs",
        )
        .unwrap();
        let checked = registry
            .call(
                &native.authority,
                &native.scope,
                "rust_verify",
                json!({"source_path":"f.rs"}),
            )
            .unwrap();
        assert_eq!(checked["value"]["passed"], false);
        assert_eq!(checked["value"]["stage"], "acceptance");
        let history = vec![
            json!({"actor":"Worker","action":"rust_verify","arguments":{"source_path":"f.rs"},"verification_identity":identity,"result":{"ok":true}}),
            json!({"summary":"Unverified claim: repaired already"}),
        ];
        let gate = gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap();
        let mut review = json!({"request":{"action":"rust_verify","arguments":{"source_path":"f.rs"}},"recent_evidence":["historical diagnostics"]});
        crate::rust_review::attach_verification(
            &intake,
            "Worker",
            &mut review,
            &json!({"native_receipts":gate}),
        );
        assert_eq!(
            review["frozen_rust_verification"]["current_check"]["passed"],
            false
        );
        assert!(unchanged_acceptance_failure(
            &history,
            "Worker",
            &identity,
            "f.rs",
            &json!({"native_receipts":gate})
        )
        .is_some());
        let calls: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(calls, 3); // Observing the rejection executed no extra check.
        registry.call(&native.authority,&native.scope,"files",json!({"op":"write","path":"f.rs","content":"pub fn f()->u32{1}","expected_sha256":written["value"]["sha256"]})).unwrap();
        let changed = gents_local_mcp::rust_verification::request_identity(
            &native.authority,
            &native.scope,
            "f.rs",
        )
        .unwrap();
        assert_ne!(identity, changed);
        let gate = gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap();
        assert!(unchanged_acceptance_failure(
            &history,
            "Worker",
            &changed,
            "f.rs",
            &json!({"native_receipts":gate})
        )
        .is_none());
        registry
            .call(
                &native.authority,
                &native.scope,
                "files",
                json!({"op":"read","path":"f.rs"}),
            )
            .unwrap();
        let gate = gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap();
        crate::rust_review::attach_verification(
            &intake,
            "Worker",
            &mut review,
            &json!({"native_receipts":gate}),
        );
        assert_eq!(
            review["frozen_rust_verification"]["current_check"]["state"],
            "not_yet_checked_for_current_bytes"
        );
        assert_eq!(
            review["frozen_rust_verification"]["successful_current_source_readbacks"],
            1
        );
        assert_eq!(
            review["frozen_rust_verification"]["authority_granted"],
            false
        );
        let checked = registry
            .call(
                &native.authority,
                &native.scope,
                "rust_verify",
                json!({"source_path":"f.rs"}),
            )
            .unwrap();
        assert_eq!(checked["value"]["passed"], true);
        assert_eq!(
            gents_local_mcp::completion::check(&native.authority, &native.scope).unwrap()["passed"],
            true
        );
    }
}

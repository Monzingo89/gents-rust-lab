//! Permission-review projection of an already owner-approved frozen job.
//! The caller owns admission/approval validation. This view creates no authority.
use serde_json::{json, Value};

pub fn review(task: &str, criteria: &[String], intake: &Value, answers: Value) -> Value {
    let plan = &intake["ceo_plan"];
    let completed = plan["status"] == "ready"
        && plan["assignments"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        && intake["ceo_planning_id"]
            .as_str()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        && intake["ceo_planning_revision"]
            .as_u64()
            .is_some_and(|n| n > 0)
        && intake["ceo_plan_sha256"] == gents_local_mcp::digest(plan.to_string().as_bytes());
    json!({"request_admission":task,"frozen_definition_of_done":criteria,
        "frozen_ceo_plan":plan,"frozen_native_bindings":intake["artifact_bindings"],
        "frozen_rust_checks":intake["required_rust_checks"],
        "frozen_owned_rust_calls":intake["required_owned_rust_calls"],
        "frozen_planning_phase":{"completed":completed,
            "phase":if completed {"execution_admission_after_owner_review"} else {"scope_admission_review"},
            "session":intake["ceo_planning_id"],"revision":intake["ceo_planning_revision"],
            "plan_sha256":intake["ceo_plan_sha256"],"authority_granted":false,
            "scope":"Retained planning phase and frozen scope only. Earlier requests to clarify belong to planning; concrete new missing intent or safety information may still require a question. This observation grants no permission, executes nothing and establishes no task completion."},
        "authority":"caller-validated owner-approved frozen intake and installed policy; typed planning repairs are diagnostics, not task requirements",
        "operator_answers":answers})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_planning_does_not_drop_confirmed_criteria_or_create_permission() {
        let plan = json!({"status":"ready","assignments":[{"actor":"A"}]});
        let intake = json!({"ceo_plan":plan,"ceo_plan_sha256":gents_local_mcp::digest(plan.to_string().as_bytes()),"ceo_planning_id":uuid::Uuid::new_v4(),"ceo_planning_revision":3,"artifact_bindings":[],"required_rust_checks":[{"actor":"A","source_path":"f.rs","tests_sha256":"a".repeat(64)}]});
        let criteria = vec!["Confirmed exact completion criterion".to_owned()];
        let answers = json!(["retained real owner answer"]);
        let view = review(
            "Earlier request: clarify before planning",
            &criteria,
            &intake,
            answers.clone(),
        );
        assert_eq!(view["frozen_planning_phase"]["completed"], true);
        assert_eq!(view["frozen_planning_phase"]["authority_granted"], false);
        assert_eq!(view["frozen_definition_of_done"], json!(criteria));
        assert_eq!(view["frozen_rust_checks"], intake["required_rust_checks"]);
        assert_eq!(view["operator_answers"], answers);
        for (field, value) in [
            ("ceo_plan_sha256", json!("forged")),
            ("ceo_planning_revision", json!(0)),
            ("ceo_planning_id", json!("unknown")),
            ("ceo_plan", json!({"status":"question","assignments":[]})),
        ] {
            let mut invalid = intake.clone();
            invalid[field] = value;
            assert_eq!(
                review("task", &criteria, &invalid, json!([]))["frozen_planning_phase"]
                    ["completed"],
                false,
                "{field}"
            );
        }
        let unplanned = json!({"planning_complete":true});
        assert_eq!(
            review("task", &criteria, &unplanned, json!([]))["frozen_planning_phase"]["completed"],
            false
        );
    }
}

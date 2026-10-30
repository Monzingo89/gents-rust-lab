use serde_json::{json, Value};
use std::collections::BTreeMap;

pub trait TraceProjector {
    fn project(
        &self,
        run_id: &str,
        status: &str,
        classification: &Value,
        events: &[Value],
    ) -> Value;
}

pub struct LocalTraceProjector;

impl TraceProjector for LocalTraceProjector {
    fn project(
        &self,
        run_id: &str,
        status: &str,
        classification: &Value,
        events: &[Value],
    ) -> Value {
        let mut calls: BTreeMap<String, Value> = BTreeMap::new();
        let mut handoffs = Vec::new();
        let mut issues = Vec::new();
        let mut feedback = Vec::new();
        for event in events {
            let kind = event["kind"].as_str().unwrap_or_default();
            if let Some(call_id) = event["call_id"].as_str() {
                let span = calls.entry(call_id.to_owned()).or_insert_with(|| {
                    json!({"call_id":call_id,"agent":event["agent"],"role":event["role"],"model":event["model"],"started_at_ms":Value::Null,"finished_at_ms":Value::Null,"state":"unknown","latency_ms":Value::Null})
                });
                if kind == "call_started" {
                    span["started_at_ms"] = event["timestamp_ms"].clone();
                    span["state"] = json!("running");
                } else if kind == "call_completed" {
                    span["finished_at_ms"] = event["timestamp_ms"].clone();
                    span["state"] = json!("completed");
                    span["latency_ms"] = event["reply"]["latency_ms"].clone();
                } else if kind == "call_failed" {
                    span["finished_at_ms"] = event["timestamp_ms"].clone();
                    span["state"] = json!("failed");
                }
            }
            if matches!(
                kind,
                "context_retrieved" | "context_staged" | "all_hands_acknowledged"
            ) {
                handoffs.push(event.clone());
            }
            if matches!(
                kind,
                "call_failed"
                    | "failed"
                    | "blocked"
                    | "timed_out"
                    | "acceptance_veto"
                    | "admin_denied"
                    | "sensitive_data_blocked"
            ) {
                issues.push(event.clone());
            }
            if matches!(
                kind,
                "repair_requested"
                    | "resource_recovery"
                    | "loop_recovery"
                    | "loop_replanning"
                    | "question_resolved_locally"
                    | "question_resolved_by_openai"
                    | "codex_phone_ringing"
                    | "codex_phone_unanswered"
                    | "cloud_escalation_failed"
                    | "executive_recovery_decision"
                    | "executive_recovery_retry"
                    | "retry_scheduled"
            ) {
                feedback.push(event.clone());
            }
        }
        json!({
            "version":1,
            "run_id":run_id,
            "status":status,
            "calls":calls.into_values().collect::<Vec<_>>(),
            "context_handoffs":handoffs,
            "issues":issues,
            "feedback_loops":feedback,
            "assurance":{
                "data_classification":classification,
                "sensitive_data_mode":"deny_raw_identifiers",
                "inference_boundary":"local_models_plus_bounded_openai_question_escalation_when_configured",
                "event_payload_policy":"metadata_only_for_context_handoffs",
                "compliance_status":"controls_enforced_not_certified",
                "frameworks":["HIPAA Security Rule readiness","PCI DSS scope avoidance","privacy and least privilege"]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_calls_handoffs_issues_and_feedback() {
        let events = vec![
            json!({"call_id":"one","kind":"call_started","agent":"A","role":"worker","model":"local","timestamp_ms":1}),
            json!({"call_id":"one","kind":"call_failed","agent":"A","role":"worker","model":"local","timestamp_ms":2}),
            json!({"kind":"context_retrieved","agent":"A","timestamp_ms":3}),
            json!({"kind":"resource_recovery","agent":"Operations Gent","timestamp_ms":4}),
        ];
        let result =
            LocalTraceProjector.project("run", "running", &json!({"level":"standard"}), &events);
        assert_eq!(result["calls"][0]["state"], "failed");
        assert_eq!(result["context_handoffs"].as_array().unwrap().len(), 1);
        assert_eq!(result["issues"].as_array().unwrap().len(), 1);
        assert_eq!(result["feedback_loops"].as_array().unwrap().len(), 1);
    }
}

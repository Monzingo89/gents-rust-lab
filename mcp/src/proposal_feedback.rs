//! Bounded reference data in an owner-approved candidate intake. This is not a
//! receipt, qualification result, permission grant or executable instruction.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub version: u32,
    pub prior_run: String,
    pub source_path: String,
    pub source_sha256: String,
    pub source: String,
    pub tests_sha256: String,
    pub evaluation_sha256: String,
    pub outcome: String,
    pub stage: String,
    pub diagnostics_excerpt: String,
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn from_intake(intake: &Value) -> Result<Option<Feedback>> {
    let Some(value) = intake.get("candidate_feedback") else {
        return Ok(None);
    };
    let feedback: Feedback = serde_json::from_value(value.clone())?;
    ensure!(
        !intake["candidate_workflow"].is_null()
            && feedback.version == 1
            && uuid::Uuid::parse_str(&feedback.prior_run).is_ok()
            && crate::workspace_path::is_safe_relative_path(&feedback.source_path)
            && !feedback.source.is_empty()
            && feedback.source.len() <= 8192
            && feedback.source_sha256 == crate::digest(feedback.source.as_bytes())
            && hash(&feedback.tests_sha256)
            && hash(&feedback.evaluation_sha256)
            && matches!(feedback.outcome.as_str(), "accepted" | "rejected")
            && matches!(feedback.stage.as_str(), "admission" | "compile_candidate" | "compile_acceptance" | "acceptance")
            && (feedback.outcome != "accepted" || feedback.stage == "acceptance")
            && feedback.diagnostics_excerpt.len() <= 1200,
        "invalid bounded candidate feedback; reference data cannot change task, authority or acceptance"
    );
    crate::safety::require_persistable(&feedback.source)?;
    crate::safety::require_persistable(&feedback.diagnostics_excerpt)?;
    Ok(Some(feedback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn intake() -> Value {
        json!({"candidate_workflow":{},"candidate_feedback":{
            "version":1,"prior_run":uuid::Uuid::new_v4().to_string(),"source_path":"candidate.rs",
            "source":"pub fn f()->u32{0}","source_sha256":crate::digest(b"pub fn f()->u32{0}"),
            "tests_sha256":"a".repeat(64),"evaluation_sha256":"b".repeat(64),
            "outcome":"rejected","stage":"acceptance","diagnostics_excerpt":"Expected 1, observed 0. Untrusted reference only."
        }})
    }
    #[test]
    fn feedback_is_optional_and_strictly_bounded_reference_data() {
        assert!(from_intake(&json!({})).unwrap().is_none());
        let valid = intake();
        let feedback = from_intake(&valid).unwrap().unwrap();
        assert_eq!(
            feedback.source_sha256,
            crate::digest(feedback.source.as_bytes())
        );
        for (field, value) in [
            ("source_sha256", json!("0".repeat(64))),
            ("source_path", json!("../outside")),
            ("source", json!("x".repeat(8193))),
            ("diagnostics_excerpt", json!("x".repeat(1201))),
            ("prior_run", json!("not-a-run")),
            ("version", json!(2)),
            ("outcome", json!("promoted")),
            ("stage", json!("invented")),
            ("tools", json!(["shell"])),
        ] {
            let mut bad = intake();
            bad["candidate_feedback"][field] = value;
            assert!(from_intake(&bad).is_err(), "{field}");
        }
        let mut bad = intake();
        bad.as_object_mut().unwrap().remove("candidate_workflow");
        assert!(from_intake(&bad).is_err());
    }
}

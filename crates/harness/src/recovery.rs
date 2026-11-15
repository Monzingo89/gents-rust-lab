//! Recovery policy consumes observations; it cannot authorize tools or certify outputs.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ARTIFACT_STALL_STEPS: usize = 12;

/// A successful transport call or source edit is not successful Rust acceptance.
pub fn resets_rust_recovery(action: &str, result: &Value) -> bool {
    action == "rust_verify" && result["ok"] == true && result["result"]["passed"] == true
}

#[derive(Default, Serialize, Deserialize)]
pub struct DeliveryWatch {
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub last_change_ms: u64,
    #[serde(default)]
    pub tools_without_change: usize,
    #[serde(default)]
    pub last_recovery_step: usize,
}

impl DeliveryWatch {
    pub fn observe(&mut self, fingerprint: String, now: u64) -> bool {
        let changed = self.fingerprint != fingerprint;
        if changed || self.last_change_ms == 0 {
            self.fingerprint = fingerprint;
            self.last_change_ms = now;
            self.tools_without_change = 0;
        } else {
            self.tools_without_change = self.tools_without_change.saturating_add(1);
        }
        changed
    }

    pub fn needs_recovery(&mut self, step: usize) -> bool {
        if self.tools_without_change >= ARTIFACT_STALL_STEPS
            && step.saturating_sub(self.last_recovery_step) >= ARTIFACT_STALL_STEPS
        {
            self.last_recovery_step = step;
            return true;
        }
        false
    }
}

/// Migrate old synthetic operator answers without treating model advice as consent.
pub fn is_advisory(item: &Value) -> bool {
    [
        "local_executive_escalation",
        "openai_codex_escalation",
        "autonomous_resolution",
    ]
    .iter()
    .any(|key| item.get(key).is_some())
}

/// Only the explicit preview workflow is gated here. Other questions still go
/// through authorization review, including questions about missing authority.
pub fn premature_preview(question: &str, has_authored_preview: bool) -> bool {
    let text = question.to_ascii_lowercase();
    !has_authored_preview
        && (text.contains("first build step") || text.contains("first-step"))
        && (text.contains("preview") || text.contains("review"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rust_recovery_survives_edits_reads_and_failed_native_checks() {
        for action in ["files", "resources", "rust_verify"] {
            assert!(!resets_rust_recovery(
                action,
                &json!({"ok":true,"result":{"passed":false}})
            ));
        }
        assert!(!resets_rust_recovery(
            "files",
            &json!({"ok":true,"result":{"passed":true}})
        ));
        assert!(!resets_rust_recovery(
            "rust_verify",
            &json!({"ok":false,"result":{"passed":true}})
        ));
        assert!(resets_rust_recovery(
            "rust_verify",
            &json!({"ok":true,"result":{"passed":true}})
        ));
    }

    #[test]
    fn reads_do_not_hide_stalls_and_real_changes_reset_them() {
        let mut watch = DeliveryWatch::default();
        watch.observe("empty".into(), 100);
        for step in 1..12 {
            assert!(!watch.observe("empty".into(), 100 + step as u64));
            assert!(!watch.needs_recovery(step));
        }
        watch.observe("empty".into(), 112);
        assert!(watch.needs_recovery(12));
        assert!(!watch.needs_recovery(13));
        assert_eq!(watch.last_change_ms, 100);
        assert!(watch.observe("source-v1".into(), 120));
        assert_eq!(watch.tools_without_change, 0);
        assert!(!watch.needs_recovery(24));
        let restored: DeliveryWatch =
            serde_json::from_value(serde_json::to_value(watch).unwrap()).unwrap();
        assert_eq!(restored.last_change_ms, 120);
    }

    #[test]
    fn model_answers_are_not_operator_authority() {
        assert!(is_advisory(
            &json!({"local_executive_escalation":{},"operator_answer":"Proceed"})
        ));
        assert!(!is_advisory(&json!({"operator_answer":"Proceed"})));
        let question = "Proceed with creating the first build step preview?";
        assert!(premature_preview(question, false));
        assert!(!premature_preview(question, true));
        assert!(!premature_preview("May I purchase a license?", false));
    }
}

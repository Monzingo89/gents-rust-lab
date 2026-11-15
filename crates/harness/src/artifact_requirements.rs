//! Owner-frozen output names, separate from prose discovery and file checking.
//! This describes obligations only; native receipts and QA still establish them.
use anyhow::Result;
use serde_json::Value;

pub fn named_paths(intake: &Value, actor: &str, final_stage: bool) -> Result<Option<Vec<String>>> {
    let bindings = gents_local_mcp::artifact_bindings::parse(intake)?;
    if bindings.is_empty() {
        return Ok(None); // Unstructured jobs retain the legacy discovery path.
    }
    Ok(Some(
        bindings
            .into_iter()
            .filter(|binding| !binding.receipt_only && (final_stage || binding.writer == actor))
            .map(|binding| binding.path)
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn stage_scope_keeps_exact_owner_paths_without_inventing_template_files() {
        let binding = |path: &str, writer: &str| json!({"path":path,"source_actor":"Analyst","writer":writer,"source_tool":"audio_inspect","arguments":{"path":"source.wav","expected_sha256":"0".repeat(64)},"pointer":"/frame_count","encoding":"integer_text"});
        let intake = json!({"artifact_bindings":[binding("Session_frames.txt","Writer"),binding("final.json","Integrator")]});
        assert_eq!(
            named_paths(&intake, "Analyst", false).unwrap(),
            Some(vec![])
        );
        assert_eq!(
            named_paths(&intake, "Writer", false).unwrap(),
            Some(vec!["Session_frames.txt".into()])
        );
        assert_eq!(
            named_paths(&intake, "Integrator", true).unwrap(),
            Some(vec!["Session_frames.txt".into(), "final.json".into()])
        );
        assert_eq!(named_paths(&json!({}), "Writer", true).unwrap(), None);
        let mut invalid = intake;
        invalid["artifact_bindings"][0]["path"] = json!("../escape.json");
        assert!(named_paths(&invalid, "Writer", true).is_err());
    }
}

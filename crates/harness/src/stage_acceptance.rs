//! Bounded structural stage-scope validation; no semantic truth judgment.
use crate::ceo_team::Stage;
use anyhow::{ensure, Result};
use gents_local_mcp::artifact_bindings::Binding;

pub fn validate(stages: &[Stage], bindings: &[Binding]) -> Result<()> {
    for (index, stage) in stages.iter().enumerate() {
        for binding in bindings.iter().filter(|binding| !binding.receipt_only) {
            let Some(writer) = stages.iter().position(|s| s.actor == binding.writer) else {
                continue; // The caller separately rejects unknown actors.
            };
            if index < writer {
                let path = binding.path.to_lowercase();
                ensure!(
                    stage
                        .acceptance
                        .iter()
                        .all(|criterion| !criterion.to_lowercase().contains(&path)),
                    "stage acceptance references a later actor's publication; keep criteria scoped to the current stage"
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn stages() -> Vec<Stage> {
        serde_json::from_value(json!([
            {"actor":"Compute","task":"Compute","tools":["integer_arithmetic"],"acceptance":["Actual native result available"]},
            {"actor":"Publish","task":"Save","tools":["files:write","files:read"],"acceptance":["Final.json matches the native result and readback"]}
        ])).unwrap()
    }
    fn binding(receipt_only: bool) -> Binding {
        serde_json::from_value(json!({"path":"Final.json","source_actor":"Compute","writer":if receipt_only{"Compute"}else{"Publish"},"source_tool":"integer_arithmetic","arguments":{"operation":"sum","values":[1,2]},"pointer":"/value","encoding":"json","receipt_only":receipt_only})).unwrap()
    }
    #[test]
    fn rejects_observed_premature_publication_criterion_without_rewriting_it() {
        let mut stages = stages();
        stages[0]
            .acceptance
            .push("The FINAL.JSON file must contain canonical output".into());
        let original = stages[0].acceptance.clone();
        assert!(validate(&stages, &[binding(false)]).is_err());
        assert_eq!(stages[0].acceptance, original);
    }
    #[test]
    fn current_writer_and_receipt_only_nodes_are_not_future_publication() {
        validate(&stages(), &[binding(false)]).unwrap();
        let mut stages = stages();
        stages[0]
            .acceptance
            .push("Final.json is a native receipt identity".into());
        validate(&stages, &[binding(true)]).unwrap();
        let mut same_stage = binding(false);
        same_stage.writer = "Compute".into();
        validate(&stages, &[same_stage]).unwrap();
    }
    #[test]
    fn validates_all_earlier_stages_and_has_explicit_semantic_limits() {
        let mut stages = stages();
        let mut intermediate = stages[0].clone();
        intermediate.actor = "Intermediate".into();
        intermediate.acceptance = vec!["Final.json has been saved".into()];
        stages.insert(1, intermediate);
        assert!(validate(&stages, &[binding(false)]).is_err());
        stages[1].acceptance = vec!["Native operation passed".into()];
        // This guard cannot prove arbitrary natural-language claims true.
        stages[0].acceptance = vec!["An unnamed future file exists".into()];
        validate(&stages, &[binding(false)]).unwrap();
    }
}

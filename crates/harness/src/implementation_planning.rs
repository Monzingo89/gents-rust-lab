//! Pure draft normalization for implementation workflows, never authorization.
//! Keep source-authoring concerns separate from native data-flow contracts.
use anyhow::{Context, Result};
use serde_json::{json, Value};

pub fn compile_readbacks(proposal: &Value, mode: &Value) -> Result<(Value, Vec<Value>)> {
    if mode != "implementation" || proposal["status"] != "ready" {
        return Ok((proposal.clone(), vec![]));
    }
    let mut compiled = proposal.clone();
    let mut additions = vec![];
    for stage in compiled["assignments"]
        .as_array_mut()
        .context("implementation staffing required")?
    {
        let actor = stage["actor"].clone();
        let tools = stage["tools"]
            .as_array_mut()
            .context("implementation tools required")?;
        // The CEO must select the actual author and verifier capabilities.
        // This derives only their read-only draft readback, before owner review.
        if tools.contains(&json!("files:write"))
            && tools.contains(&json!("rust_verify"))
            && !tools.contains(&json!("files:read"))
        {
            tools.push(json!("files:read"));
            additions.push(json!({"actor":actor,"tool":"files:read",
                "reason":"source author selected writing and native verification; current-byte readback required; draft only, owner review and approval still required"}));
        }
    }
    Ok((compiled, additions))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_selected_source_authors_gain_a_visible_read_only_draft_operation() {
        let raw = json!({"status":"ready","assignments":[
            {"actor":"Author","tools":["files:write","rust_verify"]},
            {"actor":"Reviewer","tools":["rust_verify"]},
            {"actor":"Writer","tools":["files:write"]}]});
        let (draft, additions) = compile_readbacks(&raw, &json!("implementation")).unwrap();
        assert_eq!(
            raw["assignments"][0]["tools"],
            json!(["files:write", "rust_verify"])
        );
        assert_eq!(
            draft["assignments"][0]["tools"],
            json!(["files:write", "rust_verify", "files:read"])
        );
        assert_eq!(draft["assignments"][1], raw["assignments"][1]);
        assert_eq!(draft["assignments"][2], raw["assignments"][2]);
        assert_eq!(additions.len(), 1);
        assert_eq!(additions[0]["actor"], "Author");
        assert_eq!(compile_readbacks(&raw, &Value::Null).unwrap().0, raw);
        assert!(compile_readbacks(&draft, &json!("implementation"))
            .unwrap()
            .1
            .is_empty());
        let question = json!({"status":"question","assignments":[]});
        assert_eq!(
            compile_readbacks(&question, &json!("implementation"))
                .unwrap()
                .0,
            question
        );
    }
}

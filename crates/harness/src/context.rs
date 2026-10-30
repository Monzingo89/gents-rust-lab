//! Portable chat representation and loss-aware compaction. Required task,
//! criteria, exact requests and verified handoffs are never string-truncated.
use anyhow::{bail, Result};
use serde_json::{json, Value};

pub fn messages(system: &str, input: &str) -> Value {
    json!([{"role":"system","content":system},{"role":"user","content":input}])
}

/// Compatibility for adapters/fixtures accepting either legacy string payloads
/// or version-two structured envelopes.
pub fn task_input(input: &str) -> Result<Value> {
    let outer: Value = serde_json::from_str(input)?;
    Ok(match &outer["task_input"] {
        Value::String(nested) => serde_json::from_str(nested)?,
        Value::Object(_) => outer["task_input"].clone(),
        _ => outer,
    })
}

pub fn compact(input: &str, max_bytes: usize) -> Result<String> {
    shrink(input, max_bytes, true)
}

/// The tokenizer, rather than a byte estimate, makes the final native budget
/// decision. Return all safely removable context even if a byte target cannot
/// be reached without damaging required fields.
pub fn trim_optional(input: &str, max_bytes: usize) -> Result<String> {
    shrink(input, max_bytes, false)
}

fn shrink(input: &str, max_bytes: usize, enforce_bytes: bool) -> Result<String> {
    if input.len() <= max_bytes {
        return Ok(input.into());
    }
    let mut value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(_) if !enforce_bytes => return Ok(input.into()),
        Err(_) => bail!("required plain-text context exceeds the model budget; save/retrieve smaller evidence chunks"),
    };
    // These are optional/redundant context tiers. The verified ledger remains.
    for pointer in [
        "/task_input/company_context",
        "/company_context/records",
        "/task_input/verified_progress/latest_operations",
        "/task_input/verified_progress/recent_results",
        "/global_practices",
        "/role_skills",
        "/task_input/recent_evidence",
        "/task_input/evidence",
    ] {
        while serde_json::to_vec(&value)?.len() > max_bytes {
            let Some(optional) = value.pointer_mut(pointer) else {
                break;
            };
            match optional {
                Value::Array(rows) if !rows.is_empty() => {
                    rows.remove(0);
                }
                Value::String(text) if !text.is_empty() => text.clear(),
                Value::Object(fields) if !fields.is_empty() => fields.clear(),
                _ => break,
            }
        }
    }
    let compacted = serde_json::to_string(&value)?;
    if enforce_bytes && compacted.len() > max_bytes {
        bail!("required task/tool contract/verified evidence exceeds the model context budget; use smaller evidence pages, not a truncated mission");
    }
    Ok(compacted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_decoding_accepts_both_envelope_versions() {
        let task = json!({"criteria":["same mission"]});
        assert_eq!(
            task_input(&json!({"task_input":task}).to_string()).unwrap(),
            task
        );
        assert_eq!(
            task_input(&json!({"task_input":task.to_string()}).to_string()).unwrap(),
            task
        );
        assert_eq!(task_input(&task.to_string()).unwrap(), task);
    }
    #[test]
    fn native_compaction_returns_safe_reductions_for_actual_token_recount() {
        let input = json!({"task_input":{"task":"required mission ".repeat(300),"recent_evidence":["optional transcript".repeat(1000)]}});
        assert!(compact(&input.to_string(), 1000).is_err());
        let result: Value =
            serde_json::from_str(&trim_optional(&input.to_string(), 1000).unwrap()).unwrap();
        assert_eq!(result["task_input"]["task"], input["task_input"]["task"]);
        assert!(result["task_input"]["recent_evidence"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn useful_latest_observations_outlive_redundant_manuals() {
        let input = json!({"task_input":{"task":"Frozen task","criteria":["done"],"recent_evidence":[{"action":"hf_dataset_files","result":{"pdfs":[{"path":"observed-example.pdf"}]}}]},"role_skills":["old manual".repeat(2000)],"verified_handoff":{"sources":[{"repo":"owner/examples"}]}});
        let compacted: Value =
            serde_json::from_str(&compact(&input.to_string(), 1000).unwrap()).unwrap();
        assert_eq!(
            compacted["task_input"]["recent_evidence"],
            input["task_input"]["recent_evidence"]
        );
        assert_eq!(compacted["verified_handoff"], input["verified_handoff"]);
    }
    #[test]
    fn compaction_retains_exact_task_criteria_request_and_handoff_across_models() {
        let value = json!({"task_input":{"task":"frozen task","criteria":["criterion"],"request":{"action":"files","arguments":{"op":"read","path":"source"}},"recent_evidence":["x".repeat(20_000)]},"verified_handoff":{"sources":[{"repo":"owner/source","revision":"a".repeat(40)}]},"company_context":{"records":["x".repeat(20_000)]}});
        let result: Value =
            serde_json::from_str(&compact(&value.to_string(), 1500).unwrap()).unwrap();
        for pointer in [
            "/task_input/task",
            "/task_input/criteria",
            "/task_input/request",
            "/verified_handoff",
        ] {
            assert_eq!(value.pointer(pointer), result.pointer(pointer));
        }
        assert!(compact(
            &json!({"task_input":{"task":"x".repeat(20_000)}}).to_string(),
            1500
        )
        .is_err());
        assert_eq!(messages("rules", "facts")[0]["role"], "system");
    }
}

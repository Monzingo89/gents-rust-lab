//! Fail-closed admission for host-recorded task and saved-adapter evidence.
//! Local records are not attestations against a malicious same-user process.
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::Path;

pub(super) fn validate_files(work: &Path, metrics: &Value) -> Result<()> {
    let read = |name: &str| -> Result<Value> {
        let path = work.join(name);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() > 1_048_576 {
            bail!("training evaluation must be a bounded regular file");
        }
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    };
    validate(
        metrics,
        &read("task-baseline.json")?,
        &read("task-candidate.json")?,
        &read("task-reloaded.json")?,
    )
}

fn validate(metrics: &Value, before: &Value, after: &Value, reload: &Value) -> Result<()> {
    if metrics["persisted_adapter_reload_matches"] != true {
        bail!("promotion requires saved-adapter reload evidence");
    }
    let cases = |value: &Value| -> Result<Vec<Value>> {
        let rows = value["cases"].as_array().context("task cases missing")?;
        if rows.is_empty() || rows.len() > 64 {
            bail!("task case count outside evaluation bounds");
        }
        Ok(rows.clone())
    };
    let (baseline, candidate, loaded) = (cases(before)?, cases(after)?, cases(reload)?);
    if baseline.len() != candidate.len() || baseline.len() != loaded.len() {
        bail!("evaluation case counts changed");
    }
    let mut old_passes = 0;
    let mut new_passes = 0;
    let mut seen = std::collections::HashSet::new();
    for ((old, new), saved) in baseline.iter().zip(&candidate).zip(&loaded) {
        let hash = old["prompt_sha256"]
            .as_str()
            .context("prompt hash missing")?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || !seen.insert(hash) {
            bail!("invalid or duplicate task identity");
        }
        if new["prompt_sha256"] != hash
            || saved["prompt_sha256"] != hash
            || old["expected"].as_str().is_none()
            || old["expected"] != new["expected"]
            || old["expected"] != saved["expected"]
        {
            bail!("task identity or oracle changed");
        }
        let prior = old["passed"].as_bool().context("baseline score missing")?;
        let current = new["passed"].as_bool().context("candidate score missing")?;
        if prior && !current {
            bail!("observed task regression prohibits promotion");
        }
        if new["output"].as_str().is_none()
            || saved["output"] != new["output"]
            || saved["passed"] != current
        {
            bail!("saved adapter did not reproduce the candidate");
        }
        old_passes += u64::from(prior);
        new_passes += u64::from(current);
    }
    let total = baseline.len() as u64;
    if new_passes <= old_passes {
        bail!("promotion requires measured task improvement, not lower loss alone");
    }
    for (result, count) in [
        (before, old_passes),
        (after, new_passes),
        (reload, new_passes),
    ] {
        if result["passed"] != count || result["total"] != total {
            bail!("task summary contradicts individual cases");
        }
    }
    let comparison = &metrics["task_comparison"];
    if comparison["before_passed"] != old_passes
        || comparison["after_passed"] != new_passes
        || comparison["total"] != total
        || comparison["regressions"] != 0
        || comparison["measured_improvement"] != true
    {
        bail!("training metrics contradict task evidence");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (Value, Value, Value) {
        let before = json!({"cases":[{"prompt_sha256":"a".repeat(64),"expected":"ok","output":"wrong","passed":false}],"passed":0,"total":1});
        let after = json!({"cases":[{"prompt_sha256":"a".repeat(64),"expected":"ok","output":"ok","passed":true}],"passed":1,"total":1});
        let metrics = json!({"persisted_adapter_reload_matches":true,"task_comparison":{"before_passed":0,"after_passed":1,"total":1,"regressions":0,"measured_improvement":true}});
        (metrics, before, after)
    }

    #[test]
    fn positive_evidence_is_only_eligible_for_remaining_owner_checks() {
        let (m, b, a) = fixture();
        validate(&m, &b, &a, &a).unwrap();
    }

    #[test]
    fn regression_cannot_be_hidden_by_claimed_improvement() {
        let (m, b, a) = fixture();
        assert!(validate(&m, &a, &b, &b)
            .unwrap_err()
            .to_string()
            .contains("regression"));
        assert!(validate(&m, &a, &a, &a)
            .unwrap_err()
            .to_string()
            .contains("measured task improvement"));
    }

    #[test]
    fn missing_or_changed_reload_and_oracles_fail_closed() {
        let (mut m, b, mut a) = fixture();
        m["persisted_adapter_reload_matches"] = Value::Null;
        assert!(validate(&m, &b, &a, &a).is_err());
        m["persisted_adapter_reload_matches"] = json!(true);
        a["cases"][0]["expected"] = json!("changed");
        assert!(validate(&m, &b, &a, &a).is_err());
    }
}

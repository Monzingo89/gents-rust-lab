//! Bounded local planning output adaptation from native cutoff receipts only.
use serde_json::{json, Value};

pub fn observe(body: &mut Value) {
    let Some(previous) = body["reply_token_limit"]
        .as_u64()
        .filter(|n| (1024..=4096).contains(n))
    else {
        return;
    };
    if body["status"] != "failed"
        || body["local_execution"]["finish_reason"] != "length"
        || body["local_execution"]["generation_budget"]["final_tokens_reserved"].as_u64()
            != Some(previous)
    {
        return;
    }
    let next = (previous * 2).min(4096);
    body["planning_reply_budget"] = json!(next);
    body["planning_reply_budget_adjustment"] = json!({"reason":"prior_native_output_cutoff","previous_final_tokens":previous,"next_final_tokens":next,"observed_output_tokens":body["usage"]["output"],"scope":"Bounded local output allowance only; context, task, tools, policy and acceptance unchanged. No task execution established."});
}

pub fn limit(first: bool, baseline: u32, retained: &Value) -> u32 {
    if first {
        return 256;
    }
    retained
        .as_u64()
        .map_or(baseline, |n| n.clamp(u64::from(baseline), 4096) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_failed_native_cutoff_with_matching_actual_allowance_can_adapt() {
        let original = json!({"status":"failed","reply_token_limit":2048,"usage":{"output":2048},"local_execution":{"finish_reason":"length","generation_budget":{"final_tokens_reserved":2048}}});
        let mut body = original.clone();
        observe(&mut body);
        assert_eq!(limit(false, 2048, &body["planning_reply_budget"]), 4096);
        for (field, value) in [
            ("status", json!("ready")),
            (
                "local_execution",
                json!({"finish_reason":"stop","generation_budget":{"final_tokens_reserved":2048}}),
            ),
            ("local_execution", json!({"finish_reason":"length"})),
            (
                "local_execution",
                json!({"finish_reason":"length","generation_budget":{"final_tokens_reserved":4096}}),
            ),
            ("reply_token_limit", json!(256)),
        ] {
            let mut body = original.clone();
            body[field] = value;
            observe(&mut body);
            assert!(body.get("planning_reply_budget").is_none());
        }
        assert_eq!(limit(true, 2048, &json!(4096)), 256);
        assert_eq!(limit(false, 2048, &json!(u64::MAX)), 4096);
        assert_eq!(limit(false, 2048, &json!(0)), 2048);
        assert_eq!(limit(false, 1024, &Value::Null), 1024);
        body["reply_token_limit"] = json!(4096);
        body["local_execution"]["generation_budget"]["final_tokens_reserved"] = json!(4096);
        observe(&mut body);
        assert_eq!(body["planning_reply_budget"], 4096);
    }
}

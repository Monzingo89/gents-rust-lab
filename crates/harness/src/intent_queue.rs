//! Durable unexecuted intent selection. This module grants no authority and
//! never retries an effect: execution uses the separate write-ahead pending record.
use anyhow::{ensure, Result};
use serde_json::Value;

pub fn select(queued: &mut Vec<Value>, reviewing: &mut Option<Value>) -> Option<Value> {
    if reviewing.is_none() && !queued.is_empty() {
        *reviewing = Some(queued.remove(0));
    }
    reviewing.clone()
}

pub fn consume(reviewing: &mut Option<Value>, selected: &Value) -> Result<()> {
    ensure!(
        reviewing.as_ref() == Some(selected),
        "review intent changed before disposition; no effect admitted"
    );
    *reviewing = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn checkpoint_roundtrip_keeps_unreviewed_head_and_order_without_admitting_effects() {
        let write = json!({"action":"files","arguments":{"op":"write","path":"fixture.txt","content":"fixture"}});
        let read = json!({"action":"files","arguments":{"op":"read","path":"fixture.txt"}});
        let mut state = crate::autonomy::Checkpoint {
            plan_queue: vec![write.clone(), read.clone()],
            ..Default::default()
        };
        assert_eq!(
            select(&mut state.plan_queue, &mut state.review_intent),
            Some(write.clone())
        );
        // No verdict was obtained. A new process reloads the durable state.
        let mut state: crate::autonomy::Checkpoint =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert_eq!(
            select(&mut state.plan_queue, &mut state.review_intent),
            Some(write.clone())
        );
        assert_eq!(state.plan_queue, vec![read.clone()]);
        assert!(state.pending.is_none());
        assert!(consume(&mut state.review_intent, &read).is_err());
        assert_eq!(state.review_intent, Some(write.clone()));
        // Only the independently authorized selected intent transfers to pending.
        consume(&mut state.review_intent, &write).unwrap();
        state.pending =
            Some(json!({"action":"files","arguments":write["arguments"],"phase":"started"}));
        let state: crate::autonomy::Checkpoint =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(state.review_intent.is_none());
        assert!(state.pending.is_some());
        assert_eq!(state.plan_queue, vec![read]);
        let old: crate::autonomy::Checkpoint =
            serde_json::from_value(json!({"plan_queue":[]})).unwrap();
        assert!(old.review_intent.is_none());
    }
}

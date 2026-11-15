use super::review_context;
use serde_json::json;
#[test]
fn exact_next_request_and_persisted_proposal_survive_shrinking() {
    let historical = json!({"actor":"Engineer Gent","step":17,"action":"files","arguments":{"op":"write","path":"old.py","content":"old".repeat(4096)},"result":{"ok":true},"result_excerpt":"old excerpt".repeat(500),"full_result":"tool-000017.json"});
    for (action, op, should_shrink) in [
        ("files", "write", true),
        ("files", "read", false),
        ("email_send", "write", false),
    ] {
        let request = json!({"action":action,"arguments":{"op":op,"path":"new.py","content":"EXACT NEXT SOURCE".repeat(100)}});
        let proposal = json!({"request":request,"recent_evidence":[historical],"operator_answers":[{"operator_answer":"no downloads"}],"frozen_criteria":["unchanged"]});
        let before = proposal.clone();
        let packet = review_context("exact original task", &proposal).unwrap();
        assert_eq!(proposal, before);
        assert_eq!(packet["operator_task"], "exact original task");
        assert_eq!(packet["proposal"]["request"], request);
        assert_eq!(packet["proposal"]["frozen_criteria"], json!(["unchanged"]));
        assert_eq!(
            packet["proposal"]["recent_evidence"][0]
                .get("legacy_history_view")
                .is_some(),
            should_shrink
        );
    }
}

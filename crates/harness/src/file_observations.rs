//! Historical complete-text write/readback linkage, not authorization or QA.
//! The caller owns the executed journal. These checks validate internal receipt
//! linkage, never an authenticated signature or the file's current state.
use serde_json::{json, Value};

pub fn rename_needs_reconciliation(record: &Value) -> bool {
    record["action"] == "files"
        && record["arguments"]["op"] == "rename"
        && record["result"]["ok"] != true
}
pub fn rename_reconciliation(arguments: &Value, reason: &str) -> Value {
    json!({"source_path":arguments["path"],"destination":arguments["destination"],
        "expected_sha256":arguments["expected_sha256"],"reason":reason,
        "pending_intent_retained":true,"automatic_replay":false,"independent_acceptance":false,
        "instruction":"Inspect BOTH original paths with authorized files read metadata_only=true (or list their parent if absent), reconcile their hashes and the canonical audit, and retain this run. No new move or acceptance was inferred."})
}

fn linked(record: &Value) -> bool {
    let args = &record["arguments"];
    let result = &record["result"];
    let receipt = &result["mcp_receipt"];
    record["action"] == "files"
        && result["ok"] == true
        && record["actor"].is_string()
        && receipt["actor"] == record["actor"]
        && receipt["tool"] == "files"
        && receipt["phase"] == "worker"
        && receipt["status"] == "completed"
        && receipt["provider_cost_usd"] == 0
        && receipt["policy_revision"].as_u64().is_some_and(|n| n > 0)
        && receipt["arguments_sha256"] == gents_local_mcp::digest(args.to_string().as_bytes())
        && receipt["result_sha256"]
            == gents_local_mcp::digest(result["result"].to_string().as_bytes())
}
pub fn written(record: &Value) -> Option<Value> {
    if !linked(record) || record["arguments"]["op"] != "write" {
        return None;
    }
    let args = &record["arguments"];
    let data = &record["result"]["result"];
    let text = args["content"].as_str()?;
    let path = args["path"].as_str()?;
    if data["path"] != path
        || data["bytes"] != text.len()
        || data["sha256"] != gents_local_mcp::digest(text.as_bytes())
    {
        return None;
    }
    Some(
        json!({"path":path,"step":record["step"],"actor":record["actor"],"policy_revision":record["result"]["mcp_receipt"]["policy_revision"],"sha256":data["sha256"],"bytes":text.len(),"state":"written_unverified"}),
    )
}
pub fn readback(record: &Value, write: &Value) -> Option<Value> {
    if !linked(record) || record["arguments"]["op"] != "read" {
        return None;
    }
    let args = &record["arguments"];
    let data = &record["result"]["result"];
    let text = data["text"].as_str()?;
    if data["path"] != args["path"]
        || data["path"] != write["path"]
        || record["actor"] != write["actor"]
        || record["result"]["mcp_receipt"]["policy_revision"] != write["policy_revision"]
        || data["offset_bytes"] != 0
        || data["truncated"] != false
        || !data["next_offset_bytes"].is_null()
        || data["bytes"] != text.len()
        || data["total_bytes"] != text.len()
        || data["sha256"] != write["sha256"]
        || data["sha256"] != gents_local_mcp::digest(text.as_bytes())
    {
        return None;
    }
    let mut observation = write.clone();
    observation["state"] = json!("native_readback_observed");
    observation["readback_step"] = record["step"].clone();
    Some(observation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_reads_do_not_prove_text_readback_and_unconfirmed_moves_need_reconciliation() {
        let mut row = record("read", 1, "source");
        row["result"]["result"] = json!({"path":"result.txt","sha256":gents_local_mcp::digest(b"source"),"bytes":6,"total_bytes":6,"offset_bytes":0,"truncated":false,"next_offset_bytes":null,"metadata_only":true});
        row["result"]["mcp_receipt"]["result_sha256"] = json!(gents_local_mcp::digest(
            row["result"]["result"].to_string().as_bytes()
        ));
        let written = written(&record("write", 0, "source")).unwrap();
        assert!(readback(&row, &written).is_none());
        let failed = json!({"action":"files","arguments":{"op":"rename","path":"old","destination":"new"},"result":{"ok":false}});
        assert!(rename_needs_reconciliation(&failed));
        let mut confirmed = failed.clone();
        confirmed["result"]["ok"] = json!(true);
        assert!(!rename_needs_reconciliation(&confirmed));
        let mut read = failed;
        read["arguments"]["op"] = json!("read");
        assert!(!rename_needs_reconciliation(&read));
    }
    fn record(op: &str, step: u64, text: &str) -> Value {
        let args = if op == "write" {
            json!({"op":op,"path":"result.txt","content":text})
        } else {
            json!({"op":op,"path":"result.txt"})
        };
        let data = if op == "write" {
            json!({"path":"result.txt","bytes":text.len(),"sha256":gents_local_mcp::digest(text.as_bytes())})
        } else {
            json!({"path":"result.txt","text":text,"bytes":text.len(),"total_bytes":text.len(),"offset_bytes":0,"next_offset_bytes":null,"truncated":false,"sha256":gents_local_mcp::digest(text.as_bytes())})
        };
        json!({"step":step,"action":"files","actor":"Writer","arguments":args,"result":{"ok":true,"result":data,"mcp_receipt":{"actor":"Writer","tool":"files","phase":"worker","status":"completed","provider_cost_usd":0,"policy_revision":1,"arguments_sha256":gents_local_mcp::digest(args.to_string().as_bytes()),"result_sha256":gents_local_mcp::digest(data.to_string().as_bytes())}}})
    }
    #[test]
    fn exact_complete_readback_links_to_the_write_without_claiming_correctness() {
        for text in ["", "signed −1"] {
            let write = written(&record("write", 1, text)).unwrap();
            let read = record("read", 2, text);
            let view = readback(&read, &write).unwrap();
            assert_eq!(view["state"], "native_readback_observed");
            assert_eq!(view["readback_step"], 2);
            assert!(view.get("text").is_none());
            assert_eq!(write["state"], "written_unverified");
            let other = written(&record("write", 3, "changed")).unwrap();
            assert!(readback(&read, &other).is_none());
        }
    }
    #[test]
    fn unlinked_failed_partial_other_actor_or_epoch_cannot_become_readback() {
        let write = written(&record("write", 1, "body")).unwrap();
        let whole = written(&record("write", 1, "bodybody")).unwrap();
        let mut partial = record("read", 2, "body");
        partial["result"]["result"]["sha256"] = whole["sha256"].clone();
        partial["result"]["result"]["total_bytes"] = json!(8);
        partial["result"]["result"]["truncated"] = json!(true);
        partial["result"]["result"]["next_offset_bytes"] = json!(4);
        partial["result"]["mcp_receipt"]["result_sha256"] = json!(gents_local_mcp::digest(
            partial["result"]["result"].to_string().as_bytes()
        ));
        assert!(linked(&partial));
        assert!(readback(&partial, &whole).is_none());
        let mut other = record("read", 2, "body");
        other["actor"] = json!("Other");
        other["result"]["mcp_receipt"]["actor"] = json!("Other");
        assert!(linked(&other));
        assert!(readback(&other, &write).is_none());
        for (path, value) in [
            ("/result/ok", json!(false)),
            ("/result/mcp_receipt", Value::Null),
            ("/result/result/truncated", json!(true)),
            ("/result/result/offset_bytes", json!(1)),
            ("/result/result/text", json!("fake")),
            ("/actor", json!("Other")),
            ("/result/mcp_receipt/policy_revision", json!(2)),
        ] {
            let mut read = record("read", 2, "body");
            *read.pointer_mut(path).unwrap() = value;
            assert!(readback(&read, &write).is_none(), "{path}");
        }
    }
    #[test]
    fn ledger_resets_readback_when_a_later_write_changes_the_recorded_bytes() {
        use crate::evidence::EvidenceProjection;
        let mut ledger = crate::evidence::Ledger::default();
        ledger.observe(&record("write", 1, "body"));
        ledger.observe(&record("read", 2, "body"));
        assert_eq!(
            ledger.artifacts["result.txt"]["state"],
            "native_readback_observed"
        );
        ledger.observe(&record("write", 3, "changed"));
        assert_eq!(
            ledger.artifacts["result.txt"]["state"],
            "written_unverified"
        );
        assert!(ledger.artifacts["result.txt"]
            .get("readback_step")
            .is_none());
    }
}

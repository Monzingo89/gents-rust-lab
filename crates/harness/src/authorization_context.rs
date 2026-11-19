//! Views for next-action authorization, never permissions or completion evidence.
//! Retained records stay intact; only receipt-linked historical write bodies are
//! elided from the model packet. The exact proposed request is always preserved.
use serde_json::{json, Value};

/// Selected by the host call site, never by a model proposal or tool output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReviewKind {
    WorkflowAdmission,
    ToolAction,
}
pub fn apply_review_kind(packet: &mut Value, kind: ReviewKind) {
    packet["review_kind"] = json!(match kind {
        ReviewKind::WorkflowAdmission => "workflow_admission",
        ReviewKind::ToolAction => "tool_action",
    });
    if kind == ReviewKind::WorkflowAdmission {
        let bindings = packet["proposal"]["frozen_native_bindings"].as_array();
        let methods = bindings
            .into_iter()
            .flatten()
            .filter_map(|b| b["source_tool"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let summary = json!({"artifact_binding_count":bindings.map(Vec::len),
            "source_methods":methods,
            "scope":"Methods named in the retained frozen bindings, not permissions, execution receipts or a call count. Exact actors and arguments remain in proposal.frozen_native_bindings."});
        packet["workflow_under_review"] = summary;
        packet["action_under_review"] = Value::Null;
        packet["review_scope"] = json!("Review authorization and safety of the requested workflow scope against the task, recorded answers and frozen contracts. This is admission, not an individual tool call. A null action_under_review is expected. Exact methods and arguments are retained in proposal.frozen_native_bindings; owned-library identities are in proposal.frozen_owned_rust_calls. Admission neither executes nor authorizes an individual tool call, changes policy, nor establishes completion. Independent native checks and task QA remain required. Deny or ask when concrete safety or intent facts require it.");
    } else {
        packet
            .as_object_mut()
            .unwrap()
            .remove("workflow_under_review");
        packet["action_under_review"] = action_focus(&packet["proposal"]["request"]);
        packet["review_scope"] = json!("Decide authorization and safety of the exact NEXT action, not whether the entire mission is already complete. A discovery tool obtains unknown facts; its unknown result is not a missing prerequisite. Read-only is metadata, not an automatic authorization override.");
    }
}

/// Exact path role from the caller's already-approved immutable intake. No
/// filesystem observation, permission decision or alternative action selection.
pub fn frozen_file_target(intake: &Value, actor: &str, request: &Value) -> anyhow::Result<Value> {
    if request["action"] != "files"
        || !matches!(request["arguments"]["op"].as_str(), Some("read" | "write"))
    {
        return Ok(Value::Null);
    }
    let Some(path) = request["arguments"]["path"].as_str() else {
        return Ok(Value::Null);
    };
    let bindings = gents_local_mcp::artifact_bindings::parse(intake)?;
    let output = bindings
        .iter()
        .find(|binding| !binding.receipt_only && binding.path == path && binding.writer == actor);
    let protected = gents_local_mcp::frozen_input_path(intake, path);
    Ok(
        json!({"path":path,"actor":actor,"frozen_input_exact_path_match":protected,
        "assigned_output":output.map(|binding|json!({"writer":binding.writer,"source_actor":binding.source_actor,"source_tool":binding.source_tool,"pointer":binding.pointer,"encoding":binding.encoding})),
        "scope":"Exact path role in the owner-approved frozen contract. Input and output roles are evaluated for this exact path; filesystem aliases require the native guard. No permission, execution, content correctness or current filesystem state is established. Current policy, native guards, admin decision and independent QA remain required.",
        "authority_granted":false,"execution_established":false}),
    )
}

pub fn action_focus(request: &Value) -> Value {
    let Some(action) = request["action"].as_str() else {
        return Value::Null; // Admission packets do not propose an executable action.
    };
    let args = &request["arguments"];
    json!({"action":action,"operation":args.get("op"),"path":args.get("path"),
        "arguments_sha256":gents_local_mcp::digest(args.to_string().as_bytes()),
        "scope":"Only proposal.request is the next action under review. recent_evidence contains earlier operations, never a request to repeat them. The full exact request remains in proposal.request.",
        "authority_granted":false,"execution_established":false})
}

pub fn historical_evidence(rows: &mut [Value]) {
    for row in rows {
        // Authorization needs the complete NEXT request, not duplicate source
        // bodies from earlier reads. Keep their exact journal/hash provenance.
        if historical_read(row) {
            continue;
        }
        let args = &row["arguments"];
        let result = &row["result"];
        let receipt = &result["mcp_receipt"];
        let output = &result["result"];
        let Some(content) = args["content"].as_str() else {
            continue;
        };
        // This checks the retained envelope's internal linkage, not current
        // authorization, current file bytes or an authenticated receipt signature.
        if row["action"] != "files"
            || args["op"] != "write"
            || result["ok"] != true
            || receipt["tool"] != "files"
            || receipt["phase"] != "worker"
            || receipt["status"] != "completed"
            || receipt["actor"] != row["actor"]
            || !row["actor"].is_string()
            || receipt["provider_cost_usd"] != 0
            || !receipt["policy_revision"].as_u64().is_some_and(|n| n > 0)
            || receipt["arguments_sha256"] != gents_local_mcp::digest(args.to_string().as_bytes())
            || receipt["result_sha256"] != gents_local_mcp::digest(output.to_string().as_bytes())
            || !args["path"].is_string()
            || output["path"] != args["path"]
            || output["bytes"] != content.len()
            || output["sha256"] != gents_local_mcp::digest(content.as_bytes())
        {
            continue;
        }
        let view = json!({"kind":"historical_write_body_omitted","content_bytes":content.len(),
            "content_sha256":output["sha256"],"original_record_sha256":gents_local_mcp::digest(row.to_string().as_bytes()),
            "scope":"Earlier receipt-linked write, not the proposed next action. Full body retained in the tool journal/checkpoint. This view is not replayable and does not establish current file state, authorization, readback or test success."});
        if json!({"authorization_evidence_view":&view})
            .to_string()
            .len()
            >= json!({"content":content}).to_string().len()
        {
            continue; // Do not enlarge small historical writes to summarize them.
        }
        row["arguments"].as_object_mut().unwrap().remove("content");
        row["authorization_evidence_view"] = view;
    }
}

fn historical_read(row: &mut Value) -> bool {
    let args = &row["arguments"];
    let result = &row["result"];
    let receipt = &result["mcp_receipt"];
    let output = &result["result"];
    let Some(text) = output["text"].as_str() else {
        return false;
    };
    if row["action"] != "files"
        || args["op"] != "read"
        || result["ok"] != true
        || text.len() < 1024
        || text.len() > 8192
        || output["bytes"].as_u64() != Some(text.len() as u64)
        || receipt["tool"] != "files"
        || receipt["phase"] != "worker"
        || receipt["status"] != "completed"
        || receipt["actor"] != row["actor"]
        || !row["actor"].is_string()
        || receipt["provider_cost_usd"] != 0
        || !receipt["policy_revision"].as_u64().is_some_and(|r| r > 0)
        || receipt["arguments_sha256"] != gents_local_mcp::digest(args.to_string().as_bytes())
        || receipt["result_sha256"] != gents_local_mcp::digest(output.to_string().as_bytes())
        || !args["path"].is_string()
        || output["path"] != args["path"]
    {
        return false;
    }
    let view = json!({"kind":"historical_read_body_omitted","path":output["path"],"bytes":output["bytes"],
        "offset_bytes":output["offset_bytes"],"next_offset_bytes":output["next_offset_bytes"],"source_sha256":output["sha256"],
        "original_record_sha256":gents_local_mcp::digest(row.to_string().as_bytes()),"original_result_sha256":receipt["result_sha256"],
        "policy_revision":receipt["policy_revision"],"scope":"Earlier receipt-linked source read; full text retained in the journal/checkpoint. Historical metadata only, not a current file observation, replayable request, authenticated receipt, permission or test success. The exact next request is retained separately and unchanged."});
    row["result"] = json!({"ok":true,"historical_read_observation":view});
    row["authorization_evidence_view"] = json!({"kind":"historical_read_body_omitted"});
    true
}

pub fn metadata(packet: &Value) -> Value {
    let text = packet.to_string();
    let omitted = packet["proposal"]["recent_evidence"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["authorization_evidence_view"]["kind"] == "historical_write_body_omitted")
        .count();
    json!({"review_kind":packet["review_kind"],"action_under_review":packet["action_under_review"],"historical_write_bodies_omitted":omitted,
        "pre_context_packet_bytes":text.len(),"pre_context_packet_sha256":gents_local_mcp::digest(text.as_bytes()),
        "scope":"Prepared authorization context before model_call wrapping/compaction. No payload, verdict, execution receipt or authority is supplied by this event."})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_read_source_is_omitted_without_touching_next_write_or_claiming_current_state() {
        let args = json!({"op":"read","path":"tests.py"});
        let output = json!({"path":"tests.py","text":"x".repeat(8192),"bytes":8192,"offset_bytes":0,"next_offset_bytes":8192,"sha256":"a".repeat(64)});
        let receipt = json!({"actor":"Author","tool":"files","phase":"worker","status":"completed","policy_revision":1,"provider_cost_usd":0,
            "arguments_sha256":gents_local_mcp::digest(args.to_string().as_bytes()),"result_sha256":gents_local_mcp::digest(output.to_string().as_bytes())});
        let read = json!({"actor":"Author","action":"files","arguments":args,"result":{"ok":true,"result":output,"mcp_receipt":receipt},"full_result":"tool-000001.json"});
        let mut rows = vec![read.clone()];
        historical_evidence(&mut rows);
        assert!(rows[0].to_string().len() < read.to_string().len() / 2);
        assert_eq!(
            rows[0]["result"]["historical_read_observation"]["next_offset_bytes"],
            8192
        );
        assert!(rows[0]["result"].get("mcp_receipt").is_none());
        assert_eq!(rows[0]["full_result"], "tool-000001.json");
        let mut tampered = read.clone();
        tampered["result"]["mcp_receipt"]["result_sha256"] = json!("forged");
        let mut bad = vec![tampered.clone()];
        historical_evidence(&mut bad);
        assert_eq!(bad, vec![tampered]);
        let request = json!({"action":"files","arguments":{"op":"write","path":"tests.py","content":"exact new content"}});
        let mut packet = json!({"proposal":{"request":request,"recent_evidence":[read]}});
        historical_evidence(
            packet["proposal"]["recent_evidence"]
                .as_array_mut()
                .unwrap(),
        );
        assert_eq!(packet["proposal"]["request"], request);
    }
    #[test]
    fn host_review_kind_separates_scope_admission_without_fabricating_an_action() {
        let proposal = json!({"request_admission":"scope","review_kind":"workflow_admission",
            "frozen_native_bindings":[{"source_tool":"owned_rust_call","arguments":{"private":"not duplicated"}},{"source_tool":"owned_rust_call","arguments":{}},{"source_tool":"table_join","arguments":{}}],
            "request":{"action":"files","arguments":{"op":"read","path":"x"}}});
        let mut packet = json!({"proposal":proposal,"action_under_review":action_focus(&proposal["request"]),"review_scope":"next action"});
        apply_review_kind(&mut packet, ReviewKind::ToolAction);
        assert_eq!(packet["review_kind"], "tool_action");
        assert_eq!(packet["action_under_review"]["action"], "files");
        assert!(packet.get("workflow_under_review").is_none());
        let original = packet["proposal"].clone();
        apply_review_kind(&mut packet, ReviewKind::WorkflowAdmission);
        assert_eq!(packet["review_kind"], "workflow_admission");
        assert!(packet["action_under_review"].is_null());
        assert_eq!(
            packet["workflow_under_review"]["source_methods"],
            json!(["owned_rust_call", "table_join"])
        );
        assert_eq!(packet["workflow_under_review"]["artifact_binding_count"], 3);
        assert!(!packet["workflow_under_review"]
            .to_string()
            .contains("not duplicated"));
        assert_eq!(packet["proposal"], original);
        apply_review_kind(&mut packet, ReviewKind::ToolAction);
        assert_eq!(packet["action_under_review"]["action"], "files");
        assert!(packet.get("workflow_under_review").is_none());
        for absent in [Value::Null, json!({}), json!([])] {
            let mut p = json!({"proposal":{"frozen_native_bindings":absent}});
            apply_review_kind(&mut p, ReviewKind::WorkflowAdmission);
            assert_eq!(p["workflow_under_review"]["source_methods"], json!([]));
            assert!(p.get("decision").is_none());
        }
    }
    fn written() -> Value {
        let content = format!("pub fn f() {{}}{}", " ".repeat(2048));
        let args =
            json!({"op":"write","path":"candidate.rs","content":content,"expected_sha256":null});
        let output = json!({"path":"candidate.rs","bytes":content.len(),"sha256":gents_local_mcp::digest(content.as_bytes()),"atomic_replace":true});
        json!({"action":"files","actor":"Author","step":3,"arguments":args,"result":{"ok":true,"result":output,
            "mcp_receipt":{"tool":"files","phase":"worker","status":"completed","actor":"Author","policy_revision":2,"provider_cost_usd":0,
                "arguments_sha256":gents_local_mcp::digest(args.to_string().as_bytes()),"result_sha256":gents_local_mcp::digest(output.to_string().as_bytes())}}})
    }
    #[test]
    fn completed_write_view_keeps_identity_receipt_and_precondition_without_replaying_body() {
        let original = written();
        let mut view = vec![original.clone()];
        historical_evidence(&mut view);
        assert!(view[0]["arguments"].get("content").is_none());
        for key in ["actor", "step", "action", "result"] {
            assert_eq!(view[0][key], original[key]);
        }
        for key in ["op", "path", "expected_sha256"] {
            assert_eq!(view[0]["arguments"][key], original["arguments"][key]);
        }
        assert_eq!(
            view[0]["authorization_evidence_view"]["content_bytes"],
            2061
        );
        assert_eq!(
            view[0]["authorization_evidence_view"]["original_record_sha256"],
            gents_local_mcp::digest(original.to_string().as_bytes())
        );
        assert_eq!(
            original["arguments"]["content"].as_str().unwrap().len(),
            2061
        );
        assert!(view[0].to_string().len() < original.to_string().len());
        let once = view.clone();
        historical_evidence(&mut view);
        assert_eq!(view, once);
    }
    #[test]
    fn failed_uncertain_mismatched_and_unreceipted_payloads_are_not_hidden() {
        for (pointer, value) in [
            ("/action", json!("other")),
            ("/arguments/op", json!("read")),
            ("/result/ok", json!(false)),
            ("/result/mcp_receipt/status", json!("effect_uncertain")),
            ("/result/mcp_receipt/actor", json!("Other")),
            ("/result/mcp_receipt/phase", json!("planner")),
            ("/result/mcp_receipt/policy_revision", json!(0)),
            ("/result/mcp_receipt/arguments_sha256", json!("forged")),
            ("/result/mcp_receipt/result_sha256", json!("forged")),
            ("/result/result/bytes", json!(12)),
            ("/result/result/path", json!("other.rs")),
            ("/result/result/sha256", json!("forged")),
            ("/result/mcp_receipt", Value::Null),
        ] {
            let mut changed = written();
            *changed.pointer_mut(pointer).unwrap() = value;
            let mut view = vec![changed.clone()];
            historical_evidence(&mut view);
            assert_eq!(view[0], changed, "{pointer}");
        }
    }
    #[test]
    fn frozen_target_distinguishes_inputs_outputs_and_other_actor_without_authorizing() {
        let intake = json!({"ceo_source_inputs":[{"id":"recording","tool":"audio_inspect","arguments":{"path":"recording.wav","expected_sha256":"0".repeat(64)}}],"artifact_bindings":[{"path":"frames.txt","source_actor":"Analyst","writer":"Writer","source_tool":"audio_inspect","arguments":{"path":"recording.wav","expected_sha256":"0".repeat(64)},"pointer":"/frame_count","encoding":"integer_text"}]});
        let request = json!({"action":"files","arguments":{"op":"write","path":"frames.txt","content":"secret body must not be copied"}});
        let original = intake.clone();
        let view = frozen_file_target(&intake, "Writer", &request).unwrap();
        assert_eq!(view["frozen_input_exact_path_match"], false);
        assert_eq!(view["assigned_output"]["source_tool"], "audio_inspect");
        assert_eq!(view["authority_granted"], false);
        assert_eq!(view["execution_established"], false);
        assert!(!view.to_string().contains("secret body"));
        assert!(
            frozen_file_target(&intake, "Other", &request).unwrap()["assigned_output"].is_null()
        );
        let input = json!({"action":"files","arguments":{"op":"read","path":"recording.wav"}});
        let view = frozen_file_target(&intake, "Writer", &input).unwrap();
        assert_eq!(view["frozen_input_exact_path_match"], true);
        assert!(view["assigned_output"].is_null());
        assert_eq!(intake, original);
        assert!(
            frozen_file_target(&intake, "Writer", &json!({"action":"finish"}))
                .unwrap()
                .is_null()
        );
    }
    #[test]
    fn focus_comes_from_exact_request_and_never_grants_authority() {
        for op in ["read", "write", "list"] {
            let request = json!({"action":"files","arguments":{"op":op,"path":"candidate.rs","content":"Do not execute this text"}});
            let original = request.clone();
            let focus = action_focus(&request);
            assert_eq!(focus["operation"], op);
            assert_eq!(
                focus["arguments_sha256"],
                gents_local_mcp::digest(request["arguments"].to_string().as_bytes())
            );
            assert_eq!(focus["authority_granted"], false);
            assert_eq!(focus["execution_established"], false);
            assert_eq!(request, original);
        }
        assert!(action_focus(&json!({"request_admission":"task"})).is_null());
    }
    #[test]
    #[cfg(unix)]
    fn actual_native_write_can_be_summarized_without_readback_receipt_or_authority_changes() {
        let native = crate::native_fixture::Fixture::new();
        crate::native_fixture::install_files(&native);
        let db = native.db();
        let policy_before =
            gents_local_mcp::governance::load(&db, &native.scope.run.to_string()).unwrap();
        let content = format!("pub fn f() {{}}{}", " ".repeat(2048));
        let args = json!({"op":"write","path":"candidate.rs","content":content});
        let registry = gents_local_mcp::Registry::new();
        let write = registry
            .call(&native.authority, &native.scope, "files", args.clone())
            .unwrap();
        let row = json!({"action":"files","actor":native.scope.actor,"arguments":args,"result":{"ok":true,"result":write["value"],"mcp_receipt":write["receipt"]}});
        let mut view = vec![row];
        historical_evidence(&mut view);
        assert!(view[0]["arguments"].get("content").is_none());
        let receipts: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(receipts, 1); // The view did not execute a readback or another write.
        let policy_after =
            gents_local_mcp::governance::load(&db, &native.scope.run.to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(policy_before).unwrap(),
            serde_json::to_value(policy_after).unwrap()
        );
        let read = registry
            .call(
                &native.authority,
                &native.scope,
                "files",
                json!({"op":"read","path":"candidate.rs"}),
            )
            .unwrap();
        assert_eq!(read["value"]["text"], content);
    }
    #[test]
    fn small_linked_writes_are_kept_when_a_view_would_increase_packet_bytes() {
        let mut row = written();
        row["arguments"]["content"] = json!("short");
        row["result"]["result"]["bytes"] = json!(5);
        row["result"]["result"]["sha256"] = json!(gents_local_mcp::digest(b"short"));
        row["result"]["mcp_receipt"]["arguments_sha256"] = json!(gents_local_mcp::digest(
            row["arguments"].to_string().as_bytes()
        ));
        row["result"]["mcp_receipt"]["result_sha256"] = json!(gents_local_mcp::digest(
            row["result"]["result"].to_string().as_bytes()
        ));
        let mut view = vec![row.clone()];
        historical_evidence(&mut view);
        assert_eq!(view[0], row);
    }
}

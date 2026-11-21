//! Existing CEO entry: answer an eligible new ask before requiring a model.
//! Constrained/attached/contextual requests remain intact in normal planning.
use anyhow::{ensure, Context, Result};
use gents_local_mcp::task_engine::{contracts::*, Repository};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

pub(crate) fn plain_initial(input: &Value) -> bool {
    let Some(fields) = input.as_object() else {
        return false;
    };
    if input["expected_revision"].as_u64() != Some(0)
        || !fields.keys().all(|key| {
            matches!(
                key.as_str(),
                "message"
                    | "expected_revision"
                    | "source_inputs"
                    | "source_files"
                    | "owned_invocations"
                    | "owned_argument_data"
                    | "owned_component_ids"
                    | "planning_mode"
                    | "planning_constraints"
                    | "message_kind"
                    | "task_input"
            )
        })
    {
        return false;
    }
    if !input["planning_mode"].is_null()
        || !input["planning_constraints"].is_null()
        || (!input["message_kind"].is_null() && input["message_kind"] != "requirements")
    {
        return false;
    }
    for key in [
        "source_inputs",
        "source_files",
        "owned_invocations",
        "owned_component_ids",
    ] {
        if !input[key].is_null() && !input[key].as_array().is_some_and(Vec::is_empty) {
            return false;
        }
    }
    input["owned_argument_data"].is_null()
        || input["owned_argument_data"]
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
}

pub fn try_answer(
    root: &Path,
    company: &str,
    owner: &str,
    id: Uuid,
    input: &Value,
    runtime_revision: &str,
) -> Result<Option<Value>> {
    if !plain_initial(input) {
        return Ok(None);
    }
    let message = input["message"].as_str().context("message required")?;
    ensure!(
        !message.trim().is_empty() && message.len() <= 4000,
        "message must contain 1..4000 bytes"
    );
    let mut db = crate::ceo_planning::db(root, company, owner)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM ceo_planning WHERE id=?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_some() {
        return Ok(None);
    }
    let repo = Repository::new(&tx)?;
    let scope = crate::task_frontdoor::owner_scope(&repo, company, owner, runtime_revision)?;
    repo.refresh_dependencies(&scope, root)?;
    let task_input = input
        .get("task_input")
        .cloned()
        .unwrap_or_else(|| json!({"original_ask":message}));
    let request = TaskRequest::from_input(serde_json::from_value(task_input)?)?;
    ensure!(
        request.original_ask == message,
        "task input must retain the exact original message"
    );
    let now = crate::now_ms();
    let result = repo.execute(&request, &ExecutionPolicy::read_only(scope, now), now)?;
    if result.status != "succeeded" {
        // Misses continue through the original planner. Do not leave a second,
        // unconnected task identity for the same conversation.
        return Ok(None);
    }
    let body = json!({"id":id,"revision":1,"status":"answered","messages":[
        {"role":"user","kind":"requirements","message":message,"at_ms":now},
        {"role":"ceo","message":answer_text(&result),"at_ms":now}],
        "task_request":request,"task_result":result,"policy_installed":false,"tools_executed":false,
        "source_inputs":[],"source_files":[],"proposed_artifact_bindings":[]});
    tx.execute(
        "INSERT INTO ceo_planning VALUES(?1,1,'answered',?2)",
        params![id.to_string(), body.to_string()],
    )?;
    tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'ceo_task_answered',?2,?3)", params![id.to_string(),
        json!({"task_id":result.task_id,"path":result.actual_path,"verification_state":result.verification_state,
            "generative_model_calls":0,"result_sha256":gents_local_mcp::digest(&serde_json::to_vec(&result)?),"policy_installed":false,"tools_executed":false}).to_string(),now as i64])?;
    tx.commit()?;
    Ok(Some(body))
}

pub(crate) fn answer_text(result: &TaskResult) -> String {
    let Some(answer) = &result.answer else {
        return "The saved source answer is no longer current or accessible. Start a new request to inspect available evidence.".into();
    };
    let sources = result
        .evidence
        .iter()
        .map(|source| {
            format!(
                "{} (revision {}; bytes {}..{})",
                source.title, source.source_revision, source.start_byte, source.end_byte
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    if result.verification_state == "citations_checked_answer_unverified" {
        return format!("{answer}\n\nSources: {sources}\nGenerated draft. Citation bytes were checked; factual accuracy and semantic support have not been independently verified.");
    }
    format!("{answer}\n\nSource: {sources}\nChecked against the current source text; factual accuracy has not been independently established. No generative model was called.")
}

pub fn refresh_answer(root: &Path, company: &str, owner: &str, body: &mut Value) -> Result<()> {
    if body["status"] != "answered" {
        return Ok(());
    }
    let id: Uuid = serde_json::from_value(body["task_result"]["task_id"].clone())?;
    let mut db = crate::ceo_planning::db(root, company, owner)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let repo = Repository::new(&tx)?;
    let scope = crate::task_frontdoor::owner_scope(&repo, company, owner, "status-read/v1")?;
    repo.refresh_dependencies(&scope, root)?;
    let result = repo
        .status(&scope, id, crate::now_ms())?
        .context("saved task result unavailable")?;
    body["messages"][1]["message"] = json!(answer_text(&result));
    body["task_result"] = serde_json::to_value(result)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attachments_constraints_repair_and_context_never_take_plain_path() {
        let base = json!({"message":"an exact ask","expected_revision":0,"source_inputs":[],"source_files":[],"owned_invocations":[]});
        assert!(plain_initial(&base));
        for (key, value) in [
            ("source_inputs", json!([{"id":"source"}])),
            ("source_files", json!([{"text":"private"}])),
            ("owned_invocations", json!([{"id":"call"}])),
            ("owned_component_ids", json!(["component"])),
            ("owned_argument_data", json!({"data":"private"})),
            ("planning_mode", json!("reasoning")),
            (
                "planning_constraints",
                json!({"separate_source_writers":true}),
            ),
            ("message_kind", json!("planning_repair")),
            ("expected_revision", json!(1)),
            ("tools", json!(["shell"])),
            ("source_files", json!({})),
        ] {
            let mut input = base.clone();
            input[key] = value;
            assert!(!plain_initial(&input), "{key}");
        }
    }
}

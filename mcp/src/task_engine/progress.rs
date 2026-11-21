//! Bounded host-journal metadata for an already-authorized task status read.
//! Dispatch records describe requests, never proof of inference or acceptance.
use super::contracts::AccessScope;
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub seq: u64,
    pub at_ms: u64,
    pub kind: String,
    pub call_id: Option<Uuid>,
    pub node_id: Option<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
    pub job_id: Uuid,
    pub sampled_at_ms: u64,
    pub phase: String,
    pub model_dispatches: Option<u32>,
    pub model_returns: Option<u32>,
    pub pending_model_requests: Option<u32>,
    pub unresolved_model_dispatches: Option<u32>,
    pub truncated: bool,
    pub events: Vec<Event>,
}

pub fn attach(
    db: &Connection,
    scope: &AccessScope,
    result: &mut super::contracts::TaskResult,
    now: u64,
) -> Result<()> {
    result.progress = read(db, scope, result.task_id, &result.status, now)?;
    if matches!(
        result.status.as_str(),
        "running" | "interrupted" | "cancelled"
    ) && result
        .progress
        .as_ref()
        .is_some_and(|p| p.model_dispatches.is_none_or(|n| n > 0))
    {
        // A reservation or a missing reply cannot establish zero actual usage.
        result.measurements.generative_model_calls = None;
        result.measurements.input_tokens = None;
        result.measurements.output_tokens = None;
    }
    Ok(())
}

/// Caller must first apply current owner/native authorization and source checks.
/// The subject join prevents borrowing another actor/run's job or audit stream.
pub fn read(
    db: &Connection,
    scope: &AccessScope,
    task: Uuid,
    status: &str,
    now: u64,
) -> Result<Option<Progress>> {
    ensure!(!db.is_autocommit(), "progress requires caller transaction");
    if status == "evidence_invalidated" {
        return Ok(None);
    }
    let installed:bool=db.query_row("SELECT COUNT(*)=3 FROM sqlite_master WHERE type='table' AND name IN ('task_workflow_subscribers_v1','task_workflow_jobs_v1','audit')",[],|r|r.get(0))?;
    if !installed {
        return Ok(None);
    }
    let job:Option<String>=db.query_row("SELECT j.id FROM task_workflow_subscribers_v1 s JOIN task_workflow_jobs_v1 j ON j.id=s.job AND j.tenant=s.tenant AND j.subject=s.subject WHERE s.tenant=?1 AND s.subject=?2 AND s.task=?3",params![scope.tenant,scope.subject,task.to_string()],|r|r.get(0)).optional()?;
    let Some(job) = job else { return Ok(None) };
    let mut statement=db.prepare("SELECT seq,created,kind,
        CASE WHEN length(body)<=16384 AND json_valid(body) THEN json_extract(body,'$.call_id') END,
        CASE WHEN length(body)<=16384 AND json_valid(body) THEN json_extract(body,'$.node_id') END
        FROM audit WHERE run_id=?1 AND kind IN ('task_model_dispatch','task_model_return','task_tool_dispatch','task_tool_return','task_validation_dispatch','task_validation_return','task_plan_revision') ORDER BY seq DESC LIMIT 257")?;
    let mut rows = statement.query([&job])?;
    let mut events = vec![];
    let mut dispatches = std::collections::BTreeSet::new();
    let mut returned = std::collections::BTreeSet::new();
    let mut malformed = false;
    while let Some(row) = rows.next()? {
        let kind: String = row.get(2)?;
        let call: Option<String> = row.get(3).unwrap_or(None);
        let call = call.and_then(|s| Uuid::parse_str(&s).ok());
        if kind.starts_with("task_model_") && call.is_none() {
            malformed = true;
        }
        if let Some(id) = call {
            if kind == "task_model_dispatch" {
                dispatches.insert(id);
            }
            if kind == "task_model_return" {
                returned.insert(id);
            }
        }
        let node: Option<String> = row.get(4).unwrap_or(None);
        events.push(Event {
            seq: u64::try_from(row.get::<_, i64>(0)?)?,
            at_ms: u64::try_from(row.get::<_, i64>(1)?)?,
            kind,
            call_id: call,
            node_id: node.filter(|s| {
                !s.is_empty()
                    && s.len() <= 64
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            }),
        });
    }
    let truncated = events.len() > 256;
    let complete = !truncated && !malformed && returned.is_subset(&dispatches);
    let unresolved = complete.then(|| dispatches.difference(&returned).count() as u32);
    let phase = if status != "running" {
        // Result status wins over historical unfinished spans, including a
        // cancelled subscriber whose compatible peer still uses the same job.
        match status {
            "generated" | "analyzed" => "finished",
            "cancelled" => "cancelled",
            "interrupted" => "interrupted",
            "needs_clarification" => "needs_clarification",
            _ => "stopped",
        }
    } else if !complete {
        "telemetry_unavailable"
    } else {
        match events.first().map(|e| e.kind.as_str()) {
            Some("task_model_dispatch") => "model_request_pending",
            Some("task_model_return") if unresolved.is_some_and(|n| n > 0) => {
                "model_request_pending"
            }
            Some("task_model_return") => "model_reply_recorded",
            Some("task_tool_dispatch") => "read_tool_pending",
            Some("task_tool_return") => "read_tool_returned",
            Some("task_validation_dispatch") => "validation_pending",
            Some("task_validation_return") => "validation_returned",
            Some("task_plan_revision") => "plan_recorded",
            _ => "queued",
        }
    };
    events.truncate(16);
    events.reverse();
    Ok(Some(Progress {
        job_id: Uuid::parse_str(&job)?,
        sampled_at_ms: now,
        phase: phase.into(),
        model_dispatches: complete.then_some(dispatches.len() as u32),
        model_returns: complete.then_some(returned.len() as u32),
        pending_model_requests: if status == "running" {
            unresolved
        } else {
            Some(0)
        },
        unresolved_model_dispatches: unresolved,
        truncated,
        events,
    }))
}

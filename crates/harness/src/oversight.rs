//! Event-triggered CEO review. The timer observes; model inference runs only on change/risk.
use crate::{company, emit, model_call, provider::Model, Shared};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::{atomic::AtomicBool, Arc};
use uuid::Uuid;

const REVIEW_CADENCE: std::time::Duration = std::time::Duration::from_secs(3 * 60 * 60);

pub fn risk(state: &Value, run: Uuid, now: u64, last_progress: u64) -> Value {
    let blocked: Vec<_> = state["milestones"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["run_id"] == run.to_string() && m["state"] == "blocked")
        .cloned()
        .collect();
    let overdue: Vec<_> = state["milestones"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            m["run_id"] == run.to_string()
                && m["state"] != "done"
                && m["due_ms"].as_u64().is_some_and(|t| t < now)
        })
        .cloned()
        .collect();
    let job = state["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|j| j["run_id"] == run.to_string());
    let deadline = job
        .and_then(|j| j.pointer("/intake/deadline_ms"))
        .and_then(Value::as_u64);
    json!({"overdue_milestones":overdue,"blocked_milestones":blocked,"job_deadline_ms":deadline,"deadline_missed":deadline.is_some_and(|d|d<now),
        "deadline_known":deadline.is_some(),"stalled":now.saturating_sub(last_progress)>300_000,
        "last_progress_ms":last_progress,"observed_at_ms":now})
}

pub async fn monitor(app: Shared) {
    // The first review is three hours after startup. The worker and tool events
    // continue normally between reviews without polling or cloud usage.
    let mut timer =
        tokio::time::interval_at(tokio::time::Instant::now() + REVIEW_CADENCE, REVIEW_CADENCE);
    let pending = Arc::new(tokio::sync::Mutex::new(HashSet::new()));
    loop {
        timer.tick().await;
        let jobs: Vec<_> = {
            let runs = app.runs.lock().await;
            runs.iter()
                .filter(|(_, r)| r.status == "running" && r.company_id.is_some())
                .map(|(id, r)| {
                    let progress = r.events.iter().rev().find(|e| e.agent != "CEO");
                    (
                        *id,
                        r.company_id.clone().unwrap(),
                        r.owner.clone(),
                        r.task.clone(),
                        progress.map(|e| e.timestamp_ms).unwrap_or(r.created_at_ms),
                        r.cancel.clone(),
                    )
                })
                .collect()
        };
        for (id, company, owner, task, last, cancel) in jobs {
            let Ok(root) = std::env::current_dir() else {
                continue;
            };
            // A human question is an intentional wait, not a stalled worker.
            // Do not burn shared-GPU calls (or invent progress) while waiting.
            let Ok(snapshot) = crate::autonomy::work_snapshot(&root, &company, id) else {
                continue;
            };
            if snapshot["waiting_for_operator"] == true || snapshot["pending_tool"] == true {
                continue;
            }
            if !pending.lock().await.insert(id) {
                continue;
            }
            let (app, pending) = (app.clone(), pending.clone());
            tokio::spawn(async move {
                if let Err(error) = review(&app, id, &company, &owner, &task, last, &cancel).await {
                    let _ = emit(
                        &app,
                        id,
                        ("CEO", "oversight"),
                        ("meeting", "ceo_review_failed"),
                        &format!(
                            "CEO review unavailable: {error}; acceptance criteria remain unchanged"
                        ),
                        None,
                        None,
                    )
                    .await;
                }
                pending.lock().await.remove(&id);
            });
        }
    }
}

async fn review(
    app: &crate::App,
    id: Uuid,
    company_id: &str,
    owner: &str,
    task: &str,
    last: u64,
    cancel: &AtomicBool,
) -> Result<()> {
    let root = std::env::current_dir()?;
    let state = company::operation(&root, company_id, owner, &json!({"op":"status"}))?;
    let snapshot = crate::autonomy::work_snapshot(&root, company_id, id)?;
    let last = if task.to_ascii_lowercase().contains("pdf") {
        snapshot["delivery_watch"]["last_change_ms"]
            .as_u64()
            .filter(|t| *t > 0)
            .unwrap_or(last)
    } else {
        last
    };
    let risk = risk(&state, id, crate::now_ms(), last);
    emit(
        app,
        id,
        ("CEO", "oversight"),
        ("meeting", "ceo_observation"),
        &risk.to_string(),
        None,
        None,
    )
    .await?;
    let model: Model = app
        .config
        .agents
        .iter()
        .find(|a| a.id == "Engineer Gent")
        .or_else(|| app.config.agents.first())
        .context("CEO requires local model")?
        .worker
        .clone();
    if snapshot["waiting_for_operator"] == true || snapshot["pending_tool"] == true {
        return Ok(());
    }
    let reply=model_call(app,id,("Local Executive","oversight"),&model,("COMPANY_CEO: This is the three-hour checkpoint. Return bare JSON {all_hands:boolean,next_actions:[string]}. Diagnose only observed blockers and propose a concise FUTURE repair plan for the local worker to implement. Do not supply task-domain answers, claim execution, change scope, approve tools, spend money, or expose private reasoning. Call all hands only when the evidence shows a real issue; otherwise return no actions.",&json!({"approved_task":task,"risk":risk,"verified_progress":snapshot["progress"],"implementation_owner":"strongest installed local worker"}).to_string()),cancel).await?;
    let raw: Value = serde_json::from_str(&reply.text)?;
    let decision = evidence_review(&snapshot["progress"], &raw);
    emit(
        app,
        id,
        ("CEO", "oversight"),
        ("meeting", "ceo_review"),
        &decision.to_string(),
        Some(model.model.clone()),
        None,
    )
    .await?;
    let observed = risk["blocked_milestones"]
        .as_array()
        .is_some_and(|m| !m.is_empty())
        || risk["deadline_missed"] == true
        || risk["stalled"] == true
        || risk["overdue_milestones"]
            .as_array()
            .is_some_and(|m| !m.is_empty());
    let recent_meeting = state["meetings"].as_array().into_iter().flatten().any(|m| {
        m["run_id"] == id.to_string()
            && crate::now_ms().saturating_sub(m["created_at_ms"].as_u64().unwrap_or(0))
                < REVIEW_CADENCE.as_millis() as u64
    });
    if decision["all_hands"] == true && observed && !recent_meeting {
        let brief = repair_brief(&decision);
        let meeting = company::meeting(&root, company_id, owner, &id.to_string(), &brief)?;
        emit(
            app,
            id,
            ("CEO", "oversight"),
            ("meeting", "all_hands_called"),
            &json!({"meeting_id":meeting,"brief":brief,"company_id":company_id,"run_id":id})
                .to_string(),
            Some(model.model),
            None,
        )
        .await?;
    }
    Ok(())
}

fn repair_brief(decision: &Value) -> String {
    format!("{}\nFuture repair plan for the strongest local worker to implement (advisory; every action requires independent admin review):\n{}",
        decision["summary"].as_str().unwrap_or("Observed execution blocker"),
        decision["suggested_next_actions"].as_array().into_iter().flatten()
            .filter_map(Value::as_str).take(6).map(|s| s.chars().take(350).collect::<String>()).collect::<Vec<_>>().join("\n")
    ).chars().take(3000).collect()
}

fn evidence_review(progress: &Value, raw: &Value) -> Value {
    json!({"summary":format!("{} tool results recorded. Action counts: {}. Overall completion is not verified; consult tool journals. A schedule is not an email receipt.",progress["tool_results"],progress["action_counts"]),
        "all_hands":raw["all_hands"]==true,"suggested_next_actions":raw["next_actions"].as_array().cloned().unwrap_or_default(),
        "suggestions_are_unverified":true,"verified_progress":progress})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repair_actions_reach_the_worker_brief() {
        let decision = evidence_review(
            &json!({"tool_results":12,"action_counts":{}}),
            &json!({"all_hands":true,"next_actions":["Inspect the failed authoring command; repair its source and rerun."]}),
        );
        assert!(repair_brief(&decision).contains("repair its source and rerun"));
        assert!(repair_brief(&decision).contains("admin review"));
    }
    #[test]
    fn review_cadence_is_three_hours() {
        assert_eq!(REVIEW_CADENCE.as_secs(), 10_800);
    }
    #[test]
    fn ceo_status_does_not_publish_model_completion_claims_as_evidence() {
        let review = evidence_review(
            &json!({"tool_results":200,"action_counts":{"schedule":200}}),
            &json!({"summary":"Email prepared, queued and delivered!","brief":"Everything complete","all_hands":false}),
        );
        assert!(!review.to_string().contains("Everything complete"));
        assert!(!review.to_string().contains("Email prepared"));
        assert_eq!(
            review["verified_progress"]["action_counts"]["schedule"],
            200
        );
        assert!(review["summary"].as_str().unwrap().contains("not verified"));
    }
    #[test]
    fn unknown_deadlines_are_not_reported_as_missed() {
        let id = Uuid::new_v4();
        let value = risk(
            &json!({"jobs":[{"run_id":id}],"milestones":[]}),
            id,
            1000,
            900,
        );
        assert_eq!(value["deadline_known"], false);
        assert_eq!(value["deadline_missed"], false);
        let value = risk(
            &json!({"jobs":[{"run_id":id,"intake":{"deadline_ms":500}}],"milestones":[]}),
            id,
            1000,
            900,
        );
        assert_eq!(value["deadline_missed"], true);
    }
}

//! Durable native MCP handoff to the existing harness executors. No inference
//! runs under a native tool transaction; model JSON never supplies custody.
use super::{contracts::*, routing::Hints, Repository};
use crate::{governance, Scope, ToolContext};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub instance: Uuid,
    pub runtime: String,
    pub expires_at_ms: u64,
    pub pending_limit: u32,
    pub deadline_seconds: u64,
    pub model_intake: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Custody {
    pub scope: Scope,
    pub action: String,
    pub policy_sha256: String,
    pub approval_sha256: String,
    pub worker_instance: Uuid,
    pub runtime: String,
    pub deadline_ms: u64,
    #[serde(default)]
    pub source_scope_sha256: String,
    #[serde(default)]
    pub corpus_sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub custody: Custody,
    pub request: TaskRequest,
    pub hints: Hints,
    pub analysis_only: bool,
    #[serde(default)]
    pub skip_model_intake: bool,
    pub base: TaskResult,
}
pub fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS task_native_worker_v1(singleton INTEGER PRIMARY KEY CHECK(singleton=1),body TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS task_native_queue_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,task TEXT NOT NULL,instance TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,state TEXT NOT NULL,job TEXT,created INTEGER NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(tenant,subject,task));")?;
    Ok(())
}
pub fn lease(db: &Connection) -> Result<Option<Lease>> {
    install(db)?;
    db.query_row(
        "SELECT body FROM task_native_worker_v1 WHERE singleton=1",
        [],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| serde_json::from_str(&s).map_err(Into::into))
    .transpose()
}
pub fn publish(db: &Connection, value: &Lease) -> Result<()> {
    ensure!(
        value.pending_limit > 0
            && value.pending_limit <= 64
            && (1..=300).contains(&value.deadline_seconds),
        "invalid native worker lease"
    );
    install(db)?;
    db.execute("INSERT INTO task_native_worker_v1 VALUES(1,?1) ON CONFLICT(singleton) DO UPDATE SET body=excluded.body",[serde_json::to_string(value)?])?;
    Ok(())
}
pub fn subject(scope: &Scope) -> String {
    format!("{}:{}:{}", scope.owner, scope.run, scope.actor)
}
pub fn workspace(root: &Path, scope: &Scope) -> PathBuf {
    root.join("data/companies")
        .join(&scope.company)
        .join("runs")
        .join(scope.run.to_string())
        .join("workspace")
}
pub fn access(
    repo: &Repository<'_>,
    scope: &Scope,
    policy: &governance::Policy,
    runtime: &str,
) -> Result<AccessScope> {
    scope.validate()?;
    let subject = subject(scope);
    Ok(AccessScope {
        tenant: scope.company.clone(),
        subject: subject.clone(),
        effective_access_sha256: crate::digest(&serde_json::to_vec(
            &json!({"policy":policy,"phase":scope.phase}),
        )?),
        policy_revision: policy.revision.to_string(),
        profile_revision: "native-task-tools/v2".into(),
        runtime_revision: runtime.into(),
        conversation_sha256: crate::digest(subject.as_bytes()),
        allowed_sources: repo.source_ids(&scope.company, &subject)?,
    })
}
pub fn refresh(
    repo: &Repository<'_>,
    db: &Connection,
    custody: &Custody,
    scope: &AccessScope,
    root: &Path,
) -> Result<()> {
    let policy = governance::load(db, &custody.scope.run.to_string())?
        .context("native policy unavailable")?;
    for path in repo.dependency_paths(scope)? {
        policy.authorize(
            &custody.scope.actor,
            "files",
            &json!({"op":"read","path":path}),
        )?;
    }
    repo.refresh_dependencies(scope, &workspace(root, &custody.scope))?;
    Ok(())
}
pub fn check(
    db: &Connection,
    custody: &Custody,
    now: u64,
    live: bool,
) -> Result<governance::Policy> {
    custody.scope.validate()?;
    ensure!(
        !live || now < custody.deadline_ms,
        "native task deadline exhausted"
    );
    let policy = governance::load(db, &custody.scope.run.to_string())?
        .context("native policy unavailable")?;
    ensure!(
        crate::digest(&serde_json::to_vec(&policy)?) == custody.policy_sha256
            && custody.scope.expected_revision == Some(policy.revision),
        "native task policy epoch changed"
    );
    policy.authorize(&custody.scope.actor, &custody.action, &json!({}))?;
    let (owner,encoded,digest,approval,state):(String,String,String,Option<String>,String)=db.query_row("SELECT company.owner,jobs.intake,jobs.digest,jobs.approval,jobs.state FROM company JOIN jobs ON jobs.run_id=?1 LIMIT 1",[custody.scope.run.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
    let approval = approval.context("native job approval missing")?;
    let approved: Value = serde_json::from_str(&approval)?;
    ensure!(
        owner == custody.scope.owner
            && crate::digest(encoded.as_bytes()) == digest
            && crate::digest(approval.as_bytes()) == custody.approval_sha256
            && approved["owner"] == owner
            && approved["sha256"] == digest
            && approved["type"] == "local_operator_approval"
            && (!live || matches!(state.as_str(), "approved" | "running")),
        "native parent job approval or active state changed"
    );
    let current = lease(db)?.context("native worker unavailable")?;
    ensure!(
        current.runtime == custody.runtime
            && (!live
                || (current.instance == custody.worker_instance && current.expires_at_ms > now)),
        "native worker lease changed or expired; no automatic replay"
    );
    Ok(policy)
}
pub fn authorize(db: &Connection, custody: &Custody, now: u64) -> Result<governance::Policy> {
    check(db, custody, now, true)
}
pub fn enqueue(
    context: &ToolContext<'_>,
    request: TaskRequest,
    hints: Hints,
    analysis_only: bool,
    skip_model_intake: bool,
    mut base: TaskResult,
) -> Result<TaskResult> {
    request.validate()?;
    let now = crate::now_ms();
    let Some(worker) = lease(context.db)?.filter(|l| l.expires_at_ms > now) else {
        base.blockers.push("Native asynchronous execution requires a running host with task_workflows.native_mcp enabled; no inference dispatched.".into());
        return Ok(base);
    };
    ensure!(
        analysis_only || request.input.intent == Intent::Answer,
        "actions require existing reviewed native workflows"
    );
    let mut scope = context.scope.clone();
    scope.expected_revision = Some(context.policy.revision);
    let (encoded, approval): (String, String) = context.db.query_row(
        "SELECT intake,approval FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let parent: Value = serde_json::from_str(&encoded)?;
    ensure!(
        parent["max_model_calls"]
            .as_u64()
            .is_some_and(|n| (1..=1024).contains(&n)),
        "Native asynchronous inference requires an owner-frozen parent max_model_calls allowance"
    );
    let deadline = now.saturating_add(worker.deadline_seconds * 1000);
    let deadline = parent
        .get("deadline_ms")
        .map(|n| n.as_u64().context("invalid parent deadline"))
        .transpose()?
        .map_or(deadline, |n| n.min(deadline));
    let repo = Repository::new(context.db)?;
    let source_scope = access(&repo, &scope, context.policy, &worker.runtime)?;
    let custody = Custody {
        scope,
        action: if analysis_only {
            "destructure_task"
        } else {
            "execute_task"
        }
        .into(),
        policy_sha256: crate::digest(&serde_json::to_vec(context.policy)?),
        approval_sha256: crate::digest(approval.as_bytes()),
        worker_instance: worker.instance,
        runtime: worker.runtime,
        deadline_ms: deadline,
        source_scope_sha256: crate::digest(&serde_json::to_vec(&source_scope)?),
        corpus_sha256: crate::digest(&serde_json::to_vec(
            &repo.knowledge_snapshot(&source_scope, now)?,
        )?),
    };
    authorize(context.db, &custody, now)?;
    let subject = subject(&custody.scope);
    context.db.execute("DELETE FROM task_native_queue_v1 WHERE expires<=?1 AND state NOT IN ('queued','claimed','running','cancel_requested')",[now as i64])?;
    let pending:i64=context.db.query_row("SELECT COUNT(*) FROM task_native_queue_v1 WHERE state IN ('queued','claimed','running','cancel_requested')",[],|r|r.get(0))?;
    let stored: i64 = context.db.query_row(
        "SELECT COUNT(*) FROM task_native_queue_v1 WHERE tenant=?1 AND subject=?2",
        params![custody.scope.company, subject],
        |r| r.get(0),
    )?;
    ensure!(
        pending < i64::from(worker.pending_limit) && stored < 128,
        "native task queue capacity reached"
    );
    base.status = "running".into();
    base.actual_path = "native_async_queue".into();
    base.answer = None;
    base.evidence.clear();
    base.blockers.clear();
    base.verification_state = "queued_not_executed".into();
    let packet = Packet {
        custody,
        request,
        hints,
        analysis_only,
        skip_model_intake,
        base: base.clone(),
    };
    let body = serde_json::to_string(&packet)?;
    ensure!(
        body.len() <= 65536,
        "native packet exceeds checkpoint bound"
    );
    context.db.execute(
        "INSERT INTO task_native_queue_v1 VALUES(?1,?2,?3,?4,?5,?6,'queued',NULL,?7,?8)",
        params![
            packet.custody.scope.company,
            subject,
            packet.request.task_id.to_string(),
            worker.instance.to_string(),
            body,
            crate::digest(body.as_bytes()),
            now as i64,
            (now + 3600000) as i64
        ],
    )?;
    let repo = Repository::new(context.db)?;
    let access = access(
        &repo,
        &packet.custody.scope,
        context.policy,
        &packet.custody.runtime,
    )?;
    repo.replace_result(&access, &base)?;
    Ok(base)
}
pub fn packet(
    db: &Connection,
    scope: &Scope,
    task: Uuid,
) -> Result<Option<(Packet, String, Option<String>)>> {
    install(db)?;
    let row:Option<(String,String,String,Option<String>)>=db.query_row("SELECT body,sha256,state,job FROM task_native_queue_v1 WHERE tenant=?1 AND subject=?2 AND task=?3",params![scope.company,subject(scope),task.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    row.map(|(body, sha, state, job)| {
        ensure!(
            crate::digest(body.as_bytes()) == sha,
            "native task packet integrity mismatch"
        );
        let packet: Packet = serde_json::from_str(&body)?;
        ensure!(
            packet.request.task_id == task
                && packet.custody.scope.company == scope.company
                && subject(&packet.custody.scope) == subject(scope)
                && packet.custody.scope.phase == scope.phase,
            "native packet custody mismatch"
        );
        Ok((packet, state, job))
    })
    .transpose()
}
pub fn observe(context: &ToolContext<'_>, task: Uuid, cancel: bool) -> Result<Option<TaskResult>> {
    let Some((packet, mut state, job)) = packet(context.db, context.scope, task)? else {
        return Ok(None);
    };
    if let Some(job) = &job {
        let current: Option<String> = context
            .db
            .query_row(
                "SELECT state FROM task_workflow_jobs_v1 WHERE id=?1",
                [job],
                |r| r.get(0),
            )
            .optional()?;
        if current.as_deref() == Some("completed")
            && !matches!(state.as_str(), "cancel_requested" | "cancelled")
        {
            state = "completed".into();
        }
    }
    let repo = Repository::new(context.db)?;
    let runtime = lease(context.db)?.map_or_else(|| COMPATIBILITY.into(), |l| l.runtime);
    let scope = access(&repo, context.scope, context.policy, &runtime)?;
    // A changed epoch is observable as invalidation; it never exposes old data.
    let valid = check(
        context.db,
        &packet.custody,
        crate::now_ms(),
        matches!(state.as_str(), "queued" | "claimed" | "running"),
    )
    .is_ok()
        && packet.custody.source_scope_sha256 == crate::digest(&serde_json::to_vec(&scope)?)
        && packet.custody.corpus_sha256
            == crate::digest(&serde_json::to_vec(
                &repo.knowledge_snapshot(&scope, crate::now_ms())?,
            )?);
    let mut result = if valid {
        for path in repo.dependency_paths(&scope)? {
            context.policy.authorize(
                &context.scope.actor,
                "files",
                &json!({"op":"read","path":path}),
            )?;
        }
        repo.refresh_dependencies(&scope, &context.authority.workspace(context.scope)?)?;
        repo.status(&scope, task, crate::now_ms())?
            .context("native task result missing")?
    } else {
        packet.base.clone()
    };
    let runtime_receipts_current =
        super::validators::receipts_current(context, &result.validation_receipts).unwrap_or(false);
    if (!valid || !runtime_receipts_current) && state != "cancelled" {
        result.status = "evidence_invalidated".into();
        result.answer = None;
        result.evidence.clear();
        result.verification_state = "not_verified".into();
        result.validation_receipts.clear();
        for check in &mut result.acceptance_checks {
            if matches!(
                check.name.as_str(),
                "dotnet_build/v1" | "dotnet_api/v1" | "angular_build/v1"
            ) {
                check.status = CheckStatus::NotRun;
                check.detail="Earlier native build receipt invalidated by source, SDK, access or canonical audit change.".into();
            }
        }
        result.blockers.push(
            "Native scope, parent approval, deadline or host lease changed; no automatic replay."
                .into(),
        );
    }
    if cancel
        && !matches!(
            state.as_str(),
            "completed" | "cancelled" | "failed" | "interrupted"
        )
    {
        context.db.execute("UPDATE task_native_queue_v1 SET state='cancel_requested' WHERE tenant=?1 AND subject=?2 AND task=?3",params![scope.tenant,scope.subject,task.to_string()])?;
        if job.is_some() {
            context.db.execute("UPDATE task_workflow_subscribers_v1 SET cancelled=1 WHERE tenant=?1 AND subject=?2 AND task=?3",params![scope.tenant,scope.subject,task.to_string()])?;
        }
        result.status = "cancelled".into();
        result.answer = None;
        result.evidence.clear();
        result.verification_state = "not_verified".into();
        repo.replace_result(&scope, &result)?;
    }
    if matches!(state.as_str(), "cancel_requested" | "cancelled") {
        result.status = "cancelled".into();
        result.answer = None;
        result.evidence.clear();
    }
    Ok(Some(result))
}

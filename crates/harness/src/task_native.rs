//! Native MCP's durable handoff consumer. It uses the same workflow service,
//! model pool and global task budgets as the owner API, never a second engine.
use crate::{company, Shared};
use anyhow::{Context, Result};
use gents_local_mcp::task_engine::native;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::json;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};
use uuid::Uuid;

struct Actions {
    launch: Vec<native::Packet>,
    cancel: BTreeSet<Uuid>,
}
fn terminal(
    db: &rusqlite::Connection,
    packet: &native::Packet,
    status: &str,
    detail: &str,
    uncertain: bool,
) -> Result<()> {
    let mut result = packet.base.clone();
    result.status = status.into();
    result.answer = None;
    result.evidence.clear();
    result.verification_state = "not_verified".into();
    result.blockers = vec![detail.into()];
    if uncertain {
        result.measurements.generative_model_calls = None;
        result.measurements.input_tokens = None;
        result.measurements.output_tokens = None;
    }
    let subject = native::subject(&packet.custody.scope);
    db.execute(
        "UPDATE task_records_v1 SET result=?4 WHERE tenant=?1 AND subject=?2 AND id=?3",
        params![
            packet.custody.scope.company,
            subject,
            packet.request.task_id.to_string(),
            serde_json::to_string(&result)?
        ],
    )?;
    db.execute(
        "UPDATE task_native_queue_v1 SET state=?4 WHERE tenant=?1 AND subject=?2 AND task=?3",
        params![
            packet.custody.scope.company,
            subject,
            packet.request.task_id.to_string(),
            status
        ],
    )?;
    db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'task_native_terminal',?2,?3)",params![packet.custody.scope.run.to_string(),json!({"task_id":packet.request.task_id,"status":status,"detail":detail,"uncertain_attempts":uncertain,"replayed":false}).to_string(),crate::now_ms() as i64])?;
    Ok(())
}
fn companies(root: &Path, allowed: &BTreeSet<String>) -> Result<Vec<(String, String)>> {
    let mut found = vec![];
    for id in allowed {
        let path = company::directory(root, id)?;
        let file = path.join("brain.sqlite3");
        if !file.is_file() || file.is_symlink() {
            continue;
        }
        let db = rusqlite::Connection::open_with_flags(
            file,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        if let Ok(owner) = db.query_row("SELECT owner FROM company LIMIT 1", [], |r| r.get(0)) {
            found.push((id.clone(), owner));
        }
    }
    Ok(found)
}
fn scan_company(
    root: &Path,
    instance: Uuid,
    runtime: &str,
    settings: &crate::task_workflows::Settings,
    active: &BTreeSet<Uuid>,
    company: &str,
    owner: &str,
) -> Result<Actions> {
    let mut actions = Actions {
        launch: vec![],
        cancel: BTreeSet::new(),
    };
    let mut db = company::connect(root, company, owner, false)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let now = crate::now_ms();
    if native::lease(&tx)?.is_some_and(|l| l.instance != instance && l.expires_at_ms > now) {
        return Ok(actions);
    }
    let current = native::lease(&tx)?;
    if current.is_none_or(|l| l.instance != instance || l.expires_at_ms < now + 4000) {
        native::publish(
            &tx,
            &native::Lease {
                instance,
                runtime: runtime.into(),
                expires_at_ms: now + 8000,
                pending_limit: (settings.max_pending_jobs * 8).min(64) as u32,
                deadline_seconds: settings.deadline_seconds,
                model_intake: settings
                    .adaptive
                    .as_ref()
                    .is_some_and(|a| a.intake.is_some()),
            },
        )?;
    }
    let rows:Vec<(i64,String,String,String,Option<String>)>=tx.prepare("SELECT rowid,body,sha256,state,job FROM task_native_queue_v1 WHERE state IN ('queued','claimed','running','cancel_requested') ORDER BY created,task LIMIT 64")?.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?.collect::<rusqlite::Result<_>>()?;
    for (rowid, body, sha, state, job) in rows {
        let packet = serde_json::from_str::<native::Packet>(&body).ok();
        if gents_local_mcp::digest(body.as_bytes()) != sha
            || packet.as_ref().is_none_or(|p| {
                p.custody.scope.company != company || p.custody.scope.owner != owner
            })
        {
            tx.execute(
                "UPDATE task_native_queue_v1 SET state='corrupt' WHERE rowid=?1",
                [rowid],
            )?;
            continue;
        }
        let packet = packet.context("checked native packet missing")?;
        let job = job.map(|j| Uuid::parse_str(&j)).transpose()?;
        if state == "cancel_requested" {
            if let Some(id) = job {
                tx.execute("UPDATE task_workflow_subscribers_v1 SET cancelled=1 WHERE tenant=?1 AND subject=?2 AND task=?3",params![company,native::subject(&packet.custody.scope),packet.request.task_id.to_string()])?;
                let remaining:i64=tx.query_row("SELECT COUNT(*) FROM task_workflow_subscribers_v1 WHERE job=?1 AND cancelled=0",[id.to_string()],|r|r.get(0))?;
                if remaining == 0 {
                    actions.cancel.insert(id);
                }
            }
            terminal(
                &tx,
                &packet,
                "cancelled",
                "Subscriber cancelled; other compatible subscribers retain their work.",
                job.is_some(),
            )?;
            continue;
        }
        if packet.custody.worker_instance != instance {
            terminal(
                &tx,
                &packet,
                "interrupted",
                "Original worker instance ended; inspect checkpoint. No automatic replay.",
                state != "queued",
            )?;
            continue;
        }
        if let Some(id) = job {
            let execution: Option<String> = tx
                .query_row(
                    "SELECT state FROM task_workflow_jobs_v1 WHERE id=?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            if execution.as_deref() == Some("completed") {
                tx.execute(
                    "UPDATE task_native_queue_v1 SET state='completed' WHERE task=?1",
                    [packet.request.task_id.to_string()],
                )?;
                continue;
            }
            if !active.contains(&id) {
                terminal(
                    &tx,
                    &packet,
                    "interrupted",
                    "Original executor handle is unavailable; no inference replay.",
                    true,
                )?;
                continue;
            }
        }
        if native::authorize(&tx, &packet.custody, now).is_err() {
            if let Some(id) = job {
                actions.cancel.insert(id);
            }
            terminal(
                &tx,
                &packet,
                "evidence_invalidated",
                "Native epoch, approval, parent state, deadline or host configuration changed.",
                job.is_some(),
            )?;
            continue;
        }
        if state == "queued" && actions.launch.len() < settings.max_pending_jobs {
            let changed=tx.execute("UPDATE task_native_queue_v1 SET state='claimed' WHERE tenant=?1 AND subject=?2 AND task=?3 AND state='queued'",params![company,native::subject(&packet.custody.scope),packet.request.task_id.to_string()])?;
            if changed == 1 {
                actions.launch.push(packet);
            }
        }
    }
    tx.commit()?;
    Ok(actions)
}
fn scan(
    root: &Path,
    instance: Uuid,
    runtime: &str,
    settings: &crate::task_workflows::Settings,
    active: &BTreeSet<Uuid>,
) -> Result<Actions> {
    let mut actions = Actions {
        launch: vec![],
        cancel: BTreeSet::new(),
    };
    for (company, owner) in companies(root, &settings.native_companies)? {
        match scan_company(root,instance,runtime,settings,active,&company,&owner) {
            Ok(company_actions)=>{actions.launch.extend(company_actions.launch);actions.cancel.extend(company_actions.cancel);}
            Err(_)=>eprintln!("Native company queue observation failed; its transaction rolled back and no claims were replayed"),
        }
    }
    Ok(actions)
}
fn launched(
    root: &Path,
    packet: &native::Packet,
    outcome: Result<serde_json::Value>,
) -> Result<()> {
    let mut db = company::connect(
        root,
        &packet.custody.scope.company,
        &packet.custody.scope.owner,
        false,
    )?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let state: String = tx.query_row(
        "SELECT state FROM task_native_queue_v1 WHERE task=?1",
        [packet.request.task_id.to_string()],
        |r| r.get(0),
    )?;
    match outcome {
        Ok(value) => {
            let job = value["job_id"]
                .as_str()
                .context("native workflow job id missing")?;
            let next = if state == "cancel_requested" {
                "cancel_requested"
            } else if value["result"]["status"] == "running" {
                "running"
            } else {
                "completed"
            };
            tx.execute(
                "UPDATE task_native_queue_v1 SET state=?2,job=?3 WHERE task=?1",
                params![packet.request.task_id.to_string(), next, job],
            )?;
            tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'task_native_admitted',?2,?3)",params![packet.custody.scope.run.to_string(),json!({"task_id":packet.request.task_id,"job_id":job,"actor":packet.custody.scope.actor,"policy_sha256":packet.custody.policy_sha256,"instance":packet.custody.worker_instance,"generation_established":false}).to_string(),crate::now_ms() as i64])?;
        }
        Err(error) => terminal(
            &tx,
            packet,
            if state == "cancel_requested" {
                "cancelled"
            } else {
                "failed"
            },
            &error.to_string(),
            false,
        )?,
    }
    tx.commit()?;
    Ok(())
}
pub async fn monitor(app: Shared, root: PathBuf) {
    let Some(settings) = app.config.task_workflows.clone().filter(|s| s.native_mcp) else {
        return;
    };
    let instance = Uuid::new_v4();
    let runtime = match serde_json::to_vec(&app.config) {
        Ok(v) => gents_local_mcp::digest(&v),
        Err(_) => return,
    };
    while !app.shutting_down.load(Ordering::Relaxed) {
        let (r, s, rt, active) = (
            root.clone(),
            settings.clone(),
            runtime.clone(),
            app.task_workflows.active_ids().await,
        );
        match tokio::task::spawn_blocking(move || scan(&r, instance, &rt, &s, &active)).await {
            Ok(Ok(actions)) => {
                for id in actions.cancel {
                    app.task_workflows.signal(id).await;
                }
                for packet in actions.launch {
                    let outcome = app
                        .task_workflows
                        .start_native(app.clone(), root.clone(), packet.clone())
                        .await;
                    let r = root.clone();
                    if !matches!(
                        tokio::task::spawn_blocking(move || launched(&r, &packet, outcome)).await,
                        Ok(Ok(()))
                    ) {
                        eprintln!("Native task handoff checkpoint failed; no launch replay");
                    }
                }
            }
            _ => eprintln!(
                "Native task worker observation failed; no unverified queue entries executed"
            ),
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gents_local_mcp::{governance, Phase, Registry, Scope};
    struct Fixture {
        root: PathBuf,
        scope: Scope,
        settings: crate::task_workflows::Settings,
        instance: Uuid,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("gents-native-worker-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            company::operation(
                &root,
                "one",
                "owner",
                &json!({"op":"register","name":"Fixture"}),
            )
            .unwrap();
            let run = Uuid::new_v4();
            let intake = json!({"idea":"Native queue fixture","acceptance":"Bounded execution","exclusions":"Paid services","timeline":"Synthetic","company_signer":"Fixture","provider_signer":"Fixture","budget_usd":0,"max_model_calls":4});
            company::operation(
                &root,
                "one",
                "owner",
                &json!({"op":"intake","run_id":run,"intake":intake}),
            )
            .unwrap();
            let db = company::connect(&root, "one", "owner", false).unwrap();
            let digest: String = db
                .query_row(
                    "SELECT digest FROM jobs WHERE run_id=?1",
                    [run.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            company::operation(
                &root,
                "one",
                "owner",
                &json!({"op":"approve","run_id":run,"digest":digest}),
            )
            .unwrap();
            let access = json!({"tools":["execute_task","get_task_status","cancel_task"],"skills":[],"memory_from":[]});
            let policy=serde_json::from_value(json!({"revision":1,"master":"Worker","ceilings":{"Worker":access},"grants":{"Worker":access}})).unwrap();
            governance::save(&db, &run.to_string(), &policy, 0).unwrap();
            let instance = Uuid::new_v4();
            native::publish(
                &db,
                &native::Lease {
                    instance,
                    runtime: "runtime".into(),
                    expires_at_ms: crate::now_ms() + 8000,
                    pending_limit: 8,
                    deadline_seconds: 30,
                    model_intake: false,
                },
            )
            .unwrap();
            let settings=serde_json::from_value(json!({"models":{"fixture":{"model":{"provider":"mock","model":"fixture"},"sha256":null,"memory_estimate":{"weights_bytes":1,"adapters_bytes":0,"kv_bytes":1,"context_bytes":1,"runtime_bytes":1,"headroom_bytes":1,"provenance":"fixture"},"evaluated_profiles":["general"],"evaluation_reference":"fixture"}},"default_model":"fixture","max_calls":2,"total_generation_tokens":256,"per_call_tokens":128,"deadline_seconds":30,"max_retries":0,"max_pending_jobs":2,"concurrency":1,"resident_budget_bytes":1048576,"allow_parametric_answers":true,"native_mcp":true,"native_companies":["one"]})).unwrap();
            Self {
                root,
                scope: Scope {
                    company: "one".into(),
                    owner: "owner".into(),
                    run,
                    actor: "Worker".into(),
                    phase: Phase::Worker,
                    expected_revision: None,
                },
                settings,
                instance,
            }
        }
        fn queue(&self, ask: &str) -> Uuid {
            let authority = gents_local_mcp::default_authority(self.root.clone()).unwrap();
            let value = Registry::new()
                .call(
                    &authority,
                    &self.scope,
                    "execute_task",
                    json!({"input":{"original_ask":ask}}),
                )
                .unwrap();
            value["value"]["intake"]["request"]["task_id"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap()
        }
    }
    #[test]
    fn claims_are_once_only_and_restarted_workers_never_replay() {
        let f = Fixture::new();
        let task = f.queue("Inspect a synthetic source");
        let first = scan(
            &f.root,
            f.instance,
            "runtime",
            &f.settings,
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(first.launch.len(), 1);
        let second = scan(
            &f.root,
            f.instance,
            "runtime",
            &f.settings,
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(second.launch.is_empty());
        let db = company::connect(&f.root, "one", "owner", false).unwrap();
        let mut lease = native::lease(&db).unwrap().unwrap();
        lease.expires_at_ms = crate::now_ms() - 1;
        native::publish(&db, &lease).unwrap();
        let restarted = scan(
            &f.root,
            Uuid::new_v4(),
            "runtime",
            &f.settings,
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(restarted.launch.is_empty());
        let (_, state, _) = native::packet(&db, &f.scope, task).unwrap().unwrap();
        assert_eq!(state, "interrupted");
        let body: String = db
            .query_row(
                "SELECT result FROM task_records_v1 WHERE id=?1",
                [task.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        let result: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(result["measurements"]["generative_model_calls"].is_null());
    }
    #[test]
    fn corrupt_queue_entry_is_quarantined_without_poisoning_valid_work() {
        let f = Fixture::new();
        let bad = f.queue("First fixture");
        let good = f.queue("Second fixture");
        let db = company::connect(&f.root, "one", "owner", false).unwrap();
        db.execute(
            "UPDATE task_native_queue_v1 SET sha256='corrupt' WHERE task=?1",
            [bad.to_string()],
        )
        .unwrap();
        let work = scan(
            &f.root,
            f.instance,
            "runtime",
            &f.settings,
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(work.launch.len(), 1);
        assert_eq!(work.launch[0].request.task_id, good);
        assert_eq!(
            db.query_row(
                "SELECT state FROM task_native_queue_v1 WHERE task=?1",
                [bad.to_string()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "corrupt"
        );
    }
    #[test]
    fn one_company_failure_cannot_drop_another_committed_claim() {
        let mut f = Fixture::new();
        let task = f.queue("Healthy company request");
        company::operation(
            &f.root,
            "two",
            "owner",
            &json!({"op":"register","name":"Broken lease fixture"}),
        )
        .unwrap();
        let db = company::connect(&f.root, "two", "owner", false).unwrap();
        native::install(&db).unwrap();
        db.execute(
            "INSERT INTO task_native_worker_v1 VALUES(1,'invalid-json')",
            [],
        )
        .unwrap();
        f.settings.native_companies.insert("two".into());
        let work = scan(
            &f.root,
            f.instance,
            "runtime",
            &f.settings,
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(work.launch.len(), 1);
        assert_eq!(work.launch[0].request.task_id, task);
    }
}

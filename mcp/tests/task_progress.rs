//! Fixed journal metadata/privacy and scope checks; no model/runtime calls.
use gents_local_mcp::task_engine::{contracts::AccessScope, progress};
use gents_local_mcp::task_engine::{
    contracts::{ExecutionPolicy, TaskRequest},
    Repository,
};
use rusqlite::{params, Connection};
use serde_json::json;
use uuid::Uuid;

fn scope() -> AccessScope {
    AccessScope {
        tenant: "one".into(),
        subject: "owner".into(),
        effective_access_sha256: "access".into(),
        policy_revision: "1".into(),
        profile_revision: "1".into(),
        runtime_revision: "1".into(),
        conversation_sha256: "none".into(),
        allowed_sources: Default::default(),
    }
}
fn fixture() -> (Connection, Uuid, Uuid) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN IMMEDIATE;
        CREATE TABLE task_workflow_subscribers_v1(tenant TEXT,subject TEXT,task TEXT,job TEXT);
        CREATE TABLE task_workflow_jobs_v1(tenant TEXT,subject TEXT,id TEXT,state TEXT DEFAULT 'running');
        CREATE TABLE audit(seq INTEGER PRIMARY KEY,run_id TEXT,kind TEXT,body TEXT,created INTEGER);").unwrap();
    let (task, job) = (Uuid::new_v4(), Uuid::new_v4());
    db.execute(
        "INSERT INTO task_workflow_jobs_v1(tenant,subject,id) VALUES('one','owner',?1)",
        [job.to_string()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO task_workflow_subscribers_v1 VALUES('one','owner',?1,?2)",
        params![task.to_string(), job.to_string()],
    )
    .unwrap();
    (db, task, job)
}
fn event(db: &Connection, job: Uuid, kind: &str, body: serde_json::Value) {
    db.execute(
        "INSERT INTO audit(run_id,kind,body,created) VALUES(?1,?2,?3,100)",
        params![job.to_string(), kind, body.to_string()],
    )
    .unwrap();
}
#[test]
fn projection_deduplicates_call_ids_preserves_parallel_pending_and_never_exports_payloads() {
    let (db, task, job) = fixture();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    event(
        &db,
        job,
        "task_model_dispatch",
        json!({"call_id":a,"node_id":"draft","prompt":"PRIVATE_PAYLOAD"}),
    );
    event(
        &db,
        job,
        "task_model_dispatch",
        json!({"call_id":a,"node_id":"draft"}),
    );
    event(
        &db,
        job,
        "task_model_dispatch",
        json!({"call_id":b,"node_id":"review"}),
    );
    event(
        &db,
        job,
        "task_model_return",
        json!({"call_id":a,"node_id":"draft","text":"PRIVATE_PAYLOAD","local_execution":{"resident_kib":999}}),
    );
    event(
        &db,
        Uuid::new_v4(),
        "task_model_dispatch",
        json!({"call_id":Uuid::new_v4(),"node_id":"other_job"}),
    );
    let value = progress::read(&db, &scope(), task, "running", 200)
        .unwrap()
        .unwrap();
    assert_eq!(value.model_dispatches, Some(2));
    assert_eq!(value.model_returns, Some(1));
    assert_eq!(value.pending_model_requests, Some(1));
    assert_eq!(value.phase, "model_request_pending");
    assert_eq!(value.job_id, job);
    assert!(value.events.windows(2).all(|p| p[0].seq < p[1].seq));
    let serialized = serde_json::to_string(&value).unwrap();
    for absent in [
        "PRIVATE_PAYLOAD",
        "prompt",
        "local_execution",
        "resident_kib",
        "other_job",
    ] {
        assert!(!serialized.contains(absent), "{absent}");
    }
    for status in ["cancelled", "interrupted", "failed", "generated"] {
        let stopped = progress::read(&db, &scope(), task, status, 200)
            .unwrap()
            .unwrap();
        assert_eq!(stopped.pending_model_requests, Some(0));
        assert_eq!(stopped.unresolved_model_dispatches, Some(1));
        assert_ne!(stopped.phase, "model_request_pending");
    }
}
#[test]
fn subject_tenant_job_join_and_invalidated_evidence_cannot_borrow_metadata() {
    let (db, task, job) = fixture();
    event(
        &db,
        job,
        "task_model_dispatch",
        json!({"call_id":Uuid::new_v4()}),
    );
    for changed in [
        AccessScope {
            subject: "another-actor".into(),
            ..scope()
        },
        AccessScope {
            tenant: "two".into(),
            ..scope()
        },
    ] {
        assert!(progress::read(&db, &changed, task, "running", 200)
            .unwrap()
            .is_none());
    }
    assert!(
        progress::read(&db, &scope(), task, "evidence_invalidated", 200)
            .unwrap()
            .is_none()
    );
    assert!(
        progress::read(&db, &scope(), Uuid::new_v4(), "running", 200)
            .unwrap()
            .is_none()
    );
    db.execute(
        "UPDATE task_workflow_jobs_v1 SET subject='another-actor'",
        [],
    )
    .unwrap();
    assert!(progress::read(&db, &scope(), task, "running", 200)
        .unwrap()
        .is_none());
}
#[test]
fn malformed_or_truncated_journals_have_unknown_counts_not_false_idle_or_success() {
    let (db, task, job) = fixture();
    event(
        &db,
        job,
        "task_model_return",
        json!({"call_id":Uuid::new_v4(),"node_id":"<script>PRIVATE</script>"}),
    );
    let bad = progress::read(&db, &scope(), task, "running", 200)
        .unwrap()
        .unwrap();
    assert_eq!(bad.phase, "telemetry_unavailable");
    assert!(bad.model_dispatches.is_none());
    assert!(bad.pending_model_requests.is_none());
    assert!(!serde_json::to_string(&bad).unwrap().contains("PRIVATE"));
    db.execute("DELETE FROM audit", []).unwrap();
    for _ in 0..257 {
        event(
            &db,
            job,
            "task_model_dispatch",
            json!({"call_id":Uuid::new_v4()}),
        );
    }
    let bounded = progress::read(&db, &scope(), task, "running", 200)
        .unwrap()
        .unwrap();
    assert!(bounded.truncated);
    assert_eq!(bounded.events.len(), 16);
    assert!(bounded.model_dispatches.is_none());
    assert_eq!(bounded.phase, "telemetry_unavailable");
    db.execute("DELETE FROM audit", []).unwrap();
    db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'task_model_dispatch','broken JSON',100)",[job.to_string()]).unwrap();
    assert!(progress::read(&db, &scope(), task, "running", 200)
        .unwrap()
        .unwrap()
        .pending_model_requests
        .is_none());
}

#[test]
fn pending_and_cancelled_admissions_do_not_report_initial_zero_usage_as_measurement() {
    let (db, task, job) = fixture();
    let mut request = TaskRequest::from_input(
        serde_json::from_value(json!({"original_ask":"Bounded progress measurement fixture"}))
            .unwrap(),
    )
    .unwrap();
    request.task_id = task;
    let repo = Repository::new(&db).unwrap();
    let base = repo
        .execute(&request, &ExecutionPolicy::read_only(scope(), 100), 100)
        .unwrap();
    assert_eq!(base.measurements.generative_model_calls, Some(0));
    event(
        &db,
        job,
        "task_model_dispatch",
        json!({"call_id":Uuid::new_v4(),"node_id":"draft"}),
    );
    for status in ["running", "cancelled", "interrupted"] {
        let mut result = base.clone();
        result.status = status.into();
        progress::attach(&db, &scope(), &mut result, 200).unwrap();
        assert!(result.measurements.generative_model_calls.is_none());
        assert!(result.measurements.input_tokens.is_none());
        assert!(result.measurements.output_tokens.is_none());
        assert_eq!(
            result.progress.unwrap().pending_model_requests,
            Some(u32::from(status == "running"))
        );
    }
}

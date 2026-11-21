//! Deterministic provider-response fixtures; tests custody, not model quality.
use gents_local_mcp::task_engine::{contracts::*, grounded, KnowledgeSource, Repository};
use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeSet;

const NOW: u64 = 1000;
fn scope() -> AccessScope {
    AccessScope {
        tenant: "one".into(),
        subject: "alice".into(),
        effective_access_sha256: "access-v1".into(),
        policy_revision: "owner-v1".into(),
        profile_revision: "grounded-v1".into(),
        runtime_revision: "runtime-v1".into(),
        conversation_sha256: "context-independent".into(),
        allowed_sources: BTreeSet::from(["source".into()]),
    }
}
fn source() -> KnowledgeSource {
    KnowledgeSource {
        dependencies: Default::default(),
        source_id: "source".into(),
        revision: "v1".into(),
        title: "Synthetic widget manual".into(),
        domain: "widgets".into(),
        language: "en".into(),
        text: "The widget mode is bounded.\nThe batch size is seven items.\n".into(),
        observed_at_ms: 900,
        valid_until_ms: 100000,
        lookups: vec![],
    }
}
fn policy() -> ExecutionPolicy {
    let mut p = ExecutionPolicy::read_only(scope(), NOW);
    p.max_model_calls = 1;
    p.max_tokens = 512;
    p
}
fn request() -> TaskRequest {
    TaskRequest::from_input(
        serde_json::from_value(json!({"original_ask":"What is the widget mode?"})).unwrap(),
    )
    .unwrap()
}
fn repo(db: &Connection) -> Repository<'_> {
    Repository::new(db).unwrap()
}
fn setup() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    repo(&db).ingest(&scope(), &source(), NOW).unwrap();
    db
}
fn admit(db: &Connection, allow: bool) -> grounded::Admission {
    let r = request();
    let base = repo(db).execute(&r, &policy(), NOW).unwrap();
    grounded::admit(
        &repo(db),
        &r,
        &base,
        &policy(),
        "local/test-model",
        NOW,
        allow,
    )
    .unwrap()
    .unwrap()
}
fn wire() -> String {
    json!({"status":"answer","answer":"The widget mode is bounded.","citations":[{"passage":0,"quote":"widget mode is bounded"}],"missing_evidence":[]}).to_string()
}
fn usage() -> Measurements {
    Measurements {
        generative_model_calls: Some(1),
        embedding_calls: 0,
        tool_calls: 0,
        input_tokens: Some(10),
        output_tokens: Some(5),
        elapsed_micros: Some(100),
        provider_cost_usd: Some(0),
        memory_bytes: None,
    }
}
fn finish(db: &Connection, id: uuid::Uuid, wire: &str) -> TaskResult {
    grounded::finish(&repo(db), &scope(), id, Ok(wire), usage(), NOW + 1).unwrap()
}

#[test]
fn duplicates_share_one_admission_and_reuse_stays_outside_verified_cache() {
    let db = setup();
    let a = admit(&db, true);
    let b = admit(&db, false);
    assert!(a.launch);
    assert!(!b.launch);
    assert_eq!(a.job_id, b.job_id);
    assert_eq!(
        grounded::prompt(&a.job)["task"]["original_ask"],
        a.job.request.original_ask
    );
    let result = finish(&db, a.job_id, &wire());
    assert_eq!(result.status, "generated_unverified");
    assert_eq!(
        result.verification_state,
        "citations_checked_answer_unverified"
    );
    assert_eq!(result.evidence[0].quote, "widget mode is bounded");
    let second = repo(&db)
        .status(&scope(), b.job.request.task_id, NOW + 1)
        .unwrap()
        .unwrap();
    // b.job is the shared original; the second request identity is b.result.
    let second_actual = repo(&db)
        .status(&scope(), b.result.task_id, NOW + 1)
        .unwrap()
        .unwrap();
    assert_eq!(second.measurements.generative_model_calls, Some(1));
    assert_eq!(second_actual.measurements.generative_model_calls, Some(0));
    assert_eq!(second_actual.plan.unwrap().task_id, b.result.task_id);
    assert_eq!(
        repo(&db).inspect_cache(&scope()).unwrap()["exact_entries"],
        0
    );
    let c = admit(&db, false);
    assert!(!c.launch);
    assert_eq!(c.result.cache.decision, "unverified_draft_reuse");
    assert_eq!(
        c.result.verification_state,
        "citations_checked_answer_unverified"
    );
    assert_eq!(c.result.measurements.generative_model_calls, Some(0));
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM task_grounded_jobs_v1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn model_cannot_invent_quotes_passages_authority_or_claim_missing_support_is_answered() {
    let cases = [
        json!({"status":"answer","answer":"invented","citations":[{"passage":0,"quote":"not present"}],"missing_evidence":[]}),
        json!({"status":"answer","answer":"invented","citations":[{"passage":7,"quote":"widget"}],"missing_evidence":[]}),
        json!({"status":"answer","answer":"invented","citations":[],"missing_evidence":[]}),
        json!({"status":"answer","answer":"invented","citations":[{"passage":0,"quote":"widget"}],"missing_evidence":["missing support"]}),
        json!({"status":"insufficient","answer":"still answered","citations":[],"missing_evidence":["missing"]}),
        json!({"status":"answer","answer":"invented","citations":[{"passage":0,"quote":"widget"}],"missing_evidence":[],"tools":["shell"]}),
    ];
    for value in cases {
        let db = setup();
        let a = admit(&db, true);
        let r = finish(&db, a.job_id, &value.to_string());
        assert_eq!(r.status, "failed");
        assert!(r.answer.is_none());
        let again = admit(&db, true);
        assert!(!again.launch);
        assert_eq!(again.result.status, "failed");
    }
}

#[test]
fn insufficient_support_is_not_a_false_fact_or_an_accepted_answer() {
    let db = setup();
    let a = admit(&db, true);
    let r=finish(&db,a.job_id,&json!({"status":"insufficient","answer":"","citations":[],"missing_evidence":["The source does not establish the requested date."]}).to_string());
    assert_eq!(r.status, "evidence_missing");
    assert!(r.answer.is_none());
    assert_eq!(r.blockers.len(), 1);
    assert_eq!(
        repo(&db).inspect_cache(&scope()).unwrap()["exact_entries"],
        0
    );
}

#[test]
fn changed_corpus_revoked_scope_and_expired_sources_invalidate_generation() {
    for variant in 0..3 {
        let db = setup();
        let a = admit(&db, true);
        let mut p = scope();
        let now = match variant {
            0 => {
                let mut s = source();
                s.revision = "v2".into();
                s.text = "The widget mode changed.".into();
                repo(&db).ingest(&scope(), &s, NOW).unwrap();
                NOW + 1
            }
            1 => {
                p.allowed_sources.clear();
                NOW + 1
            }
            _ => 100000,
        };
        let r = grounded::finish(&repo(&db), &p, a.job_id, Ok(&wire()), usage(), now).unwrap();
        assert!(matches!(
            r.status.as_str(),
            "evidence_invalidated" | "timed_out"
        ));
        assert!(r.answer.is_none());
    }
}

#[test]
fn one_subscriber_cancel_does_not_cancel_another_subscriber() {
    let db = setup();
    let a = admit(&db, true);
    let b = admit(&db, false);
    assert!(
        !grounded::cancel(&repo(&db), &scope(), a.result.task_id, NOW)
            .unwrap()
            .1
    );
    finish(&db, a.job_id, &wire());
    let first = repo(&db)
        .status(&scope(), a.result.task_id, NOW + 1)
        .unwrap()
        .unwrap();
    let second = repo(&db)
        .status(&scope(), b.result.task_id, NOW + 1)
        .unwrap()
        .unwrap();
    assert_eq!(first.status, "cancelled");
    assert!(first.answer.is_none());
    assert_eq!(second.status, "generated_unverified");
}

#[test]
fn final_subscriber_cancel_requests_stop_and_late_reply_cannot_publish_answer() {
    let db = setup();
    let a = admit(&db, true);
    assert!(
        grounded::cancel(&repo(&db), &scope(), a.result.task_id, NOW)
            .unwrap()
            .1
    );
    let result = finish(&db, a.job_id, &wire());
    assert_eq!(result.status, "cancelled");
    assert!(result.answer.is_none());
    assert!(!admit(&db, false).launch);
}

#[test]
fn uncertain_admission_survives_reopening_without_dispatch_or_refund() {
    let db = setup();
    let a = admit(&db, true);
    let reopened = admit(&db, false);
    assert_eq!(reopened.job_id, a.job_id);
    assert!(!reopened.launch);
    assert_eq!(reopened.result.status, "running");
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM task_grounded_jobs_v1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let mut wrong = scope();
    wrong.subject = "mallory".into();
    assert!(grounded::job_for_task(&repo(&db), &wrong, a.result.task_id)
        .unwrap()
        .is_none());
}

#[test]
fn deadline_failure_and_checkpoint_corruption_never_become_successful_reuse() {
    let db = setup();
    let a = admit(&db, true);
    let r = grounded::finish(
        &repo(&db),
        &scope(),
        a.job_id,
        Ok(&wire()),
        usage(),
        a.job.deadline_ms,
    )
    .unwrap();
    assert_eq!(r.status, "timed_out");
    db.execute("UPDATE task_grounded_jobs_v1 SET result='{}'", [])
        .unwrap();
    let req = request();
    let base = repo(&db).execute(&req, &policy(), NOW).unwrap();
    assert!(grounded::admit(
        &repo(&db),
        &req,
        &base,
        &policy(),
        "local/test-model",
        NOW,
        true
    )
    .is_err());
}

#[test]
fn no_evidence_actions_and_extra_constraints_do_not_admit_a_generator() {
    for mode in 0..4 {
        let db = setup();
        let mut req = request();
        match mode {
            0 => req.input.intent = Intent::Action,
            1 => req
                .input
                .proposed_acceptance
                .push("Independent proof".into()),
            2 => {
                req.original_ask = "Unknown topic".into();
                req.objective = req.original_ask.clone();
                req.input.original_ask = req.original_ask.clone();
            }
            _ => req
                .input
                .supplied_context
                .insert("other".into(), "context".into())
                .map(|_| ())
                .unwrap_or(()),
        };
        let base = repo(&db).execute(&req, &policy(), NOW).unwrap();
        assert!(grounded::admit(
            &repo(&db),
            &req,
            &base,
            &policy(),
            "local/test-model",
            NOW,
            true
        )
        .unwrap()
        .is_none());
    }
}

#[test]
fn active_task_records_survive_normal_history_eviction() {
    let db = setup();
    let a = admit(&db, true);
    for _ in 0..1026 {
        repo(&db).execute(&request(), &policy(), NOW + 1).unwrap();
    }
    assert_eq!(
        repo(&db)
            .status(&scope(), a.result.task_id, NOW + 1)
            .unwrap()
            .unwrap()
            .status,
        "running"
    );
    finish(&db, a.job_id, &wire());
}

#[test]
fn untrusted_budget_and_excess_subscribers_cannot_expand_one_call_or_storage() {
    let db = setup();
    let mut p = policy();
    p.max_model_calls = 2;
    let req = request();
    let base = repo(&db).execute(&req, &p, NOW).unwrap();
    assert!(grounded::admit(&repo(&db), &req, &base, &p, "local/test-model", NOW, true).is_err());
    let a = admit(&db, true);
    for _ in 1..64 {
        assert!(!admit(&db, false).launch);
    }
    let req = request();
    let base = repo(&db).execute(&req, &policy(), NOW).unwrap();
    assert!(grounded::admit(
        &repo(&db),
        &req,
        &base,
        &policy(),
        "local/test-model",
        NOW,
        true
    )
    .is_err());
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM task_grounded_jobs_v1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        grounded::job_for_task(&repo(&db), &scope(), a.result.task_id).unwrap(),
        Some(a.job_id)
    );
}

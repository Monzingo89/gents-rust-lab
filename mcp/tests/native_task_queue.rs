use gents_local_mcp::{governance, task_engine::native, Registry};
use serde_json::{json, Value};
mod support;
use support::Fixture;
fn setup() -> Fixture {
    let f = Fixture::new();
    let db = f.db();
    let encoded = json!({"max_model_calls":12}).to_string();
    let sha = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            sha,
            json!({"owner":f.scope.owner,"sha256":sha,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    for map in [&mut policy.ceilings, &mut policy.grants] {
        for actor in ["Worker", "Reader"] {
            map.get_mut(actor).unwrap().tools.extend(
                [
                    "execute_task",
                    "get_task_status",
                    "cancel_task",
                    "destructure_task",
                    "ingest_sources",
                    "search_knowledge",
                    "list_capabilities",
                ]
                .into_iter()
                .map(str::to_owned),
            );
        }
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    native::publish(
        &db,
        &native::Lease {
            instance: uuid::Uuid::new_v4(),
            runtime: "frozen-host-config".into(),
            expires_at_ms: gents_local_mcp::now_ms() + 8000,
            pending_limit: 4,
            deadline_seconds: 30,
            model_intake: true,
        },
    )
    .unwrap();
    f
}
fn call(f: &Fixture, name: &str, args: Value) -> Value {
    Registry::new()
        .call(&f.authority, &f.scope, name, args)
        .unwrap()["value"]
        .clone()
}
#[test]
fn queue_freezes_native_custody_and_cancellation_never_creates_worker_authority() {
    let f = setup();
    let value = call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Describe a scoped source"}}),
    );
    assert_eq!(value["result"]["status"], "running");
    assert_eq!(value["result"]["verification_state"], "queued_not_executed");
    let id = value["intake"]["request"]["task_id"].as_str().unwrap();
    let db = f.db();
    let (packet, state, job) = native::packet(&db, &f.scope, id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(state, "queued");
    assert!(job.is_none());
    assert_eq!(packet.request.original_ask, "Describe a scoped source");
    assert_eq!(packet.custody.scope.actor, "Worker");
    assert_eq!(packet.custody.scope.expected_revision, Some(2));
    native::authorize(&db, &packet.custody, gents_local_mcp::now_ms()).unwrap();
    assert_eq!(
        call(&f, "get_task_status", json!({"task_id":id}))["status"],
        "running"
    );
    assert_eq!(
        call(&f, "cancel_task", json!({"task_id":id}))["status"],
        "cancelled"
    );
    assert_eq!(
        call(&f, "get_task_status", json!({"task_id":id}))["status"],
        "cancelled"
    );
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "execute_task",
            json!({"input":{"original_ask":"x"},"policy":{"max_model_calls":999}})
        )
        .is_err());
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM task_native_queue_v1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn native_queue_is_actor_private_and_cancel_requires_its_own_current_grant() {
    let f = setup();
    let value = call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Private request"}}),
    );
    let id = &value["intake"]["request"]["task_id"];
    let reader = Registry::new()
        .call(
            &f.authority,
            &f.actor("Reader"),
            "get_task_status",
            json!({"task_id":id}),
        )
        .unwrap();
    assert!(reader["value"].is_null());
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("cancel_task");
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "cancel_task", json!({"task_id":id}))
        .is_err());
    let status = call(&f, "get_task_status", json!({"task_id":id}));
    assert_eq!(status["status"], "evidence_invalidated");
    assert!(status["answer"].is_null());
    assert!(status["evidence"].as_array().unwrap().is_empty());
}
#[test]
fn cache_fast_path_precedes_queue_and_absent_worker_is_explicit() {
    let f = setup();
    let now = gents_local_mcp::now_ms();
    call(
        &f,
        "ingest_sources",
        json!({"source":{"source_id":"literal","revision":"v1","title":"Synthetic","domain":"general","language":"en","text":"v5","observed_at_ms":now,"valid_until_ms":now+60000,"lookups":[{"exact_ask":"Read the literal","temporal_scope":"","context_independent":true,"start_byte":0,"end_byte":2}]}}),
    );
    for expected in ["deterministic_lookup", "exact_answer_cache"] {
        let value = call(
            &f,
            "execute_task",
            json!({"input":{"original_ask":"Read the literal"}}),
        );
        assert_eq!(value["result"]["actual_path"], expected);
        assert_eq!(value["result"]["measurements"]["generative_model_calls"], 0);
    }
    let db = f.db();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM task_native_queue_v1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let mut lease = native::lease(&db).unwrap().unwrap();
    lease.expires_at_ms = now - 1;
    native::publish(&db, &lease).unwrap();
    let missing = call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Uncached ask"}}),
    );
    assert_eq!(missing["result"]["status"], "requires_planning");
    assert!(missing["result"]["blockers"]
        .to_string()
        .contains("native_mcp"));
}
#[test]
fn analysis_queue_and_parent_allowance_are_bounded_and_epoch_frozen() {
    let f = setup();
    let analyzed = call(
        &f,
        "destructure_task",
        json!({"input":{"original_ask":"Inspect this work"}}),
    );
    let id = analyzed["intake"]["request"]["task_id"].as_str().unwrap();
    let db = f.db();
    let (packet, _, _) = native::packet(&db, &f.scope, id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert!(packet.analysis_only);
    assert_eq!(packet.custody.action, "destructure_task");
    let mut lease = native::lease(&db).unwrap().unwrap();
    lease.instance = uuid::Uuid::new_v4();
    native::publish(&db, &lease).unwrap();
    assert!(native::authorize(&db, &packet.custody, gents_local_mcp::now_ms()).is_err());
    let mut changed = packet.custody.clone();
    changed.policy_sha256 = "0".repeat(64);
    assert!(native::authorize(&db, &changed, gents_local_mcp::now_ms()).is_err());
    let encoded = json!({}).to_string();
    let sha = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            sha,
            json!({"owner":f.scope.owner,"sha256":sha,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "execute_task",
            json!({"input":{"original_ask":"No owner call allowance"}})
        )
        .is_err());
}
#[test]
fn structured_native_request_retains_host_identity_and_skips_model_intake() {
    let f = setup();
    let request=gents_local_mcp::task_engine::contracts::TaskRequest::from_input(serde_json::from_value(json!({"original_ask":"Preserve the typed task","constraints":["Keep a concise answer"]})).unwrap()).unwrap();
    let id = request.task_id;
    let result = call(
        &f,
        "execute_task",
        json!({"request":request,"hints":{"evidence_required":false}}),
    );
    assert_eq!(result["intake"]["request"]["task_id"], id.to_string());
    let (packet, _, _) = native::packet(&f.db(), &f.scope, id).unwrap().unwrap();
    assert!(packet.skip_model_intake);
    assert_eq!(
        packet.request.input.constraints,
        vec!["Keep a concise answer"]
    );
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "execute_task",
            json!({"request":request,"input":{"original_ask":"Other ask"}})
        )
        .is_err());
}
#[test]
fn oversized_native_response_rolls_back_before_any_queue_admission() {
    let f = setup();
    let long = "Describe a synthetic fixture ".repeat(330);
    let failed = Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "execute_task",
            json!({"input":{"original_ask":long}}),
        )
        .unwrap_err();
    assert!(failed.to_string().contains("bounded frame"), "{failed}");
    let db = f.db();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM task_native_queue_v1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='task_records_v1')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    if exists {
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM task_records_v1", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
#[test]
fn queued_and_completed_native_status_invalidates_all_source_snapshot_changes() {
    let f = setup();
    let queued = call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Inspect source version"}}),
    );
    let task = &queued["intake"]["request"]["task_id"];
    let now = gents_local_mcp::now_ms();
    call(
        &f,
        "ingest_sources",
        json!({"source":{"source_id":"changed","revision":"v1","title":"New source","domain":"general","language":"en","text":"New contradictory context.","observed_at_ms":now,"valid_until_ms":now+60000,"lookups":[]}}),
    );
    let result = call(
        &f,
        "get_task_status",
        json!({"task_id":task,"view":"summary"}),
    );
    assert_eq!(result["status"], "evidence_invalidated");
    assert!(result["answer"].is_null());
    let queued = call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Second request"}}),
    );
    let task = queued["intake"]["request"]["task_id"].as_str().unwrap();
    let db = f.db();
    let mut result = queued["result"].clone();
    result["status"] = json!("generated");
    result["answer"] = json!("Synthetic draft");
    db.execute(
        "UPDATE task_records_v1 SET result=?2 WHERE id=?1",
        rusqlite::params![task, result.to_string()],
    )
    .unwrap();
    db.execute(
        "UPDATE task_native_queue_v1 SET state='completed' WHERE task=?1",
        [task],
    )
    .unwrap();
    assert_eq!(
        call(&f, "get_task_status", json!({"task_id":task}))["answer"],
        "Synthetic draft"
    );
    db.execute("DELETE FROM task_sources_v1 WHERE id='changed'", [])
        .unwrap();
    assert_eq!(
        call(&f, "get_task_status", json!({"task_id":task}))["status"],
        "evidence_invalidated"
    );
}
#[test]
fn pending_native_records_survive_unrelated_retention_cleanup() {
    let f = setup();
    let first = call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Keep queued original task"}}),
    );
    let id = first["intake"]["request"]["task_id"].as_str().unwrap();
    let db = f.db();
    db.execute("UPDATE task_records_v1 SET created=0 WHERE id=?1", [id])
        .unwrap();
    call(
        &f,
        "execute_task",
        json!({"input":{"original_ask":"Another independent task"}}),
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM task_records_v1 WHERE id=?1",
            [id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

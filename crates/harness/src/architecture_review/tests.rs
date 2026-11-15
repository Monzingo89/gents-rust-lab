//! Explicit simulated model receipts; not native task competence evidence.
use super::*;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
struct Hardware(AtomicUsize);
#[async_trait]
impl HardwarePort for Hardware {
    async fn capture(&self) -> Result<Value> {
        Ok(json!({"observation":self.0.fetch_add(1,Ordering::SeqCst),"gpu_memory_bytes":null}))
    }
}
struct Models {
    calls: AtomicUsize,
    mismatch: bool,
    revision: Option<Arc<AtomicU64>>,
}
#[async_trait]
impl ModelPort for Models {
    async fn complete(&self, judge: bool, packet: &Value) -> Result<Reply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if !judge {
            if let Some(rev) = &self.revision {
                rev.store(3, Ordering::SeqCst);
            }
        }
        let mut content = json!({"architecture":"ordered","agent_count":1,"rationale":"SIMULATED minimal known steps"});
        if packet["output_intent_only"] == true {
            content = json!({"modalities":packet["user_brief"]["SIMULATED_modalities"].as_array().cloned().unwrap_or_else(||vec![json!("text")])});
        } else if judge {
            content["verdict"] = json!("accept");
            if self.mismatch {
                content["agent_count"] = json!(2);
            }
        }
        Ok(serde_json::from_value(
            json!({"text":content.to_string(),"usage":{"input":10,"output":5,"cost_usd":0.0,"source":"SIMULATED"},"latency_ms":1,"local_execution":{"finish_reason":"stop","task_input_sha256":crate::company_factory::digest(packet)?,"backend_pid":1,"port":1,"dispatched_at_ms":1,"received_at_ms":2}}),
        )?)
    }
}
struct Memory {
    revision: Arc<AtomicU64>,
    states: Vec<String>,
}
impl JournalPort for Memory {
    fn save(&mut self, state: &str, _: &Value) -> Result<()> {
        self.states.push(state.into());
        Ok(())
    }
    fn current_revision(&self) -> Result<u64> {
        Ok(self.revision.load(Ordering::SeqCst))
    }
}
fn body() -> Value {
    json!({"id":Uuid::new_v4(),"planning_id":Uuid::new_v4(),"request":{"expected_revision":2,"speed":"patient","maximum_elapsed_seconds":1209600},"context":{"available_actors":["A","B"],"proposal":{"assignments":[{"actor":"A"}]},"configured_max_run_seconds":3600,"architecture_menu":[{"coordination":"ordered","executor":"actual-flow"}]},"status":"pending_ceo","hardware_observations":{},"dispatch":{},"receipts":{},"execution_authorized":false,"task_accepted":false})
}
#[tokio::test]
async fn two_opinions_refresh_hardware_without_granting_execution_or_inventing_speed() {
    let hardware = Hardware(AtomicUsize::new(0));
    let models = Models {
        calls: AtomicUsize::new(0),
        mismatch: false,
        revision: None,
    };
    let mut journal = Memory {
        revision: Arc::new(AtomicU64::new(2)),
        states: vec![],
    };
    let result = run(&models, &hardware, &mut journal, body()).await.unwrap();
    assert_eq!(result["status"], "ready_for_owner_review");
    assert_eq!(models.calls.load(Ordering::SeqCst), 3);
    assert_eq!(hardware.0.load(Ordering::SeqCst), 2);
    assert_eq!(result["execution_authorized"], false);
    assert_eq!(result["recommended_agent_count"], 1);
    assert!(result["speed_assessment"]["task_seconds_estimate"].is_null());
    assert_eq!(result["long_run_configuration_required"], true);
}
#[tokio::test]
async fn stale_planning_stops_judge_and_mismatched_acceptance_retains_usage() {
    let hardware = Hardware(AtomicUsize::new(0));
    let revision = Arc::new(AtomicU64::new(2));
    let models = Models {
        calls: AtomicUsize::new(0),
        mismatch: false,
        revision: Some(revision.clone()),
    };
    let mut journal = Memory {
        revision,
        states: vec![],
    };
    let result = run(&models, &hardware, &mut journal, body()).await.unwrap();
    assert_eq!(result["status"], "stale");
    assert_eq!(models.calls.load(Ordering::SeqCst), 1);
    let models = Models {
        calls: AtomicUsize::new(0),
        mismatch: true,
        revision: None,
    };
    journal.revision.store(2, Ordering::SeqCst);
    let result = run(&models, &hardware, &mut journal, body()).await.unwrap();
    assert_eq!(result["status"], "failed");
    assert_eq!(result["receipts"]["judge"]["reply"]["usage"]["input"], 10);
    assert_eq!(result["receipts"]["judge"]["accepted_protocol"], false);
}
#[test]
fn sqlite_records_are_owner_scoped_immutable_and_pending_is_not_replayed() {
    let root = std::env::temp_dir().join(format!("arch-review-{}", Uuid::new_v4()));
    crate::company::operation(
        &root,
        "fixture",
        "owner",
        &json!({"op":"register","name":"Fixture"}),
    )
    .unwrap();
    let mut b = body();
    let id = Uuid::parse_str(b["id"].as_str().unwrap()).unwrap();
    let planning = Uuid::parse_str(b["planning_id"].as_str().unwrap()).unwrap();
    let request: Request = serde_json::from_value(b["request"].clone()).unwrap();
    let db = crate::company::connect(&root, "fixture", "owner", false).unwrap();
    db.execute_batch(
        "CREATE TABLE ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)",
    )
    .unwrap();
    db.execute(
        "INSERT INTO ceo_planning VALUES(?1,2,'ready','{}')",
        [planning.to_string()],
    )
    .unwrap();
    let (mut journal, prior) =
        journal::SqliteJournal::open(&root, "fixture", "owner", id, &request).unwrap();
    assert!(prior.is_none());
    journal.bind(planning, &b).unwrap();
    assert!(journal::SqliteJournal::open(&root, "fixture", "owner", id, &request).is_err());
    assert!(read(&root, "fixture", "wrong", id).is_err());
    let mut changed = b.clone();
    changed["request"]["speed"] = json!("fast");
    assert!(journal.save("pending_ceo", &changed).is_err());
    b["status"] = json!("failed");
    // A valid wall-clock timestamp can accidentally satisfy Luhn. System
    // receipt metadata is typed provenance, not user-supplied payment data.
    b["receipts"]["ceo"]["reply"]["local_execution"]["received_at_ms"] = json!(1794578007004_u64);
    assert!(!crate::security::assess(&b.to_string()).persisted);
    journal.save("failed", &b).unwrap();
    let mut restricted = b.clone();
    restricted["recommendation"] = json!({"rationale":"Card 4111111111111111"});
    assert!(journal.save("failed", &restricted).is_err());
    drop(journal);
    let (_, prior) = journal::SqliteJournal::open(&root, "fixture", "owner", id, &request).unwrap();
    assert_eq!(prior.unwrap()["status"], "failed");
    db.execute("UPDATE architecture_assessments SET sha256='corrupt'", [])
        .unwrap();
    assert!(read(&root, "fixture", "owner", id).is_err());
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn factory_preparation_binds_the_canonical_review_and_rejects_a_forged_reference() {
    let root = std::env::temp_dir().join(format!("arch-factory-{}", Uuid::new_v4()));
    crate::company::operation(
        &root,
        "fixture",
        "owner",
        &json!({"op":"register","name":"Fixture"}),
    )
    .unwrap();
    let model = json!({"provider":"local","model":"fixture","path":"models/not-installed.gguf"});
    let policy = json!({"failures_before_next":2,"candidates":[{"id":"ceo","model":model,"sha256":"a".repeat(64),"minimum_memory_mib":512},{"id":"judge","model":{"provider":"local","model":"judge fixture","path":"models/other-not-installed.gguf"},"sha256":"b".repeat(64),"minimum_memory_mib":512}]});
    let config:crate::Config=serde_json::from_value(json!({"company_mode":true,"company_tools":true,"agents":[{"id":"A","assignment":"Fixture","planner":model,"worker":model}],"planning_model_policy":policy,"max_steps":1,"max_tokens":256})).unwrap();
    let mut b = body();
    let id = Uuid::parse_str(b["id"].as_str().unwrap()).unwrap();
    let planning = Uuid::parse_str(b["planning_id"].as_str().unwrap()).unwrap();
    let proposal = json!({"status":"ready","question":"","definition_of_done":["Return a concise synthetic summary"],"assignments":[{"actor":"A","task":"Summarize supplied text","tools":[],"acceptance":["Concise summary"]}]});
    b["context"]["proposal"] = proposal.clone();
    b["context"]["proposal_sha256"] = json!(crate::company_factory::digest(&proposal).unwrap());
    b["models"] = json!({"ceo":{"sha256":"a".repeat(64)},"judge":{"sha256":"b".repeat(64)},"policy_sha256":config.planning_model_policy.as_ref().unwrap().digest().unwrap()});
    let db = crate::company::connect(&root, "fixture", "owner", false).unwrap();
    db.execute_batch(
        "CREATE TABLE ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)",
    )
    .unwrap();
    db.execute(
        "INSERT INTO ceo_planning VALUES(?1,2,'ready',?2)",
        rusqlite::params![
            planning.to_string(),
            json!({"id":planning,"status":"ready","revision":2,"proposal":proposal}).to_string()
        ],
    )
    .unwrap();
    let request: Request = serde_json::from_value(b["request"].clone()).unwrap();
    let (mut journal, _) =
        journal::SqliteJournal::open(&root, "fixture", "owner", id, &request).unwrap();
    journal.bind(planning, &b).unwrap();
    let models = Models {
        calls: AtomicUsize::new(0),
        mismatch: false,
        revision: None,
    };
    let result = run(&models, &Hardware(AtomicUsize::new(0)), &mut journal, b)
        .await
        .unwrap();
    assert_eq!(result["status"], "ready_for_owner_review");
    drop(journal);
    let input = json!({"expected_revision":2,"specification":{"version":1,"objective":"Summarize a synthetic note","inputs":["Supplied synthetic text"],"outputs":["Concise text summary"],"definition_of_done":proposal["definition_of_done"],"exclusions":["Remote services"],"coordination":"ordered","data_classification":"synthetic","control_profiles":["local_baseline"],"required_capabilities":[]},"input":{"text":"Synthetic note"},"model_sha256":{"A":"a".repeat(64)},"max_delegations":1,"peer_rounds":1,"architecture_assessment":id});
    let prepared = crate::company_factory::preparation::prepare(
        &root,
        "fixture",
        "owner",
        planning,
        &config,
        input.clone(),
    )
    .unwrap();
    let mut intake = prepared["intake_patch"].clone();
    intake["ceo_plan"] = proposal;
    validate_intake(&root, "fixture", "owner", &intake).unwrap();
    intake["factory_binding"]["assessment_sha256"] = json!("0".repeat(64));
    assert!(validate_intake(&root, "fixture", "owner", &intake).is_err());
    let mut wrong = input;
    wrong["architecture_assessment"] = json!(Uuid::new_v4());
    assert!(crate::company_factory::preparation::prepare(
        &root, "fixture", "owner", planning, &config, wrong
    )
    .is_err());
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn inferred_media_needs_configured_engine_and_frozen_staffing_without_new_authority() {
    for (mode, configured, assigned, expected) in [
        ("image", true, true, "ready_for_owner_review"),
        ("audio", true, true, "ready_for_owner_review"),
        ("image", true, false, "media_plan_review_required"),
        ("video", false, false, "missing_capability"),
    ] {
        let hardware = Hardware(AtomicUsize::new(0));
        let models = Models {
            calls: AtomicUsize::new(0),
            mismatch: false,
            revision: None,
        };
        let mut journal = Memory {
            revision: Arc::new(AtomicU64::new(2)),
            states: vec![],
        };
        let mut input = body();
        input["context"]["user_brief"] = json!({"SIMULATED_modalities":[mode]});
        let tool = format!("{mode}_generate");
        input["context"]["media_capabilities"] = json!([{"tool":tool,"configured":configured}]);
        input["context"]["proposal"]["assignments"][0]["tools"] =
            if assigned { json!([tool]) } else { json!([]) };
        let frozen = input["context"].clone();
        let result = run(&models, &hardware, &mut journal, input).await.unwrap();
        assert_eq!(result["context"], frozen);
        assert_eq!(result["status"], expected);
        assert_eq!(result["media_routing"]["modalities"], json!([mode]));
        assert_eq!(result["execution_authorized"], false);
        assert_eq!(result["task_accepted"], false);
    }
}

struct GuidedModels(crate::company_factory::contracts::Coordination);
#[async_trait]
impl ModelPort for GuidedModels {
    async fn complete(&self, judge: bool, packet: &Value) -> Result<Reply> {
        let count = packet["context"]["proposal"]["assignments"]
            .as_array()
            .map_or(0, Vec::len);
        let mut content = if packet["output_intent_only"] == true {
            json!({"modalities":["text"]})
        } else {
            json!({"architecture":self.0,"agent_count":count,"rationale":"SIMULATED architecture for preparation safety tests"})
        };
        if judge && packet["output_intent_only"] != true {
            content["verdict"] = json!("accept");
        }
        Ok(serde_json::from_value(
            json!({"text":content.to_string(),"usage":{"input":10,"output":5,"cost_usd":0.0,"source":"SIMULATED"},"latency_ms":1,"local_execution":{"finish_reason":"stop","task_input_sha256":crate::company_factory::digest(packet)?,"backend_pid":1,"port":1,"dispatched_at_ms":1,"received_at_ms":2}}),
        )?)
    }
}

#[tokio::test]
async fn guided_factory_preserves_scope_and_has_no_authority_for_all_installed_patterns() {
    use crate::company_factory::{contracts::Coordination, guided};
    for (coordination, native) in [
        (Coordination::Ordered, false),
        (Coordination::AdaptiveDelegation, false),
        (Coordination::PeerReview, false),
        (Coordination::Ordered, true),
        (Coordination::AdaptiveDelegation, true),
    ] {
        let root = std::env::temp_dir().join(format!("guided-factory-{}", Uuid::new_v4()));
        crate::company::operation(
            &root,
            "fixture",
            "owner",
            &json!({"op":"register","name":"Synthetic guided safety fixture"}),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("models")).unwrap();
        let model_bytes = b"SIMULATED weights: hash admission only, never inference";
        std::fs::write(root.join("models/worker.gguf"), model_bytes).unwrap();
        let sha = gents_local_mcp::digest(model_bytes);
        let model =
            json!({"provider":"local","model":"SIMULATED worker","path":"models/worker.gguf"});
        let policy = json!({"failures_before_next":2,"candidates":[{"id":"ceo","model":model,"sha256":sha,"minimum_memory_mib":512},{"id":"judge","model":{"provider":"local","model":"SIMULATED judge","path":"models/judge.gguf"},"sha256":"b".repeat(64),"minimum_memory_mib":512}]});
        let config:crate::Config=serde_json::from_value(json!({"company_mode":true,"company_tools":true,"agents":[{"id":"A","assignment":"Draft","planner":model,"worker":model},{"id":"B","assignment":"Review","planner":model,"worker":model}],"planning_model_policy":policy,"max_steps":1,"max_tokens":256})).unwrap();
        let tools = if native {
            json!(["integer_arithmetic"])
        } else {
            json!([])
        };
        let proposal = json!({"status":"ready","question":"","definition_of_done":["Return the requested synthetic result"],"assignments":[{"actor":"A","task":"Draft the result","tools":tools,"acceptance":["Frozen criteria"]},{"actor":"B","task":"Review the result","tools":[],"acceptance":["Frozen criteria"]}]});
        let mut b = body();
        let id = Uuid::parse_str(b["id"].as_str().unwrap()).unwrap();
        let planning = Uuid::parse_str(b["planning_id"].as_str().unwrap()).unwrap();
        let prompt = "Use this exact sample:\n    A = 2\n    B = 3";
        let session = json!({"id":planning,"status":"ready","revision":2,"planning_mode":if native {Value::Null}else{json!("reasoning")},"messages":[{"role":"user","message":prompt},{"role":"user","kind":"planning_repair","message":"Internal repair must not become the objective"}],"proposal":proposal});
        b["context"]["proposal"] = proposal.clone();
        b["context"]["proposal_sha256"] = json!(crate::company_factory::digest(&proposal).unwrap());
        b["context"]["architecture_menu"] =
            json!([{"coordination":coordination,"executor":"SIMULATED installed descriptor"}]);
        b["models"] = json!({"ceo":{"sha256":sha},"judge":{"sha256":"b".repeat(64)},"policy_sha256":config.planning_model_policy.as_ref().unwrap().digest().unwrap()});
        let db = crate::company::connect(&root, "fixture", "owner", false).unwrap();
        db.execute_batch(
            "CREATE TABLE ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)",
        )
        .unwrap();
        db.execute(
            "INSERT INTO ceo_planning VALUES(?1,2,'ready',?2)",
            rusqlite::params![planning.to_string(), session.to_string()],
        )
        .unwrap();
        let request: Request = serde_json::from_value(b["request"].clone()).unwrap();
        let (mut journal, _) =
            journal::SqliteJournal::open(&root, "fixture", "owner", id, &request).unwrap();
        journal.bind(planning, &b).unwrap();
        let result = run(
            &GuidedModels(coordination),
            &Hardware(AtomicUsize::new(0)),
            &mut journal,
            b,
        )
        .await
        .unwrap();
        assert_eq!(result["status"], "ready_for_owner_review");
        drop(journal);
        let counts = ["jobs", "audit", "run_governance"].map(|table| {
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
        });
        let input = json!({"expected_revision":2,"architecture_assessment":id,"data_classification":"synthetic","artifact_bindings":[]});
        let mut frozen = None;
        let prepare =
            |input: Value| guided::prepare(&root, "fixture", "owner", planning, &config, input);
        if native && coordination != Coordination::Ordered {
            assert!(prepare(input)
                .unwrap_err()
                .to_string()
                .contains("tool-enabled hub/peer"));
        } else {
            let prepared = prepare(input.clone()).unwrap();
            assert_eq!(prepared["execution_authorized"], false);
            assert_eq!(prepared["candidate_only"], false);
            assert_eq!(
                !prepared["draft"]["intake"]["reasoning_acceptance"].is_null(),
                !native
            );
            assert_eq!(prepared["draft"]["task"], prompt);
            assert_eq!(
                prepared["blueprint"]["architecture"]["coordination"],
                json!(coordination)
            );
            assert_eq!(prepared["draft"]["intake"]["ceo_plan"], proposal);
            let mut public = input.clone();
            public["data_classification"] = json!("public");
            prepare(public).unwrap();
            let intake = &prepared["draft"]["intake"];
            frozen = Some(prepared["draft"].clone());
            guided::validate_frozen(intake).unwrap();
            if native {
                guided::validate_execution(&root, "fixture", "owner", &config, intake).unwrap();
                let mut changed = intake.clone();
                changed["native_factory_binding"]["model_sha256"]["B"] = json!("invalid");
                assert!(guided::validate_frozen(&changed).is_err());
                std::fs::write(root.join("models/worker.gguf"), b"Changed weights").unwrap();
                assert!(
                    guided::validate_execution(&root, "fixture", "owner", &config, intake).is_err()
                );
            } else {
                assert_eq!(
                    intake["coordination_contract"]["input"]["user_conversation"],
                    json!([prompt])
                );
                assert_eq!(
                    prepared["draft"]["policy"]["grants"]["A"]["tools"],
                    json!([])
                );
                crate::company_factory::preparation::validate_execution(&config, intake).unwrap();
                let mut changed = input.clone();
                changed["artifact_bindings"] = json!([{}]);
                assert!(prepare(changed).is_err());
            }
            for (key, value) in [
                ("expected_revision", json!(3)),
                ("architecture_assessment", json!(Uuid::new_v4())),
                ("data_classification", json!("regulated")),
                ("data_classification", json!("confidential")),
                ("architecture", json!("ordered")),
                ("model_sha256", json!({"A":"0".repeat(64)})),
            ] {
                let mut changed = input.clone();
                changed[key] = value;
                assert!(prepare(changed).is_err(), "accepted changed {key}");
            }
            assert!(
                guided::prepare(&root, "fixture", "other", planning, &config, input.clone())
                    .is_err()
            );
            let mut changed_config = config.clone();
            changed_config.agents[0].worker.model = "Changed worker config".into();
            assert!(
                guided::prepare(&root, "fixture", "owner", planning, &changed_config, input)
                    .is_err()
            );
        }
        assert_eq!(
            crate::ceo_planning::read(&root, "fixture", "owner", planning).unwrap(),
            session
        );
        for (table, count) in ["jobs", "audit", "run_governance"].into_iter().zip(counts) {
            assert_eq!(
                db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                count
            );
        }
        if let Some(draft) = frozen {
            let run = Uuid::new_v4();
            // This verifies integration with the ordinary frozen approval path,
            // without starting a model or executing the synthetic task.
            crate::company::operation(
                &root,
                "fixture",
                "owner",
                &json!({"op":"intake","run_id":run,"intake":draft["intake"]}),
            )
            .unwrap();
            let (state, encoded, digest): (String, String, String) = db
                .query_row(
                    "SELECT state,intake,digest FROM jobs WHERE run_id=?1",
                    [run.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(state, "draft");
            let actual: Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(actual["idea"], prompt);
            crate::company::operation(
                &root,
                "fixture",
                "owner",
                &json!({"op":"approve","run_id":run,"digest":digest}),
            )
            .unwrap();
            crate::company::operation(&root,"fixture","owner",&json!({"op":"governance_install","run_id":run,"expected_revision":0,"policy":draft["policy"]})).unwrap();
            let policy = crate::company::governance(&root, "fixture", "owner", run)
                .unwrap()
                .unwrap();
            if !native {
                crate::coordination::admission::validate_policy(&actual, &policy).unwrap();
            }
        }
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }
}

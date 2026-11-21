//! All receipts in this unit module are explicitly simulated protocol fixtures.
use super::*;
use crate::{
    company_factory::contracts::Coordination,
    governance::{Access, Policy},
    provider::{LocalExecution, Reply, Usage},
};
use program::{Role, Turn};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub(super) fn contract(pattern: Coordination) -> Contract {
    Contract {
        version: 1,
        coordination: pattern,
        objective: "Return sorted IDs from the supplied synthetic records".into(),
        definition_of_done: vec!["Return exactly the requested JSON ID list in sorted order".into()],
        input: json!({"records":[{"id":"z"},{"id":"b"},{"id":"m"}]}),
        roles: [
            ("Hub", "Integrate the requested result"),
            ("Worker", "Extract and sort IDs"),
            ("Critic", "Independently inspect the requested shape"),
        ]
        .map(|(actor, task)| Role {
            actor: actor.into(),
            assignment: task.into(),
            model_sha256: "a".repeat(64),
        })
        .to_vec(),
        max_delegations: 3,
        peer_rounds: 1,
    }
}
pub(super) fn intake(contract: &Contract) -> Value {
    json!({"idea":contract.objective,"acceptance":contract.definition_of_done.join("\n"),"exclusions":"Network and paid services","timeline":"Synthetic protocol fixture","company_signer":"Fixture owner","provider_signer":"Fixture provider","budget_usd":0,"deliverable_type":"text","coordination_contract":contract,"ceo_plan":{"status":"ready","definition_of_done":contract.definition_of_done,"assignments":contract.roles.iter().map(|r|json!({"actor":r.actor,"task":r.assignment,"tools":[],"acceptance":["Bounded contribution"]})).collect::<Vec<_>>()},"ceo_host_controls":admission::controls(contract).unwrap()})
}
pub(super) fn policy(contract: &Contract) -> Policy {
    let controls = admission::controls(contract).unwrap();
    let access = contract
        .roles
        .iter()
        .map(|r| {
            let memory = controls["handoff_from"][&r.actor]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_string())
                .collect();
            (
                r.actor.clone(),
                Access {
                    memory_from: memory,
                    ..Default::default()
                },
            )
        })
        .collect();
    Policy {
        revision: 1,
        master: contract.roles[0].actor.clone(),
        ceilings: access,
        grants: BTreeMap::new(),
    }
}
pub(super) struct AccessFixture {
    pub policy: Mutex<Policy>,
    pub sender: watch::Sender<u64>,
}
impl AccessFixture {
    pub fn new(contract: &Contract) -> Self {
        let mut policy = policy(contract);
        policy.grants = policy.ceilings.clone();
        Self {
            policy: Mutex::new(policy),
            sender: watch::channel(1).0,
        }
    }
}
impl AccessRepository for AccessFixture {
    fn policy(&self) -> Result<Policy> {
        Ok(self.policy.lock().unwrap().clone())
    }
    fn subscribe(&self) -> Result<watch::Receiver<u64>> {
        Ok(self.sender.subscribe())
    }
}
#[derive(Default)]
struct MemoryJournal {
    history: Vec<Message>,
    pending: bool,
}
impl JournalRepository for MemoryJournal {
    fn bind(&mut self, _: &Contract, _: u64) -> Result<()> {
        Ok(())
    }
    fn history(&self) -> Result<Vec<Message>> {
        ensure!(!self.pending, "ambiguous pending call");
        Ok(self.history.clone())
    }
    fn begin(&mut self, turn: &Turn, _: &str) -> Result<()> {
        ensure!(
            !self.pending && turn.ordinal == self.history.len(),
            "duplicate dispatch"
        );
        self.pending = true;
        Ok(())
    }
    fn finish(&mut self, message: &Message, _: &Reply) -> Result<()> {
        ensure!(self.pending, "no dispatch");
        self.pending = false;
        self.history.push(message.clone());
        Ok(())
    }
    fn reject(&mut self, _: &Turn, _: &Reply) -> Result<()> {
        Ok(())
    }
}
pub(super) fn receipt(packet: &Value, content: Value) -> Reply {
    Reply {
        text: content.to_string(),
        usage: Usage {
            input: Some(10),
            output: Some(5),
            cost_usd: Some(0.0),
            source: "SIMULATED protocol receipt".into(),
            ..Default::default()
        },
        latency_ms: 1,
        response_id: None,
        local_execution: Some(LocalExecution {
            finish_reason: Some("stop".into()),
            task_input_sha256: Some(crate::company_factory::digest(packet).unwrap()),
            backend_pid: 1,
            port: 1,
            dispatched_at_ms: 1,
            received_at_ms: 2,
            resident_kib: None,
            launch_policy: None,
            observed_context_tokens: None,
            generation_budget: None,
            separated_reasoning_observed: None,
            prompt_preflight: None,
        }),
    }
}
pub(super) struct Workers {
    calls: Mutex<Vec<(String, program::Purpose)>>,
    routes: Mutex<Vec<Value>>,
    fail: bool,
}
impl Workers {
    pub(super) fn new() -> Self {
        Self{calls:Mutex::new(vec![]),routes:Mutex::new(vec![json!({"action":"delegate","recipient":"Worker","assignment":"Extract the IDs"}),json!({"action":"delegate","recipient":"Critic","assignment":"Inspect format"}),json!({"action":"synthesize","recipient":"","assignment":""})].into_iter().rev().collect()),fail:false}
    }
}
#[async_trait]
impl WorkerPort for Workers {
    async fn complete(&self, turn: &Turn, packet: &Value, _: u64, _: &AtomicBool) -> Result<Reply> {
        self.calls
            .lock()
            .unwrap()
            .push((turn.actor.clone(), turn.purpose));
        ensure!(!self.fail, "simulated unreceived failure");
        let output = if turn.purpose == Purpose::Route {
            self.routes
                .lock()
                .unwrap()
                .pop()
                .context("no route fixture")?
        } else {
            json!({"deliverable":"SIMULATED candidate","evidence":if packet["contract_version"] == 2 { json!([packet["evidence_references"][0]]) } else { json!(["Protocol fixture"]) },"unknowns":[]})
        };
        Ok(receipt(packet, output))
    }
    async fn cancel(&self, _: &Turn, _: u64) {}
}

#[tokio::test]
async fn strategies_have_distinct_real_dispatch_shapes_and_return_unaccepted_candidates() {
    for (pattern, expected) in [
        (
            Coordination::Ordered,
            vec![
                ("Hub", Purpose::Contribute),
                ("Worker", Purpose::Contribute),
                ("Critic", Purpose::Contribute),
            ],
        ),
        (
            Coordination::AdaptiveDelegation,
            vec![
                ("Hub", Purpose::Route),
                ("Worker", Purpose::Delegate),
                ("Hub", Purpose::Route),
                ("Critic", Purpose::Delegate),
                ("Hub", Purpose::Route),
                ("Hub", Purpose::Synthesize),
            ],
        ),
        (
            Coordination::PeerReview,
            vec![
                ("Hub", Purpose::Contribute),
                ("Worker", Purpose::Critique),
                ("Critic", Purpose::Critique),
                ("Hub", Purpose::Revise),
            ],
        ),
    ] {
        let contract = contract(pattern);
        let access = AccessFixture::new(&contract);
        let workers = Workers::new();
        let mut journal = MemoryJournal::default();
        let result = Engine {
            workers: &workers,
            access: &access,
            journal: &mut journal,
        }
        .run(&contract, &AtomicBool::new(false))
        .await
        .unwrap();
        assert_eq!(result["task_accepted"], false);
        assert_eq!(
            workers.calls.lock().unwrap().clone(),
            expected
                .into_iter()
                .map(|(a, p)| (a.to_string(), p))
                .collect::<Vec<_>>()
        );
        let count = workers.calls.lock().unwrap().len();
        Engine {
            workers: &workers,
            access: &access,
            journal: &mut journal,
        }
        .run(&contract, &AtomicBool::new(false))
        .await
        .unwrap();
        assert_eq!(
            workers.calls.lock().unwrap().len(),
            count,
            "completed checkpoints must avoid new inference"
        );
    }
}

#[tokio::test]
async fn hub_relay_does_not_erase_cross_worker_lineage_permissions() {
    let contract = contract(Coordination::AdaptiveDelegation);
    let access = AccessFixture::new(&contract);
    access
        .policy
        .lock()
        .unwrap()
        .grants
        .get_mut("Critic")
        .unwrap()
        .memory_from
        .remove("Worker");
    let workers = Workers::new();
    let mut journal = MemoryJournal::default();
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    assert_eq!(
        workers
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|(a, _)| a.as_str())
            .collect::<Vec<_>>(),
        vec!["Hub", "Worker", "Hub"]
    );
    assert!(
        !journal.pending,
        "denied recipient must never be dispatched"
    );
}

#[tokio::test]
async fn missing_receipts_are_not_replayed_and_unknown_actors_cannot_be_dispatched() {
    let contract = contract(Coordination::AdaptiveDelegation);
    let access = AccessFixture::new(&contract);
    let mut workers = Workers::new();
    workers.fail = true;
    let mut journal = MemoryJournal::default();
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    assert!(journal.pending);
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    assert_eq!(workers.calls.lock().unwrap().len(), 1);
    let workers = Workers::new();
    *workers.routes.lock().unwrap() =
        vec![json!({"action":"delegate","recipient":"Foreign actor","assignment":"Cross scope"})];
    let mut journal = MemoryJournal::default();
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    assert_eq!(workers.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn owner_policy_change_cancels_the_exact_pending_generation() {
    struct Waiting {
        started: tokio::sync::Notify,
        cancelled: Arc<AtomicBool>,
    }
    #[async_trait]
    impl WorkerPort for Waiting {
        async fn complete(&self, _: &Turn, _: &Value, _: u64, _: &AtomicBool) -> Result<Reply> {
            self.started.notify_one();
            std::future::pending().await
        }
        async fn cancel(&self, _: &Turn, _: u64) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    let contract = contract(Coordination::PeerReview);
    let access = AccessFixture::new(&contract);
    let cancelled = Arc::new(AtomicBool::new(false));
    let workers = Waiting {
        started: tokio::sync::Notify::new(),
        cancelled: cancelled.clone(),
    };
    let mut journal = MemoryJournal::default();
    let mut engine = Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal,
    };
    let cancel = AtomicBool::new(false);
    let work = engine.run(&contract, &cancel);
    let revoke = async {
        workers.started.notified().await;
        access.policy.lock().unwrap().revision = 2;
        access.sender.send_replace(2);
    };
    let (result, _) = tokio::join!(work, revoke);
    assert!(result.is_err());
    assert!(cancelled.load(Ordering::Relaxed));
    assert!(journal.pending);
}

#[test]
fn frozen_contract_and_native_receipt_boundaries_reject_authority_and_cutoffs() {
    let contract = contract(Coordination::PeerReview);
    let mut intake = intake(&contract);
    let mut policy = policy(&contract);
    policy.grants = policy.ceilings.clone();
    admission::validate_policy(&intake, &policy).unwrap();
    intake["ceo_plan"]["assignments"][0]["tools"] = json!(["files:read"]);
    assert!(admission::from_intake(&intake).is_err());
    let intake = super::tests::intake(&contract);
    policy
        .ceilings
        .get_mut("Hub")
        .unwrap()
        .tools
        .insert("files:read".into());
    assert!(admission::validate_policy(&intake, &policy).is_err());
    let mut reply = receipt(&json!({}), json!({}));
    reply.local_execution.as_mut().unwrap().finish_reason = Some("length".into());
    assert!(validate_receipt(&reply).is_err());
    reply.local_execution.as_mut().unwrap().finish_reason = Some("stop".into());
    reply.usage.input = None;
    assert!(validate_receipt(&reply).is_err());
    let mut contract = contract.clone();
    contract.roles[1].actor = contract.roles[0].actor.clone();
    assert!(contract.validate().is_err());
}

#[test]
fn changed_transitive_lineage_and_early_hub_finish_cannot_be_replayed_as_valid_history() {
    let contract = contract(Coordination::AdaptiveDelegation);
    let turn = program::next(&contract, &[]).unwrap().unwrap();
    let mut history = vec![Message {
        turn,
        content: json!({"action":"synthesize","recipient":"","assignment":""}),
        lineage: BTreeSet::new(),
    }];
    assert!(program::next(&contract, &history).is_err());
    history[0].content = json!({"action":"delegate","recipient":"Worker","assignment":"Extract"});
    history[0].lineage.insert("Foreign".into());
    assert!(program::next(&contract, &history).is_err());
}

pub(super) fn install(
    root: &std::path::Path,
    company: &str,
    run: uuid::Uuid,
    contract: &Contract,
) -> Result<()> {
    install_with_config(root, company, run, contract, None, None)
}

pub(super) fn install_prepared(
    root: &std::path::Path,
    company: &str,
    run: uuid::Uuid,
    contract: &Contract,
    config: &crate::Config,
) -> Result<()> {
    install_with_config(root, company, run, contract, Some(config), None)
}

pub(super) fn install_prepared_reviewed(
    root: &std::path::Path,
    company: &str,
    run: uuid::Uuid,
    contract: &Contract,
    config: &crate::Config,
    review: &review::contract::ReviewContract,
) -> Result<()> {
    install_with_config(root, company, run, contract, Some(config), Some(review))
}
pub(super) fn install_reviewed(
    root: &std::path::Path,
    company: &str,
    run: uuid::Uuid,
    contract: &Contract,
    review: &review::contract::ReviewContract,
) -> Result<()> {
    install_with_config(root, company, run, contract, None, Some(review))
}
fn install_with_config(
    root: &std::path::Path,
    company: &str,
    run: uuid::Uuid,
    contract: &Contract,
    config: Option<&crate::Config>,
    review: Option<&review::contract::ReviewContract>,
) -> Result<()> {
    let owner = "coordination-fixture-owner";
    crate::company::operation(
        root,
        company,
        owner,
        &json!({"op":"register","name":"Explicit synthetic coordination fixture"}),
    )?;
    let mut intake = intake(contract);
    if let Some(review) = review {
        intake["reasoning_acceptance"] = json!(review);
    }
    let sid = uuid::Uuid::new_v4();
    let db = crate::company::connect(root, company, owner, false)?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)")?;
    db.execute("INSERT INTO ceo_planning VALUES(?1,2,'ready',?2)",rusqlite::params![sid.to_string(),json!({"revision":2,"status":"ready","proposal":intake["ceo_plan"],"fixture":"SIMULATED CEO plan, not actual CEO inference"}).to_string()])?;
    intake["ceo_planning_id"] = json!(sid);
    intake["ceo_planning_revision"] = json!(2);
    if let Some(config) = config {
        let request = json!({"expected_revision":2,"specification":{"version":1,"objective":contract.objective,"inputs":["Supplied synthetic records"],"outputs":["Requested JSON text candidate"],"definition_of_done":contract.definition_of_done,"exclusions":[intake["exclusions"]],"coordination":contract.coordination,"data_classification":"synthetic","control_profiles":["local_baseline"],"required_capabilities":[]},"input":contract.input,"model_sha256":contract.roles.iter().map(|r|(r.actor.clone(),r.model_sha256.clone())).collect::<BTreeMap<_,_>>(),"max_delegations":contract.max_delegations,"peer_rounds":contract.peer_rounds});
        let prepared = crate::company_factory::preparation::prepare(
            root, company, owner, sid, config, request,
        )?;
        ensure!(
            prepared["intake_patch"]["coordination_contract"] == json!(contract),
            "prepared fixture program changed"
        );
        for (key, value) in prepared["intake_patch"]
            .as_object()
            .context("preparation patch missing")?
        {
            intake[key] = value.clone();
        }
    }
    let drafted = crate::company::operation(
        root,
        company,
        owner,
        &json!({"op":"intake","run_id":run,"intake":intake}),
    )?;
    let job = drafted["jobs"]
        .as_array()
        .context("fixture jobs missing")?
        .iter()
        .find(|j| j["run_id"] == run.to_string())
        .context("fixture job missing")?;
    if let Some(config) = config {
        crate::company_factory::preparation::validate_execution(config, &job["intake"])?;
    }
    crate::company::operation(
        root,
        company,
        owner,
        &json!({"op":"approve","run_id":run,"digest":job["digest"]}),
    )?;
    let mut policy = policy(contract);
    policy.grants = policy.ceilings.clone();
    crate::company::operation(
        root,
        company,
        owner,
        &json!({"op":"governance_install","run_id":run,"expected_revision":0,"policy":policy}),
    )?;
    crate::company::admit(root, company, owner, run, &contract.objective)?;
    Ok(())
}

#[tokio::test]
async fn canonical_sqlite_journal_fences_replay_detects_corruption_and_stages_per_actor_memory() {
    let root = std::env::temp_dir().join(format!("coordination-journal-{}", uuid::Uuid::new_v4()));
    let contract = contract(Coordination::Ordered);
    let run = uuid::Uuid::new_v4();
    install(&root, "fixture", run, &contract).unwrap();
    let access = AccessFixture::new(&contract);
    let workers = Workers::new();
    let mut journal =
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).unwrap();
    assert!(
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).is_err()
    );
    Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal,
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .unwrap();
    assert_eq!(journal.history().unwrap().len(), 3);
    drop(journal);
    let mut journal =
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).unwrap();
    Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal,
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .unwrap();
    assert_eq!(workers.calls.lock().unwrap().len(), 3);
    let db =
        crate::company::connect(&root, "fixture", "coordination-fixture-owner", false).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT COUNT(DISTINCT agent) FROM context WHERE run_id=?1",
            [run.to_string()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        3
    );
    db.execute(
        "UPDATE coordination_turns SET sha256='corrupted' WHERE run_id=?1 AND ordinal=0",
        [run.to_string()],
    )
    .unwrap();
    assert!(journal.history().is_err());
    drop(journal);
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn actual_sqlite_pending_generation_survives_controller_restart_without_replay() {
    let root = std::env::temp_dir().join(format!("coordination-pending-{}", uuid::Uuid::new_v4()));
    let run = uuid::Uuid::new_v4();
    let contract = contract(Coordination::PeerReview);
    install(&root, "fixture", run, &contract).unwrap();
    let access = AccessFixture::new(&contract);
    let mut workers = Workers::new();
    workers.fail = true;
    let mut journal =
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).unwrap();
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    drop(journal);
    let mut journal =
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).unwrap();
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    assert_eq!(workers.calls.lock().unwrap().len(), 1);
    drop(journal);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn revoked_epoch_cannot_commit_either_a_reply_or_a_memory_record() {
    let root = std::env::temp_dir().join(format!("coordination-revoked-{}", uuid::Uuid::new_v4()));
    let run = uuid::Uuid::new_v4();
    let contract = contract(Coordination::Ordered);
    install(&root, "fixture", run, &contract).unwrap();
    let mut journal =
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).unwrap();
    journal.bind(&contract, 1).unwrap();
    let turn = program::next(&contract, &[]).unwrap().unwrap();
    let packet = json!({"synthetic":"simulated dispatch"});
    journal
        .begin(&turn, &crate::company_factory::digest(&packet).unwrap())
        .unwrap();
    let db =
        crate::company::connect(&root, "fixture", "coordination-fixture-owner", false).unwrap();
    let mut policy = policy(&contract);
    policy.revision = 2;
    policy.grants = policy.ceilings.clone();
    crate::governance::save(&db, &run.to_string(), &policy, 1).unwrap();
    let content = json!({"deliverable":"SIMULATED output","evidence":[],"unknowns":[]});
    let message = Message {
        turn,
        content: content.clone(),
        lineage: BTreeSet::new(),
    };
    assert!(journal
        .finish(&message, &receipt(&packet, content))
        .is_err());
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM context WHERE run_id=?1",
            [run.to_string()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT state FROM coordination_turns WHERE run_id=?1",
            [run.to_string()],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "pending"
    );
    drop(journal);
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn routing_decoder_keeps_all_eligible_choices_and_disjoint_action_fields() {
    let contract = contract(Coordination::AdaptiveDelegation);
    let mut packet = json!({"roles":contract.roles,"delegated_turns":0,"max_delegations":3});
    let first = program::bound_route_schema(&packet).unwrap();
    assert_eq!(first["oneOf"].as_array().unwrap().len(), 1);
    assert_eq!(
        first["oneOf"][0]["properties"]["recipient"]["enum"],
        json!(["Worker", "Critic"])
    );
    packet["delegated_turns"] = json!(1);
    let later = program::bound_route_schema(&packet).unwrap();
    assert_eq!(later["oneOf"].as_array().unwrap().len(), 2);
    assert_eq!(later["oneOf"][1]["properties"]["recipient"]["const"], "");
    assert_eq!(later["oneOf"][1]["properties"]["assignment"]["const"], "");
    packet["delegated_turns"] = json!(3);
    assert!(program::bound_route_schema(&packet).is_err());
    packet["delegated_turns"] = json!(0);
    packet["roles"][1]["actor"] = json!("Hub");
    assert!(program::bound_route_schema(&packet).is_err());
}

#[tokio::test]
async fn rejected_native_reply_preserves_usage_and_hash_without_content_or_memory_admission() {
    let root = std::env::temp_dir().join(format!("coordination-rejected-{}", uuid::Uuid::new_v4()));
    let run = uuid::Uuid::new_v4();
    let contract = contract(Coordination::AdaptiveDelegation);
    install(&root, "fixture", run, &contract).unwrap();
    let access = AccessFixture::new(&contract);
    let workers = Workers::new();
    let invalid =
        json!({"action":"synthesize","recipient":"Worker","assignment":"INVALID_RAW_REPLY_MARKER"});
    *workers.routes.lock().unwrap() = vec![invalid.clone()];
    let mut journal =
        journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run).unwrap();
    assert!(Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal
    }
    .run(&contract, &AtomicBool::new(false))
    .await
    .is_err());
    let db =
        crate::company::connect(&root, "fixture", "coordination-fixture-owner", false).unwrap();
    let encoded: String = db
        .query_row(
            "SELECT receipt FROM coordination_turns WHERE run_id=?1",
            [run.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let record: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(record["status"], "rejected");
    assert_eq!(record["native_reply"]["usage"]["input"], 10);
    assert_eq!(
        record["rejected_output_sha256"],
        gents_local_mcp::digest(invalid.to_string().as_bytes())
    );
    assert!(!encoded.contains("INVALID_RAW_REPLY_MARKER"));
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM context WHERE run_id=?1",
            [run.to_string()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert!(journal.history().is_err());
    drop(journal);
    drop(db);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn typed_citation_programs_commit_and_replay_exact_references_for_each_pattern() {
    for pattern in [
        Coordination::Ordered,
        Coordination::AdaptiveDelegation,
        Coordination::PeerReview,
    ] {
        let mut contract = contract(pattern);
        contract.version = 2;
        let root =
            std::env::temp_dir().join(format!("typed-coordination-{}", uuid::Uuid::new_v4()));
        let run = uuid::Uuid::new_v4();
        install(&root, "fixture", run, &contract).unwrap();
        let access = AccessFixture::new(&contract);
        let workers = Workers::new();
        let mut journal =
            journal::SqliteJournal::open(&root, "fixture", "coordination-fixture-owner", run)
                .unwrap();
        let candidate = Engine {
            workers: &workers,
            access: &access,
            journal: &mut journal,
        }
        .run(&contract, &AtomicBool::new(false))
        .await
        .unwrap();
        assert_eq!(candidate["task_accepted"], false);
        let history = journal.history().unwrap();
        assert!(program::next(&contract, &history).unwrap().is_none());
        let calls = workers.calls.lock().unwrap().len();
        Engine {
            workers: &workers,
            access: &access,
            journal: &mut journal,
        }
        .run(&contract, &AtomicBool::new(false))
        .await
        .unwrap();
        assert_eq!(workers.calls.lock().unwrap().len(), calls);
        drop(journal);
        std::fs::remove_dir_all(root).unwrap();
    }
}

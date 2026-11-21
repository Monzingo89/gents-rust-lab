//! Quarantined analysis and global-budget adaptation; synthetic providers only.
use gents_local_mcp::task_engine::{
    adaptive::*,
    contracts::*,
    intake::{self, IntakeWorker, Packet, Proposal},
    resources::*,
    routing::{self, Hints},
    workflows::*,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
fn request(ask: &str) -> TaskRequest {
    TaskRequest::from_input(serde_json::from_value(json!({"original_ask":ask})).unwrap()).unwrap()
}
fn proposal() -> Proposal {
    serde_json::from_value(json!({"domain":"general","task_type":"answer","ambiguity":"clear","difficulty":"routine","evidence_required":false,"decomposition_useful":false,"work_shape":"atomic","subtasks":[],"relevant_literals":[],"environment_requirements":[],"assumptions":[],"essential_questions":[],"routing_justification":"fixture work dependency assessment"})).unwrap()
}
fn packet(r: &TaskRequest) -> Packet {
    Packet::new(
        r,
        Hints::default(),
        routing::profiles().keys().cloned().collect(),
        vec![],
    )
    .unwrap()
}
fn policy() -> ExecutionPolicy {
    let mut p = ExecutionPolicy::read_only(
        AccessScope {
            tenant: "t".into(),
            subject: "s".into(),
            effective_access_sha256: "a".into(),
            policy_revision: "p".into(),
            profile_revision: "r".into(),
            runtime_revision: "m".into(),
            conversation_sha256: "c".into(),
            allowed_sources: BTreeSet::new(),
        },
        gents_local_mcp::now_ms(),
    );
    p.max_model_calls = 8;
    p.max_tokens = 1024;
    p.evidence_required = false;
    p.max_fan_out = 2;
    p.max_depth = 2;
    p.max_concurrency = 1;
    p.deadline_ms = gents_local_mcp::now_ms() + 5000;
    p
}
fn memory() -> MemoryEstimate {
    MemoryEstimate {
        weights_bytes: 1,
        adapters_bytes: 0,
        kv_bytes: 1,
        context_bytes: 1,
        runtime_bytes: 1,
        headroom_bytes: 1,
        provenance: "fixture".into(),
    }
}
fn models() -> BTreeMap<String, ResolvedModel> {
    ["small", "fallback"]
        .into_iter()
        .map(|id| {
            (
                id.into(),
                ResolvedModel {
                    id: id.into(),
                    identity: format!("fixture/{id}@digest"),
                    memory: memory(),
                    local: true,
                    context_bytes: 65536,
                    evaluated_profiles: BTreeSet::from([
                        "intake".into(),
                        "general".into(),
                        "history".into(),
                    ]),
                },
            )
        })
        .collect()
}
fn plan(r: &TaskRequest, template: &str) -> ExecutionPlan {
    let mut p = routing::factory(r, template, "general", Some("small")).unwrap();
    p.resolved_models = models()
        .into_iter()
        .map(|(id, m)| (id, m.identity))
        .collect();
    p
}
struct Fixture {
    intakes: AtomicUsize,
    calls: Mutex<Vec<(String, usize)>>,
    broken_first: bool,
    invalid_intake_first: bool,
    insufficient_first: bool,
    proposal: Proposal,
}
impl Fixture {
    fn new() -> Self {
        Self {
            intakes: AtomicUsize::new(0),
            calls: Mutex::new(vec![]),
            broken_first: false,
            invalid_intake_first: false,
            insufficient_first: false,
            proposal: proposal(),
        }
    }
}
fn output(value: Value) -> ModelOutput {
    ModelOutput {
        text: value.to_string(),
        input_tokens: Some(10),
        output_tokens: Some(20),
        memory_bytes: Some(100),
        generative_model_calls: Some(0),
    }
}
impl IntakeWorker for Fixture {
    fn analyze<'a>(
        &'a self,
        _: &'a str,
        p: &'a Packet,
        _: u32,
        _: &'a AtomicBool,
    ) -> FutureResult<'a, ModelOutput> {
        Box::pin(async move {
            let n = self.intakes.fetch_add(1, Ordering::SeqCst);
            if self.invalid_intake_first && n == 0 {
                return Ok(output(json!({"permission":"all"})));
            }
            if n > 0 && self.invalid_intake_first {
                assert!(!p.repair_feedback.is_empty());
            }
            Ok(output(serde_json::to_value(&self.proposal)?))
        })
    }
}
impl Worker for Fixture {
    fn generate<'a>(
        &'a self,
        id: &'a str,
        input: &'a StageInput,
        _: u32,
        _: &'a AtomicBool,
    ) -> FutureResult<'a, ModelOutput> {
        Box::pin(async move {
            let mut calls = self.calls.lock().unwrap();
            calls.push((id.into(), input.evidence.len()));
            let n = calls.len();
            if self.broken_first && n == 1 {
                return Ok(output(json!({"invalid":true})));
            }
            if input.allowed_tools.iter().any(|t| t == "search_knowledge")
                && input.tool_results.is_empty()
            {
                return Ok(output(
                    json!({"status":"tool","answer":"","citations":[],"missing_evidence":[],"tool":{"name":"search_knowledge","arguments":{"query":"reference"}}}),
                ));
            }
            if self.insufficient_first && n == 1 {
                return Ok(output(
                    json!({"status":"insufficient","answer":"","citations":[],"missing_evidence":["need independent corroboration"],"tool":null}),
                ));
            }
            Ok(output(
                json!({"status":"answer","answer":"fixture answer","citations":if input.evidence.is_empty(){json!([])}else{json!([{"passage":0,"quote":input.evidence[0].quote}])},"missing_evidence":[],"tool":null}),
            ))
        })
    }
}
impl ReadTools for Fixture {
    fn call<'a>(&'a self, _: &'a ToolRequest) -> FutureResult<'a, Value> {
        Box::pin(async { Ok(json!({"evidence":[evidence()]})) })
    }
}
fn evidence() -> Evidence {
    serde_json::from_value(json!({"source_id":"source","source_revision":"v1","content_sha256":"hash","title":"fixture","quote":"source-supported fixture","start_byte":0,"end_byte":24,"observed_at_ms":1,"valid_until_ms":9999999999999u64})).unwrap()
}
fn settings() -> Settings {
    Settings {
        intake: Some(IntakeSettings {
            model: "small".into(),
            tokens: 128,
            max_repairs: 1,
        }),
        ..Settings::default()
    }
}
#[test]
fn proposal_cannot_create_authority_facts_or_cyclic_work() {
    let r = request("Inspect API in 2026; do not assume versions");
    let pkt = packet(&r);
    let p = proposal();
    p.validate(&pkt).unwrap();
    for bad in [
        json!({"endpoint":"http://elsewhere"}),
        json!({"max_tokens":50000}),
        json!({"task_id":"override"}),
    ] {
        let mut v = serde_json::to_value(&p).unwrap();
        v.as_object_mut()
            .unwrap()
            .extend(bad.as_object().unwrap().clone());
        assert!(serde_json::from_value::<Proposal>(v).is_err());
    }
    let mut p = proposal();
    p.relevant_literals = vec!["Angular 27".into()];
    assert!(p.validate(&pkt).is_err());
    p.relevant_literals.clear();
    p.subtasks=serde_json::from_value(json!([{"id":"a","objective":"inspect","dependencies":["b"]},{"id":"b","objective":"build","dependencies":["a"]}])).unwrap();
    assert!(p.validate(&pkt).is_err());
}
#[test]
fn route_work_not_length_and_keep_constraints() {
    for (ask, shape, decomp, template) in [
        (
            "Answer these 100 easy arithmetic questions",
            "atomic",
            false,
            "direct/v1",
        ),
        (
            "Audit isolation",
            "tool_investigation",
            true,
            "tool_loop/v1",
        ),
        (
            "Preserve cross-domain contract",
            "dependent_stages",
            true,
            "pipeline/v1",
        ),
    ] {
        let r = request(ask);
        let pkt = packet(&r);
        let mut v = serde_json::to_value(proposal()).unwrap();
        v["work_shape"] = json!(shape);
        v["decomposition_useful"] = json!(decomp);
        v["subtasks"] = json!([{"id":"a","objective":"inspect","dependencies":[]},{"id":"b","objective":"validate","dependencies":["a"]}]);
        let p: Proposal = serde_json::from_value(v).unwrap();
        let out =
            intake::apply(&r, &Hints::default(), &p, &pkt, &routing::profiles(), true).unwrap();
        assert_eq!(out.proposed_plan.template, template);
        assert_eq!(out.request.input, r.input);
        assert_eq!(out.request.task_id, r.task_id);
    }
    let r = request("Who was president in 1976?");
    let pkt = packet(&r);
    let mut p = proposal();
    p.domain = "history".into();
    assert!(
        intake::apply(&r, &Hints::default(), &p, &pkt, &routing::profiles(), true)
            .unwrap()
            .proposed_plan
            .evidence_required
    );
    let mut r = request("Summarize this");
    r.input = serde_json::from_value(serde_json::json!({
        "original_ask": r.input.original_ask,
        "constraints": ["Use citations"]
    }))
    .unwrap();
    let pkt = packet(&r);
    assert!(
        intake::apply(
            &r,
            &Hints::default(),
            &proposal(),
            &pkt,
            &routing::profiles(),
            true
        )
        .unwrap()
        .proposed_plan
        .evidence_required
    );
}
#[tokio::test]
async fn intake_repairs_and_workers_share_one_budget() {
    let mut f = Fixture::new();
    f.invalid_intake_first = true;
    let r = request("Explain a fictional greeting");
    let m = models();
    let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
    let executor = Executor {
        validators: None,
        worker: &f,
        tools: &f,
        scheduler: &scheduler,
        models: &m,
        limits: Limits {
            per_call_tokens: 128,
            ..Limits::default()
        },
        profiles: None,
    };
    let c = Coordinator {
        executor: &executor,
        intake_worker: &f,
        observer: &NoopObserver,
        settings: settings(),
        allow_parametric: true,
    };
    let mut p = policy();
    p.max_model_calls = 2;
    p.max_tokens = 256;
    let out = c
        .run(
            &r,
            &plan(&r, "rag/v1"),
            Hints::default(),
            &p,
            vec![],
            Arc::new(AtomicBool::new(false)),
            false,
        )
        .await
        .unwrap();
    assert_eq!(out.consumption.reserved_calls, 2);
    assert_eq!(out.intake.unwrap().attempts, 2);
    assert!(f.calls.lock().unwrap().is_empty());
    assert!(out.answer.is_none());
    assert_eq!(out.request.input, r.input);
}
#[tokio::test]
async fn analysis_only_and_missing_intent_do_not_dispatch_workers() {
    for missing in [false, true] {
        let mut f = Fixture::new();
        if missing {
            f.proposal.ambiguity = "essential_intent_missing".into();
            f.proposal.essential_questions = vec!["Which deliverable?".into()];
        }
        let r = request("Prepare a result");
        let m = models();
        let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
        let ex = Executor {
            validators: None,
            worker: &f,
            tools: &f,
            scheduler: &scheduler,
            models: &m,
            limits: Limits {
                per_call_tokens: 128,
                ..Limits::default()
            },
            profiles: None,
        };
        let c = Coordinator {
            executor: &ex,
            intake_worker: &f,
            observer: &NoopObserver,
            settings: settings(),
            allow_parametric: true,
        };
        let o = c
            .run(
                &r,
                &plan(&r, "rag/v1"),
                Hints::default(),
                &policy(),
                vec![],
                Arc::new(AtomicBool::new(false)),
                true,
            )
            .await
            .unwrap();
        assert_eq!(
            o.status,
            if missing {
                "needs_clarification"
            } else {
                "analyzed"
            }
        );
        assert_eq!(o.consumption.reserved_calls, 1);
        assert!(f.calls.lock().unwrap().is_empty());
    }
}
#[tokio::test]
async fn invalid_output_switches_compatible_model_without_resetting_budget() {
    let mut f = Fixture::new();
    f.broken_first = true;
    let r = request("Generate fixture");
    let m = models();
    let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
    let ex = Executor {
        validators: None,
        worker: &f,
        tools: &f,
        scheduler: &scheduler,
        models: &m,
        limits: Limits {
            per_call_tokens: 128,
            ..Limits::default()
        },
        profiles: None,
    };
    let c = Coordinator {
        executor: &ex,
        intake_worker: &f,
        observer: &NoopObserver,
        settings: Settings {
            intake: None,
            max_replans: 1,
            rules: vec![Escalation {
                on: StopReason::InvalidOutput,
                template: "direct/v1".into(),
                model: Some("fallback".into()),
            }],
        },
        allow_parametric: true,
    };
    let out = c
        .run(
            &r,
            &plan(&r, "direct/v1"),
            Hints::default(),
            &policy(),
            vec![],
            Arc::new(AtomicBool::new(false)),
            false,
        )
        .await
        .unwrap();
    assert_eq!(out.status, "generated");
    assert_eq!(out.consumption.reserved_calls, 2);
    assert_eq!(out.attempts.len(), 2);
    assert_eq!(
        out.attempts[1].plan.previous_plan,
        Some(out.attempts[0].plan.plan_id)
    );
    assert_eq!(out.attempts[1].plan.revision, 2);
    assert_eq!(out.attempts[1].plan.task_id, r.task_id);
    assert_eq!(f.calls.lock().unwrap()[1].0, "fallback");
}
#[tokio::test]
async fn insufficient_evidence_escalates_and_retains_passages() {
    let mut f = Fixture::new();
    f.insufficient_first = true;
    let r = request("Corroborate fixture");
    let m = models();
    let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
    let ex = Executor {
        validators: None,
        worker: &f,
        tools: &f,
        scheduler: &scheduler,
        models: &m,
        limits: Limits {
            per_call_tokens: 128,
            ..Limits::default()
        },
        profiles: None,
    };
    let c = Coordinator {
        executor: &ex,
        intake_worker: &f,
        observer: &NoopObserver,
        settings: Settings {
            intake: None,
            max_replans: 1,
            rules: vec![Escalation {
                on: StopReason::EvidenceInsufficient,
                template: "tool_loop/v1".into(),
                model: None,
            }],
        },
        allow_parametric: false,
    };
    let mut p = policy();
    p.evidence_required = true;
    let out = c
        .run(
            &r,
            &plan(&r, "rag/v1"),
            Hints::default(),
            &p,
            vec![evidence()],
            Arc::new(AtomicBool::new(false)),
            false,
        )
        .await
        .unwrap();
    assert_eq!(out.status, "generated");
    assert_eq!(out.attempts.len(), 2);
    assert_eq!(out.consumption.reserved_calls, 3);
    assert_eq!(out.consumption.tool_calls, 1);
    assert_eq!(f.calls.lock().unwrap()[1].1, 1);
}
#[test]
fn stopping_conditions_never_authorize_replans() {
    for reason in [
        StopReason::Cancelled,
        StopReason::Deadline,
        StopReason::BudgetExhausted,
        StopReason::SourceChanged,
    ] {
        assert!(Settings {
            intake: None,
            max_replans: 1,
            rules: vec![Escalation {
                on: reason,
                template: "hub/v1".into(),
                model: None
            }]
        }
        .validate(&models())
        .is_err());
    }
}

#[tokio::test]
async fn capacity_and_context_pressure_use_only_feasible_host_fallback() {
    for capacity in [true, false] {
        let f = Fixture::new();
        let r = request("Fixture fallback");
        let mut m = models();
        if capacity {
            m.get_mut("small").unwrap().memory.weights_bytes = 2 * 1024 * 1024;
        } else {
            m.get_mut("small").unwrap().context_bytes = 1;
        }
        let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
        let ex = Executor {
            validators: None,
            worker: &f,
            tools: &f,
            scheduler: &scheduler,
            models: &m,
            limits: Limits {
                per_call_tokens: 128,
                ..Limits::default()
            },
            profiles: None,
        };
        let c = Coordinator {
            executor: &ex,
            intake_worker: &f,
            observer: &NoopObserver,
            settings: Settings {
                intake: None,
                max_replans: 1,
                rules: vec![Escalation {
                    on: if capacity {
                        StopReason::Capacity
                    } else {
                        StopReason::ContextPressure
                    },
                    template: "direct/v1".into(),
                    model: Some("fallback".into()),
                }],
            },
            allow_parametric: true,
        };
        let o = c
            .run(
                &r,
                &plan(&r, "direct/v1"),
                Hints::default(),
                &policy(),
                vec![],
                Arc::new(AtomicBool::new(false)),
                false,
            )
            .await
            .unwrap();
        assert_eq!(o.status, "generated");
        assert_eq!(o.attempts.len(), 2);
        assert_eq!(o.consumption.reserved_calls, 1);
        assert_eq!(
            f.calls.lock().unwrap().as_slice(),
            &[("fallback".into(), 0)]
        );
    }
}
#[tokio::test]
async fn retrieval_miss_escalates_without_spending_a_rag_generation() {
    let f = Fixture::new();
    let r = request("Retrieve fixture");
    let m = models();
    let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
    let ex = Executor {
        validators: None,
        worker: &f,
        tools: &f,
        scheduler: &scheduler,
        models: &m,
        limits: Limits {
            per_call_tokens: 128,
            ..Limits::default()
        },
        profiles: None,
    };
    let c = Coordinator {
        executor: &ex,
        intake_worker: &f,
        observer: &NoopObserver,
        settings: Settings {
            intake: None,
            max_replans: 1,
            rules: vec![Escalation {
                on: StopReason::EvidenceInsufficient,
                template: "tool_loop/v1".into(),
                model: None,
            }],
        },
        allow_parametric: false,
    };
    let mut p = policy();
    p.evidence_required = true;
    p.max_model_calls = 2;
    p.max_tokens = 256;
    let o = c
        .run(
            &r,
            &plan(&r, "rag/v1"),
            Hints::default(),
            &p,
            vec![],
            Arc::new(AtomicBool::new(false)),
            false,
        )
        .await
        .unwrap();
    assert_eq!(o.status, "generated");
    assert_eq!(o.consumption.reserved_calls, 2);
    assert!(o.attempts[0].outcome.as_ref().unwrap().receipts.is_empty());
    assert_eq!(o.consumption.tool_calls, 1);
}
struct EpochChanged;
impl Observer for EpochChanged {
    fn plan<'a>(
        &'a self,
        _: &'a TaskRequest,
        _: &'a ExecutionPlan,
        _: Option<&'a TaskAnalysis>,
        _: Option<&'a StopReason>,
        _: &'a Consumption,
    ) -> FutureResult<'a, ()> {
        Box::pin(async {
            Err(ExecutionFailure {
                kind: StopReason::SourceChanged,
                message: "fixture current epoch rejected".into(),
            }
            .into())
        })
    }
}
#[tokio::test]
async fn source_epoch_and_cancel_are_rechecked_before_dispatch() {
    let f = Fixture::new();
    let r = request("Fixture");
    let m = models();
    let scheduler = Scheduler::new(1, 1024 * 1024).unwrap();
    let ex = Executor {
        validators: None,
        worker: &f,
        tools: &f,
        scheduler: &scheduler,
        models: &m,
        limits: Limits {
            per_call_tokens: 128,
            ..Limits::default()
        },
        profiles: None,
    };
    let c = Coordinator {
        executor: &ex,
        intake_worker: &f,
        observer: &EpochChanged,
        settings: Settings::default(),
        allow_parametric: true,
    };
    assert!(c
        .run(
            &r,
            &plan(&r, "direct/v1"),
            Hints::default(),
            &policy(),
            vec![],
            Arc::new(AtomicBool::new(false)),
            false
        )
        .await
        .is_err());
    let c = Coordinator {
        observer: &NoopObserver,
        ..c
    };
    let o = c
        .run(
            &r,
            &plan(&r, "direct/v1"),
            Hints::default(),
            &policy(),
            vec![],
            Arc::new(AtomicBool::new(true)),
            false,
        )
        .await
        .unwrap();
    assert_eq!(o.status, "cancelled");
    assert_eq!(o.consumption.reserved_calls, 0);
    assert!(f.calls.lock().unwrap().is_empty());
}

#[test]
fn formatting_constraints_do_not_create_evidence_requirements() {
    let mut r = request("Write a fictional greeting");
    r.input.constraints = vec!["Two short sentences".into()];
    r.input.proposed_acceptance = vec!["Friendly tone".into()];
    assert!(!intake::mandatory_evidence(
        &r,
        &Hints::default(),
        "general",
        true
    ));
    r.input
        .proposed_acceptance
        .push("Use current citations".into());
    assert!(intake::mandatory_evidence(
        &r,
        &Hints::default(),
        "general",
        true
    ));
    assert!(intake::mandatory_evidence(
        &request("Inspect package metadata"),
        &Hints::default(),
        "angular",
        true
    ));
}

#[test]
fn structured_bypass_retains_existing_intent_questions_and_annotations() {
    let mut r = request("Prepare the requested deliverable");
    r.clarification_questions = vec!["Which deliverable?".into()];
    r.assumptions = vec!["Owner supplied assumption".into()];
    r.confirmed_facts = vec!["Owner supplied fact".into()];
    let out = routing::destructure_task(r.clone(), Hints::default()).unwrap();
    assert_eq!(out.request.task_id, r.task_id);
    assert_eq!(out.request.input, r.input);
    assert_eq!(
        out.request.clarification_questions,
        r.clarification_questions
    );
    assert_eq!(out.request.assumptions, r.assumptions);
    assert_eq!(out.request.confirmed_facts, r.confirmed_facts);
    assert_eq!(out.readiness, "needs_clarification");
}

#[test]
fn structured_annotations_do_not_bypass_the_existing_data_gate() {
    for field in 0..3 {
        let mut r = request("Prepare a bounded public deliverable");
        let values = match field {
            0 => &mut r.assumptions,
            1 => &mut r.confirmed_facts,
            _ => &mut r.clarification_questions,
        };
        values.push("Contact sample.person@example.org".into());
        assert!(r.validate().is_err());
    }
}

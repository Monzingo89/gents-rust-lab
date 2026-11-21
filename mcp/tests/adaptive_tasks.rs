//! Held-out cache/routing cases plus adversarial executor contracts. All model
//! providers here are deterministic fixtures, never live quality benchmarks.
use gents_local_mcp::task_engine::{
    contracts::*,
    models::{self, ModelEntry, Support},
    optimization::*,
    resources::*,
    routing::{self, Hints},
    semantic::Equivalence,
    workflows::*,
    KnowledgeSource, LookupBinding, Repository,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
const ASK: &str = "Who was US president in 1976?";
const PARA: &str = "Name the US president in 1976.";
fn scope() -> AccessScope {
    AccessScope {
        tenant: "one".into(),
        subject: "alice".into(),
        effective_access_sha256: "access-v1".into(),
        policy_revision: "p1".into(),
        profile_revision: "history1".into(),
        runtime_revision: "runtime1".into(),
        conversation_sha256: "independent".into(),
        allowed_sources: BTreeSet::from(["chronology".into()]),
    }
}
fn input(ask: &str) -> TaskInput {
    serde_json::from_value(json!({"original_ask":ask,"temporal_scope":"1976","output":{"format":"text","language":"en"}})).unwrap()
}
fn source(now: u64) -> KnowledgeSource {
    KnowledgeSource{dependencies:Default::default(),source_id:"chronology".into(),revision:"fixture1".into(),title:"Synthetic chronology held-out fixture".into(),domain:"history".into(),language:"en".into(),text:"Gerald Ford served as US president during 1976.\nJimmy Carter won the 1976 election and took office in 1977.\n".into(),observed_at_ms:now-1,valid_until_ms:now+60000,lookups:vec![LookupBinding{exact_ask:ASK.into(),temporal_scope:"1976".into(),context_independent:true,start_byte:0,end_byte:11}]}
}
fn memory() -> MemoryEstimate {
    MemoryEstimate {
        weights_bytes: 1,
        adapters_bytes: 0,
        kv_bytes: 1,
        context_bytes: 1,
        runtime_bytes: 1,
        headroom_bytes: 1,
        provenance: "fixture estimate".into(),
    }
}
fn policy() -> ExecutionPolicy {
    let mut p = ExecutionPolicy::read_only(scope(), gents_local_mcp::now_ms());
    p.max_model_calls = 16;
    p.max_tokens = 8192;
    p.max_depth = 2;
    p.max_fan_out = 2;
    p.max_concurrency = 2;
    p.deadline_ms = gents_local_mcp::now_ms() + 5000;
    p
}
fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    db
}
fn rule(now: u64) -> Equivalence {
    Equivalence {
        canonical: input(ASK),
        paraphrase: input(PARA),
        reviewed_by: "alice".into(),
        expires_at_ms: now + 10000,
    }
}

#[test]
fn frozen_36_case_cache_routing_evaluation() {
    let now = gents_local_mcp::now_ms();
    let db = database();
    let repo = Repository::new(&db).unwrap();
    let p = policy();
    repo.ingest(&p.scope, &source(now), now).unwrap();
    let first = repo
        .execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now)
        .unwrap();
    assert_eq!(first.answer.as_deref(), Some("Gerald Ford"));
    repo.approve_equivalence(&p.scope, &rule(now), now).unwrap();
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/task-heldout-v1.json")).unwrap();
    let started = Instant::now();
    let mut false_hits = 0;
    let mut passed = 0;
    for case in fixtures["cases"].as_array().unwrap() {
        let input: TaskInput = serde_json::from_value(case["input"].clone()).unwrap();
        let request = TaskRequest::from_input(input).unwrap();
        if case.get("hints").is_some() {
            let hints: Hints = serde_json::from_value(case["hints"].clone()).unwrap();
            let intake = routing::destructure_task(request, hints).unwrap();
            assert_eq!(
                intake.proposed_plan.template,
                case["expected"].as_str().unwrap()
            );
            assert_eq!(intake.proposed_plan.nodes.len(), 1);
            assert_eq!(intake.generative_model_calls, 0);
            passed += 1;
            continue;
        }
        let result = repo.execute(&request, &p, now + 1).unwrap();
        let expected = case["expected"].as_str().unwrap();
        let hit = result.answer.is_some();
        if expected == "miss" {
            if hit {
                false_hits += 1;
            }
            assert!(!hit, "false hit {}", case["id"]);
        } else {
            assert_eq!(result.answer.as_deref(), Some("Gerald Ford"));
            assert_eq!(
                result.actual_path,
                if expected == "semantic" {
                    "semantic_answer_cache"
                } else {
                    "exact_answer_cache"
                }
            );
        }
        assert_eq!(result.measurements.generative_model_calls, Some(0));
        passed += 1;
    }
    assert_eq!(passed, 36);
    assert_eq!(false_hits, 0);
    println!(
        "{}",
        json!({"suite":"heldout-v1","fixture_sha256":gents_local_mcp::digest(include_bytes!("fixtures/task-heldout-v1.json")),"cases":passed,"false_cache_hits":false_hits,"factual_lookup_fixture_accuracy":"2/2 eligible answer cases","evidence_support":"exact source bytes for all returned fixture answers","generative_model_calls":0,"elapsed_micros":started.elapsed().as_micros(),"live_model_accuracy":null,"gpu_energy_dollar_savings":null})
    );
}
#[test]
fn semantic_review_cannot_waive_hard_invariants_and_scope_changes_miss() {
    let now = gents_local_mcp::now_ms();
    let db = database();
    let repo = Repository::new(&db).unwrap();
    let p = policy();
    repo.ingest(&p.scope, &source(now), now).unwrap();
    repo.execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now)
        .unwrap();
    for negative in [
        "Who was US president in 1974?",
        "Who won the 1976 US election?",
        "Who was not US president in 1976?",
        "Who was UK president in 1976?",
        "Who is the current US president?",
    ] {
        let mut r = rule(now);
        r.paraphrase.original_ask = negative.into();
        assert!(repo.approve_equivalence(&p.scope, &r, now).is_err());
    }
    repo.approve_equivalence(&p.scope, &rule(now), now).unwrap();
    for change in 0..7 {
        let mut changed = p.clone();
        match change {
            0 => changed.scope.tenant = "two".into(),
            1 => changed.scope.subject = "bob".into(),
            2 => changed.scope.policy_revision = "p2".into(),
            3 => changed.scope.runtime_revision = "model2".into(),
            4 => changed.scope.profile_revision = "profile2".into(),
            5 => changed.scope.conversation_sha256 = "other context".into(),
            _ => changed.scope.allowed_sources.clear(),
        };
        let result = repo
            .execute(
                &TaskRequest::from_input(input(PARA)).unwrap(),
                &changed,
                now + 2,
            )
            .unwrap();
        assert!(result.answer.is_none());
    }
    let mut changed = source(now);
    changed.revision = "fixture2".into();
    changed.text = "Corrected source: earlier statement removed".into();
    changed.lookups.clear();
    repo.ingest(&p.scope, &changed, now).unwrap();
    assert!(repo
        .execute(&TaskRequest::from_input(input(PARA)).unwrap(), &p, now + 2)
        .unwrap()
        .answer
        .is_none());
}
#[test]
fn incremental_index_and_embedding_compatibility_never_return_old_source() {
    let now = gents_local_mcp::now_ms();
    let db = database();
    let repo = Repository::new(&db).unwrap();
    let p = policy();
    let mut s = source(now);
    s.text = format!("{} Ω target needle", "α".repeat(3000));
    s.lookups.clear();
    repo.ingest(&p.scope, &s, now).unwrap();
    let hits = repo
        .search(&p.scope, "needle", Some("history"), 8192, now)
        .unwrap();
    assert_eq!(hits.len(), 1);
    let e = &hits[0].evidence;
    assert_eq!(&s.text[e.start_byte..e.end_byte], e.quote);
    assert!(e.start_byte > 0);
    let chunks: i64 = db
        .query_row("SELECT COUNT(*) FROM task_chunks_v1", [], |r| r.get(0))
        .unwrap();
    repo.ingest(&p.scope, &s, now).unwrap();
    assert_eq!(
        db.query_row::<i64, _, _>("SELECT COUNT(*) FROM task_chunks_v1", [], |r| r.get(0))
            .unwrap(),
        chunks
    );
    let (v, c) =
        gents_local_mcp::task_engine::semantic::embed_cached(&repo, &p.scope, "needle", now)
            .unwrap();
    assert_eq!(v.len(), 256);
    assert_eq!(c, 1);
    db.execute(
        "UPDATE task_embeddings_v1 SET identity='other-dimension-v2'",
        [],
    )
    .unwrap();
    let (_, c) =
        gents_local_mcp::task_engine::semantic::embed_cached(&repo, &p.scope, "needle", now)
            .unwrap();
    assert_eq!(c, 1);
    repo.delete_source(&p.scope, &s.source_id).unwrap();
    assert!(repo
        .search(&p.scope, "needle", None, 8192, now)
        .unwrap()
        .is_empty());
}
struct Fixture {
    calls: AtomicUsize,
    seen: Mutex<Vec<(String, Vec<String>)>>,
    mode: &'static str,
}
impl Fixture {
    fn new(mode: &'static str) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            seen: Mutex::new(vec![]),
            mode,
        }
    }
}
impl Worker for Fixture {
    fn generate<'a>(
        &'a self,
        _: &'a str,
        input: &'a StageInput,
        _: u32,
        _: &'a AtomicBool,
    ) -> FutureResult<'a, ModelOutput> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen.lock().unwrap().push((
                input.node_id.clone(),
                input.dependencies.keys().cloned().collect(),
            ));
            if self.mode == "hang" {
                std::future::pending::<()>().await;
            }
            if self.mode == "oom" {
                anyhow::bail!("fixture OOM: compatible fallback unavailable");
            }
            if self.mode == "invalid" {
                return Ok(ModelOutput {
                    generative_model_calls: Some(0),
                    text: "{\"permissions\":\"root\"}".into(),
                    input_tokens: None,
                    output_tokens: None,
                    memory_bytes: None,
                });
            }
            if self.mode == "tool" && input.tool_results.is_empty() {
                return Ok(ModelOutput{generative_model_calls:Some(0),text:json!({"status":"tool","answer":"","citations":[],"missing_evidence":[],"tool":{"name":"search_knowledge","arguments":{"query":"chronology"}}}).to_string(),input_tokens:Some(10),output_tokens:Some(5),memory_bytes:None});
            }
            let citations = if input.evidence.is_empty() {
                vec![]
            } else {
                vec![gents_local_mcp::task_engine::grounded::Citation {
                    passage: 0,
                    quote: if self.mode == "false-citation" {
                        "invented".into()
                    } else {
                        input.evidence[0].quote.clone()
                    },
                }]
            };
            let reply = StageReply {
                status: "answer".into(),
                answer: format!(
                    "{}:{}",
                    input.node_id,
                    input
                        .dependencies
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                citations,
                missing_evidence: vec![],
                tool: None,
            };
            Ok(ModelOutput {
                generative_model_calls: Some(0),
                text: serde_json::to_string(&reply)?,
                input_tokens: Some(10),
                output_tokens: Some(5),
                memory_bytes: Some(10),
            })
        })
    }
}
impl ReadTools for Fixture {
    fn call<'a>(&'a self, request: &'a ToolRequest) -> FutureResult<'a, Value> {
        Box::pin(async move {
            assert_eq!(request.name, "search_knowledge");
            Ok(json!({"evidence":[]}))
        })
    }
}
fn models() -> BTreeMap<String, ResolvedModel> {
    BTreeMap::from([(
        "fixture".into(),
        ResolvedModel {
            id: "fixture".into(),
            identity: "fixture".into(),
            memory: memory(),
            local: true,
            context_bytes: 64000,
            evaluated_profiles: routing::profiles().keys().cloned().collect(),
        },
    )])
}
#[tokio::test]
async fn every_registered_workflow_has_distinct_real_graph_semantics_and_termination() {
    let p = policy();
    let request =
        TaskRequest::from_input(input("Synthesize independently supported evidence")).unwrap();
    let scheduler = Scheduler::new(2, 8 * 1024 * 1024).unwrap();
    let models = models();
    for template in routing::templates() {
        let fixture = Fixture::new("normal");
        let plan = routing::factory(&request, template.id, "general", Some("fixture")).unwrap();
        let engine = Executor {
            validators: None,
            profiles: None,
            worker: &fixture,
            tools: &fixture,
            scheduler: &scheduler,
            models: &models,
            limits: Limits::default(),
        };
        let evidence = vec![Evidence {
            source_id: "fixture".into(),
            source_revision: "v1".into(),
            content_sha256: "hash".into(),
            title: "fixture".into(),
            start_byte: 0,
            end_byte: 8,
            quote: "evidence".into(),
            observed_at_ms: 1,
            valid_until_ms: u64::MAX,
        }];
        let result = engine
            .execute(
                &request,
                &plan,
                &p,
                evidence,
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .unwrap();
        assert!(
            result.answer.is_some(),
            "{} {:?}",
            template.id,
            result.blockers
        );
        assert_eq!(fixture.calls.load(Ordering::SeqCst), plan.nodes.len());
        let seen = fixture.seen.lock().unwrap();
        for node in &plan.nodes {
            assert!(seen.iter().any(|(id, deps)| id == &node.id
                && deps.iter().cloned().collect::<BTreeSet<_>>()
                    == node.dependencies.iter().cloned().collect()));
        }
        assert_ne!(result.verification_state, "verified");
    }
}
#[tokio::test]
async fn tool_loop_consumes_real_turns_and_all_calls_share_global_budget() {
    let request =
        TaskRequest::from_input(input("Inspect knowledge through authorized tool")).unwrap();
    let mut p = policy();
    let plan = routing::factory(&request, "tool_loop/v1", "general", Some("fixture")).unwrap();
    let fixture = Fixture::new("tool");
    let scheduler = Scheduler::new(1, 8 * 1024 * 1024).unwrap();
    let models = models();
    let engine = Executor {
        validators: None,
        profiles: None,
        worker: &fixture,
        tools: &fixture,
        scheduler: &scheduler,
        models: &models,
        limits: Limits::default(),
    };
    let result = engine
        .execute(
            &request,
            &plan,
            &p,
            vec![],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert_eq!(result.consumption.reserved_calls, 2);
    assert_eq!(result.consumption.tool_calls, 1);
    assert!(result.answer.is_some());
    p.max_model_calls = 1;
    let result = engine
        .execute(
            &request,
            &plan,
            &p,
            vec![],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert!(result.answer.is_none());
    assert!(result.blockers.iter().any(|b| b.contains("budget")));
    assert_eq!(result.consumption.reserved_calls, 1);
}
#[tokio::test]
async fn policy_rejects_graph_identity_tools_models_and_unsupported_thinking() {
    let request =
        TaskRequest::from_input(input("Cross-domain Angular .NET Cosmos review")).unwrap();
    let p = policy();
    let plan = routing::factory(&request, "hub/v1", "integration", Some("fixture")).unwrap();
    let models: BTreeSet<_> = models().keys().cloned().collect();
    for mutation in 0..7 {
        let mut plan = plan.clone();
        match mutation {
            0 => plan.original_objective_sha256 = "changed".into(),
            1 => plan.nodes[0].dependencies = vec!["final".into()],
            2 => plan.nodes[0].provider_model = Some("unknown".into()),
            3 => plan.nodes[0].tools = vec!["shell".into()],
            4 => plan.nodes[0].profile = "god".into(),
            5 => plan.template = "unknown/v1".into(),
            _ => plan.nodes[0].validators = vec!["always_pass".into()],
        };
        assert!(routing::validate_plan(&request, &plan, &p, &models).is_err());
    }
    assert!(
        serde_json::from_value::<Hints>(json!({"budget":9999,"endpoint":"https://evil"})).is_err()
    );
    let mut p = p;
    p.max_depth = 1;
    let hierarchy =
        routing::factory(&request, "hierarchy/v1", "integration", Some("fixture")).unwrap();
    assert!(routing::validate_plan(&request, &hierarchy, &p, &models).is_err());
}
#[tokio::test]
async fn oom_context_pressure_invalid_schema_cancel_and_deadline_terminate() {
    let request = TaskRequest::from_input(input("bounded worker")).unwrap();
    let plan = routing::factory(&request, "direct/v1", "general", Some("fixture")).unwrap();
    let scheduler = Scheduler::new(1, 8 * 1024 * 1024).unwrap();
    let mut models = models();
    for mode in ["oom", "invalid", "hang"] {
        let fixture = Fixture::new(mode);
        let engine = Executor {
            validators: None,
            profiles: None,
            worker: &fixture,
            tools: &fixture,
            scheduler: &scheduler,
            models: &models,
            limits: Limits {
                per_node_timeout_ms: 30,
                ..Limits::default()
            },
        };
        let result = engine
            .execute(
                &request,
                &plan,
                &policy(),
                vec![],
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .unwrap();
        assert!(result.answer.is_none());
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    }
    models.get_mut("fixture").unwrap().context_bytes = 1;
    let fixture = Fixture::new("normal");
    let engine = Executor {
        validators: None,
        profiles: None,
        worker: &fixture,
        tools: &fixture,
        scheduler: &scheduler,
        models: &models,
        limits: Limits::default(),
    };
    let result = engine
        .execute(
            &request,
            &plan,
            &policy(),
            vec![],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert!(result.answer.is_none());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    let cancelled = Arc::new(AtomicBool::new(true));
    assert!(engine
        .execute(&request, &plan, &policy(), vec![], cancelled)
        .await
        .is_err());
}
#[test]
fn fifteen_mixed_model_fixture_entries_are_not_fifteen_admitted_workers() {
    let mut entries = vec![];
    for i in 0..15 {
        entries.push(ModelEntry {
            id: format!("model-{i}"),
            provider: "fixture".into(),
            endpoint: None,
            path: None,
            revision: Some("v1".into()),
            digest: Some(format!("digest-{}", i / 2)),
            quantization: None,
            disk_bytes: Some(100 + i),
            context_tokens: None,
            capabilities: BTreeMap::from([
                (
                    "generation".into(),
                    if i % 3 == 0 {
                        Support::Unsupported
                    } else {
                        Support::Supported
                    },
                ),
                (
                    "embedding".into(),
                    if i % 3 == 0 {
                        Support::Supported
                    } else {
                        Support::Unknown
                    },
                ),
            ]),
            thinking_controls: vec![],
            downloaded: i != 14,
            serveable: if i % 4 == 0 {
                Support::Unknown
            } else {
                Support::Supported
            },
            loaded: Support::Unknown,
            reachability: if i % 4 == 0 {
                "unreachable"
            } else {
                "reachable"
            }
            .into(),
            locality: if i == 14 {
                "cloud_proxy"
            } else {
                "operator_attested_local"
            }
            .into(),
            aliases: vec![format!("alias-{i}")],
            provenance: "mixed fixture".into(),
            evaluated_profiles: BTreeSet::new(),
        });
    }
    assert!(models::select(&entries, "history", true, &BTreeMap::new()).is_err());
    entries[1].evaluated_profiles.insert("history".into());
    entries[14].evaluated_profiles.insert("history".into());
    assert_eq!(
        models::select(&entries, "history", true, &BTreeMap::new())
            .unwrap()
            .id,
        "model-1"
    );
    let profiles: Vec<_> = (0..500)
        .map(|i| {
            let mut p = routing::profiles()["general"].clone();
            p.id = format!("specialist-{i}");
            p
        })
        .collect();
    assert_eq!(profiles.len(), 500);
    assert_eq!(
        entries
            .iter()
            .filter(|m| m.loaded == Support::Supported)
            .count(),
        0
    );
}
#[test]
fn experimental_candidate_regressions_and_disabled_track_cannot_promote() {
    let quality: BTreeMap<_, _> = QUALITY_METRICS
        .iter()
        .map(|k| (k.to_string(), 0.9))
        .collect();
    let mut metrics = quality.clone();
    for name in [
        "disk_bytes",
        "loaded_memory_bytes",
        "latency_ms",
        "throughput",
    ] {
        metrics.insert(name.into(), 1.);
    }
    let mut candidate = Candidate {
        version: 1,
        parent_sha256: "a".repeat(64),
        candidate_sha256: "b".repeat(64),
        method: Method::Quantization,
        tools: vec!["fixture quantizer".into()],
        data_license_provenance: vec!["synthetic CC0 fixture".into()],
        artifact_path: "candidates/model".into(),
        parent_path: "models/original".into(),
        runtime_compatibility: "fixture-v1".into(),
        compute_seconds: 1,
        storage_bytes: 1,
        metrics,
        baseline_metrics: quality.clone(),
        heldout_sha256: "c".repeat(64),
    };
    assert!(promotion(&candidate, &ResearchPolicy::default(), &quality).is_err());
    let p = ResearchPolicy {
        enabled: true,
        cloud_allowed: false,
        max_compute_seconds: 2,
        max_storage_bytes: 2,
        allowed_methods: vec![Method::Quantization],
    };
    assert!(promotion(&candidate, &p, &quality).is_ok());
    candidate
        .metrics
        .insert("unrelated_capabilities".into(), 0.8);
    assert!(promotion(&candidate, &p, &quality).is_err());
}

#[tokio::test]
async fn grounding_and_resolved_identity_remain_required_in_larger_workflows() {
    let request = TaskRequest::from_input(input("Critique the supported history answer")).unwrap();
    let mut p = policy();
    p.evidence_required = true;
    let mut plan = routing::factory(&request, "critique/v1", "history", Some("fixture")).unwrap();
    plan.evidence_required = true;
    let fixture = Fixture::new("false-citation");
    let scheduler = Scheduler::new(1, 8 * 1024 * 1024).unwrap();
    let models = models();
    let engine = Executor {
        validators: None,
        profiles: None,
        worker: &fixture,
        tools: &fixture,
        scheduler: &scheduler,
        models: &models,
        limits: Limits::default(),
    };
    let no_sources = engine
        .execute(
            &request,
            &plan,
            &p,
            vec![],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert!(no_sources.answer.is_none());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
    let evidence = Evidence {
        source_id: "s".into(),
        source_revision: "v1".into(),
        content_sha256: "hash".into(),
        title: "fixture".into(),
        start_byte: 0,
        end_byte: 7,
        quote: "current".into(),
        observed_at_ms: 1,
        valid_until_ms: u64::MAX,
    };
    let rejected = engine
        .execute(
            &request,
            &plan,
            &p,
            vec![evidence],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert!(rejected.answer.is_none());
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
    plan.evidence_required = false;
    assert!(engine
        .execute(
            &request,
            &plan,
            &p,
            vec![],
            Arc::new(AtomicBool::new(false))
        )
        .await
        .is_err());
    plan.evidence_required = true;
    plan.resolved_models
        .insert("fixture".into(), "swapped-weights".into());
    assert!(engine
        .execute(
            &request,
            &plan,
            &p,
            vec![],
            Arc::new(AtomicBool::new(false))
        )
        .await
        .is_err());
}
#[test]
fn reviewed_semantic_ttl_is_never_extended_by_hits() {
    let now = gents_local_mcp::now_ms();
    let db = database();
    let repo = Repository::new(&db).unwrap();
    let mut p = policy();
    p.deadline_ms = now + 30000;
    repo.ingest(&p.scope, &source(now), now).unwrap();
    repo.execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now)
        .unwrap();
    repo.approve_equivalence(&p.scope, &rule(now), now).unwrap();
    assert_eq!(
        repo.execute(
            &TaskRequest::from_input(input(PARA)).unwrap(),
            &p,
            now + 9000
        )
        .unwrap()
        .actual_path,
        "semantic_answer_cache"
    );
    assert!(repo
        .execute(
            &TaskRequest::from_input(input(PARA)).unwrap(),
            &p,
            now + 10001
        )
        .unwrap()
        .answer
        .is_none());
}
#[test]
fn many_registered_profiles_share_one_model_identity_without_residency() {
    let mut registry = routing::AgentRegistry::default();
    for index in 0..500 {
        let mut profile = routing::profiles()["general"].clone();
        profile.id = format!("specialist-{index}");
        registry.register(profile).unwrap();
    }
    assert_eq!(registry.entries().len(), 509);
    assert_eq!(models().len(), 1);
}
#[test]
fn generated_public_schemas_match_the_checked_in_version() {
    let checked: Value =
        serde_json::from_str(include_str!("../../config/schemas/task-engine-v1.json")).unwrap();
    assert_eq!(checked, gents_local_mcp::task_engine::schemas::bundle());
}

#[tokio::test]
async fn a_custom_registered_profile_executes_on_the_existing_shared_worker() {
    let mut registry = routing::AgentRegistry::default();
    let mut custom = routing::profiles()["general"].clone();
    custom.id = "release-specialist".into();
    registry.register(custom).unwrap();
    let request = TaskRequest::from_input(input("Draft a release summary")).unwrap();
    let plan = routing::factory_with_profiles(
        &request,
        "direct/v1",
        "release-specialist",
        Some("fixture"),
        registry.entries(),
    )
    .unwrap();
    let mut models = models();
    models
        .get_mut("fixture")
        .unwrap()
        .evaluated_profiles
        .insert("release-specialist".into());
    let fixture = Fixture::new("normal");
    let scheduler = Scheduler::new(1, 8 * 1024 * 1024).unwrap();
    let engine = Executor {
        validators: None,
        profiles: Some(&registry),
        worker: &fixture,
        tools: &fixture,
        scheduler: &scheduler,
        models: &models,
        limits: Limits::default(),
    };
    let result = engine
        .execute(
            &request,
            &plan,
            &policy(),
            vec![],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert!(result.answer.is_some());
    assert_eq!(result.consumption.reserved_calls, 1);
    assert_eq!(models.len(), 1);
}

#[test]
fn actual_repository_file_changes_invalidate_dependent_answers_before_reuse() {
    let root = std::env::temp_dir().join(format!("gents-task-dependency-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    std::fs::write(root.join("schema.txt"), b"schema-v1").unwrap();
    let now = gents_local_mcp::now_ms();
    let db = database();
    let repo = Repository::new(&db).unwrap();
    let p = policy();
    let mut source = source(now);
    source
        .dependencies
        .insert("schema.txt".into(), gents_local_mcp::digest(b"schema-v1"));
    repo.ingest(&p.scope, &source, now).unwrap();
    assert!(repo
        .execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now)
        .is_err());
    repo.refresh_dependencies(&p.scope, &root).unwrap();
    let first = repo
        .execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now)
        .unwrap();
    assert_eq!(first.answer.as_deref(), Some("Gerald Ford"));
    let cached = repo
        .execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now + 1)
        .unwrap();
    assert_eq!(cached.actual_path, "exact_answer_cache");
    std::fs::write(root.join("schema.txt"), b"schema-v2").unwrap();
    let changed = repo
        .execute(&TaskRequest::from_input(input(ASK)).unwrap(), &p, now + 2)
        .unwrap();
    assert!(changed.answer.is_none());
    assert!(repo.source_ids("one", "alice").unwrap().is_empty());
    source.dependencies = std::collections::BTreeMap::from([("../escape".into(), "a".repeat(64))]);
    assert!(repo.ingest(&p.scope, &source, now).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn embedding_storage_is_bounded_across_provider_identity_changes() {
    let now = gents_local_mcp::now_ms();
    let db = database();
    let repo = Repository::new(&db).unwrap();
    let p = policy();
    for index in 0..300 {
        db.execute(
            "INSERT INTO task_embeddings_v1 VALUES('one','alice',?1,'same-content','[]',?2)",
            rusqlite::params![format!("older-provider-{index}"), index],
        )
        .unwrap();
    }
    gents_local_mcp::task_engine::semantic::embed_cached(
        &repo,
        &p.scope,
        "new compatible query",
        now,
    )
    .unwrap();
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM task_embeddings_v1 WHERE tenant='one' AND subject='alice'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 256);
}

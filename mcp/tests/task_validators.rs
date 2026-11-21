//! Registry protocol fixtures. Mock receipts never qualify an installed SDK.
use gents_local_mcp::task_engine::{
    contracts::*,
    resources::*,
    routing::{self, Profile},
    validators::{self, Registry, Report},
    workflows::*,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
fn request() -> TaskRequest {
    TaskRequest::from_input(
        serde_json::from_value(json!({"original_ask":"Describe the .NET source build boundary"}))
            .unwrap(),
    )
    .unwrap()
}
fn policy() -> ExecutionPolicy {
    let scope = AccessScope {
        tenant: "one".into(),
        subject: "owner:run:actor".into(),
        effective_access_sha256: "scope".into(),
        policy_revision: "1".into(),
        profile_revision: "fixture".into(),
        runtime_revision: "fixture".into(),
        conversation_sha256: "independent".into(),
        allowed_sources: BTreeSet::new(),
    };
    let mut policy = ExecutionPolicy::read_only(scope, gents_local_mcp::now_ms());
    policy.deadline_ms += 30000;
    policy.max_model_calls = 3;
    policy.max_tokens = 2048;
    policy.permitted_validators.insert("dotnet_build".into());
    policy
}
fn memory() -> MemoryEstimate {
    MemoryEstimate {
        weights_bytes: 1,
        adapters_bytes: 0,
        kv_bytes: 0,
        context_bytes: 1,
        runtime_bytes: 1,
        headroom_bytes: 1,
        provenance: "synthetic fixture".into(),
    }
}
fn models() -> BTreeMap<String, ResolvedModel> {
    BTreeMap::from([(
        "fixture".into(),
        ResolvedModel {
            id: "fixture".into(),
            identity: "fixture-sha".into(),
            memory: memory(),
            local: true,
            context_bytes: 100000,
            evaluated_profiles: BTreeSet::from([
                "dotnet".into(),
                "general".into(),
                "angular".into(),
                "cosmos".into(),
                "history".into(),
            ]),
        },
    )])
}
struct WorkerMock(AtomicUsize);
impl Worker for WorkerMock {
    fn generate<'a>(
        &'a self,
        _: &'a str,
        _: &'a StageInput,
        _: u32,
        _: &'a AtomicBool,
    ) -> FutureResult<'a, ModelOutput> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(ModelOutput{text:json!({"status":"answer","answer":"Synthetic draft; no factual or functional qualification.","citations":[],"missing_evidence":[],"tool":null}).to_string(),input_tokens:Some(1),output_tokens:Some(1),memory_bytes:None,generative_model_calls:Some(0)})
        })
    }
}
struct NoTools;
impl ReadTools for NoTools {
    fn call<'a>(&'a self, _: &'a ToolRequest) -> FutureResult<'a, serde_json::Value> {
        Box::pin(async { anyhow::bail!("unexpected read tool") })
    }
}
struct CheckHost {
    passed: bool,
    receipt: bool,
    calls: AtomicUsize,
}
impl validators::Host for CheckHost {
    fn check<'a>(&'a self, name: &'a str, budget: &'a Budget) -> FutureResult<'a, Report> {
        Box::pin(async move {
            assert_eq!(name, "dotnet_build");
            budget.reserve_tool()?;
            self.calls.fetch_add(1, Ordering::Relaxed);
            let outcome = if self.passed {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            };
            let native_receipts = if self.receipt {
                vec![validators::NativeReceipt {
                    schema_version: VERSION,
                    validator: "dotnet_build".into(),
                    tool: "dotnet_check".into(),
                    contract_id: "fixture-only".into(),
                    source_identity: "a".repeat(64),
                    policy_revision: 1,
                    arguments_sha256: "b".repeat(64),
                    result_sha256: "c".repeat(64),
                    input_snapshot_sha256: "d".repeat(64),
                    artifact_sha256: None,
                    api_contract_sha256: None,
                    outcome: outcome.clone(),
                    elapsed_micros: 1,
                    sampled_process_group_rss_bytes: None,
                }]
            } else {
                vec![]
            };
            Ok(Report {
                checks: vec![AcceptanceCheck {
                    name: "dotnet_build/v1".into(),
                    status: outcome,
                    detail: "Mock fixture only; not an actual SDK receipt or expert qualification"
                        .into(),
                }],
                native_receipts,
            })
        })
    }
}
#[test]
fn profile_checks_are_mandatory_and_unknown_names_fail_closed() {
    let r = request();
    let mut plan = routing::factory(&r, "direct/v1", "dotnet", Some("fixture")).unwrap();
    plan.resolved_models
        .insert("fixture".into(), "fixture-sha".into());
    assert_eq!(
        plan.nodes[0].validators,
        vec![
            "dotnet_api",
            "dotnet_build",
            "input_custody",
            "output_shape"
        ]
    );
    routing::validate_plan(&r, &plan, &policy(), &BTreeSet::from(["fixture".into()])).unwrap();
    plan.nodes[0].validators.retain(|n| n != "dotnet_build");
    assert!(
        routing::validate_plan(&r, &plan, &policy(), &BTreeSet::from(["fixture".into()])).is_err()
    );
    let mut profiles = routing::AgentRegistry::default();
    let base = &routing::profiles()["general"];
    assert!(profiles
        .register(Profile {
            id: "new".into(),
            validators: BTreeSet::from(["model-self-approval".into()]),
            ..base.clone()
        })
        .is_err());
    assert!(Registry::validate_names(["unknown"]).is_err());
    let mut mixed = routing::factory(&r, "pipeline/v1", "general", Some("fixture")).unwrap();
    mixed.nodes[1].profile = "dotnet".into();
    mixed.nodes[1].validators =
        Registry::profile_checks(&routing::profiles()["dotnet"].validators).unwrap();
    routing::validate_plan(&r, &mixed, &policy(), &BTreeSet::from(["fixture".into()])).unwrap();
    mixed.nodes[1].validators.retain(|v| v != "dotnet_build");
    assert!(
        routing::validate_plan(&r, &mixed, &policy(), &BTreeSet::from(["fixture".into()])).is_err()
    );
}
#[tokio::test]
async fn host_pass_failure_missing_receipt_and_shared_budget_have_distinct_results() {
    let models = models();
    let scheduler = Scheduler::new(1, 1048576).unwrap();
    let request = request();
    let mut plan = routing::factory(&request, "direct/v1", "dotnet", Some("fixture")).unwrap();
    plan.resolved_models
        .insert("fixture".into(), "fixture-sha".into());
    for (passed, receipt, max_tools, answer) in [
        (true, true, 1, true),
        (false, true, 1, false),
        (true, false, 1, false),
        (true, true, 0, false),
    ] {
        let worker = WorkerMock(AtomicUsize::new(0));
        let host = CheckHost {
            passed,
            receipt,
            calls: AtomicUsize::new(0),
        };
        let executor = Executor {
            worker: &worker,
            tools: &NoTools,
            scheduler: &scheduler,
            models: &models,
            limits: Limits {
                max_tool_calls: max_tools,
                ..Limits::default()
            },
            profiles: None,
            validators: Some(&host),
        };
        let outcome = executor
            .execute(
                &request,
                &plan,
                &policy(),
                vec![],
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .unwrap();
        assert_eq!(outcome.answer.is_some(), answer, "{:?}", outcome.blockers);
        assert_eq!(worker.0.load(Ordering::Relaxed), 1);
        assert_eq!(outcome.consumption.reserved_calls, 1);
        assert_eq!(
            outcome.consumption.tool_calls,
            if max_tools == 0 { 0 } else { 1 }
        );
        assert_eq!(
            host.calls.load(Ordering::Relaxed),
            if max_tools == 0 { 0 } else { 1 }
        );
        assert_ne!(outcome.verification_state, "verified");
    }
}
#[tokio::test]
async fn absent_host_and_unimplemented_specialists_never_claim_pass() {
    let models = models();
    let scheduler = Scheduler::new(1, 1048576).unwrap();
    let request = request();
    let worker = WorkerMock(AtomicUsize::new(0));
    for (profile, expected, status) in [
        ("dotnet", "dotnet_build/v1", CheckStatus::NotRun),
        ("angular", "angular_build/v1", CheckStatus::NotRun),
        ("cosmos", "cosmos_receipts/v1", CheckStatus::Unsupported),
    ] {
        let mut plan = routing::factory(&request, "direct/v1", profile, Some("fixture")).unwrap();
        plan.resolved_models
            .insert("fixture".into(), "fixture-sha".into());
        let executor = Executor {
            worker: &worker,
            tools: &NoTools,
            scheduler: &scheduler,
            models: &models,
            limits: Limits::default(),
            profiles: None,
            validators: None,
        };
        let result = executor
            .execute(
                &request,
                &plan,
                &policy(),
                vec![],
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .unwrap();
        assert_eq!(
            result
                .validation
                .checks
                .iter()
                .find(|c| c.name == expected)
                .unwrap()
                .status,
            status
        );
        assert!(result.validation.native_receipts.is_empty());
    }
}
#[tokio::test]
async fn an_available_host_cannot_bypass_the_trusted_validator_allowlist() {
    let models = models();
    let scheduler = Scheduler::new(1, 1048576).unwrap();
    let request = request();
    let worker = WorkerMock(AtomicUsize::new(0));
    let mut plan = routing::factory(&request, "direct/v1", "dotnet", Some("fixture")).unwrap();
    plan.resolved_models
        .insert("fixture".into(), "fixture-sha".into());
    let host = CheckHost {
        passed: true,
        receipt: true,
        calls: AtomicUsize::new(0),
    };
    let executor = Executor {
        worker: &worker,
        tools: &NoTools,
        scheduler: &scheduler,
        models: &models,
        limits: Limits::default(),
        profiles: None,
        validators: Some(&host),
    };
    let mut p = policy();
    p.permitted_validators.clear();
    let result = executor
        .execute(
            &request,
            &plan,
            &p,
            vec![],
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert_eq!(host.calls.load(Ordering::Relaxed), 0);
    assert_eq!(result.consumption.tool_calls, 0);
    assert_eq!(
        result
            .validation
            .checks
            .iter()
            .find(|c| c.name == "dotnet_build/v1")
            .unwrap()
            .status,
        CheckStatus::NotRun
    );
}
struct CancelHost(Arc<AtomicBool>);
impl validators::Host for CancelHost {
    fn check<'a>(&'a self, _: &'a str, budget: &'a Budget) -> FutureResult<'a, Report> {
        Box::pin(async move {
            budget.reserve_tool()?;
            self.0.store(true, Ordering::Relaxed);
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn cancellation_during_validation_preserves_budget_and_stops_delivery() {
    let models = models();
    let scheduler = Scheduler::new(1, 1048576).unwrap();
    let request = request();
    let worker = WorkerMock(AtomicUsize::new(0));
    let mut plan = routing::factory(&request, "direct/v1", "dotnet", Some("fixture")).unwrap();
    plan.resolved_models
        .insert("fixture".into(), "fixture-sha".into());
    let started = Arc::new(AtomicBool::new(false));
    let host = CancelHost(started.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    let task_cancel = cancel.clone();
    let signal = async move {
        while !started.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        task_cancel.store(true, Ordering::Relaxed);
    };
    let executor = Executor {
        worker: &worker,
        tools: &NoTools,
        scheduler: &scheduler,
        models: &models,
        limits: Limits::default(),
        profiles: None,
        validators: Some(&host),
    };
    let p = policy();
    let (outcome, ()) = tokio::join!(
        executor.execute(&request, &plan, &p, vec![], cancel),
        signal
    );
    let outcome = outcome.unwrap();
    assert!(outcome.answer.is_none());
    assert_eq!(outcome.consumption.reserved_calls, 1);
    assert_eq!(outcome.consumption.tool_calls, 1);
}

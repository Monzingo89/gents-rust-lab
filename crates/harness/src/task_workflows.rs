//! Owner-authenticated async adapter for the shared registered executors.
//! Uses the existing native model pool and company SQLite journal.
use crate::{
    company,
    provider::{LanguageModel, Model},
    task_frontdoor, Shared,
};
use anyhow::{ensure, Context, Result};
use gents_local_mcp::task_engine::{
    adaptive,
    contracts::*,
    intake, native, read_tools,
    resources::{MemoryEstimate, Scheduler},
    routing::{self, Hints},
    validators,
    workflows::{self, *},
    Repository,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerSettings {
    pub model: Model,
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_sha256: Option<String>,
    pub memory_estimate: MemoryEstimate,
    pub evaluated_profiles: BTreeSet<String>,
    pub evaluation_reference: String,
    /// Measured full-path time including cold/warm inference, tools and checks.
    /// Missing measurements never become invented benchmark numbers.
    #[serde(default)]
    pub measured_total_latency_ms: Option<u64>,
    /// Serialized input ceiling; native token context remains independently enforced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_bytes: Option<usize>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub models: BTreeMap<String, WorkerSettings>,
    pub default_model: String,
    pub max_calls: u32,
    pub total_generation_tokens: u32,
    pub per_call_tokens: u32,
    pub deadline_seconds: u64,
    pub max_retries: u32,
    pub max_pending_jobs: usize,
    pub concurrency: u32,
    pub resident_budget_bytes: u64,
    pub allow_parametric_answers: bool,
    #[serde(default)]
    pub profiles: Vec<routing::Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive: Option<adaptive::Settings>,
    #[serde(default, skip_serializing_if = "disabled")]
    pub workspace_reads: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(default, skip_serializing_if = "disabled")]
    pub native_mcp: bool,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub native_companies: BTreeSet<String>,
}
fn disabled(enabled: &bool) -> bool {
    !*enabled
}

impl Settings {
    pub fn registry(&self) -> Result<routing::AgentRegistry> {
        let mut registry = routing::AgentRegistry::default();
        for profile in &self.profiles {
            registry.register(profile.clone())?;
        }
        Ok(registry)
    }
    pub fn validate(&self) -> Result<()> {
        let registry = self.registry()?;
        self.workspace_relative_root("validation-company")?;
        ensure!(
            !self.native_mcp || !self.native_companies.is_empty(),
            "native MCP worker requires an explicit company allowlist"
        );
        ensure!(
            self.native_companies.len() <= 64,
            "native company allowlist exceeds bound"
        );
        for company in &self.native_companies {
            company::identifier(company)?;
        }
        ensure!(
            !self.models.is_empty()
                && self.models.len() <= 16
                && self.models.contains_key(&self.default_model),
            "registered default model required"
        );
        ensure!(
            (1..=32).contains(&self.max_calls)
                && (1..=65536).contains(&self.total_generation_tokens)
                && (64..=4096).contains(&self.per_call_tokens)
                && (1..=300).contains(&self.deadline_seconds)
                && self.max_retries <= 2
                && (1..=8).contains(&self.max_pending_jobs)
                && (1..=2).contains(&self.concurrency)
                && self.resident_budget_bytes > 0,
            "invalid task workflow limits"
        );
        for worker in self.models.values() {
            worker.model.validate()?;
            ensure!(
                matches!(worker.model.provider.as_str(), "local" | "mlx" | "mock"),
                "workflow workers require existing owned native runtime; mock is fixtures only"
            );
            ensure!(
                (worker.model.adapter.is_some() == worker.adapter_sha256.is_some())
                    && worker.model.local_reasoning_tokens.unwrap_or(0) < self.per_call_tokens
                    && worker.model.local_final_tokens.is_none_or(|n| n
                        .saturating_add(worker.model.local_reasoning_tokens.unwrap_or(0))
                        <= self.per_call_tokens),
                "workflow generation tokens must fit host allowance; unsupported thinking control"
            );
            ensure!(
                !worker.evaluation_reference.is_empty()
                    && !worker.evaluated_profiles.is_empty()
                    && worker
                        .evaluated_profiles
                        .iter()
                        .all(|p| registry.entries().contains_key(p)),
                "evaluated profile admission evidence required"
            );
            if matches!(worker.model.provider.as_str(), "local" | "mlx") {
                ensure!(
                    worker.sha256.as_ref().is_some_and(|h| h.len() == 64
                        && h.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
                    "immutable native model digest required"
                );
            }
            if let Some(hash) = &worker.adapter_sha256 {
                ensure!(
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                    "immutable adapter package digest required"
                );
            }
            ensure!(
                worker
                    .max_input_bytes
                    .is_none_or(|n| (1024..=65536).contains(&n)),
                "invalid host input byte ceiling"
            );
            worker.memory_estimate.total()?;
        }
        if let Some(adaptive) = &self.adaptive {
            adaptive.validate(&self.resolved_models())?;
            if let Some(intake) = &adaptive.intake {
                let model = &self.models[&intake.model].model;
                ensure!(
                    intake.tokens
                        >= model
                            .local_reasoning_tokens
                            .unwrap_or(0)
                            .saturating_add(model.local_final_tokens.unwrap_or(64)),
                    "intake token reservation below native generation controls"
                );
            }
        }
        Ok(())
    }
    fn workspace_relative_root(&self, tenant: &str) -> Result<String> {
        let pattern = self
            .workspace_root
            .as_deref()
            .unwrap_or("data/companies/{company}/task-workspace");
        ensure!(
            !pattern.starts_with("data/") || pattern.starts_with("data/companies/{company}/"),
            "private company data roots must be company-scoped"
        );
        let path = pattern.replace("{company}", tenant);
        ensure!(
            crate::task_frontdoor::safe_workspace_project_root(&path),
            "workspace project root must be a confined non-reserved subdirectory"
        );
        Ok(path)
    }
    fn workspace_path(&self, base: &Path, tenant: &str) -> Result<PathBuf> {
        Ok(base.join(self.workspace_relative_root(tenant)?))
    }
    fn workspace_policy(
        &self,
        policy: &mut ExecutionPolicy,
        base: &Path,
        tenant: &str,
    ) -> Result<()> {
        if self.workspace_reads {
            let path = self.workspace_path(base, tenant)?;
            policy.workspace_roots = vec![path
                .to_str()
                .context("workspace root is not UTF-8")?
                .to_owned()];
            policy
                .permitted_tools
                .extend(["workspace_read".into(), "workspace_list".into()]);
        }
        Ok(())
    }
    fn resolved_models(&self) -> BTreeMap<String, ResolvedModel> {
        self.models
            .iter()
            .map(|(id, s)| {
                (
                    id.clone(),
                    ResolvedModel {
                        id: id.clone(),
                        identity: worker_identity(s),
                        memory: s.memory_estimate.clone(),
                        local: matches!(s.model.provider.as_str(), "local" | "mlx" | "mock"),
                        context_bytes: s.max_input_bytes.unwrap_or_else(|| {
                            (s.model.context_tokens.unwrap_or(4096) as usize)
                                .saturating_sub(self.per_call_tokens as usize + 256)
                        }),
                        evaluated_profiles: s.evaluated_profiles.clone(),
                    },
                )
            })
            .collect()
    }
}
fn worker_identity(settings: &WorkerSettings) -> String {
    let mut identity = format!(
        "{}/{}@{}",
        settings.model.provider,
        settings.model.model,
        settings.sha256.as_deref().unwrap_or("simulated-fixture")
    );
    if let Some(hash) = &settings.adapter_sha256 {
        identity.push_str(&format!("+adapter:{hash}"));
    }
    identity
}
#[derive(Default)]
pub struct Service {
    active: Mutex<HashMap<Uuid, Arc<AtomicBool>>>,
    scheduler: OnceCell<Scheduler>,
}
fn install(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS task_workflow_jobs_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,key TEXT NOT NULL,id TEXT NOT NULL UNIQUE,body TEXT NOT NULL,sha256 TEXT NOT NULL,state TEXT NOT NULL,result TEXT,expires INTEGER NOT NULL,retained_until INTEGER NOT NULL,PRIMARY KEY(tenant,subject,key));
        CREATE TABLE IF NOT EXISTS task_workflow_subscribers_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,task TEXT NOT NULL,job TEXT NOT NULL,cancelled INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(tenant,subject,task));
        CREATE TABLE IF NOT EXISTS task_workflow_plan_revisions_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,job TEXT NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(tenant,subject,job,revision));")?;
    let columns: Vec<String> = db
        .prepare("PRAGMA table_info(task_workflow_jobs_v1)")?
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<_>>()?;
    if !columns.iter().any(|c| c == "retained_until") {
        db.execute_batch("ALTER TABLE task_workflow_jobs_v1 ADD COLUMN retained_until INTEGER NOT NULL DEFAULT 0; UPDATE task_workflow_jobs_v1 SET retained_until=expires WHERE retained_until=0;")?;
    }
    Ok(())
}
fn snapshot(repo: &Repository<'_>, scope: &AccessScope, now: u64) -> Result<String> {
    // Full current authorized source revision/content/freshness snapshot, not
    // query results alone. A corrected/deleted source invalidates delivery.
    Ok(gents_local_mcp::digest(&serde_json::to_vec(
        &repo.knowledge_snapshot(scope, now)?,
    )?))
}
#[derive(Clone, Deserialize, Serialize)]
struct Job {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    validation_bindings: Vec<validators::Binding>,
    request: TaskRequest,
    plan: ExecutionPlan,
    evidence: Vec<Evidence>,
    scope_hash: String,
    corpus_hash: String,
    runtime: String,
    deadline_ms: u64,
    base: TaskResult,
    #[serde(default)]
    caller_hints: Hints,
    #[serde(default)]
    analysis_only: bool,
    #[serde(default)]
    skip_model_intake: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace_relative_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    native: Option<native::Custody>,
}
fn job_owner<'a>(subject: &'a str, native: Option<&'a native::Custody>) -> &'a str {
    native.map_or(subject, |n| n.scope.owner.as_str())
}
#[allow(clippy::too_many_arguments)]
fn execution_scope(
    repo: &Repository<'_>,
    db: &rusqlite::Connection,
    root: &Path,
    tenant: &str,
    subject: &str,
    runtime: &str,
    profile: &str,
    native: Option<&native::Custody>,
) -> Result<AccessScope> {
    if let Some(n) = native {
        let policy = gents_local_mcp::governance::load(db, &n.scope.run.to_string())?
            .context("native policy unavailable")?;
        let scope = native::access(repo, &n.scope, &policy, runtime)?;
        native::refresh(repo, db, n, &scope, root)?;
        Ok(scope)
    } else {
        let mut scope = task_frontdoor::owner_scope(repo, tenant, subject, runtime)?;
        scope.profile_revision = format!("workflow-profile/v1/{profile}");
        repo.refresh_dependencies(&scope, root)?;
        Ok(scope)
    }
}
fn execution_tools(
    settings: &Settings,
    policy: &mut ExecutionPolicy,
    db: &rusqlite::Connection,
    root: &Path,
    tenant: &str,
    native: Option<&native::Custody>,
) -> Result<()> {
    if let Some(n) = native {
        let current = native::authorize(db, n, crate::now_ms())?;
        policy.permitted_tools.clear();
        if current
            .authorize(&n.scope.actor, "search_knowledge", &json!({}))
            .is_ok()
        {
            policy.permitted_tools.insert("search_knowledge".into());
        }
        if current
            .authorize(&n.scope.actor, "dotnet_check", &json!({}))
            .is_ok()
        {
            policy.permitted_validators.insert("dotnet_build".into());
        }
        if current
            .authorize(&n.scope.actor, "dotnet_api_check", &json!({}))
            .is_ok()
        {
            policy.permitted_validators.insert("dotnet_api".into());
        }
        if current
            .authorize(&n.scope.actor, "angular_check", &json!({}))
            .is_ok()
        {
            policy.permitted_validators.insert("angular_build".into());
        }
        // Workspace operations retain their native files/contract boundaries.
        // This handoff never substitutes the owner's configured project root.
        policy.workspace_roots.clear();
    } else {
        settings.workspace_policy(policy, root, tenant)?;
    }
    Ok(())
}
fn validation_bindings(
    db: &rusqlite::Connection,
    root: &Path,
    native: Option<&native::Custody>,
    plan: &ExecutionPlan,
) -> Result<Vec<validators::Binding>> {
    let Some(n) = native else { return Ok(vec![]) };
    let wanted = plan
        .nodes
        .iter()
        .flat_map(|n| n.validators.iter().cloned())
        .collect::<BTreeSet<_>>();
    if !wanted
        .iter()
        .any(|v| matches!(v.as_str(), "dotnet_build" | "dotnet_api" | "angular_build"))
    {
        return Ok(vec![]);
    }
    let current = native::authorize(db, n, crate::now_ms())?;
    let authority = gents_local_mcp::default_authority(root.to_owned())?;
    let context = gents_local_mcp::ToolContext {
        authority: &authority,
        scope: &n.scope,
        policy: &current,
        db,
    };
    let mut bindings = vec![];
    if current
        .authorize(&n.scope.actor, "dotnet_check", &json!({}))
        .is_ok()
    {
        bindings.extend(gents_local_mcp::dotnet_check::runtime_bindings(
            &context, &wanted,
        )?);
    }
    bindings.extend(gents_local_mcp::angular_check::runtime_bindings(
        &context, &wanted,
    )?);
    Ok(bindings)
}

impl Service {
    pub(crate) async fn active_ids(&self) -> BTreeSet<Uuid> {
        self.active.lock().await.keys().copied().collect()
    }
    pub(crate) async fn signal(&self, id: Uuid) {
        if let Some(cancel) = self.active.lock().await.get(&id) {
            cancel.store(true, Ordering::Relaxed);
        }
    }
    pub async fn start(
        &self,
        app: Shared,
        root: PathBuf,
        tenant: String,
        subject: String,
        input: TaskInput,
        hints: Hints,
    ) -> Result<Value> {
        self.start_mode(app, root, tenant, subject, input, hints, false)
            .await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn start_mode(
        &self,
        app: Shared,
        root: PathBuf,
        tenant: String,
        subject: String,
        input: TaskInput,
        hints: Hints,
        analysis_only: bool,
    ) -> Result<Value> {
        self.start_request(
            app,
            root,
            tenant,
            subject,
            TaskRequest::from_input(input)?,
            hints,
            analysis_only,
            false,
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn start_request(
        &self,
        app: Shared,
        root: PathBuf,
        tenant: String,
        subject: String,
        request: TaskRequest,
        hints: Hints,
        analysis_only: bool,
        skip_model_intake: bool,
    ) -> Result<Value> {
        self.start_inner(
            app,
            root,
            tenant,
            subject,
            request,
            hints,
            analysis_only,
            skip_model_intake,
            None,
        )
        .await
    }
    pub(crate) async fn start_native(
        &self,
        app: Shared,
        root: PathBuf,
        packet: native::Packet,
    ) -> Result<Value> {
        let tenant = packet.custody.scope.company.clone();
        let subject = native::subject(&packet.custody.scope);
        self.start_inner(
            app,
            root,
            tenant,
            subject,
            packet.request,
            packet.hints,
            packet.analysis_only,
            packet.skip_model_intake,
            Some((packet.custody, packet.base)),
        )
        .await
    }
    #[allow(clippy::too_many_arguments)]
    async fn start_inner(
        &self,
        app: Shared,
        root: PathBuf,
        tenant: String,
        subject: String,
        request: TaskRequest,
        hints: Hints,
        analysis_only: bool,
        skip_model_intake: bool,
        native_packet: Option<(native::Custody, TaskResult)>,
    ) -> Result<Value> {
        request.validate()?;
        let settings = app
            .config
            .task_workflows
            .clone()
            .context("workflow execution disabled in trusted host config")?;
        settings.validate()?;
        let registry = settings.registry()?;
        let intake =
            routing::destructure_with_profiles(request, hints.clone(), registry.entries())?;
        ensure!(
            analysis_only || intake.request.input.intent == Intent::Answer,
            "actions require existing reviewed native workflow authorization"
        );
        let runtime = gents_local_mcp::digest(&serde_json::to_vec(&app.config)?);
        let mut active = self.active.lock().await;
        let native = native_packet.as_ref().map(|(n, _)| n);
        let mut db = company::connect(&root, &tenant, job_owner(&subject, native), false)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        install(&tx)?;
        let repo = Repository::new(&tx)?;
        let now = crate::now_ms();
        if let Some(n) = native {
            ensure!(
                settings.native_mcp && n.runtime == runtime,
                "native execution config changed or disabled"
            );
            native::authorize(&tx, n, now)?;
            let state: String = tx.query_row(
                "SELECT state FROM task_native_queue_v1 WHERE tenant=?1 AND subject=?2 AND task=?3",
                params![tenant, subject, intake.request.task_id.to_string()],
                |r| r.get(0),
            )?;
            ensure!(
                state == "claimed",
                "native queue admission changed; no launch"
            );
        }
        let scope = execution_scope(
            &repo,
            &tx,
            &root,
            &tenant,
            &subject,
            &runtime,
            intake.analysis.domain.as_deref().unwrap_or("general"),
            native,
        )?;
        if let Some(n) = native {
            if n.source_scope_sha256 != gents_local_mcp::digest(&serde_json::to_vec(&scope)?)
                || n.corpus_sha256 != snapshot(&repo, &scope, now)?
            {
                return Err(ExecutionFailure {
                    kind: StopReason::SourceChanged,
                    message: "Native admission source/access snapshot changed while queued".into(),
                }
                .into());
            }
        }
        let mut policy = ExecutionPolicy::read_only(scope, now);
        // Every launch still begins at the exact/semantic/extraction fast path.
        let base = if let Some((_, base)) = &native_packet {
            base.clone()
        } else {
            repo.execute(&intake.request, &policy, now)?
        };
        if !analysis_only && base.status == "succeeded" {
            tx.commit()?;
            return Ok(json!({"request":intake.request,"result":base}));
        }
        let profile = intake.analysis.domain.as_deref().unwrap_or("general");
        let selected = settings
            .models
            .iter()
            .filter(|(_, m)| {
                m.evaluated_profiles.contains(profile)
                    && m.memory_estimate
                        .total()
                        .is_ok_and(|n| n <= settings.resident_budget_bytes)
            })
            .min_by_key(|(id, m)| {
                (
                    m.measured_total_latency_ms.unwrap_or(u64::MAX),
                    if id.as_str() == settings.default_model {
                        0
                    } else {
                        1
                    },
                    m.memory_estimate.weights_bytes,
                )
            })
            .map(|(id, _)| id.clone())
            .or_else(|| {
                settings
                    .adaptive
                    .as_ref()
                    .and_then(|a| a.intake.as_ref())
                    .map(|_| settings.default_model.clone())
            })
            .context("no evaluated feasible worker; discovery alone cannot admit a model")?;
        let model = &settings.models[&selected];
        ensure!(
            model.evaluated_profiles.contains(profile)
                || settings
                    .adaptive
                    .as_ref()
                    .is_some_and(|a| a.intake.is_some()),
            "default model has no admitted evaluation for requested profile"
        );
        let mut plan = routing::factory_with_profiles(
            &intake.request,
            &intake.proposed_plan.template,
            profile,
            Some(&selected),
            registry.entries(),
        )?;
        plan.routing_justification=format!("Selected {selected} from host-admitted profile evaluations and explicit memory reservations; full-path latency measurement {:?}. No optimality claim.",model.measured_total_latency_ms);
        plan.evidence_required = intake.analysis.evidence_required || plan.evidence_required;
        plan.resolved_models = settings
            .models
            .iter()
            .map(|(id, s)| (id.clone(), worker_identity(s)))
            .collect();
        let evidence: Vec<_> = repo
            .search(&policy.scope, &intake.request.original_ask, None, 8192, now)?
            .into_iter()
            .map(|h| h.evidence)
            .collect();
        if intake.analysis.evidence_required {
            for node in &mut plan.nodes {
                if node.strategy == Strategy::Direct {
                    node.strategy = Strategy::GroundedGeneration;
                }
            }
            if plan.strategy == Strategy::Direct {
                plan.strategy = Strategy::GroundedGeneration;
            }
        }
        ensure!(
            intake.analysis.evidence_required || settings.allow_parametric_answers,
            "parametric answers disabled by host"
        );
        policy.max_model_calls = settings.max_calls;
        policy.evidence_required = if settings
            .adaptive
            .as_ref()
            .is_some_and(|a| a.intake.is_some())
        {
            intake::mandatory_evidence(
                &intake.request,
                &hints,
                profile,
                settings.allow_parametric_answers,
            )
        } else {
            plan.evidence_required
        };
        policy.max_tokens = settings.total_generation_tokens;
        policy.max_retries = settings.max_retries;
        policy.max_fan_out = 2;
        policy.max_depth = 2;
        policy.max_concurrency = settings.concurrency;
        policy.deadline_ms = now + settings.deadline_seconds * 1000;
        if let Some(n) = native {
            policy.deadline_ms = policy.deadline_ms.min(n.deadline_ms);
        }
        execution_tools(&settings, &mut policy, &tx, &root, &tenant, native)?;
        routing::configure_tools(&mut plan, &policy, registry.entries())?;
        let validation_bindings = validation_bindings(&tx, &root, native, &plan)?;
        let key = gents_local_mcp::digest(&serde_json::to_vec(
            &json!({"input":intake.request.input,"hints":hints,"validation_bindings":validation_bindings,"analysis_only":analysis_only,"skip_model_intake":skip_model_intake,"template":plan.template,"nodes":plan.nodes,"resolved_models":plan.resolved_models,"evidence_required":intake.analysis.evidence_required,"scope":policy.scope,"runtime":runtime,"corpus":snapshot(&repo,&policy.scope,now)?}),
        )?);
        tx.execute("DELETE FROM task_workflow_plan_revisions_v1 WHERE tenant=?1 AND subject=?2 AND job IN (SELECT id FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed' AND retained_until<=?3)",params![tenant,subject,now as i64])?;
        tx.execute("DELETE FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job IN (SELECT id FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed' AND retained_until<=?3)",params![tenant,subject,now as i64])?;
        tx.execute("DELETE FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed' AND retained_until<=?3",params![tenant,subject,now as i64])?;
        workflows::prune_audit(&tx)?;
        let mut previous:Option<(String,String,String,String,Option<String>,i64)>=tx.query_row("SELECT id,body,sha256,state,result,expires FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND key=?3",params![tenant,subject,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
        if let Some((id, body, sha, state, prior, expires)) = &previous {
            ensure!(
                gents_local_mcp::digest(body.as_bytes()) == *sha,
                "workflow checkpoint integrity mismatch; no replay"
            );
            let frozen: Job = serde_json::from_str(body)?;
            let capture_root = settings.workspace_path(&root, &tenant)?;
            let mut current = repo.workspace_task_current(
                &policy.scope,
                frozen.request.task_id,
                &capture_root,
                now,
            )?;
            if let Some(prior) = prior {
                let prior: TaskResult = serde_json::from_str(prior)?;
                for e in prior
                    .evidence
                    .iter()
                    .filter(|e| e.source_id.starts_with("workspace:v1:"))
                {
                    current &=
                        repo.workspace_evidence_current(&policy.scope, e, &capture_root, now)?;
                }
            }
            let eligible =
                state != "completed" || (*expires > now as i64 && validation_bindings.is_empty());
            if !current || !eligible {
                // Keep the original job/subscribers for invalidated history,
                // but do not attach a fresh request to obsolete file inputs.
                tx.execute("UPDATE task_workflow_jobs_v1 SET key=?4 WHERE tenant=?1 AND subject=?2 AND id=?3",params![tenant,subject,id,format!("{key}/ineligible/{id}")])?;
                if !current && state != "completed" {
                    if let Some(cancel) = active.get(&Uuid::parse_str(id)?) {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
                tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'task_workflow_reuse_rejected',?2,?3)",params![id,json!({"reason":if current{"negative or draft reuse TTL expired; history retained separately"}else{"captured file/root/scope/TTL changed"},"new_task_id":intake.request.task_id}).to_string(),now as i64])?;
                previous = None;
            }
        }
        let (id, job, launch, mut result) = if let Some((id, body, sha, state, prior, _)) = previous
        {
            ensure!(
                gents_local_mcp::digest(body.as_bytes()) == sha,
                "workflow checkpoint integrity mismatch; no replay"
            );
            let job: Job = serde_json::from_str(&body)?;
            let id = Uuid::parse_str(&id)?;
            let mut result: TaskResult = if let Some(prior) = prior {
                serde_json::from_str(&prior)?
            } else {
                job.base.clone()
            };
            result.cache.origin_task_id = Some(job.request.task_id);
            result.cache.decision = "coalesced_workflow_not_verified_answer_cache".into();
            result.measurements.generative_model_calls = Some(0);
            result.measurements.tool_calls = 0;
            result.measurements.memory_bytes = None;
            result.measurements.input_tokens = Some(0);
            result.measurements.output_tokens = Some(0);
            if state != "completed" && !active.contains_key(&id) {
                result.status = "interrupted".into();
                result.blockers.push(
                    "Original host lost; inspect checkpoint. No automatic inference replay.".into(),
                );
            }
            if state == "cancel_requested" {
                result.status = "cancelled".into();
                result.verification_state = "not_verified".into();
                result.answer = None;
            }
            (id, job, false, result)
        } else {
            ensure!(
                active.len() < settings.max_pending_jobs,
                "workflow pending capacity reached"
            );
            let count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2",
                params![tenant, subject],
                |r| r.get(0),
            )?;
            ensure!(count < 128, "workflow record capacity reached");
            let id = Uuid::new_v4();
            let mut result = base;
            result.status = "running".into();
            result.actual_path = "registered_workflow".into();
            result.plan = Some(plan.clone());
            result.blockers.clear();
            result.cache.decision = "not_eligible_for_verified_answer_cache".into();
            result.measurements.generative_model_calls = None;
            let job = Job {
                validation_bindings,
                request: intake.request.clone(),
                plan,
                evidence,
                scope_hash: gents_local_mcp::digest(&serde_json::to_vec(&policy.scope)?),
                corpus_hash: snapshot(&repo, &policy.scope, now)?,
                runtime,
                deadline_ms: policy.deadline_ms,
                base: result.clone(),
                caller_hints: hints,
                analysis_only,
                skip_model_intake,
                workspace_relative_root: if settings.workspace_reads && native.is_none() {
                    Some(settings.workspace_relative_root(&tenant)?)
                } else {
                    None
                },
                native: native.cloned(),
            };
            let body = serde_json::to_string(&job)?;
            ensure!(body.len() <= 65536, "workflow checkpoint bound exceeded");
            tx.execute(
                "INSERT INTO task_workflow_jobs_v1(tenant,subject,key,id,body,sha256,state,result,expires,retained_until) VALUES(?1,?2,?3,?4,?5,?6,'running',NULL,?7,?7)",
                params![
                    tenant,
                    subject,
                    key,
                    id.to_string(),
                    body,
                    gents_local_mcp::digest(body.as_bytes()),
                    (now + 3600000) as i64
                ],
            )?;
            (id, job, true, result)
        };
        result.task_id = intake.request.task_id;
        let subscribers:i64=tx.query_row("SELECT COUNT(*) FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job=?3",params![tenant,subject,id.to_string()],|r|r.get(0))?;
        ensure!(subscribers < 64, "workflow subscriber capacity reached");
        tx.execute(
            "INSERT INTO task_workflow_subscribers_v1 VALUES(?1,?2,?3,?4,?5)",
            params![
                tenant,
                subject,
                result.task_id.to_string(),
                id.to_string(),
                i64::from(result.status == "cancelled")
            ],
        )?;
        repo.replace_result(&policy.scope, &result)?;
        tx.commit()?;
        if launch {
            let cancel = Arc::new(AtomicBool::new(false));
            active.insert(id, cancel.clone());
            tokio::spawn(run(app, root, tenant, subject, id, job, settings, cancel));
        }
        Ok(json!({"request":intake.request,"result":result,"job_id":id}))
    }
    pub async fn observe(
        &self,
        root: &Path,
        tenant: &str,
        subject: &str,
        runtime: &str,
        task: Uuid,
        cancel_task: bool,
    ) -> Result<Option<Value>> {
        let active = self.active.lock().await;
        let mut db = company::connect(root, tenant, subject, false)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        install(&tx)?;
        let repo = Repository::new(&tx)?;
        let mut scope = task_frontdoor::owner_scope(&repo, tenant, subject, runtime)?;
        let job:Option<String>=tx.query_row("SELECT job FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND task=?3",params![tenant,subject,task.to_string()],|r|r.get(0)).optional()?;
        let Some(job) = job else { return Ok(None) };
        let id = Uuid::parse_str(&job)?;
        let (body, state, sha): (String, String, String) = tx.query_row(
            "SELECT body,state,sha256 FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND id=?3",
            params![tenant, subject, job],
            |r| Ok((r.get(0)?, r.get(1)?,r.get(2)?)),
        )?;
        ensure!(
            gents_local_mcp::digest(body.as_bytes()) == sha,
            "workflow checkpoint integrity mismatch; no replay"
        );
        let frozen: Job = serde_json::from_str(&body)?;
        scope.profile_revision = format!(
            "workflow-profile/v1/{}",
            frozen
                .plan
                .nodes
                .first()
                .context("workflow profile missing")?
                .profile
        );
        repo.refresh_dependencies(&scope, root)?;
        if let Some(relative) = &frozen.workspace_relative_root {
            repo.workspace_status_root(root.join(relative));
        }
        let mut result = repo
            .status(&scope, task, crate::now_ms())?
            .context("workflow task missing")?;
        if state != "completed" && !active.contains_key(&id) {
            result.status = "interrupted".into();
            result.answer = None;
        }
        if frozen.runtime != runtime
            || frozen.scope_hash != gents_local_mcp::digest(&serde_json::to_vec(&scope)?)
            || frozen.corpus_hash != snapshot(&repo, &scope, crate::now_ms())?
        {
            result.answer = None;
            result.evidence.clear();
            result.status = "evidence_invalidated".into();
            result.verification_state = "not_verified".into();
        }
        if cancel_task && state != "completed" {
            tx.execute("UPDATE task_workflow_subscribers_v1 SET cancelled=1 WHERE tenant=?1 AND subject=?2 AND task=?3",params![tenant,subject,task.to_string()])?;
            let remaining:i64=tx.query_row("SELECT COUNT(*) FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job=?3 AND cancelled=0",params![tenant,subject,job],|r|r.get(0))?;
            if remaining == 0 {
                tx.execute("UPDATE task_workflow_jobs_v1 SET state='cancel_requested' WHERE tenant=?1 AND subject=?2 AND id=?3",params![tenant,subject,job])?;
                if let Some(cancel) = active.get(&id) {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            result.status = "cancelled".into();
            result.answer = None;
            result.evidence.clear();
            repo.replace_result(&scope, &result)?;
        }
        tx.commit()?;
        Ok(Some(serde_json::to_value(result)?))
    }
}
struct Adapter {
    app: Shared,
    settings: Settings,
    root: PathBuf,
    tenant: String,
    subject: String,
    session: String,
    job_id: Uuid,
    frozen: Job,
    validation_results: Mutex<BTreeMap<String, validators::Report>>,
}
impl Worker for Adapter {
    fn generate<'a>(
        &'a self,
        id: &'a str,
        input: &'a StageInput,
        tokens: u32,
        cancel: &'a AtomicBool,
    ) -> FutureResult<'a, ModelOutput> {
        Box::pin(async move {
            self.ensure_workspace_evidence(&input.evidence)?;
            self.generate_wire(
                id,
                workflows::INSTRUCTION,
                &serde_json::to_string(input)?,
                tokens,
                cancel,
            )
            .await
        })
    }
}
impl intake::IntakeWorker for Adapter {
    fn analyze<'a>(
        &'a self,
        id: &'a str,
        packet: &'a intake::Packet,
        tokens: u32,
        cancel: &'a AtomicBool,
    ) -> FutureResult<'a, ModelOutput> {
        Box::pin(async move {
            self.generate_wire(
                id,
                intake::INSTRUCTION,
                &serde_json::to_string(packet)?,
                tokens,
                cancel,
            )
            .await
        })
    }
}
impl Adapter {
    async fn generate_wire(
        &self,
        id: &str,
        instruction: &str,
        prompt: &str,
        tokens: u32,
        cancel: &AtomicBool,
    ) -> Result<ModelOutput> {
        let settings = &self.settings.models[id];
        let pin = crate::task_answers::Settings {
            model: settings.model.clone(),
            model_sha256: settings.sha256.clone(),
            adapter_sha256: settings.adapter_sha256.clone(),
            final_tokens: tokens,
            deadline_seconds: self.settings.deadline_seconds,
            draft_ttl_seconds: 60,
            max_pending_jobs: 1,
        };
        if settings.model.provider == "mlx" {
            let package = crate::model_package::snapshot(
                &self.root,
                settings.model.path.as_deref().context("MLX path missing")?,
                crate::model_package::Kind::Model,
            )?;
            ensure!(
                package.weight_bytes <= settings.memory_estimate.weights_bytes,
                "package weights exceed explicit reservation"
            );
            if let Some(adapter) = &settings.model.adapter {
                ensure!(
                    crate::model_package::safe_relative(adapter)
                        && adapter
                            .starts_with(&format!("data/companies/{}/training/runs/", self.tenant)),
                    "adapter outside current company"
                );
                let package = crate::model_package::snapshot(
                    &self.root,
                    adapter,
                    crate::model_package::Kind::Adapter,
                )?;
                ensure!(
                    package.weight_bytes <= settings.memory_estimate.adapters_bytes,
                    "adapter weights exceed explicit reservation"
                );
            }
        } else if let Some(path) = &settings.model.path {
            ensure!(
                std::fs::metadata(self.root.join(path))?.len()
                    <= settings.memory_estimate.weights_bytes,
                "weight reservation below actual artifact size"
            );
        }
        self.app
            .task_answers
            .verify_pin(&self.root, &pin, &self.tenant)
            .await?;
        let final_tokens =
            tokens.saturating_sub(settings.model.local_reasoning_tokens.unwrap_or(0));
        let call_id = Uuid::new_v4();
        if let Some(n) = &self.frozen.native {
            crate::inference_budget::reserve(
                &self.root,
                &self.tenant,
                &n.scope.owner,
                n.scope.run,
                call_id,
                (&n.scope.actor, "native_task"),
            )
            .map_err(|error| {
                if error
                    .downcast_ref::<crate::inference_budget::Exhausted>()
                    .is_some()
                {
                    anyhow::Error::new(ExecutionFailure {
                        kind: StopReason::BudgetExhausted,
                        message: error.to_string(),
                    })
                } else {
                    error
                }
            })?
            .context("native parent call allowance disappeared")?;
        }
        self.checkpoint("task_model_dispatch",json!({"call_id":call_id,"model":id,"identity":self.frozen.plan.resolved_models[id],"phase":if instruction.starts_with("LOCAL_TASK_INTAKE:"){"intake"}else{"worker"},"reserved_tokens":tokens,"native_dispatch_receipt":false}),None)?;
        let reply = if settings.model.provider == "mock" {
            self.app
                .gateway
                .complete(&settings.model, instruction, prompt, final_tokens, cancel)
                .await?
        } else {
            self.app
                .gateway
                .local
                .complete_scoped(
                    &settings.model,
                    instruction,
                    prompt,
                    final_tokens,
                    cancel,
                    &self.session,
                )
                .await
                .map_err(|error| {
                    if error
                        .downcast_ref::<crate::local_generation::PromptBudgetExceeded>()
                        .is_some()
                    {
                        anyhow::Error::new(ExecutionFailure {
                            kind: StopReason::ContextPressure,
                            message: error.to_string(),
                        })
                    } else {
                        error
                    }
                })?
        };
        self.checkpoint("task_model_return",json!({"call_id":call_id,"model":id,"native_dispatch_receipt":reply.local_execution.is_some(),"local_execution":reply.local_execution,"input_tokens":reply.usage.input,"output_tokens":reply.usage.output,"fixture":settings.model.provider=="mock"}),None)?;
        ensure!(
            reply
                .local_execution
                .as_ref()
                .is_none_or(|e| e.finish_reason.as_deref() != Some("length")),
            "generation truncated; no partial acceptance"
        );
        self.app
            .task_answers
            .verify_pin(&self.root, &pin, &self.tenant)
            .await?;
        Ok(ModelOutput {
            generative_model_calls: if settings.model.provider == "mock" {
                Some(0)
            } else {
                reply.local_execution.as_ref().map(|_| 1)
            },
            text: reply.text,
            input_tokens: reply.usage.input,
            output_tokens: reply.usage.output,
            memory_bytes: reply
                .local_execution
                .and_then(|e| e.resident_kib)
                .and_then(|n| n.checked_mul(1024)),
        })
    }
}
impl Adapter {
    fn checkpoint(
        &self,
        kind: &str,
        body: Value,
        revision: Option<(&ExecutionPlan, &TaskRequest, Option<&TaskAnalysis>)>,
    ) -> Result<()> {
        let mut db = company::connect(
            &self.root,
            &self.tenant,
            job_owner(&self.subject, self.frozen.native.as_ref()),
            false,
        )?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        install(&tx)?;
        let repo = Repository::new(&tx)?;
        let runtime = gents_local_mcp::digest(&serde_json::to_vec(&self.app.config)?);
        let scope = execution_scope(
            &repo,
            &tx,
            &self.root,
            &self.tenant,
            &self.subject,
            &runtime,
            &self.frozen.plan.nodes[0].profile,
            self.frozen.native.as_ref(),
        )?;
        if let Some(n) = &self.frozen.native {
            native::authorize(&tx, n, crate::now_ms()).map_err(|_| ExecutionFailure {
                kind: StopReason::SourceChanged,
                message: "Native custody changed; no new dispatch".into(),
            })?;
        }
        if runtime != self.frozen.runtime
            || self.frozen.scope_hash != gents_local_mcp::digest(&serde_json::to_vec(&scope)?)
            || self.frozen.corpus_hash != snapshot(&repo, &scope, crate::now_ms())?
            || !repo.workspace_task_current(
                &scope,
                self.frozen.request.task_id,
                &self.settings.workspace_path(&self.root, &self.tenant)?,
                crate::now_ms(),
            )?
        {
            return Err(ExecutionFailure {
                kind: StopReason::SourceChanged,
                message: "Admission source/access/configuration epoch changed; no new dispatch"
                    .into(),
            }
            .into());
        }
        if validation_bindings(
            &tx,
            &self.root,
            self.frozen.native.as_ref(),
            &self.frozen.plan,
        )? != self.frozen.validation_bindings
        {
            return Err(ExecutionFailure {
                kind: StopReason::SourceChanged,
                message:
                    "Frozen runtime-check source identity changed; no new dispatch or delivery"
                        .into(),
            }
            .into());
        }
        if let Some((plan, request, analysis)) = revision {
            ensure!(
                request.input == self.frozen.request.input
                    && request.task_id == self.frozen.request.task_id,
                "revision changed frozen input"
            );
            tx.execute(
                "INSERT INTO task_workflow_plan_revisions_v1 VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    self.tenant,
                    self.subject,
                    self.job_id.to_string(),
                    plan.revision,
                    body.to_string(),
                    crate::now_ms() as i64
                ],
            )?;
            let mut result = self.frozen.base.clone();
            result.plan = Some(plan.clone());
            result.analysis = analysis.cloned();
            result.assumptions = request.assumptions.clone();
            result.clarification_questions = request.clarification_questions.clone();
            let tasks:Vec<String> = tx.prepare("SELECT task FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job=?3 AND cancelled=0")?.query_map(params![self.tenant,self.subject,self.job_id.to_string()],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
            for task in tasks {
                result.task_id = Uuid::parse_str(&task)?;
                repo.replace_result(&scope, &result)?;
            }
        }
        tx.execute(
            "INSERT INTO audit(run_id,kind,body,created) VALUES(?1,?2,?3,?4)",
            params![
                self.job_id.to_string(),
                kind,
                body.to_string(),
                crate::now_ms() as i64
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}
impl adaptive::Observer for Adapter {
    fn intake<'a>(&'a self, receipt: &'a adaptive::IntakeReceipt) -> FutureResult<'a, ()> {
        Box::pin(
            async move { self.checkpoint("task_intake", serde_json::to_value(receipt)?, None) },
        )
    }
    fn plan<'a>(
        &'a self,
        request: &'a TaskRequest,
        plan: &'a ExecutionPlan,
        analysis: Option<&'a TaskAnalysis>,
        trigger: Option<&'a StopReason>,
        consumption: &'a gents_local_mcp::task_engine::resources::Consumption,
    ) -> FutureResult<'a, ()> {
        Box::pin(async move {
            self.checkpoint("task_plan_revision",json!({"plan":plan,"analysis":analysis,"trigger":trigger,"consumption":consumption}),Some((plan,request,analysis)))
        })
    }
}
impl Adapter {
    fn ensure_workspace_evidence(&self, evidence: &[Evidence]) -> Result<()> {
        if !evidence
            .iter()
            .any(|e| e.source_id.starts_with("workspace:v1:"))
        {
            return Ok(());
        }
        ensure!(self.settings.workspace_reads, "workspace authority revoked");
        let mut db = company::connect(
            &self.root,
            &self.tenant,
            job_owner(&self.subject, self.frozen.native.as_ref()),
            false,
        )?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let repo = Repository::new(&tx)?;
        let mut scope =
            task_frontdoor::owner_scope(&repo, &self.tenant, &self.subject, &self.frozen.runtime)?;
        scope.profile_revision =
            format!("workflow-profile/v1/{}", self.frozen.plan.nodes[0].profile);
        for e in evidence
            .iter()
            .filter(|e| e.source_id.starts_with("workspace:v1:"))
        {
            if !repo.workspace_evidence_current(
                &scope,
                e,
                &self.settings.workspace_path(&self.root, &self.tenant)?,
                crate::now_ms(),
            )? {
                return Err(ExecutionFailure {
                    kind: StopReason::SourceChanged,
                    message:
                        "Captured workspace evidence changed or was removed; no model dispatched"
                            .into(),
                }
                .into());
            }
        }
        tx.commit()?;
        Ok(())
    }
}
impl ReadTools for Adapter {
    fn call<'a>(&'a self, request: &'a ToolRequest) -> FutureResult<'a, Value> {
        Box::pin(async move {
            let arguments = read_tools::validate(request)?;
            let began = std::time::Instant::now();
            let call_id = Uuid::new_v4();
            self.checkpoint(
                "task_tool_dispatch",
                json!({"call_id":call_id,"request":request,"side_effect":false}),
                None,
            )?;
            let result = (|| -> Result<Value> {
                match arguments {
                    read_tools::Arguments::Search(query) => {
                        if let Some(n) = &self.frozen.native {
                            let mut db =
                                company::connect(&self.root, &self.tenant, &n.scope.owner, false)?;
                            let tx =
                                db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                            let current = native::authorize(&tx, n, crate::now_ms())?;
                            current.authorize(
                                &n.scope.actor,
                                "search_knowledge",
                                &json!({"query":query.query}),
                            )?;
                            let repo = Repository::new(&tx)?;
                            let scope = execution_scope(
                                &repo,
                                &tx,
                                &self.root,
                                &self.tenant,
                                &self.subject,
                                &self.frozen.runtime,
                                &self.frozen.plan.nodes[0].profile,
                                Some(n),
                            )?;
                            let evidence: Vec<_> = repo
                                .search(&scope, &query.query, None, 8192, crate::now_ms())?
                                .into_iter()
                                .map(|h| h.evidence)
                                .collect();
                            tx.commit()?;
                            return Ok(json!({"evidence":evidence}));
                        }
                        let runtime =
                            gents_local_mcp::digest(&serde_json::to_vec(&self.app.config)?);
                        let value = task_frontdoor::operation(
                            &self.root,
                            &self.tenant,
                            &self.subject,
                            &runtime,
                            task_frontdoor::Operation::Search {
                                query: query.query,
                                domain: None,
                            },
                        )?;
                        Ok(
                            json!({"evidence":value["hits"].as_array().context("search hits missing")?.iter().map(|h|h["evidence"].clone()).collect::<Vec<_>>()}),
                        )
                    }
                    _ => {
                        ensure!(
                            self.settings.workspace_reads,
                            "workspace reads disabled in host configuration"
                        );
                        let mut db = company::connect(
                            &self.root,
                            &self.tenant,
                            job_owner(&self.subject, self.frozen.native.as_ref()),
                            false,
                        )?;
                        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                        let repo = Repository::new(&tx)?;
                        let mut scope = task_frontdoor::owner_scope(
                            &repo,
                            &self.tenant,
                            &self.subject,
                            &self.frozen.runtime,
                        )?;
                        scope.profile_revision =
                            format!("workflow-profile/v1/{}", self.frozen.plan.nodes[0].profile);
                        let mut policy = ExecutionPolicy::read_only(scope, crate::now_ms());
                        policy.deadline_ms = self.frozen.deadline_ms;
                        self.settings
                            .workspace_policy(&mut policy, &self.root, &self.tenant)?;
                        let value = repo.capture_workspace(
                            &self.frozen.request,
                            &policy,
                            &self.settings.workspace_path(&self.root, &self.tenant)?,
                            request,
                            crate::now_ms(),
                        )?;
                        tx.commit()?;
                        Ok(value)
                    }
                }
            })();
            self.checkpoint("task_tool_return",json!({"call_id":call_id,"name":request.name,"elapsed_micros":began.elapsed().as_micros(),"status":if result.is_ok(){"succeeded"}else{"failed"},"error":result.as_ref().err().map(ToString::to_string),"result":result.as_ref().ok()}),None)?;
            result
        })
    }
}
impl validators::Host for Adapter {
    fn check<'a>(
        &'a self,
        name: &'a str,
        budget: &'a gents_local_mcp::task_engine::resources::Budget,
    ) -> FutureResult<'a, validators::Report> {
        Box::pin(async move {
            ensure!(
                matches!(name, "dotnet_build" | "dotnet_api" | "angular_build"),
                "unregistered runtime validator"
            );
            let tool = if name == "angular_build" {
                "angular_check"
            } else if name == "dotnet_api" {
                "dotnet_api_check"
            } else {
                "dotnet_check"
            };
            let bindings = self
                .frozen
                .validation_bindings
                .iter()
                .filter(|b| b.validator == name)
                .collect::<Vec<_>>();
            let Some(n) = &self.frozen.native else {
                return Ok(validators::Report{checks:vec![AcceptanceCheck{name:format!("{name}/v1"),status:CheckStatus::NotRun,detail:"Owner answer context has no approved native actor/run build contract. Use the existing native company workflow with explicit grants and frozen inputs.".into()}],native_receipts:vec![]});
            };
            if bindings.is_empty() {
                return Ok(validators::Report{checks:vec![AcceptanceCheck{name:format!("{name}/v1"),status:CheckStatus::NotRun,detail:"No authorized runtime contract was frozen for this task/profile at admission; no SDK dispatched.".into()}],native_receipts:vec![]});
            }
            let mut report = validators::Report::default();
            for binding in bindings {
                if matches!(name, "dotnet_build" | "angular_build") {
                    let cached = self
                        .validation_results
                        .lock()
                        .await
                        .get(&format!("{name}/{}", binding.contract_id))
                        .cloned();
                    if let Some(mut cached) = cached {
                        self.checkpoint("task_validation_return",json!({"validator":name,"contract_id":binding.contract_id,"same_task_compile_facet_reused":!cached.native_receipts.is_empty(),"compile_not_replayed_without_receipt":cached.native_receipts.is_empty()}),None)?;
                        let base_authority = gents_local_mcp::default_authority(self.root.clone())?;
                        let authority = gents_local_mcp::angular_check::ReceiptAuthority::for_scope(
                            &base_authority,
                            &n.scope,
                        );
                        let mut db =
                            company::connect(&self.root, &self.tenant, &n.scope.owner, false)?;
                        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                        let policy = native::authorize(&tx, n, crate::now_ms())?;
                        let context = gents_local_mcp::ToolContext {
                            authority: &authority,
                            scope: &n.scope,
                            policy: &policy,
                            db: &tx,
                        };
                        ensure!(
                            validators::receipts_current(&context, &cached.native_receipts)?,
                            "Same-task native receipt is no longer current; no automatic replay"
                        );
                        tx.commit()?;
                        report.checks.append(&mut cached.checks);
                        report.native_receipts.append(&mut cached.native_receipts);
                        continue;
                    }
                }
                let scheduler = self
                    .app
                    .task_workflows
                    .scheduler
                    .get()
                    .context("validator scheduler unavailable")?;
                let estimate=MemoryEstimate{weights_bytes:0,adapters_bytes:0,kv_bytes:0,context_bytes:0,runtime_bytes:if name=="angular_build" {3*1024*1024*1024}else{2*1024*1024*1024},headroom_bytes:0,provenance:"Configured native validator process-group RSS enforcement-limit reservation; estimate, not measured use.".into()};
                let _reservation = scheduler.reserve(&estimate, budget).await?;
                let mut compile_report = validators::Report::default();
                budget.check()?;
                self.checkpoint("task_validation_dispatch",json!({"validator":name,"binding":binding,"scope":if name=="angular_build" {"Owner-frozen copied Angular application build and finite host DOM checks in an OS-confined browser; no promotion or general correctness"}else if name=="dotnet_api" {"Owner-frozen copied application and finite host HTTP cases through an inherited loopback socket; no promotion or general correctness"}else{"Owner-frozen scratch-only offline compilation; no app execution, promotion or independent acceptance"}}),None)?;
                budget.reserve_tool()?;
                if name == "dotnet_api" {
                    // A failed/lost API invocation may already have compiled.
                    // Its missing canonical receipt must not trigger another
                    // build as an implicit retry of that same contract.
                    self.validation_results.lock().await.insert(
                        format!("dotnet_build/{}", binding.contract_id),
                        validators::Report { checks: vec![AcceptanceCheck {
                            name: "dotnet_build/v1".into(),
                            status: CheckStatus::NotRun,
                            detail: "The API invocation did not establish a current canonical compilation receipt. No separate SDK build is automatically dispatched for this same contract.".into(),
                        }], native_receipts: vec![] },
                    );
                }
                if name == "angular_build" {
                    self.validation_results.lock().await.insert(format!("angular_build/{}",binding.contract_id),validators::Report{checks:vec![AcceptanceCheck{name:"angular_build/v1".into(),status:CheckStatus::NotRun,detail:"This task's Angular invocation did not establish a current canonical receipt. No automatic SDK replay for this frozen contract.".into()}],native_receipts:vec![]});
                }
                let args = json!({"contract_id":binding.contract_id,"expected_source_identity":binding.source_identity});
                let value = gents_local_mcp::client::invoke(
                    &self.root,
                    &n.scope,
                    tool,
                    &args,
                    &budget.cancel,
                )
                .await?;
                ensure!(
                    value["ok"] == true,
                    "native SDK check failed: {}",
                    value["error"]
                );
                let reference = {
                    let base_authority = gents_local_mcp::default_authority(self.root.clone())?;
                    let authority = gents_local_mcp::angular_check::ReceiptAuthority::for_scope(
                        &base_authority,
                        &n.scope,
                    );
                    let mut db = company::connect(&self.root, &self.tenant, &n.scope.owner, false)?;
                    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                    let policy = native::authorize(&tx, n, crate::now_ms())?;
                    let context = gents_local_mcp::ToolContext {
                        authority: &authority,
                        scope: &n.scope,
                        policy: &policy,
                        db: &tx,
                    };
                    let reference = if name == "angular_build" {
                        gents_local_mcp::angular_check::reference(
                            &context,
                            binding,
                            &value["result"],
                            &value["mcp_receipt"],
                        )?
                    } else {
                        gents_local_mcp::dotnet_check::reference(
                            &context,
                            binding,
                            &value["result"],
                            &value["mcp_receipt"],
                        )?
                    };
                    if name == "dotnet_api" {
                        let compile = gents_local_mcp::dotnet_check::compile_reference(
                            &context,
                            &reference,
                            &value["result"],
                        )?;
                        compile_report.checks.push(AcceptanceCheck{name:"dotnet_build/v1".into(),status:compile.outcome.clone(),detail:format!("Compilation facet of this same task's native API invocation; canonical receipt {}. No duplicate SDK build or general correctness claim.",compile.result_sha256)});
                        compile_report.native_receipts.push(compile);
                    }
                    tx.commit()?;
                    reference
                };
                self.checkpoint(
                    "task_validation_return",
                    serde_json::to_value(&reference)?,
                    None,
                )?;
                let check=AcceptanceCheck{name:format!("{name}/v1"),status:reference.outcome.clone(),detail:format!("Contract {}: {}; canonical receipt {}, source identity {}. Explicit copied inputs and frozen cases only; no general correctness, expert qualification or promotion.",reference.contract_id,if name=="angular_build" {"actual installed Angular build and frozen host browser DOM checks"}else if name=="dotnet_api" {"actual host HTTP status/JSON checks against the isolated application"}else{"actual installed-SDK compilation"},reference.result_sha256,reference.source_identity)};
                if name == "angular_build" {
                    self.validation_results.lock().await.insert(
                        format!("angular_build/{}", binding.contract_id),
                        validators::Report {
                            checks: vec![check.clone()],
                            native_receipts: vec![reference.clone()],
                        },
                    );
                }
                report.checks.push(check);
                report.native_receipts.push(reference);
                if name == "dotnet_api" {
                    self.validation_results.lock().await.insert(
                        format!("dotnet_build/{}", binding.contract_id),
                        compile_report,
                    );
                }
            }
            Ok(report)
        })
    }
}
#[allow(clippy::too_many_arguments)]
async fn run(
    app: Shared,
    root: PathBuf,
    tenant: String,
    subject: String,
    id: Uuid,
    job: Job,
    settings: Settings,
    cancel: Arc<AtomicBool>,
) {
    let result = async {
        let policy = {
            let mut db = company::connect(
                &root,
                &tenant,
                job_owner(&subject, job.native.as_ref()),
                false,
            )?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let repo = Repository::new(&tx)?;
            let scope = execution_scope(
                &repo,
                &tx,
                &root,
                &tenant,
                &subject,
                &job.runtime,
                &job.plan.nodes[0].profile,
                job.native.as_ref(),
            )?;
            let mut policy = ExecutionPolicy::read_only(scope, crate::now_ms());
            policy.deadline_ms = job.deadline_ms;
            policy.max_model_calls = settings.max_calls;
            policy.evidence_required = if settings
                .adaptive
                .as_ref()
                .is_some_and(|a| a.intake.is_some())
            {
                intake::mandatory_evidence(
                    &job.request,
                    &job.caller_hints,
                    &job.plan.nodes[0].profile,
                    settings.allow_parametric_answers,
                )
            } else {
                job.plan.evidence_required
            };
            policy.max_tokens = settings.total_generation_tokens;
            policy.max_retries = settings.max_retries;
            policy.max_concurrency = settings.concurrency;
            policy.max_fan_out = 2;
            policy.max_depth = 2;
            execution_tools(
                &settings,
                &mut policy,
                &tx,
                &root,
                &tenant,
                job.native.as_ref(),
            )?;
            tx.commit()?;
            policy
        };
        let scheduler = app
            .task_workflows
            .scheduler
            .get_or_try_init(|| async {
                let observed = gents_local_mcp::host_memory::Snapshot::capture(
                    &gents_local_mcp::host_memory::NativeMemoryProbe,
                )?;
                Scheduler::new(
                    settings.concurrency as usize,
                    settings.resident_budget_bytes.min(
                        observed
                            .effective_available_bytes
                            .context("host memory unavailable")?,
                    ),
                )
            })
            .await?;
        let adapter = Adapter {
            app: app.clone(),
            settings: settings.clone(),
            root: root.clone(),
            tenant: tenant.clone(),
            subject: subject.clone(),
            session: format!("{tenant}:task:{id}:{subject}"),
            job_id: id,
            frozen: job.clone(),
            validation_results: Mutex::new(BTreeMap::new()),
        };
        let models = settings.resolved_models();
        let registry = settings.registry()?;
        let executor = Executor {
            validators: Some(&adapter),
            profiles: Some(&registry),
            worker: &adapter,
            tools: &adapter,
            scheduler,
            models: &models,
            limits: Limits {
                per_call_tokens: settings.per_call_tokens,
                per_node_timeout_ms: settings.deadline_seconds * 1000,
                ..Limits::default()
            },
        };
        let mut adaptive_settings = settings.adaptive.clone().unwrap_or_default();
        if job.skip_model_intake {
            adaptive_settings.intake = None;
        }
        adaptive::Coordinator {
            executor: &executor,
            intake_worker: &adapter,
            observer: &adapter,
            settings: adaptive_settings,
            allow_parametric: settings.allow_parametric_answers,
        }
        .run(
            &job.request,
            &job.plan,
            job.caller_hints.clone(),
            &policy,
            job.evidence.clone(),
            cancel.clone(),
            job.analysis_only,
        )
        .await
    }
    .await;
    // Signal the native adapter before stopping only this job's owned session.
    cancel.store(true, Ordering::Relaxed);
    app.gateway
        .local
        .stop_session(&format!("{tenant}:task:{id}:{subject}"))
        .await;
    let save = (|| -> Result<()> {
        let mut db = company::connect(
            &root,
            &tenant,
            job_owner(&subject, job.native.as_ref()),
            false,
        )?;
        let receipt_authority_base = gents_local_mcp::default_authority(root.clone())?;
        let receipt_authority = job.native.as_ref().map(|n| {
            gents_local_mcp::angular_check::ReceiptAuthority::for_scope(
                &receipt_authority_base,
                &n.scope,
            )
        });
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let repo = Repository::new(&tx)?;
        let scope = execution_scope(
            &repo,
            &tx,
            &root,
            &tenant,
            &subject,
            &job.runtime,
            &job.plan.nodes[0].profile,
            job.native.as_ref(),
        )?;
        let runtime_validation_invalid =
            !validation_bindings(&tx, &root, job.native.as_ref(), &job.plan)
                .is_ok_and(|current| current == job.validation_bindings);
        let receipt_current = if let (Some(n), Ok(outcome)) = (&job.native, &result) {
            let references = outcome
                .attempts
                .last()
                .and_then(|a| a.outcome.as_ref())
                .map(|o| o.validation.native_receipts.as_slice())
                .unwrap_or(&[]);
            let policy = native::authorize(&tx, n, crate::now_ms())?;
            validators::receipts_current(
                &gents_local_mcp::ToolContext {
                    authority: receipt_authority.as_ref().unwrap(),
                    scope: &n.scope,
                    policy: &policy,
                    db: &tx,
                },
                references,
            )
            .unwrap_or(false)
        } else {
            true
        };
        let mut delivered = job.base.clone();
        let mut receipts = json!([]);
        match result {
            Ok(outcome) => {
                delivered.analysis = outcome.analysis.clone();
                if let Some(report) = outcome
                    .attempts
                    .last()
                    .and_then(|a| a.outcome.as_ref())
                    .map(|o| &o.validation)
                {
                    delivered.acceptance_checks = report.checks.clone();
                    delivered.validation_receipts = report.native_receipts.clone();
                }

                delivered.clarification_questions = outcome.request.clarification_questions.clone();
                delivered.assumptions = outcome.request.assumptions.clone();
                delivered.plan = outcome
                    .attempts
                    .last()
                    .map(|a| a.plan.clone())
                    .or(delivered.plan);
                let node_receipts: Vec<_> = outcome
                    .attempts
                    .iter()
                    .filter_map(|a| a.outcome.as_ref())
                    .flat_map(|o| &o.receipts)
                    .collect();
                let intake_attempts = outcome.intake.as_ref().map_or(0, |i| i.attempts);
                delivered.status = outcome.status;
                delivered.answer = outcome.answer;
                delivered.blockers = outcome.blockers;
                delivered.verification_state = outcome.verification_state;
                delivered.measurements.generative_model_calls =
                    if outcome.consumption.reserved_calls
                        == intake_attempts + node_receipts.iter().map(|r| r.attempts).sum::<u32>()
                    {
                        node_receipts.iter().try_fold(
                            outcome
                                .intake
                                .as_ref()
                                .map_or(Some(0), |i| i.generative_model_calls)
                                .unwrap_or(0),
                            |sum, r| r.generative_model_calls.map(|n| sum.saturating_add(n)),
                        )
                    } else {
                        None
                    };
                delivered.measurements.tool_calls = outcome.consumption.tool_calls;
                delivered.measurements.elapsed_micros = Some(outcome.elapsed_micros);
                receipts = json!({"intake":outcome.intake,"attempts":outcome.attempts,"consumption":outcome.consumption});
                delivered.evidence = outcome.evidence;
                delivered.measurements.input_tokens = node_receipts.iter().try_fold(
                    outcome
                        .intake
                        .as_ref()
                        .map_or(Some(0), |i| i.input_tokens)
                        .unwrap_or(0),
                    |sum, r| r.input_tokens.map(|n| sum.saturating_add(n)),
                );
                delivered.measurements.output_tokens = node_receipts.iter().try_fold(
                    outcome
                        .intake
                        .as_ref()
                        .map_or(Some(0), |i| i.output_tokens)
                        .unwrap_or(0),
                    |sum, r| r.output_tokens.map(|n| sum.saturating_add(n)),
                );
                delivered.measurements.memory_bytes =
                    node_receipts.iter().filter_map(|r| r.memory_bytes).max();
                if outcome.consumption.reserved_calls
                    != intake_attempts + node_receipts.iter().map(|r| r.attempts).sum::<u32>()
                {
                    delivered.measurements.input_tokens = None;
                    delivered.measurements.output_tokens = None;
                }
                if let Some(i) = &outcome.intake {
                    if i.generative_model_calls.is_none() {
                        delivered.measurements.generative_model_calls = None;
                    }
                    if i.input_tokens.is_none() {
                        delivered.measurements.input_tokens = None;
                    }
                    if i.output_tokens.is_none() {
                        delivered.measurements.output_tokens = None;
                    }
                    delivered.measurements.memory_bytes = delivered
                        .measurements
                        .memory_bytes
                        .into_iter()
                        .chain(i.memory_bytes)
                        .max();
                }
            }
            Err(error) => {
                delivered.status = "failed".into();
                delivered.blockers.push(error.to_string());
                delivered.measurements.generative_model_calls = None;
            }
        }
        delivered.measurements.provider_cost_usd = Some(0);
        let workspace_root = settings.workspace_path(&root, &tenant)?;
        let workspace_invalid = !repo.workspace_task_current(
            &scope,
            job.request.task_id,
            &workspace_root,
            crate::now_ms(),
        )? || delivered
            .evidence
            .iter()
            .filter(|e| e.source_id.starts_with("workspace:v1:"))
            .any(|e| {
                !repo
                    .workspace_evidence_current(&scope, e, &workspace_root, crate::now_ms())
                    .unwrap_or(false)
            });
        if workspace_invalid
            || runtime_validation_invalid
            || !receipt_current
            || snapshot(&repo, &scope, crate::now_ms())? != job.corpus_hash
            || gents_local_mcp::digest(&serde_json::to_vec(&scope)?) != job.scope_hash
        {
            delivered.status = "evidence_invalidated".into();
            delivered.verification_state = "not_verified".into();
            delivered.answer = None;
            delivered.evidence.clear();
            delivered.validation_receipts.clear();
            for check in &mut delivered.acceptance_checks {
                if matches!(
                    check.name.as_str(),
                    "dotnet_build/v1" | "dotnet_api/v1" | "angular_build/v1"
                ) {
                    check.status = CheckStatus::NotRun;
                    check.detail = "Runtime build observation invalidated before delivery.".into();
                }
            }
        }
        delivered.acceptance_checks.push(AcceptanceCheck{name:"independent_factual_or_build_acceptance".into(),status:CheckStatus::NotRun,detail:"Generated answer and workflow completion do not establish factual correctness, builds, executed Cosmos queries or side effects.".into()});
        if !workspace_invalid
            && delivered.answer.is_some()
            && delivered
                .evidence
                .iter()
                .any(|e| e.source_id.starts_with("workspace:v1:"))
        {
            delivered.acceptance_checks.push(AcceptanceCheck{name:"current_workspace_source_bytes/v1".into(),status:CheckStatus::Passed,detail:"Host rechecked captured file/listing hashes, byte pages, root identity, scope and TTL. This check establishes source observations, not installed packages, a build, a query or factual entailment.".into()});
        }
        tx.execute("UPDATE task_workflow_jobs_v1 SET state='completed',result=?4,expires=CASE WHEN ?5 THEN ?6 ELSE expires END WHERE tenant=?1 AND subject=?2 AND id=?3",params![tenant,subject,id.to_string(),serde_json::to_string(&delivered)?,delivered.answer.is_none(),(crate::now_ms()+1000) as i64])?;
        let tasks:Vec<(String,bool)>=tx.prepare("SELECT task,cancelled FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job=?3")?.query_map(params![tenant,subject,id.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        for (task, cancelled) in tasks {
            let mut result = delivered.clone();
            result.task_id = Uuid::parse_str(&task)?;
            if result.task_id != job.request.task_id {
                result.cache.origin_task_id = Some(job.request.task_id);
                result.measurements.generative_model_calls = Some(0);
                result.measurements.tool_calls = 0;
                result.measurements.memory_bytes = None;
                result.measurements.input_tokens = Some(0);
                result.measurements.output_tokens = Some(0);
            }
            if cancelled {
                result.status = "cancelled".into();
                result.verification_state = "not_verified".into();
                result.answer = None;
                result.evidence.clear();
            }
            repo.replace_result(&scope, &result)?;
        }
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'task_workflow_finished',?2,?3)",params![id.to_string(),json!({"result":delivered,"receipts":receipts,"model_reservations_are_not_proof_of_dispatch":true}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        Ok(())
    })();
    if save.is_err() {
        eprintln!(
            "Workflow result checkpoint unavailable; retain original admission without replay"
        );
    }
    app.task_workflows.active.lock().await.remove(&id);
}

#[cfg(test)]
mod workspace_settings_tests {
    use super::*;
    fn settings() -> Settings {
        serde_json::from_value(json!({"models":{"fixture":{"model":{"provider":"mock","model":"fixture"},"sha256":null,"memory_estimate":{"weights_bytes":1,"adapters_bytes":0,"kv_bytes":1,"context_bytes":1,"runtime_bytes":1,"headroom_bytes":1,"provenance":"fixture"},"evaluated_profiles":["general"],"evaluation_reference":"fixture"}},"default_model":"fixture","max_calls":4,"total_generation_tokens":512,"per_call_tokens":128,"deadline_seconds":30,"max_retries":0,"max_pending_jobs":1,"concurrency":1,"resident_budget_bytes":1048576,"allow_parametric_answers":false})).unwrap()
    }
    #[test]
    fn mlx_admission_requires_base_and_adapter_pins_and_isolates_adapter_identity() {
        let mut settings = settings();
        let worker = settings.models.get_mut("fixture").unwrap();
        worker.model.provider = "mlx".into();
        worker.model.path = Some("models/existing-package".into());
        worker.model.context_tokens = Some(2048);
        assert!(settings.validate().is_err());
        settings.models.get_mut("fixture").unwrap().sha256 = Some("a".repeat(64));
        settings.validate().unwrap();
        let base = worker_identity(&settings.models["fixture"]);
        settings.models.get_mut("fixture").unwrap().model.adapter =
            Some("data/companies/one/training/runs/adapter".into());
        assert!(settings.validate().is_err());
        settings.models.get_mut("fixture").unwrap().adapter_sha256 = Some("b".repeat(64));
        settings.validate().unwrap();
        let adapted = worker_identity(&settings.models["fixture"]);
        assert_ne!(base, adapted);
        assert_eq!(settings.resolved_models()["fixture"].identity, adapted);
        settings.models.get_mut("fixture").unwrap().adapter_sha256 = Some("c".repeat(64));
        assert_ne!(worker_identity(&settings.models["fixture"]), adapted);
        settings.models.get_mut("fixture").unwrap().sha256 = Some("A".repeat(64));
        assert!(settings.validate().is_err());
    }
    #[test]
    fn default_workspace_is_company_private_and_does_not_change_old_config_serialization() {
        let settings = settings();
        settings.validate().unwrap();
        assert_eq!(
            settings.workspace_relative_root("one").unwrap(),
            "data/companies/one/task-workspace"
        );
        assert_ne!(
            settings.workspace_relative_root("one").unwrap(),
            settings.workspace_relative_root("two").unwrap()
        );
        let value = serde_json::to_value(settings).unwrap();
        assert!(value.get("workspace_reads").is_none());
        assert!(value.get("workspace_root").is_none());
        assert!(value.get("native_mcp").is_none());
        assert!(value.get("native_companies").is_none());
        assert!(value["models"]["fixture"].get("max_input_bytes").is_none());
    }
    #[test]
    fn native_worker_requires_explicit_valid_company_allowlist() {
        let mut settings = settings();
        settings.native_mcp = true;
        assert!(settings.validate().is_err());
        settings.native_companies.insert("one".into());
        settings.validate().unwrap();
        settings.native_companies.insert("../other".into());
        assert!(settings.validate().is_err());
    }
    #[test]
    fn workspace_configuration_cannot_expose_the_app_root_or_another_company_store() {
        let mut settings = settings();
        settings.workspace_reads = true;
        for root in [
            ".",
            "",
            "../outside",
            "/outside",
            "data",
            "data/companies/other/task-workspace",
            "config",
            "models",
            "target",
            "project/../config",
        ] {
            settings.workspace_root = Some(root.into());
            assert!(settings.validate().is_err(), "{root}");
        }
        for root in [
            "project",
            "projects/{company}",
            "data/companies/{company}/task-workspace",
        ] {
            settings.workspace_root = Some(root.into());
            settings.validate().unwrap();
        }
    }
    #[test]
    fn existing_job_retention_migration_preserves_body_and_does_not_replay() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("BEGIN IMMEDIATE; CREATE TABLE task_workflow_jobs_v1(tenant TEXT,subject TEXT,key TEXT,id TEXT,body TEXT,sha256 TEXT,state TEXT,result TEXT,expires INTEGER,PRIMARY KEY(tenant,subject,key)); INSERT INTO task_workflow_jobs_v1 VALUES('one','owner','key','old-job','frozen-body','frozen-sha','completed',NULL,42);").unwrap();
        install(&db).unwrap();
        install(&db).unwrap();
        let row: (String, String, i64, i64) = db
            .query_row(
                "SELECT body,sha256,expires,retained_until FROM task_workflow_jobs_v1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, ("frozen-body".into(), "frozen-sha".into(), 42, 42));
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM task_workflow_subscribers_v1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}

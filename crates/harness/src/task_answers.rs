//! Async adapter over shared grounded-task custody and the existing model pool.
use crate::{
    company,
    provider::{LanguageModel, Model, Reply},
    task_frontdoor, Shared,
};
use anyhow::{ensure, Context, Result};
use gents_local_mcp::task_engine::{contracts::*, grounded, Repository};
use rusqlite::{params, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub model: Model,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_sha256: Option<String>,
    pub final_tokens: u32,
    pub deadline_seconds: u64,
    pub draft_ttl_seconds: u64,
    pub max_pending_jobs: usize,
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        self.model.validate()?;
        if matches!(self.model.provider.as_str(), "local" | "mlx") {
            ensure!(
                self.model_sha256
                    .as_ref()
                    .is_some_and(|hash| hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
                "native grounded model requires an owner-frozen SHA-256"
            );
        }
        ensure!(
            matches!(self.model.provider.as_str(), "local" | "mlx" | "mock"),
            "grounded answers require owned local inference; mock is a protocol fixture only"
        );
        ensure!(
            self.model.adapter.is_some() == self.adapter_sha256.is_some(),
            "adapter path and SHA-256 must be supplied together"
        );
        if let Some(hash) = &self.adapter_sha256 {
            ensure!(
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "immutable adapter SHA-256 required"
            );
        }
        ensure!(
            (64..=2048).contains(&self.final_tokens)
                && (1..=300).contains(&self.deadline_seconds)
                && (1..=3600).contains(&self.draft_ttl_seconds)
                && (1..=8).contains(&self.max_pending_jobs),
            "invalid grounded limits"
        );
        ensure!(
            self.model
                .local_final_tokens
                .is_none_or(|n| n <= self.final_tokens),
            "model final allowance exceeds grounded policy"
        );
        ensure!(
            self.model.local_reasoning_tokens.unwrap_or(0) + self.final_tokens <= 4096,
            "grounded total generation allowance exceeds bound"
        );
        Ok(())
    }
    fn identity(&self) -> Result<String> {
        Ok(format!(
            "{}/{}@{}",
            self.model.provider,
            self.model.model,
            gents_local_mcp::digest(&serde_json::to_vec(self)?)
        ))
    }
}
#[derive(Default)]
pub struct Service {
    active: Mutex<HashMap<Uuid, Arc<AtomicBool>>>,
    pins: Mutex<HashMap<String, Value>>,
}

fn scoped(
    repo: &Repository<'_>,
    tenant: &str,
    subject: &str,
    runtime: &str,
    root: &Path,
) -> Result<AccessScope> {
    let mut scope = task_frontdoor::owner_scope(repo, tenant, subject, runtime)?;
    repo.refresh_dependencies(&scope, root)?;
    scope.profile_revision = "local-grounded-answer/v1".into();
    Ok(scope)
}
impl Service {
    pub(super) async fn verify_pin(
        &self,
        root: &Path,
        settings: &Settings,
        company: &str,
    ) -> Result<()> {
        if settings.model.provider == "mock" {
            return Ok(());
        }
        if settings.model.provider == "mlx" {
            let relative = settings.model.path.as_deref().context("MLX path missing")?;
            ensure!(
                relative.starts_with("models/") && crate::model_package::safe_relative(relative),
                "confined MLX package required"
            );
            self.verify_package(
                root,
                relative,
                crate::model_package::Kind::Model,
                settings
                    .model_sha256
                    .as_deref()
                    .context("MLX model pin missing")?,
            )
            .await?;
            if let Some(adapter) = &settings.model.adapter {
                crate::company::identifier(company)?;
                ensure!(
                    crate::model_package::safe_relative(adapter)
                        && adapter.starts_with(&format!("data/companies/{company}/training/runs/")),
                    "adapter outside this company's training root"
                );
                self.verify_package(
                    root,
                    adapter,
                    crate::model_package::Kind::Adapter,
                    settings
                        .adapter_sha256
                        .as_deref()
                        .context("adapter pin missing")?,
                )
                .await?;
            }
            return Ok(());
        }
        let relative = settings
            .model
            .path
            .as_deref()
            .context("native model path absent")?;
        ensure!(
            crate::model_artifact::relative_path(relative),
            "grounded model requires a confined models/ GGUF path"
        );
        let fingerprint = || -> Result<Value> {
            let mut path = root.to_path_buf();
            let mut parts = vec![];
            for part in std::path::Path::new(relative).components() {
                path.push(part);
                let info = std::fs::symlink_metadata(&path)?;
                ensure!(!info.is_symlink(), "model symlink rejected");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    parts.push(json!([
                        info.dev(),
                        info.ino(),
                        info.uid(),
                        info.mode(),
                        info.nlink(),
                        info.len(),
                        info.mtime(),
                        info.mtime_nsec(),
                        info.ctime(),
                        info.ctime_nsec()
                    ]));
                }
                #[cfg(not(unix))]
                {
                    parts.push(json!([info.len(), format!("{:?}", info.modified()?)]));
                }
            }
            Ok(json!(parts))
        };
        let key = format!("{}:{}", relative, settings.model_sha256.as_deref().unwrap());
        let mut pins = self.pins.lock().await;
        let observed = fingerprint()?;
        if pins.get(&key) == Some(&observed) {
            return Ok(());
        }
        let (root, path, hash) = (
            root.to_path_buf(),
            relative.to_owned(),
            settings.model_sha256.clone().unwrap(),
        );
        tokio::task::spawn_blocking(move || {
            crate::model_artifact::verify(&root, &path, &hash).map_err(anyhow::Error::msg)
        })
        .await??;
        pins.insert(key, fingerprint()?);
        Ok(())
    }
    async fn verify_package(
        &self,
        root: &Path,
        relative: &str,
        kind: crate::model_package::Kind,
        expected: &str,
    ) -> Result<()> {
        let before = crate::model_package::snapshot(root, relative, kind)?;
        let key = format!("package/v1/{kind:?}/{relative}/{expected}");
        let mut pins = self.pins.lock().await;
        if pins.get(&key) == Some(&before.fingerprint) {
            return Ok(());
        }
        let (root, relative, expected) =
            (root.to_path_buf(), relative.to_owned(), expected.to_owned());
        let verified = tokio::task::spawn_blocking(move || {
            crate::model_package::verify(&root, &relative, kind, &expected)
        })
        .await??;
        ensure!(
            verified.fingerprint == before.fingerprint,
            "package changed during pin admission"
        );
        pins.insert(key, verified.fingerprint);
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        app: Shared,
        root: PathBuf,
        tenant: String,
        subject: String,
        request: TaskRequest,
        base: TaskResult,
        fence: Option<(Uuid, i64)>,
    ) -> Result<TaskResult> {
        let Some(settings) = app.config.task_answers.clone() else {
            return Ok(base);
        };
        settings.validate()?;
        self.verify_pin(&root, &settings, &tenant).await?;
        let runtime = gents_local_mcp::digest(&serde_json::to_vec(&app.config)?);
        // Hold only admission, never inference, under this lock. Status cannot
        // misidentify a just-claimed local job before its active handle exists.
        let mut active = self.active.lock().await;
        let allow_new = active.len() < settings.max_pending_jobs;
        let (root2, tenant2, subject2, settings2, runtime2) = (
            root.clone(),
            tenant.clone(),
            subject.clone(),
            settings.clone(),
            runtime.clone(),
        );
        let admitted=tokio::task::spawn_blocking(move ||->Result<_>{
            let mut db=company::connect(&root2,&tenant2,&subject2,false)?;
            let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if let Some((session,revision))=fence {
                let valid:i64=tx.query_row("SELECT COUNT(*) FROM ceo_planning WHERE id=?1 AND revision=?2 AND state='generating'",params![session.to_string(),revision],|row|row.get(0))?;
                ensure!(valid==1,"CEO checkpoint changed before grounded admission; no provider called");
            }
            let repo=Repository::new(&tx)?;
            let now=crate::now_ms();
            let mut policy=ExecutionPolicy::read_only(scoped(&repo,&tenant2,&subject2,&runtime2,&root2)?,now);
            policy.max_model_calls=1;policy.max_tokens=settings2.final_tokens+settings2.model.local_reasoning_tokens.unwrap_or(0);
            policy.deadline_ms=now.saturating_add(settings2.deadline_seconds*1000);policy.cache_ttl_ms=settings2.draft_ttl_seconds*1000;
            let admitted=grounded::admit(&repo,&request,&base,&policy,&settings2.identity()?,now,allow_new)?;
            if let Some(a)=&admitted {
                if a.launch {tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'grounded_call_reserved',?2,?3)",params![a.job_id.to_string(),json!({"task_id":request.task_id,"reserved_calls":1,"model_identity":a.job.model_identity,"worker_tools":[],"deadline_ms":a.job.deadline_ms}).to_string(),now as i64])?;}
            }
            tx.commit()?;Ok((admitted,base))
        }).await??;
        let Some(admitted) = admitted.0 else {
            return Ok(admitted.1);
        };
        let result = admitted.result.clone();
        if admitted.launch {
            let cancel = Arc::new(AtomicBool::new(false));
            active.insert(admitted.job_id, cancel.clone());
            tokio::spawn(run(
                app, root, tenant, subject, runtime, settings, admitted, cancel,
            ));
        }
        Ok(result)
    }
    pub async fn status(
        &self,
        root: &Path,
        tenant: &str,
        subject: &str,
        runtime: &str,
        task: Uuid,
    ) -> Result<Value> {
        let (mut result, id) = {
            let mut db = company::connect(root, tenant, subject, false)?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let repo = Repository::new(&tx)?;
            let scope = scoped(&repo, tenant, subject, runtime, root)?;
            let result = repo
                .status(&scope, task, crate::now_ms())?
                .context("task unavailable to this scope")?;
            let id = grounded::job_for_task(&repo, &scope, task)?;
            tx.commit()?;
            (result, id)
        };
        let live = if let Some(id) = id {
            self.active.lock().await.contains_key(&id)
        } else {
            false
        };
        if result.status == "running" && !live {
            result.status = "needs_reconciliation".into();
            result.blockers.push("Original generation admission retained; runtime liveness is unknown in this process. Inspect the original job, never replay it on restart.".into());
        }
        Ok(json!({"result":result,"grounded_job_id":id,"runtime_live_in_this_process":live}))
    }
    pub async fn cancel(
        &self,
        root: &Path,
        tenant: &str,
        subject: &str,
        runtime: &str,
        task: Uuid,
    ) -> Result<Value> {
        let (id, stop) = {
            let mut db = company::connect(root, tenant, subject, false)?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let repo = Repository::new(&tx)?;
            let scope = scoped(&repo, tenant, subject, runtime, root)?;
            let (id, stop) = grounded::cancel(&repo, &scope, task, crate::now_ms())?;
            tx.commit()?;
            (id, stop)
        };
        if stop {
            if let Some(cancel) = self.active.lock().await.get(&id) {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        self.status(root, tenant, subject, runtime, task).await
    }
    pub async fn wait(
        &self,
        root: &Path,
        tenant: &str,
        subject: &str,
        runtime: &str,
        task: Uuid,
    ) -> Result<TaskResult> {
        loop {
            let value = self.status(root, tenant, subject, runtime, task).await?;
            let result: TaskResult = serde_json::from_value(value["result"].clone())?;
            if result.status != "running" {
                return Ok(result);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run(
    app: Shared,
    root: PathBuf,
    tenant: String,
    subject: String,
    runtime: String,
    settings: Settings,
    admitted: grounded::Admission,
    cancel: Arc<AtomicBool>,
) {
    let began = Instant::now();
    let prompt = grounded::prompt(&admitted.job).to_string();
    let scope = crate::context::session_key(
        &tenant,
        &admitted.job.request.task_id.to_string(),
        &format!("grounded answer:{subject}"),
        0,
    );
    let call = async {
        app.task_answers
            .verify_pin(&root, &settings, &tenant)
            .await?;
        if settings.model.provider == "mock" {
            app.gateway
                .complete(
                    &settings.model,
                    grounded::INSTRUCTION,
                    &prompt,
                    settings.final_tokens,
                    &cancel,
                )
                .await
        } else {
            app.gateway
                .local
                .complete_scoped(
                    &settings.model,
                    grounded::INSTRUCTION,
                    &prompt,
                    settings.final_tokens,
                    &cancel,
                    &scope,
                )
                .await
        }
    };
    tokio::pin!(call);
    let remaining = Duration::from_millis(admitted.job.deadline_ms.saturating_sub(crate::now_ms()));
    let reply: Result<Reply> = tokio::select! {
        reply=&mut call=>reply,
        _=tokio::time::sleep(remaining)=>{cancel.store(true,Ordering::Relaxed);call.await},
    };
    let measurements = Measurements {
        generative_model_calls: if settings.model.provider == "mock" {
            Some(0)
        } else {
            reply
                .as_ref()
                .ok()
                .and_then(|reply| reply.local_execution.as_ref())
                .map(|_| 1)
        },
        embedding_calls: 0,
        tool_calls: 0,
        input_tokens: reply.as_ref().ok().and_then(|r| r.usage.input),
        output_tokens: reply.as_ref().ok().and_then(|r| r.usage.output),
        elapsed_micros: Some(began.elapsed().as_micros().try_into().unwrap_or(u64::MAX)),
        provider_cost_usd: Some(0),
        memory_bytes: reply
            .as_ref()
            .ok()
            .and_then(|r| r.local_execution.as_ref())
            .and_then(|e| e.resident_kib)
            .and_then(|n| n.checked_mul(1024)),
    };
    let call_evidence = match &reply {
        Ok(r) => {
            json!({"usage":r.usage,"local_execution":r.local_execution,"latency_ms":r.latency_ms})
        }
        Err(error) => json!({"inference_failure":crate::local_generation::planning_failure(error)}),
    };
    let pin_current = app
        .task_answers
        .verify_pin(&root, &settings, &tenant)
        .await
        .is_ok();
    let (job_id, root2, tenant2, subject2) = (
        admitted.job_id,
        root.clone(),
        tenant.clone(),
        subject.clone(),
    );
    let finished=tokio::task::spawn_blocking(move ||->Result<()> {
        let mut db=company::connect(&root2,&tenant2,&subject2,false)?;
        let tx=db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let repo=Repository::new(&tx)?;let scope=scoped(&repo,&tenant2,&subject2,&runtime,&root2)?;
        let wire=if pin_current {reply.as_ref().map(|reply|reply.text.as_str()).map_err(|_|anyhow::anyhow!("provider invocation failed; inspect original call evidence"))} else {Err(anyhow::anyhow!("model pin changed"))};
        let result=grounded::finish(&repo,&scope,job_id,wire,measurements,crate::now_ms())?;
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'grounded_call_finished',?2,?3)",params![job_id.to_string(),json!({"task_id":result.task_id,"status":result.status,"verification_state":result.verification_state,"call_evidence":call_evidence,"model_pin_current":pin_current,"provider_invocations":1,"provider_fees_usd":0,"scope":"Generated draft, no worker execution, no verified answer-cache promotion"}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;Ok(())
    }).await;
    if !matches!(finished, Ok(Ok(()))) {
        eprintln!(
            "Grounded result checkpoint unavailable; retain original admission for reconciliation"
        );
    }
    app.task_answers
        .active
        .lock()
        .await
        .remove(&admitted.job_id);
}

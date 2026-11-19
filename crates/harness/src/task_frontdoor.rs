//! Authenticated owner adapter to shared task services. No worker grants change.
use crate::{company, now_ms};
use anyhow::{Context, Result};
use gents_local_mcp::task_engine::{contracts::*, KnowledgeSource, Repository};
use rusqlite::TransactionBehavior;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) fn owner_scope(
    repo: &Repository<'_>,
    tenant: &str,
    subject: &str,
    runtime_revision: &str,
) -> Result<AccessScope> {
    Ok(AccessScope {
        tenant: tenant.into(),
        subject: subject.into(),
        effective_access_sha256: gents_local_mcp::digest(
            format!("owner-private-knowledge/v1:{tenant}:{subject}").as_bytes(),
        ),
        policy_revision: "owner-private-knowledge/v1".into(),
        profile_revision: "source-extraction/v1".into(),
        runtime_revision: runtime_revision.into(),
        conversation_sha256: gents_local_mcp::digest(
            b"no implicit conversation; explicit supplied context is preserved",
        ),
        allowed_sources: repo.source_ids(tenant, subject)?,
    })
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Execute {
        input: TaskInput,
    },
    Ingest {
        source: KnowledgeSource,
    },
    DeleteSource {
        source_id: String,
    },
    Search {
        query: String,
        domain: Option<String>,
    },
    Status {
        task_id: Uuid,
    },
    CancelTask {
        task_id: Uuid,
    },
    InspectCache,
    InvalidateCache,
    Capabilities,
}

/// Shared async owner entry. Cache/extraction complete before any generator is
/// admitted. Long generation returns a task ID and uses status/cancel APIs.
pub async fn dispatch(
    app: crate::Shared,
    root: PathBuf,
    tenant: String,
    subject: String,
    input: Operation,
) -> Result<Value> {
    let runtime = gents_local_mcp::digest(&serde_json::to_vec(&app.config)?);
    match input {
        Operation::Status { task_id } => Ok(app
            .task_answers
            .status(&root, &tenant, &subject, &runtime, task_id)
            .await?["result"]
            .clone()),
        Operation::CancelTask { task_id } => {
            app.task_answers
                .cancel(&root, &tenant, &subject, &runtime, task_id)
                .await
        }
        input => {
            let execute = matches!(&input, Operation::Execute { .. });
            let capabilities = matches!(&input, Operation::Capabilities);
            let (r, t, s, rt) = (
                root.clone(),
                tenant.clone(),
                subject.clone(),
                runtime.clone(),
            );
            let mut value =
                tokio::task::spawn_blocking(move || operation(&r, &t, &s, &rt, input)).await??;
            if execute
                && app.config.task_answers.is_some()
                && value["result"]["status"] == "requires_planning"
            {
                let request = serde_json::from_value(value["request"].clone())?;
                let base = serde_json::from_value(value["result"].clone())?;
                value["result"] = serde_json::to_value(
                    app.task_answers
                        .start(app.clone(), root, tenant, subject, request, base, None)
                        .await?,
                )?;
            }
            if capabilities {
                value["grounded_generation"] = json!({"installed":true,"enabled":app.config.task_answers.is_some(),"template":"grounded_answer/v1","one_provider_invocation_per_job":true,"answer_verification":"citations_checked_answer_unverified","verified_answer_cache_promotion":false,"async_status_and_cancel":true});
            }
            Ok(value)
        }
    }
}

pub fn operation(
    root: &Path,
    tenant: &str,
    subject: &str,
    runtime_revision: &str,
    operation: Operation,
) -> Result<Value> {
    let mut db = company::connect(root, tenant, subject, false)?;
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let repo = Repository::new(&tx)?;
    let scope = owner_scope(&repo, tenant, subject, runtime_revision)?;
    let now = now_ms();
    let value = match operation {
        Operation::Execute { input } => {
            let request = TaskRequest::from_input(input)?;
            let result = repo.execute(&request, &ExecutionPolicy::read_only(scope, now), now)?;
            json!({"request":request,"result":result})
        }
        Operation::Ingest { source } => {
            json!({"schema_version":VERSION,"source_id":source.source_id,"source_snapshot_sha256":repo.ingest(&scope,&source,now)?})
        }
        Operation::DeleteSource { source_id } => {
            json!({"schema_version":VERSION,"deleted":repo.delete_source(&scope,&source_id)?})
        }
        Operation::Search { query, domain } => {
            json!({"schema_version":VERSION,"retrieval":"lexical_exact_coordinates/v1","hits":repo.search(&scope,&query,domain.as_deref(),8192,now)?,"generative_model_calls":0})
        }
        Operation::Status { task_id } => serde_json::to_value(
            repo.status(&scope, task_id, now)?
                .context("task unavailable to this scope")?,
        )?,
        Operation::CancelTask { .. } => {
            anyhow::bail!("cancellation requires the async task service")
        }
        Operation::InspectCache => repo.inspect_cache(&scope)?,
        Operation::InvalidateCache => {
            json!({"schema_version":VERSION,"deleted":repo.invalidate_cache(&scope)?})
        }
        Operation::Capabilities => {
            json!({"schema_version":VERSION,"entry":"authenticated owner task frontdoor", "enabled_executors":["deterministic_lookup/v1","source_extraction/v1","exact_answer_cache"],
            "requires_planning":"Unknown asks return a durable requires_planning result; this first stage never fabricates generation or automatically approves work.",
            "semantic_cache":"unavailable_pending_evaluation", "grounded_generation":"not_integrated_yet", "worker_side_effects":false})
        }
    };
    tx.commit()?;
    Ok(value)
}

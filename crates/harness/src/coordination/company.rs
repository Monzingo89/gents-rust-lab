//! Company runtime adapter. Coordination submits candidates; a separate frozen
//! owner-approved local reviewer and transactional lifecycle gate may accept one.
use super::*;
use crate::{provider::Reply, App};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};
use uuid::Uuid;

pub(super) struct CompanyAccess {
    root: PathBuf,
    company: String,
    owner: String,
    run: Uuid,
}
impl CompanyAccess {
    pub(super) fn new(root: PathBuf, company: String, owner: String, run: Uuid) -> Self {
        Self {
            root,
            company,
            owner,
            run,
        }
    }
}
impl AccessRepository for CompanyAccess {
    fn policy(&self) -> Result<crate::governance::Policy> {
        crate::company::governance(&self.root, &self.company, &self.owner, self.run)?
            .context("coordination policy missing")
    }
    fn subscribe(&self) -> Result<watch::Receiver<u64>> {
        crate::governance::subscribe(&self.company, self.run, self.policy()?.revision)
    }
}
pub(super) async fn inference_event(
    app: &App,
    ids: (Uuid, Uuid),
    actor: (&str, &str),
    kind: &str,
    message: &str,
    model: Option<String>,
    reply: Option<Reply>,
) -> Result<()> {
    crate::record(
        app,
        ids.0,
        crate::Event {
            company_id: None,
            run_id: Some(ids.0),
            seq: 0,
            timestamp_ms: 0,
            call_id: Some(ids.1),
            agent: actor.0.into(),
            role: actor.1.into(),
            room: "planning".into(),
            kind: kind.into(),
            message: message.into(),
            model,
            reply,
        },
    )
    .await
}
struct ObservedWorkers<'a> {
    native: native::NativeWorkers<'a>,
    app: &'a App,
    run: Uuid,
    active: Mutex<Option<(Uuid, String)>>,
}
impl ObservedWorkers<'_> {
    async fn event(
        &self,
        id: Uuid,
        turn: &Turn,
        kind: &str,
        message: &str,
        reply: Option<Reply>,
    ) -> Result<()> {
        let model = self
            .app
            .config
            .agents
            .iter()
            .find(|a| a.id == turn.actor)
            .map(|a| format!("local / {}", a.worker.model));
        inference_event(
            self.app,
            (self.run, id),
            (&turn.actor, "coordination"),
            kind,
            message,
            model,
            reply,
        )
        .await
    }
}
#[async_trait]
impl WorkerPort for ObservedWorkers<'_> {
    async fn complete(
        &self,
        turn: &Turn,
        packet: &Value,
        revision: u64,
        cancel: &AtomicBool,
    ) -> Result<Reply> {
        let id = Uuid::new_v4();
        *self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("coordination telemetry lock failed"))? =
            Some((id, turn.actor.clone()));
        self.event(id,turn,"call_started",&json!({"turn":turn,"packet_sha256":crate::company_factory::digest(packet)?,"policy_revision":revision,"task_accepted":false}).to_string(),None).await?;
        let result = self.native.complete(turn, packet, revision, cancel).await;
        *self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("coordination telemetry lock failed"))? = None;
        match result {
            Ok(reply) => {
                if crate::security::require_persistable(&reply.text).is_err() {
                    let safe = Reply {
                        text: "[Coordination reply rejected by data policy]".into(),
                        ..reply
                    };
                    self.event(
                        id,
                        turn,
                        "call_failed",
                        "Reply content blocked before persistence; pending turn retained",
                        Some(safe),
                    )
                    .await?;
                    anyhow::bail!("coordination reply failed data policy");
                }
                self.event(
                    id,
                    turn,
                    "call_completed",
                    &json!({"ordinal":turn.ordinal,"purpose":turn.purpose,"task_accepted":false})
                        .to_string(),
                    Some(reply.clone()),
                )
                .await?;
                Ok(reply)
            }
            Err(error) => {
                self.event(id,turn,"call_failed","Native inference failed; unobserved usage remains unknown. Pending turn cannot automatically replay.",None).await?;
                Err(error)
            }
        }
    }
    async fn cancel(&self, turn: &Turn, revision: u64) {
        self.native.cancel(turn, revision).await;
        let active = self.active.lock().ok().and_then(|mut v| v.take());
        if let Some((id, actor)) = active {
            if actor == turn.actor {
                let _ = self
                    .event(
                        id,
                        turn,
                        "policy_epoch_cancelled",
                        "Owner policy changed; native turn stopped and pending receipt retained",
                        None,
                    )
                    .await;
            }
        }
    }
}

pub async fn maybe_execute(
    app: &App,
    id: Uuid,
    task: &str,
    criteria: &[String],
    cancel: &AtomicBool,
) -> Result<Option<crate::judges::Outcome>> {
    let (company, owner) = {
        let runs = app.runs.lock().await;
        let run = runs.get(&id).context("run missing")?;
        (
            run.company_id.clone().context("company missing")?,
            run.owner.clone(),
        )
    };
    let root = std::env::current_dir()?;
    let db = crate::company::connect(&root, &company, &owner, false)?;
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [id.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    drop(db);
    let Some(contract) = admission::from_intake(&intake)? else {
        return Ok(None);
    };
    crate::architecture_review::validate_intake(&root, &company, &owner, &intake)?;
    crate::company_factory::preparation::validate_execution(&app.config, &intake)?;
    let review_contract = review::contract::from_intake(&intake)?;
    if let Some(contract) = &review_contract {
        review::contract::validate_config(&app.config, contract)?;
    }
    ensure!(
        task == contract.objective && criteria == contract.definition_of_done,
        "coordination start differs from owner contract"
    );
    let mut models = BTreeMap::new();
    for role in &contract.roles {
        let agent = app
            .config
            .agents
            .iter()
            .find(|a| a.id == role.actor)
            .context("coordination actor not configured")?;
        models.insert(role.actor.clone(), agent.worker.clone());
    }
    let workers = ObservedWorkers {
        native: native::NativeWorkers::new(
            &root,
            &app.gateway.local,
            &contract,
            models,
            &format!("{company}:{id}"),
        )?,
        app,
        run: id,
        active: Mutex::new(None),
    };
    let access = CompanyAccess::new(root.clone(), company.clone(), owner.clone(), id);
    let mut journal = journal::SqliteJournal::open(&root, &company, &owner, id)?;
    let mut engine = Engine {
        workers: &workers,
        access: &access,
        journal: &mut journal,
    };
    let candidate = engine.run(&contract, cancel).await?;
    crate::emit(
        app,
        id,
        ("Company Factory", "coordination"),
        ("planning", "coordination_candidate_submitted"),
        &candidate.to_string(),
        None,
        None,
    )
    .await?;
    if let Some(review_contract) = review_contract {
        let scope = format!("{company}:{id}");
        let reviewer = review::observed::ObservedReviewer {
            native: review::native::NativeReviewer {
                root: &root,
                runtime: &app.gateway.local,
                contract: &review_contract,
                scope: &scope,
            },
            app,
            run: id,
            active: std::sync::Mutex::new(None),
        };
        let result = review::ReviewEngine {
            reviewer: &reviewer,
            access: &access,
            journal: &mut journal,
        }
        .run(
            &review_contract,
            candidate["policy_revision"]
                .as_u64()
                .context("candidate policy missing")?,
            cancel,
        )
        .await?;
        review::contract::validate_config(&app.config, &review_contract)?;
        crate::company_factory::preparation::validate_execution(&app.config, &intake)?;
        crate::emit(
            app,
            id,
            ("Company Factory", "reasoning_acceptance"),
            ("planning", "coordination_independent_review"),
            &result.to_string(),
            None,
            None,
        )
        .await?;
        return Ok(Some(crate::judges::Outcome {
            status:if result["accepted"]==true {"completed"}else{"incomplete"},
            output:json!({"deliverable":candidate["candidate"]["deliverable"],"candidate":candidate,"independent_review":result}).to_string(),
        }));
    }
    Ok(Some(crate::judges::Outcome {
        status: "candidate_submitted",
        output: candidate.to_string(),
    }))
}

//! Narrow owner-approved source-proposal workflow. Models author text, while
//! deterministic stages own writes, exact readback and quarantined submission.
use crate::{company, judges, App};
use anyhow::{bail, ensure, Context, Result};
use gents_local_mcp::{
    proposals::{self, Repository, Stage},
    Phase, Scope,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;
pub fn checkpoint_exists(root: &Path, company: &str, owner: &str, id: Uuid) -> Result<bool> {
    let db = company::connect(root, company, owner, false)?;
    Repository::exists(&db, &id.to_string())
}
async fn event(app: &App, id: Uuid, actor: &str, kind: &str, value: Value) -> Result<()> {
    crate::emit(
        app,
        id,
        (actor, "worker"),
        ("engineering", kind),
        &value.to_string(),
        None,
        None,
    )
    .await
}
async fn invoke(
    app: &App,
    id: Uuid,
    root: &Path,
    scope: &Scope,
    args: &Value,
    cancel: &AtomicBool,
) -> Result<Value> {
    event(app,id,&scope.actor,"tool_started",json!({"action":"files","op":args["op"],"path":args["path"],"transport":gents_local_mcp::client::transport_label(),"candidate_only":true})).await?;
    let result = gents_local_mcp::client::invoke(root, scope, "files", args, cancel).await;
    event(app,id,&scope.actor,"tool_result",match &result{Ok(value)=>json!({"ok":value["ok"],"mcp_receipt":value["mcp_receipt"],"source_sha256":value["result"]["sha256"],"payload_logged":false}),Err(_)=>json!({"ok":false,"effect":"inspect_checkpoint_before_retry","payload_logged":false})}).await?;
    result
}
pub async fn maybe_execute(
    app: &App,
    id: Uuid,
    task: &str,
    criteria: &[String],
    cancel: &AtomicBool,
) -> Result<Option<judges::Outcome>> {
    let (company, owner) = {
        let runs = app.runs.lock().await;
        let run = runs.get(&id).context("candidate run missing")?;
        (
            run.company_id
                .clone()
                .context("candidate company missing")?,
            run.owner.clone(),
        )
    };
    let root = std::env::current_dir()?;
    let db = company::connect(&root, &company, &owner, false)?;
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [id.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let Some(spec) = proposals::from_intake(&intake)? else {
        return Ok(None);
    };
    ensure!(
        intake["idea"] == task && criteria == [proposals::ACCEPTANCE],
        "candidate task or acceptance differs from frozen owner contract"
    );
    let actor = app
        .config
        .agents
        .iter()
        .find(|a| a.id == spec.actor)
        .context("candidate actor is not configured")?;
    ensure!(
        matches!(actor.worker.provider.as_str(), "local" | "mlx" | "mock"),
        "candidate workflow cannot use paid/remote inference"
    );
    let _controller = crate::workflow::ControllerLease::acquire(&root, &company, &owner, id)?;
    let scope = Scope {
        company,
        owner,
        run: id,
        actor: spec.actor.clone(),
        phase: Phase::Worker,
        expected_revision: None,
    };
    let authority = gents_local_mcp::default_authority(root.clone())?;
    let mut repository = Repository::open(&authority, scope.clone())?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("candidate workflow cancelled with checkpoint retained");
        }
        let state = repository.state()?;
        event(app,id,&scope.actor,"candidate_stage",json!({"stage":state.stage,"attempt":state.attempt,"offset":state.offset,"source_sha256":state.source_sha256,"qualified":false})).await?;
        match state.stage {
            Stage::Author | Stage::Generating => {
                let attempt = repository.begin_author()?;
                let prompt = json!({"task":task,"path":repository.spec().path,"max_utf8_bytes":repository.spec().max_bytes,"attempt":attempt,"host_feedback":state.feedback,"stage":"author_quarantined_text","completion_contract":"Only source generation is requested. The controller will perform the governed write and readback. Independent host review, compilation and promotion happen later. No tool calls or host-test claims belong in the response."});
                let result=crate::model_call(app,id,(&scope.actor,"worker"),&actor.worker,("LOCAL_CANDIDATE_AUTHOR: Return bare JSON {source:string,summary:string}. Author the candidate requested by the frozen task yourself. Source is untrusted text and will not be executed or installed by this workflow. Do not invent tests, tool receipts, successful execution or promotion. Do not ask for shell or credentials. Do not choose paths, actors, permissions, stage order or tools; those belong to the host.",&prompt.to_string()),cancel).await;
                match result {
                    Ok(reply) => {
                        let accepted = serde_json::from_str::<Value>(&reply.text)
                            .map_err(anyhow::Error::from)
                            .and_then(|v| repository.accept_source(v));
                        if accepted.is_err() {
                            repository.author_failed("Candidate response failed strict JSON shape, size, frame or privacy checks. Return only the bounded source and summary fields; do not add host-control fields.")?;
                        }
                    }
                    Err(error) => {
                        if cancel.load(Ordering::Relaxed) {
                            return Err(error);
                        }
                        repository.author_failed("Local author call failed; retry the same frozen source contract without expanding permissions")?;
                    }
                }
            }
            Stage::WriteReady => {
                let args = repository.prepare_write()?;
                let result = invoke(app, id, &root, &scope, &args, cancel).await?;
                repository.wrote(&args, &result)?;
            }
            Stage::WritePending | Stage::Readback => {
                let args = repository.read_arguments()?;
                let result = invoke(app, id, &root, &scope, &args, cancel).await?;
                repository.read(&args,&result).context("Inspect retained candidate write/source hashes; no unproven write was replayed")?;
            }
            Stage::Ready | Stage::Submitted => {
                let result = repository.submit(&authority)?;
                event(app, id, &scope.actor, "candidate_submitted", result.clone()).await?;
                return Ok(Some(judges::Outcome {
                    status: "candidate_submitted",
                    output: result.to_string(),
                }));
            }
        }
    }
}

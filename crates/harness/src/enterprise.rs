//! Local synthetic-banking controller. Models propose; the domain contract,
//! governance service and durable DAG own acceptance, access and recovery.
use crate::{
    banking::{self, Task, ORDER},
    company,
    workflow::{Claim, ControllerLease, Graph, Node, Output, SqliteWorkflows, WorkflowRepository},
    App, Config,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub profile: String,
    #[serde(default)]
    pub compiled_release_policy: bool,
    #[serde(default)]
    pub governed_triage_memory: bool,
    #[serde(default)]
    pub triage_memory_protocol: u32,
}
impl Settings {
    fn validate_memory_protocol(&self) -> Result<()> {
        anyhow::ensure!(
            !self.governed_triage_memory || self.triage_memory_protocol == banking::TRIAGE_MEMORY_PROTOCOL,
            "triage memory protocol is missing or stale; do not reuse an older task's input contract"
        );
        Ok(())
    }
}
pub fn validate_config(config: &Config) -> Result<()> {
    let settings = config
        .enterprise
        .as_ref()
        .context("enterprise settings missing")?;
    anyhow::ensure!(
        settings.profile == banking::PROFILE
            && config.company_mode
            && !config.company_tools
            && config.campaign.is_none()
            && config.poc.is_none()
            && config.judges.is_none(),
        "enterprise profile requires isolated local company execution"
    );
    settings.validate_memory_protocol()?;
    let roster: Value =
        serde_json::from_str(include_str!("../../../config/enterprise-model-roster.json"))?;
    anyhow::ensure!(
        config.agents.len() == ORDER.len(),
        "enterprise profile requires ten configured roles"
    );
    for id in ORDER {
        let row = roster["models"]
            .as_array()
            .context("roster missing")?
            .iter()
            .find(|m| m["id"] == id)
            .context("roster role missing")?;
        let agent = config
            .agents
            .iter()
            .find(|a| a.id == id)
            .context("configured role missing")?;
        for model in [&agent.planner, &agent.worker] {
            anyhow::ensure!(
                model.provider == "local"
                    && model.path.as_deref() == row["path"].as_str()
                    && model.model == row["publisher"]
                    && model.context_tokens == Some(8192)
                    && model.adapter.is_none()
                    && model.key_env.is_none()
                    && model.url.is_none(),
                "enterprise role model differs from the pinned local roster"
            );
        }
    }
    Ok(())
}
fn sha(value: &Value) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
async fn attest(app: &App, id: Uuid, root: &Path) -> Result<Value> {
    let roster: Value =
        serde_json::from_str(include_str!("../../../config/enterprise-model-roster.json"))?;
    let readiness: Value = serde_json::from_slice(&std::fs::read(
        root.join("data/model-registry/readiness-index.json"),
    )?)?;
    let resolver = crate::local_runtime::LocalRuntime::new(root.to_path_buf())?;
    let mut publishers = BTreeSet::new();
    let mut weights = BTreeSet::new();
    for row in roster["models"].as_array().context("roster missing")? {
        let actor = row["id"].as_str().context("role identity missing")?;
        anyhow::ensure!(
            publishers.insert(row["publisher"].as_str().context("publisher missing")?)
                && weights.insert(row["sha256"].as_str().context("weight hash missing")?),
            "aliases cannot add enterprise model identities"
        );
        let proof: Value = serde_json::from_slice(&std::fs::read(
            root.join("data/model-registry/provenance")
                .join(actor)
                .join("result.json"),
        )?)?;
        anyhow::ensure!(
            proof["status"] == "PASS"
                && proof["sha256"] == row["sha256"]
                && proof["publisher"] == row["publisher"]
                && proof["source_revision"] == row["source_revision"],
            "model artifact provenance is missing or stale"
        );
        let ready = readiness["models"]
            .as_array()
            .context("readiness index missing")?
            .iter()
            .find(|r| r["id"] == actor)
            .context("role readiness missing")?;
        anyhow::ensure!(
            ready["ready"] == true
                && ready["weight_sha256"] == row["sha256"]
                && ready["publisher"] == row["publisher"],
            "model readiness does not match artifact"
        );
        let receipt = std::path::PathBuf::from(
            ready["evidence"]
                .as_str()
                .context("readiness receipt missing")?,
        );
        let safe_root = root.join("data").canonicalize()?;
        anyhow::ensure!(
            !receipt.is_symlink() && receipt.canonicalize()?.starts_with(safe_root),
            "readiness receipt escaped the private registry"
        );
        anyhow::ensure!(
            company::file_hash(&receipt)?
                == ready["evidence_sha256"]
                    .as_str()
                    .context("readiness receipt hash missing")?,
            "readiness receipt integrity failed"
        );
        crate::role_probe::validate_readiness(
            actor,
            row,
            &serde_json::from_slice(&std::fs::read(&receipt)?)?,
        )?;
        let path = resolver.resolve_model(
            row["path"].as_str().context("weight path missing")?,
            "local",
        )?;
        let actual = tokio::task::spawn_blocking(move || company::file_hash(&path)).await??;
        anyhow::ensure!(
            actual == row["sha256"],
            "model weight changed after registry review"
        );
        crate::emit(app,id,(actor,"worker"),("testing","model_attested"),&json!({"weight_sha256":actual,"readiness_only":true,"production_admitted":false,"payload_logged":false}).to_string(),None,None).await?;
    }
    anyhow::ensure!(
        publishers.len() == 10,
        "ten distinct publisher model identities required"
    );
    Ok(roster)
}
fn security_controls(root: &Path, tenant: &str, owner: &str, id: Uuid) -> Result<Value> {
    let sentinel = "private-company-isolation-sentinel";
    company::operation(
        root,
        tenant,
        owner,
        &json!({"op":"ingest","source":"security-negative-fixture","text":sentinel}),
    )?;
    let visible = company::retrieve(root, tenant, owner, id, "classification", sentinel)?;
    anyhow::ensure!(
        !visible.to_string().contains(sentinel),
        "ungranted company memory became visible"
    );
    anyhow::ensure!(
        company::retrieve(root, tenant, "wrong-owner", id, "classification", sentinel).is_err(),
        "owner boundary failed"
    );
    let other = format!("bank-control-{}", id.simple());
    company::operation(
        root,
        &other,
        "other-owner",
        &json!({"op":"register","name":"Private cross-tenant control fixture"}),
    )?;
    anyhow::ensure!(
        company::retrieve(root, &other, owner, id, "classification", sentinel).is_err(),
        "cross-tenant owner boundary failed"
    );
    let policy = company::governance(root, tenant, owner, id)?.context("governance missing")?;
    anyhow::ensure!(
        policy
            .authorize("classification", "files", &json!({"op":"write"}))
            .is_err(),
        "ungranted write permitted"
    );
    let grant = json!({"tools":["email_send"],"skills":[],"memory_from":[]});
    anyhow::ensure!(
        company::delegate_access(
            root,
            tenant,
            owner,
            id,
            "orchestrator",
            &json!({"recipient":"classification","expected_revision":policy.revision,"grant":grant})
        )
        .is_err(),
        "owner ceiling expansion succeeded"
    );
    anyhow::ensure!(
        company::governance(root, tenant, owner, id)?
            .unwrap()
            .revision
            == policy.revision,
        "denied delegation changed policy"
    );
    Ok(
        json!({"private_memory_hidden":true,"cross_owner_read_denied":true,"cross_tenant_read_denied":true,"ungranted_tool_denied":true,"ceiling_expansion_denied":true}),
    )
}
fn apply_delegation(
    root: &Path,
    tenant: &str,
    owner: &str,
    id: Uuid,
    proposal: &Value,
) -> Result<()> {
    anyhow::ensure!(
        proposal["grant"] == json!(["integer_arithmetic", "skill:finance-reconciliation"]),
        "master grant proposal not verified"
    );
    let policy = company::governance(root, tenant, owner, id)?.context("governance missing")?;
    let mut grant = policy
        .grants
        .get("reconciliation")
        .context("reconciliation identity missing")?
        .clone();
    if grant.tools.contains("integer_arithmetic") && grant.skills.contains("finance-reconciliation")
    {
        return Ok(());
    }
    grant.tools.insert("integer_arithmetic".into());
    grant.skills.insert("finance-reconciliation".into());
    company::delegate_access(
        root,
        tenant,
        owner,
        id,
        "orchestrator",
        &json!({"recipient":"reconciliation","expected_revision":policy.revision,"grant":grant}),
    )?;
    Ok(())
}
async fn execute_arithmetic(
    root: &Path,
    tenant: &str,
    owner: &str,
    id: Uuid,
    task: &Task,
    cancel: &AtomicBool,
) -> Result<Value> {
    let args = json!({"calculations":task.cases.iter().map(|c|json!({"id":c.case_id,"values":c.payments.iter().map(|p|p.cents).collect::<Vec<_>>(),"subtract":c.expected_cents})).collect::<Vec<_>>()});
    company::governance(root, tenant, owner, id)?
        .context("governance missing")?
        .authorize("reconciliation", "integer_arithmetic", &args)?;
    use crate::contracts::ToolContracts;
    crate::contracts::EmbeddedContracts.validate("integer_arithmetic", &args)?;
    let receipt = crate::autonomy::broker(tenant, id, "integer_arithmetic", &args, cancel).await?;
    anyhow::ensure!(receipt["ok"] == true, "integer capability failed");
    Ok(receipt["result"].clone())
}
fn triage_memory_records(
    root: &Path,
    company: &str,
    owner: &str,
    run: Uuid,
    task: &Task,
    state: &Value,
) -> Result<Vec<crate::memory_handoff::Record>> {
    use crate::memory_handoff::{record_hash, HandoffRepository, Query, SqliteHandoffs};
    company::acknowledge(root, company, owner, run, "triage")?;
    let mut db = company::connect(root, company, owner, false)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let repo = SqliteHandoffs {
        db: &tx,
        run,
        recipient: "triage",
    };
    let mut records = Vec::new();
    for case in &task.cases {
        let accepted = state["contributions"]["reconciliation"]
            .as_array()
            .context("accepted predecessor records missing")?
            .iter()
            .find(|v| v["case_id"] == case.case_id)
            .context("accepted predecessor case missing")?;
        let hash = record_hash(accepted)?;
        records.push(repo.find(&Query {
            sender: "reconciliation",
            correlation_key: "case_id",
            correlation_value: &case.case_id,
            accepted_record_sha256: &hash,
        })?);
    }
    tx.commit()?;
    Ok(records)
}

fn revalidate_triage_memory(
    root: &Path,
    company: &str,
    owner: &str,
    run: Uuid,
    task: &Task,
    state: &Value,
) -> Result<()> {
    use crate::memory_handoff::{record_hash, Binding, HandoffRepository, SqliteHandoffs};
    anyhow::ensure!(
        state["memory_handoff_protocol"] == banking::TRIAGE_MEMORY_PROTOCOL,
        "cached triage memory protocol changed"
    );
    let bindings: Vec<Binding> =
        serde_json::from_value(state["memory_handoffs"]["triage"].clone())?;
    anyhow::ensure!(
        bindings.len() == task.cases.len(),
        "triage memory case coverage changed"
    );
    let mut seen = BTreeSet::new();
    let mut db = company::connect(root, company, owner, false)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let repo = SqliteHandoffs {
        db: &tx,
        run,
        recipient: "triage",
    };
    for binding in bindings {
        anyhow::ensure!(
            binding.sender == "reconciliation"
                && binding.correlation_key == "case_id"
                && task
                    .cases
                    .iter()
                    .any(|c| c.case_id == binding.correlation_value)
                && seen.insert(binding.correlation_value.clone()),
            "triage memory source binding changed"
        );
        let accepted = state["contributions"]["reconciliation"]
            .as_array()
            .context("accepted predecessor missing")?
            .iter()
            .find(|v| v["case_id"] == binding.correlation_value)
            .context("accepted predecessor case missing")?;
        anyhow::ensure!(
            record_hash(accepted)? == binding.accepted_record_sha256,
            "triage memory differs from accepted predecessor"
        );
        repo.revalidate(&binding)?;
    }
    tx.commit()?;
    Ok(())
}

async fn perform(
    app: &App,
    id: Uuid,
    actor: &str,
    task: &Task,
    state: &mut Value,
    cancel: &AtomicBool,
    retry: bool,
) -> Result<Vec<crate::provider::Reply>> {
    let role = crate::role_probe::roles()?
        .into_iter()
        .find(|r| r.id == actor)
        .context("role protocol missing")?;
    let agent = app
        .config
        .agents
        .iter()
        .find(|a| a.id == actor)
        .context("runtime actor missing")?;
    let planner = matches!(actor, "orchestrator" | "policy");
    let model = if planner {
        &agent.planner
    } else {
        &agent.worker
    };
    let governed_triage = actor == "triage"
        && app
            .config
            .enterprise
            .as_ref()
            .is_some_and(|s| s.governed_triage_memory);
    let memory_scope = if governed_triage {
        let runs = app.runs.lock().await;
        let run = runs.get(&id).context("triage run missing")?;
        Some((
            std::env::current_dir()?,
            run.company_id.clone().context("triage company missing")?,
            run.owner.clone(),
        ))
    } else {
        None
    };
    let memory_records = if let Some((root, company, owner)) = &memory_scope {
        triage_memory_records(root, company, owner, id, task, state)?
    } else {
        Vec::new()
    };
    let system=match actor {
        "policy"=>"ENTERPRISE_ROLE/policy: Implement the supplied original banking contract as exactly five ordered {when,status} rules. Choose predicates and outcomes from the supplied catalogs; first match wins. You must derive the order and mapping from the contract. Return case_id and rules only. No executable source code or claim of execution.".to_string(),
        "orchestrator"=>"ENTERPRISE_ROLE/orchestrator: Propose the intersection of requested capabilities and the owner ceiling as grant, and the remaining requests as denied. Sort both arrays. skill: entries assign procedures; other entries are tools. Never expand the owner ceiling or execute an external action. Return case_id, grant, denied.".to_string(),
        "triage" if governed_triage=>banking::TRIAGE_MEMORY_SYSTEM.to_string(),
        _=>crate::role_probe::system(&role),
    };
    let mut replies = Vec::new();
    let mut rows = Vec::new();
    let mut decision_inputs = Vec::new();
    if actor == "risk"
        && app
            .config
            .enterprise
            .as_ref()
            .is_some_and(|s| s.compiled_release_policy)
    {
        let request = crate::release_policy::request(false);
        let reply = crate::model_call(
            app,
            id,
            (actor, "scoped_worker"),
            model,
            (crate::release_policy::SYSTEM, &request.to_string()),
            cancel,
        )
        .await?;
        let program: crate::release_policy::Program = serde_json::from_str(&reply.text)?;
        let checked = crate::release_policy::validate(&program, false)?;
        let input = banking::jobs(actor, task, state)?.remove(0);
        let result = crate::release_policy::execute(&program, &input)?;
        banking::validate(actor, &input, &result, task)?;
        state["contributions"][actor] = json!([result]);
        state["release_policy"] = json!({"version":1,"method":"model_authored_bounded_rules","program":program,"validation_cases":checked});
        return Ok(vec![reply]);
    }
    for mut input in banking::jobs(actor, task, state)? {
        if governed_triage {
            let source = memory_records
                .iter()
                .find(|r| r.binding.correlation_value == input["case_id"])
                .context("triage original source missing")?;
            input = banking::triage_memory_packet(&input, &source.packet())?;
            crate::emit(app,id,(actor,"scoped_worker"),("testing","memory_decision_source"),&json!({"case_id":input["case_id"],"sender":source.binding.sender,"source_sha256":source.binding.source_sha256,"record_sha256":source.binding.accepted_record_sha256,"policy_revision":source.binding.policy_revision,"payload_logged":false,"expected_queue_supplied":false}).to_string(),None,None).await?;
        }
        let packet = crate::role_probe::packet(actor, &input)?;
        // The portable envelope nests data under task_input. Preserve protocol
        // field names at that level so small models need no JSON-path guessing.
        let mut request = packet;
        request["source_binding"] = json!({"source_task_sha256":sha(&serde_json::to_value(task)?)?,"retry_after_validation_failure":retry});
        let reply = crate::model_call(
            app,
            id,
            (actor, if planner { "planner" } else { "scoped_worker" }),
            model,
            (&system, &request.to_string()),
            cancel,
        )
        .await?;
        let parsed: Value =
            serde_json::from_str(&reply.text).context("role output was not JSON")?;
        if governed_triage {
            let fingerprint = crate::context::task_fingerprint(&request.to_string());
            anyhow::ensure!(
                reply
                    .local_execution
                    .as_ref()
                    .and_then(|r| r.task_input_sha256.as_deref())
                    == Some(fingerprint.as_str()),
                "native triage task differs from retrieved source packet"
            );
            decision_inputs.push(
                json!({"case_id":input["case_id"],"input":request,"task_input_sha256":fingerprint}),
            );
        }
        banking::validate(actor, &input, &parsed, task)?;
        replies.push(reply);
        rows.push(parsed);
    }
    state["contributions"][actor] = json!(rows);
    if let Some((root, company, owner)) = &memory_scope {
        state["memory_handoff_protocol"] = json!(banking::TRIAGE_MEMORY_PROTOCOL);
        state["memory_handoffs"]["triage"] = json!(memory_records
            .iter()
            .map(|r| &r.binding)
            .collect::<Vec<_>>());
        state["memory_decision_inputs"]["triage"] = json!(decision_inputs);
        revalidate_triage_memory(root, company, owner, id, task, state)?;
    }
    Ok(replies)
}

pub async fn execute(
    app: &App,
    id: Uuid,
    text: &str,
    _criteria: &[String],
    cancel: &AtomicBool,
) -> Result<crate::judges::Outcome> {
    let task: Task = serde_json::from_str(text)?;
    task.validate()?;
    let (tenant, owner) = {
        let runs = app.runs.lock().await;
        let run = runs.get(&id).context("run missing")?;
        (
            run.company_id.clone().context("company required")?,
            run.owner.clone(),
        )
    };
    let root = std::env::current_dir()?;
    let controller = ControllerLease::acquire(&root, &tenant, &owner, id)?;
    let roster = attest(app, id, &root).await?;
    let controls = security_controls(&root, &tenant, &owner, id)?;
    let mut store = SqliteWorkflows::open(&root, &tenant, &owner, id)?;
    store.bind_controller(&controller)?;
    let consume_memory = app
        .config
        .semantic_memory
        .as_ref()
        .is_some_and(|s| s.verify_consumption);
    let mut nodes = ORDER
        .iter()
        .enumerate()
        .map(|(n, actor)| {
            let row = roster["models"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == *actor)
                .unwrap();
            Node {
                id: (*actor).into(),
                actor: (*actor).into(),
                model_sha256: row["sha256"].as_str().unwrap().into(),
                depends_on: if n == 0 {
                    Default::default()
                } else {
                    [ORDER[n - 1].to_string()].into()
                },
                max_attempts: 3,
                lease_ms: 1_800_000,
            }
        })
        .collect::<Vec<_>>();
    if consume_memory {
        for actor in ORDER {
            let pin = roster["models"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == actor)
                .unwrap();
            nodes.push(Node {
                id: format!("memory-{actor}"),
                actor: actor.into(),
                model_sha256: pin["sha256"].as_str().unwrap().into(),
                depends_on: [actor.to_string()].into(),
                max_attempts: 2,
                lease_ms: 1_800_000,
            });
        }
    }
    store.install(&Graph {
        version: 1,
        contract_sha256: store.contract_sha256()?,
        execution: "read_only_local_computation".into(),
        nodes,
    })?;
    let recovered = store.recover_orphaned(&controller, crate::lease_clock::now_ms())?;
    if recovered > 0 {
        crate::emit(app,id,("harness","system"),("testing","workflow_controller_recovered"),
            &json!({"orphaned_nodes":recovered,"effect":"read_only_local_computation","external_effects_replayed":false}).to_string(),None,None).await?;
    }
    let initial = json!({"task":task,"contributions":{},"references":[],"controls":controls});
    let mut final_state = Value::Null;
    for actor in ORDER {
        let pin = roster["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == actor)
            .unwrap()["sha256"]
            .as_str()
            .unwrap();
        let mut accepted = None;
        for _ in 0..3 {
            anyhow::ensure!(!cancel.load(Ordering::Relaxed), "banking run cancelled");
            let lease = match store.claim(actor, actor, pin, crate::lease_clock::now_ms())? {
                Claim::Cached(output) => {
                    if actor == "triage"
                        && app
                            .config
                            .enterprise
                            .as_ref()
                            .is_some_and(|s| s.governed_triage_memory)
                    {
                        revalidate_triage_memory(
                            &root,
                            &tenant,
                            &owner,
                            id,
                            &task,
                            &output.data["handoff"],
                        )?;
                    }
                    crate::emit(
                        app,
                        id,
                        (actor, "worker"),
                        ("testing", "workflow_checkpoint_reused"),
                        &json!({"stage":actor,"new_model_calls":0,"payload_logged":false})
                            .to_string(),
                        None,
                        None,
                    )
                    .await?;
                    accepted = Some(output.data["handoff"].clone());
                    break;
                }
                Claim::Acquired(lease) => lease,
            };
            let attempt = lease.attempt();
            let inputs = store.inputs(&lease, crate::lease_clock::now_ms())?;
            let mut state = if actor == "orchestrator" {
                initial.clone()
            } else {
                inputs[0]["data"]["handoff"].clone()
            };
            anyhow::ensure!(
                state["task"] == serde_json::to_value(&task)?,
                "DAG handoff changed the frozen task"
            );
            if actor != "orchestrator" {
                state["references"]
                    .as_array_mut()
                    .context("handoff references missing")?
                    .push(json!({"id":inputs[0]["stage"],"sha256":inputs[0]["output_sha256"]}));
            }
            if actor == "reconciliation" {
                state["arithmetic"] =
                    execute_arithmetic(&root, &tenant, &owner, id, &task, cancel).await?;
                let receipt_sha = sha(&state["arithmetic"])?;
                state["references"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id":"integer-arithmetic","sha256":receipt_sha}));
                crate::emit(app,id,(actor,"tool"),("testing","tool_completed"),&json!({"action":"integer_arithmetic","receipt_sha256":receipt_sha,"calculations":task.cases.len(),"payload_logged":false}).to_string(),None,None).await?;
            }
            if actor == "risk" {
                state["controls"]["independent_qa"] = json!(state["contributions"]["qa"]
                    .as_array()
                    .is_some_and(|r| r.len() == task.cases.len() + 1));
                state["controls"]["scoped_dag_integrity"] = json!(true);
            }
            crate::emit(
                app,
                id,
                (actor, "worker"),
                ("testing", "workflow_stage_started"),
                &json!({"stage":actor,"attempt":attempt,"payload_logged":false}).to_string(),
                None,
                None,
            )
            .await?;
            match perform(app, id, actor, &task, &mut state, cancel, attempt > 1).await {
                Ok(calls) => {
                    store.finish(
                        &lease,
                        &Output {
                            data: json!({"role":actor,"handoff":state}),
                            calls,
                        },
                        crate::lease_clock::now_ms(),
                    )?;
                    crate::emit(
                        app,
                        id,
                        (actor, "worker"),
                        ("testing", "workflow_stage_checkpointed"),
                        &json!({"stage":actor,"payload_logged":false,"task_accepted":false})
                            .to_string(),
                        None,
                        None,
                    )
                    .await?;
                    accepted = Some(state);
                    break;
                }
                Err(error) => {
                    store.fail(&lease, true, crate::lease_clock::now_ms())?;
                    let detail = error.to_string();
                    let diagnostic = if crate::security::require_persistable(&detail).is_ok() {
                        detail.chars().take(400).collect::<String>()
                    } else {
                        "diagnostic blocked by data policy".into()
                    };
                    crate::emit(app,id,(actor,"worker"),("testing","workflow_stage_failed"),&json!({"stage":actor,"attempt":attempt,"diagnostic":"role output or source verification failed; original model-call receipts retained","expected_answer_supplied":false,"payload_logged":false}).to_string(),None,None).await?;
                    crate::emit(
                        app,
                        id,
                        (actor, "worker"),
                        ("testing", "banking_validation_failed"),
                        &json!({"stage":actor,"diagnostic":diagnostic,"payload_logged":false})
                            .to_string(),
                        None,
                        None,
                    )
                    .await?;
                }
            }
        }
        let state = accepted.context(format!(
            "banking stage {actor} exhausted its bounded retry budget"
        ))?;
        if actor == "orchestrator" {
            apply_delegation(
                &root,
                &tenant,
                &owner,
                id,
                &state["contributions"][actor][0],
            )?;
        }
        final_state = state;
    }
    let mut results = Vec::new();
    if app
        .config
        .enterprise
        .as_ref()
        .is_some_and(|s| s.governed_triage_memory)
    {
        revalidate_triage_memory(&root, &tenant, &owner, id, &task, &final_state)?;
    }
    if app
        .config
        .enterprise
        .as_ref()
        .is_some_and(|s| s.compiled_release_policy)
    {
        let program: crate::release_policy::Program =
            serde_json::from_value(final_state["release_policy"]["program"].clone())?;
        crate::release_policy::validate(&program, false)?;
        let input = banking::jobs("risk", &task, &final_state)?.remove(0);
        anyhow::ensure!(
            crate::release_policy::execute(&program, &input)?
                == final_state["contributions"]["risk"][0],
            "cached compiled risk output changed"
        );
    }
    for case in &task.cases {
        let mut row = banking::candidate(&final_state, case)?;
        row["case_id"] = json!(case.case_id);
        anyhow::ensure!(
            row == banking::reference(case),
            "independent final banking check failed"
        );
        results.push(row);
    }
    let semantic_memory = crate::semantic_memory::flush_run(app, id).await?;
    let mut memory_consumption = Vec::new();
    if consume_memory {
        for actor in ORDER {
            let stage = format!("memory-{actor}");
            let pin = roster["models"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == actor)
                .unwrap()["sha256"]
                .as_str()
                .unwrap();
            let mut accepted = None;
            for _ in 0..2 {
                anyhow::ensure!(!cancel.load(Ordering::Relaxed), "memory audit cancelled");
                let lease = match store.claim(&stage, actor, pin, crate::lease_clock::now_ms())? {
                    Claim::Cached(output) => {
                        crate::memory_consumption::revalidate(
                            &root,
                            &tenant,
                            &owner,
                            id,
                            actor,
                            &output.data,
                        )?;
                        accepted = Some(output.data);
                        break;
                    }
                    Claim::Acquired(lease) => lease,
                };
                store.inputs(&lease, crate::lease_clock::now_ms())?;
                match crate::memory_consumption::execute(app, id, actor, cancel).await {
                    Ok(output) => {
                        store.finish(&lease, &output, crate::lease_clock::now_ms())?;
                        accepted = Some(output.data);
                        break;
                    }
                    Err(error) => {
                        store.fail(&lease, true, crate::lease_clock::now_ms())?;
                        crate::emit(app,id,(actor,"memory_extract"),("testing","memory_consumption_failed"),"Read-only memory audit failed; no consumption accepted and bounded retry retained.",None,None).await?;
                        if lease.attempt() >= 2 {
                            return Err(error);
                        }
                    }
                }
            }
            let result = accepted.context("memory audit exhausted bounded attempts")?;
            crate::emit(app,id,(actor,"memory_extract"),("testing","memory_consumption_verified"),&json!({"recipient":actor,"memo_input_sha256":result["memo_input_sha256"],"source_sha256":result["source_sha256"],"policy_revision":result["policy_revision"],"payload_logged":false}).to_string(),None,None).await?;
            memory_consumption.push(result);
        }
    }
    let graph = store.snapshot()?;
    anyhow::ensure!(
        graph["nodes"]
            .as_array()
            .context("checkpoint nodes missing")?
            .iter()
            .all(|r| r["state"] == "completed"),
        "workflow checkpoints incomplete"
    );
    let artifact = company::scoped(&root, &tenant, &["runs", &id.to_string(), "workspace"])?
        .join("banking-report.json");
    std::fs::create_dir_all(artifact.parent().unwrap())?;
    anyhow::ensure!(!artifact.is_symlink(), "banking report symlink rejected");
    let report = json!({"version":1,"profile":banking::PROFILE,"passed":true,"source_task_sha256":sha(&serde_json::to_value(&task)?)?,
        "models":roster["models"],"results":results,"state":final_state,"workflow":graph,"semantic_memory":semantic_memory,"memory_consumption":memory_consumption,"compliance":"scoped synthetic controls only; no regulatory certification or production authorization"});
    let body = serde_json::to_vec_pretty(&report)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options.open(&artifact)?.write_all(&body)?;
    let artifact_sha = format!("{:x}", Sha256::digest(&body));
    crate::emit(app,id,("harness","judge"),("review","banking_acceptance"),&json!({"passed":true,"cases":task.cases.len(),"models":10,"artifact_sha256":artifact_sha,"production_authorized":false}).to_string(),None,None).await?;
    Ok(crate::judges::Outcome{status:"completed",output:json!({"artifact":artifact,"sha256":artifact_sha,"case_count":task.cases.len(),"distinct_models":10,"production_authorized":false}).to_string()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_memory_protocol_cannot_be_resumed_as_current() {
        let legacy: Settings = serde_json::from_value(json!({"profile":banking::PROFILE})).unwrap();
        legacy.validate_memory_protocol().unwrap();
        for version in [0, 1, 3] {
            let settings:Settings=serde_json::from_value(json!({"profile":banking::PROFILE,"governed_triage_memory":true,"triage_memory_protocol":version})).unwrap();
            assert!(settings.validate_memory_protocol().is_err());
        }
        let current:Settings=serde_json::from_value(json!({"profile":banking::PROFILE,"governed_triage_memory":true,"triage_memory_protocol":banking::TRIAGE_MEMORY_PROTOCOL})).unwrap();
        current.validate_memory_protocol().unwrap();
    }
}

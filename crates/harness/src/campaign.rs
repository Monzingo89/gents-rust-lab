//! Evidence-producing local capability trials. Typed model work is separate
//! from deterministic finance checks, storage, capacity admission and policy.
use crate::workflow::{Claim, Graph, Node, Output, SqliteWorkflows, WorkflowRepository};
use crate::{company, judges, provider::Reply, Agent, App};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub variant: Variant,
    pub require_parallel: bool,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    Baseline,
    Focused,
    ToolGrounded,
    Decomposed,
    CompiledPolicy,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Input {
    campaign: String,
    cases: Vec<Case>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    case_id: String,
    expected_cents: i64,
    currency: String,
    payments: Vec<Payment>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Payment {
    id: String,
    cents: i64,
    currency: String,
}
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CaseResult {
    case_id: String,
    currency_mismatch: bool,
    duplicate_ids: bool,
    empty: bool,
    status: String,
    observed_cents: i64,
    difference_cents: i64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Work {
    results: Vec<CaseResult>,
    summary: String,
}

const CONTRACT:&str = "Contract v1: For each case sum ALL payment cents, including duplicate IDs and other currencies. difference_cents is observed_cents minus expected_cents. Status priority: invalid_currency if ANY payment currency differs from the case currency; otherwise duplicate if ANY payment ID repeats within that case; otherwise missing if the payments array is empty; otherwise matched if the difference is zero; otherwise mismatch. Do not discard or deduplicate payments. Use exact integer cents; no floats, advice, transfers or real customer data. Return each case_id exactly once.";

fn expected(case: &Case) -> CaseResult {
    let total = case.payments.iter().map(|p| p.cents).sum::<i64>();
    let mut ids = BTreeSet::new();
    let duplicate = case.payments.iter().any(|p| !ids.insert(&p.id));
    let status = if case.payments.iter().any(|p| p.currency != case.currency) {
        "invalid_currency"
    } else if duplicate {
        "duplicate"
    } else if case.payments.is_empty() {
        "missing"
    } else if total == case.expected_cents {
        "matched"
    } else {
        "mismatch"
    };
    CaseResult {
        case_id: case.case_id.clone(),
        currency_mismatch: case.payments.iter().any(|p| p.currency != case.currency),
        duplicate_ids: duplicate,
        empty: case.payments.is_empty(),
        status: status.into(),
        observed_cents: total,
        difference_cents: total - case.expected_cents,
    }
}

fn validate_input(input: &Input) -> Result<()> {
    if input.campaign != "synthetic-reconciliation-v1" || !(2..=24).contains(&input.cases.len()) {
        bail!("invalid campaign or case count");
    }
    let mut ids = BTreeSet::new();
    let token = |s: &str| {
        !s.is_empty() && s.len() <= 40 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    let currency = |s: &str| s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase());
    for case in &input.cases {
        if !token(&case.case_id)
            || !ids.insert(&case.case_id)
            || !currency(&case.currency)
            || case.expected_cents.unsigned_abs() > 10_000_000
            || case.payments.len() > 12
            || case.payments.iter().any(|p| {
                !token(&p.id) || !currency(&p.currency) || p.cents.unsigned_abs() > 10_000_000
            })
        {
            bail!("campaign requires bounded synthetic case IDs, currencies and integer amounts");
        }
    }
    Ok(())
}

fn verify(cases: &[Case], work: &Work) -> Result<()> {
    if work.results.len() != cases.len() {
        bail!("case coverage mismatch");
    }
    let mut seen = BTreeSet::new();
    for row in &work.results {
        let case = cases
            .iter()
            .find(|c| c.case_id == row.case_id)
            .context("unknown case ID in model result")?;
        if !seen.insert(&row.case_id) || *row != expected(case) {
            bail!(
                "deterministic reconciliation check failed for {}",
                row.case_id
            );
        }
    }
    Ok(())
}

fn overlap(left: &[Reply], right: &[Reply]) -> u64 {
    left.iter()
        .filter_map(|r| r.local_execution.as_ref())
        .flat_map(|a| {
            right
                .iter()
                .filter_map(|r| r.local_execution.as_ref())
                .filter_map(move |b| {
                    (a.backend_pid != b.backend_pid && a.port != b.port).then_some(
                        a.received_at_ms
                            .min(b.received_at_ms)
                            .saturating_sub(a.dispatched_at_ms.max(b.dispatched_at_ms)),
                    )
                })
        })
        .max()
        .unwrap_or(0)
}

async fn plan_pair(
    app: &App,
    id: Uuid,
    agent: &Agent,
    cases: &[Case],
    variant: Variant,
    cancel: &AtomicBool,
) -> Result<Output> {
    let input = json!({"contract_version":1,"cases":cases,"contract":CONTRACT});
    let planner_system = if matches!(variant, Variant::CompiledPolicy) {
        "LOCAL_CAMPAIGN_POLICY_PLAN: Implement the frozen status contract as a bounded declarative rule table. Return arithmetic_cases (all assigned case IDs), rules (five ordered {when,status} rules), steps and risks. Available when tests: currency_mismatch, duplicate_ids, empty, zero_difference, always. Use the original contract to choose each return status and rule order; first matching rule wins. This is your implementation artifact, not a claim of execution. Rules cannot execute code or grant permissions."
    } else {
        "LOCAL_CAMPAIGN_PLAN: Make a brief public execution checklist for your assigned reconciliation cases. Return {arithmetic_cases:[every assigned case_id exactly once],steps:[1..4 strings],risks:[0..3 strings]}. arithmetic_cases requests the approved exact-integer tool for those input cases when tool grounding is enabled. Do not compute final case answers or claim execution."
    };
    let plan = crate::model_call(
        app,
        id,
        (&agent.id, "planner"),
        &agent.planner,
        (planner_system, &input.to_string()),
        cancel,
    )
    .await?;
    let plan_value: Value =
        serde_json::from_str(&plan.text).context("invalid local planner output")?;
    if plan_value["steps"]
        .as_array()
        .is_none_or(|s| s.is_empty() || s.len() > 4)
    {
        bail!("local plan checklist invalid");
    }
    Ok(Output {
        data: plan_value,
        calls: vec![plan],
    })
}

#[allow(clippy::too_many_arguments)]
async fn pair_work(
    app: &App,
    id: Uuid,
    agent: &Agent,
    cases: &[Case],
    variant: Variant,
    cancel: &AtomicBool,
    plan: Reply,
) -> Result<(Work, Vec<Reply>)> {
    let input = json!({"contract_version":1,"cases":cases,"contract":CONTRACT});
    let plan_value: Value = serde_json::from_str(&plan.text)?;
    let arithmetic = if matches!(
        variant,
        Variant::ToolGrounded | Variant::Decomposed | Variant::CompiledPolicy
    ) {
        let selected = plan_value["arithmetic_cases"]
            .as_array()
            .context("planner arithmetic case IDs required")?;
        let wanted = cases
            .iter()
            .map(|c| c.case_id.as_str())
            .collect::<BTreeSet<_>>();
        let selected_ids = selected
            .iter()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        if selected.len() != cases.len() || selected_ids != wanted {
            bail!("planner selected incomplete or unauthorized arithmetic case IDs");
        }
        let args = json!({"calculations":cases.iter().map(|c|json!({"id":c.case_id,"values":c.payments.iter().map(|p|p.cents).collect::<Vec<_>>(),"subtract":c.expected_cents})).collect::<Vec<_>>()});
        let (tenant, owner) = {
            let runs = app.runs.lock().await;
            let run = runs.get(&id).context("run missing")?;
            (
                run.company_id.clone().context("company required")?,
                run.owner.clone(),
            )
        };
        let root = std::env::current_dir()?;
        let policy =
            company::governance(&root, &tenant, &owner, id)?.context("governance required")?;
        policy.authorize(&agent.id, "integer_arithmetic", &args)?;
        use crate::contracts::ToolContracts;
        crate::contracts::EmbeddedContracts.validate("integer_arithmetic", &args)?;
        let result =
            crate::autonomy::broker(&tenant, id, "integer_arithmetic", &args, cancel).await?;
        if result["ok"] != true {
            bail!("exact-integer tool failed");
        }
        crate::security::require_persistable(&result.to_string())?;
        use sha2::{Digest, Sha256};
        let filename = format!(
            "campaign-arithmetic-{:x}.json",
            Sha256::digest(agent.id.as_bytes())
        );
        let folder = company::scoped(&root, &tenant, &["runs", &id.to_string()])?;
        let receipt = folder.join(&filename);
        if receipt.is_symlink() {
            bail!("arithmetic receipt symlink rejected");
        }
        std::fs::write(
            &receipt,
            serde_json::to_vec_pretty(
                &json!({"actor":agent.id,"policy_revision":policy.revision,"request":{"action":"integer_arithmetic","arguments":args},"result":result}),
            )?,
        )?;
        crate::emit(app,id,(&agent.id,"tool"),("testing","tool_completed"),
            &json!({"action":"integer_arithmetic","receipt":filename,"calculations":cases.len(),"policy_revision":policy.revision,"payload_logged":false}).to_string(),None,None).await?;
        result["result"].clone()
    } else {
        Value::Null
    };
    let system=match variant {
        Variant::Baseline=>"LOCAL_CAMPAIGN_WORK: Apply the frozen reconciliation contract to every assigned case. Use your planner checklist. Return {results:[{case_id,status,observed_cents,difference_cents}],summary:string}.",
        Variant::Focused=>"LOCAL_CAMPAIGN_WORK: Execute the contract mechanically, independently for each case. First add every integer cents value exactly once. Subtract expected_cents for the signed difference. Then evaluate status tests in the declared priority order, stopping at the first true test. Empty payments always means missing, even for an expected zero. Preserve every case ID. Check arithmetic and coverage before emitting the typed results. The checklist is advisory; the original contract is authoritative.",
        Variant::ToolGrounded=>"LOCAL_CAMPAIGN_WORK: Use the actual exact-integer tool result as arithmetic evidence: copy sum into observed_cents and difference into difference_cents for the matching calculation ID. Do not substitute expected_cents for the observed sum. Independently classify each original case using the frozen status priority. Currency errors precede duplicates; duplicates precede empty payments; only then does a zero tool difference mean matched, otherwise mismatch. Return all assigned cases exactly once. Tool output establishes arithmetic, not financial policy or permission.",
        Variant::Decomposed=>"LOCAL_CAMPAIGN_WORK: Handle ONLY the single case in task.cases. First extract currency_mismatch (any payment currency differs from the case currency), duplicate_ids (any payment ID repeats), and empty (no payments) from the ORIGINAL payments. Keep those booleans true even if arithmetic difference is zero. Copy the executed tool sum and difference into the numeric fields. Apply status priority to those facts: currency_mismatch -> invalid_currency; else duplicate_ids -> duplicate; else empty -> missing; else zero difference -> matched; else mismatch. Return exactly one case result. Tool output establishes arithmetic only, not source facts or the policy decision.",
        Variant::CompiledPolicy=>"LOCAL_CAMPAIGN_OBSERVE: Copy ONLY the single case's source columns from task.cases. Return one results row with case_id, payment_ids (each payment.id in ORIGINAL order, keeping repetitions), and payment_currencies (each payment.currency in the same order). Empty payments means both arrays are empty. Do not deduplicate, infer flags, calculate amounts or decide status. Bounded code calculates predicates and arithmetic; the planner-authored rules decide status. Return a brief summary.",
    };
    if matches!(variant, Variant::Decomposed | Variant::CompiledPolicy) {
        let mut calls = vec![plan];
        let mut combined=Work{results:Vec::new(),summary:"Case-scoped local workers; original contract and exact arithmetic receipts retained".into()};
        for case in cases {
            let observed = arithmetic["calculations"]
                .as_array()
                .context("tool calculations missing")?
                .iter()
                .find(|row| row["id"] == case.case_id)
                .context("case arithmetic receipt missing")?;
            let mut worker_input = json!({"contract_version":1,"task":{"contract":CONTRACT,"cases":[case]},"planner_checklist":plan_value["steps"]});
            if matches!(variant, Variant::CompiledPolicy) {
                use sha2::{Digest, Sha256};
                // Cross-case planner prose is not evidence about this case.
                worker_input
                    .as_object_mut()
                    .unwrap()
                    .remove("planner_checklist");
                worker_input["planner_handoff"] = json!({"version":1,"planner_output_sha256":format!("{:x}",Sha256::digest(calls[0].text.as_bytes())),"authority":"implementation reference, not source facts"});
                let mut accepted = false;
                for attempt in 0..3 {
                    let work = crate::model_call(
                        app,
                        id,
                        (&agent.id, "observer"),
                        &agent.worker,
                        (system, &worker_input.to_string()),
                        cancel,
                    )
                    .await?;
                    let value: Value = serde_json::from_str(&work.text)
                        .context("invalid local observation JSON")?;
                    let rows = value["results"]
                        .as_array()
                        .context("local observation rows missing")?;
                    let errors = if rows.len() == 1 {
                        observation_errors(case, &rows[0])
                    } else {
                        vec!["case_coverage"]
                    };
                    calls.push(work);
                    if errors.is_empty() {
                        let facts = projection_facts(&rows[0], &case.currency)?;
                        let row = assemble_observation(
                            &plan_value["rules"],
                            &facts,
                            observed,
                            &case.case_id,
                        )?;
                        combined.results.push(serde_json::from_value(row)?);
                        accepted = true;
                        break;
                    }
                    crate::emit(app,id,(&agent.id,"judge"),("testing","observation_rejected"),
                        &json!({"case_id":case.case_id,"attempt":attempt+1,"mismatched_fields":errors,"answer_supplied":false,"payload_logged":false}).to_string(),None,None).await?;
                    worker_input["verification_feedback"] = json!({"attempt":attempt+1,"mismatched_fields":errors,
                        "instruction":"Previous copied columns failed source verification. Re-read ONLY the original case records and copy them exactly in original order, including repetitions. No expected values or financial decisions are supplied."});
                }
                if !accepted {
                    bail!("local observation recovery exhausted for {}", case.case_id);
                }
                continue;
            }
            worker_input["arithmetic_tool_evidence"] = observed.clone();
            let work = crate::model_call(
                app,
                id,
                (&agent.id, "worker"),
                &agent.worker,
                (system, &worker_input.to_string()),
                cancel,
            )
            .await?;
            let parsed_value: Value =
                serde_json::from_str(&work.text).context("invalid case-scoped local output")?;
            let parsed: Work = serde_json::from_value(parsed_value)?;
            // Preserve a bad result for the independent verifier; never replace
            // model answers with the deterministic reference implementation.
            combined.results.extend(parsed.results);
            calls.push(work);
        }
        return Ok((combined, calls));
    }
    let work = crate::model_call(
        app,
        id,
        (&agent.id, "worker"),
        &agent.worker,
        (
            system,
            &json!({"contract_version":1,"task":input,"planner_checklist":plan_value,"arithmetic_tool_evidence":arithmetic}).to_string(),
        ),
        cancel,
    )
    .await?;
    let parsed: Work = serde_json::from_str(&work.text).context("invalid local worker output")?;
    Ok((parsed, vec![plan, work]))
}

#[allow(clippy::too_many_arguments)]
async fn phase<F, Fut>(
    app: &App,
    id: Uuid,
    actor: &str,
    store: &mut SqliteWorkflows,
    stage: &str,
    pin: &str,
    work: F,
) -> Result<Output>
where
    F: FnOnce(Value) -> Fut,
    Fut: std::future::Future<Output = Result<Output>>,
{
    let role = if stage.ends_with("-plan") {
        "planner"
    } else {
        "worker"
    };
    let lease = match store.claim(stage, actor, pin, crate::now_ms())? {
        Claim::Cached(output) => {
            crate::emit(app,id,(actor,role),("testing","workflow_checkpoint_reused"),
                &json!({"stage":stage,"new_model_calls":0,"receipt_calls":output.calls.len(),"payload_logged":false}).to_string(),None,None).await?;
            return Ok(output);
        }
        Claim::Acquired(lease) => lease,
    };
    let inputs = store.inputs(&lease, crate::now_ms())?;
    crate::emit(
        app,
        id,
        (actor, role),
        ("testing", "workflow_stage_started"),
        &json!({"stage":stage,"payload_logged":false}).to_string(),
        None,
        None,
    )
    .await?;
    match work(inputs).await {
        Ok(output) => {
            store.finish(&lease, &output, crate::now_ms())?;
            crate::emit(app,id,(actor,role),("testing","workflow_stage_checkpointed"),
                &json!({"stage":stage,"calls":output.calls.len(),"payload_logged":false,"task_accepted":false}).to_string(),None,None).await?;
            Ok(output)
        }
        Err(error) => {
            store.fail(&lease, true, crate::now_ms())?;
            crate::emit(
                app,
                id,
                (actor, role),
                ("testing", "workflow_stage_failed"),
                &json!({"stage":stage,"payload_logged":false,"bounded_retry_only":true})
                    .to_string(),
                None,
                None,
            )
            .await?;
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn pair(
    app: &App,
    id: Uuid,
    agent: &Agent,
    cases: &[Case],
    variant: Variant,
    cancel: &AtomicBool,
    ordinal: usize,
    pins: &[String; 2],
) -> Result<(Work, Vec<Reply>)> {
    let (tenant, owner) = {
        let runs = app.runs.lock().await;
        let run = runs.get(&id).context("run missing")?;
        (
            run.company_id.clone().context("company required")?,
            run.owner.clone(),
        )
    };
    let mut store = SqliteWorkflows::open(&std::env::current_dir()?, &tenant, &owner, id)?;
    let plan = phase(
        app,
        id,
        &agent.id,
        &mut store,
        &format!("pair-{ordinal}-plan"),
        &pins[0],
        |_| plan_pair(app, id, agent, cases, variant, cancel),
    )
    .await?;
    let plan_reply = plan
        .calls
        .first()
        .context("planner checkpoint receipt missing")?
        .clone();
    let output = phase(
        app,
        id,
        &agent.id,
        &mut store,
        &format!("pair-{ordinal}-work"),
        &pins[1],
        |inputs| async move {
            if inputs[0]["data"] != plan.data {
                bail!("worker handoff does not match planner checkpoint");
            }
            let (work, calls) =
                pair_work(app, id, agent, cases, variant, cancel, plan_reply).await?;
            // Checkpoint validity never substitutes for independent finance checks.
            verify(cases, &work)?;
            Ok(Output {
                data: serde_json::to_value(&work)?,
                calls,
            })
        },
    )
    .await?;
    Ok((serde_json::from_value(output.data)?, output.calls))
}

/// Verification reports field names only; local workers must correct their
/// own observations from source, rather than receive injected answer values.
fn observation_errors(case: &Case, row: &Value) -> Vec<&'static str> {
    let fields = [
        (
            "payment_ids",
            json!(case.payments.iter().map(|p| &p.id).collect::<Vec<_>>()),
        ),
        (
            "payment_currencies",
            json!(case
                .payments
                .iter()
                .map(|p| &p.currency)
                .collect::<Vec<_>>()),
        ),
    ];
    let mut errors = Vec::new();
    if row["case_id"] != case.case_id {
        errors.push("case_id");
    }
    for (field, expected) in fields {
        if row[field] != expected {
            errors.push(field);
        }
    }
    errors
}

/// Generic column predicates operate on the worker's extracted source, never
/// on the reference checker's expected answers. They cannot choose a status.
fn projection_facts(row: &Value, currency: &str) -> Result<Value> {
    let fields = row
        .as_object()
        .context("source projection must be an object")?;
    if fields.len() != 3 || row["case_id"].as_str().is_none() {
        bail!("source projection must contain only case_id and two source columns");
    }
    let ids = row["payment_ids"].as_array().context("ID column missing")?;
    let currencies = row["payment_currencies"]
        .as_array()
        .context("currency column missing")?;
    if ids.len() != currencies.len()
        || ids.len() > 12
        || ids.iter().chain(currencies).any(|v| v.as_str().is_none())
    {
        bail!("source columns must be equally sized bounded string arrays");
    }
    let mut seen = BTreeSet::new();
    Ok(json!({"case_id":row["case_id"],"empty":ids.is_empty(),
        "duplicate_ids":ids.iter().any(|v| !seen.insert(v.as_str().unwrap())),
        "currency_mismatch":currencies.iter().any(|v| v.as_str()!=Some(currency))}))
}

/// Trusted tool fields never make a round trip through model-generated JSON.
/// The model owns source observations and the rule artifact, not arithmetic.
fn assemble_observation(
    rules: &Value,
    observation: &Value,
    arithmetic: &Value,
    case_id: &str,
) -> Result<Value> {
    let fields = observation
        .as_object()
        .context("worker observation must be an object")?;
    if fields.len() != 4
        || ["case_id", "currency_mismatch", "duplicate_ids", "empty"]
            .iter()
            .any(|field| !fields.contains_key(*field))
    {
        bail!("worker may return only the assigned source facts, not tool-owned amounts or status");
    }
    if observation["case_id"] != case_id || arithmetic["id"] != case_id {
        bail!("observation/tool case binding failed");
    }
    if arithmetic["sum"].as_i64().is_none() || arithmetic["difference"].as_i64().is_none() {
        bail!("tool arithmetic is not exact signed integers");
    }
    let mut row = observation.clone();
    row["observed_cents"] = arithmetic["sum"].clone();
    row["difference_cents"] = arithmetic["difference"].clone();
    row["status"] = json!(apply_rules(rules, &row)?);
    Ok(row)
}

/// A tiny, bounded interpreter for a LOCAL MODEL'S rule artifact. It does not
/// consult the reference verifier, reorder rules or repair incorrect mappings.
fn apply_rules(rules: &Value, row: &Value) -> Result<String> {
    let rules = rules.as_array().context("planner rules missing")?;
    if rules.len() != 5 {
        bail!("planner must author five bounded rules");
    }
    for rule in rules {
        let applies = match rule["when"].as_str().context("rule predicate missing")? {
            field @ ("currency_mismatch" | "duplicate_ids" | "empty") => {
                row[field].as_bool().context("worker fact is not boolean")?
            }
            "zero_difference" => {
                row["difference_cents"]
                    .as_i64()
                    .context("worker difference missing")?
                    == 0
            }
            "always" => true,
            _ => bail!("unsupported rule predicate; code execution is forbidden"),
        };
        let status = rule["status"].as_str().context("rule result missing")?;
        if ![
            "invalid_currency",
            "duplicate",
            "missing",
            "matched",
            "mismatch",
        ]
        .contains(&status)
        {
            bail!("unsupported rule result");
        }
        if applies {
            return Ok(status.into());
        }
    }
    bail!("model-authored rules did not cover observation")
}

pub async fn execute(
    app: &App,
    id: Uuid,
    task: &str,
    criteria: &[String],
    cancel: &AtomicBool,
) -> Result<judges::Outcome> {
    let settings = app
        .config
        .campaign
        .as_ref()
        .context("campaign configuration required")?;
    let input: Input =
        serde_json::from_str(task).context("campaign task must contain the frozen input JSON")?;
    validate_input(&input)?;
    let (tenant, owner) = {
        let runs = app.runs.lock().await;
        let run = runs.get(&id).context("run missing")?;
        (
            run.company_id.clone().context("company required")?,
            run.owner.clone(),
        )
    };
    let root = std::env::current_dir()?;
    let policy = company::governance(&root, &tenant, &owner, id)?
        .context("campaign requires operator-installed governance")?;
    if criteria.is_empty() {
        bail!("campaign requires frozen acceptance criteria");
    }
    for agent in &app.config.agents {
        if !policy.grants.contains_key(&agent.id)
            || [&agent.planner, &agent.worker]
                .iter()
                .any(|m| m.provider != "local")
        {
            bail!("campaign requires explicitly assigned real local planner/worker pairs");
        }
    }
    let mut store = SqliteWorkflows::open(&root, &tenant, &owner, id)?;
    let resolver = crate::local_runtime::LocalRuntime::new(root.clone())?;
    let mut hashes = std::collections::BTreeMap::new();
    let mut nodes = Vec::new();
    let mut pins = Vec::new();
    for (ordinal, agent) in app.config.agents.iter().enumerate() {
        let mut pair_pins = Vec::new();
        for (role, model) in [("plan", &agent.planner), ("work", &agent.worker)] {
            let path = resolver.resolve_model(
                model
                    .path
                    .as_deref()
                    .context("campaign model path required")?,
                "local",
            )?;
            let sha = if let Some(sha) = hashes.get(&path) {
                String::clone(sha)
            } else {
                let observed_path = path.clone();
                let sha = tokio::task::spawn_blocking(move || company::file_hash(&observed_path))
                    .await??;
                hashes.insert(path, sha.clone());
                sha
            };
            nodes.push(Node {
                id: format!("pair-{ordinal}-{role}"),
                actor: agent.id.clone(),
                model_sha256: sha.clone(),
                depends_on: if role == "work" {
                    [format!("pair-{ordinal}-plan")].into()
                } else {
                    Default::default()
                },
                max_attempts: 3,
                lease_ms: 600_000,
            });
            pair_pins.push(sha);
        }
        pins.push([pair_pins.remove(0), pair_pins.remove(0)]);
    }
    store.install(&Graph {
        version: 1,
        contract_sha256: store.contract_sha256()?,
        execution: "read_only_local_computation".into(),
        nodes,
    })?;
    let split = input.cases.len().div_ceil(2);
    let (left, right) = tokio::join!(
        pair(
            app,
            id,
            &app.config.agents[0],
            &input.cases[..split],
            settings.variant,
            cancel,
            0,
            &pins[0]
        ),
        pair(
            app,
            id,
            &app.config.agents[1],
            &input.cases[split..],
            settings.variant,
            cancel,
            1,
            &pins[1]
        )
    );
    let (left, left_calls) = left?;
    let (right, right_calls) = right?;
    let checks = [
        verify(&input.cases[..split], &left),
        verify(&input.cases[split..], &right),
    ];
    let overlap_ms = overlap(&left_calls, &right_calls);
    let correct = checks.iter().all(Result::is_ok);
    let passed = correct && (!settings.require_parallel || overlap_ms > 0);
    let errors = checks
        .iter()
        .filter_map(|r| r.as_ref().err().map(ToString::to_string))
        .collect::<Vec<_>>();
    let report = json!({"version":1,"profile":input.campaign,"passed":passed,"correct":correct,
        "variant":settings.variant,"case_count":input.cases.len(),"overlap_ms":overlap_ms,
        "parallel_required":settings.require_parallel,"policy_revision":policy.revision,
        "results":[left,right],"errors":errors,"calls":[left_calls,right_calls],"workflow":store.snapshot()?,
        "compliance":"synthetic workflow controls only; not certified","hardware_energy_cost_excluded":true});
    let work = company::scoped(&root, &tenant, &["runs", &id.to_string(), "workspace"])?;
    std::fs::create_dir_all(&work)?;
    let artifact = work.join("reconciliation-evidence.json");
    if artifact.is_symlink() {
        bail!("campaign evidence symlink rejected");
    }
    std::fs::write(&artifact, serde_json::to_vec_pretty(&report)?)?;
    crate::emit(app,id,("Campaign Verifier","judge"),("testing","campaign_verified"),
        &json!({"passed":passed,"correct":correct,"case_count":input.cases.len(),"overlap_ms":overlap_ms,"artifact":"reconciliation-evidence.json","payload_logged":false}).to_string(),None,None).await?;
    Ok(judges::Outcome {
        status: if passed { "completed" } else { "blocked" },
        output: report.to_string(),
    })
}

pub fn schema(system: &str) -> Option<Value> {
    let object = |p: Value, r: Vec<&str>| json!({"type":"object","properties":p,"required":r,"additionalProperties":false});
    if system.starts_with("LOCAL_CAMPAIGN_POLICY_PLAN") {
        let mut value = schema("LOCAL_CAMPAIGN_PLAN")?;
        value["properties"]["rules"] = json!({"type":"array","minItems":5,"maxItems":5,"items":object(json!({
            "when":{"type":"string","enum":["currency_mismatch","duplicate_ids","empty","zero_difference","always"]},
            "status":{"type":"string","enum":["invalid_currency","duplicate","missing","matched","mismatch"]}
        }),vec!["when","status"])});
        value["required"] = json!(["arithmetic_cases", "rules", "steps", "risks"]);
        return Some(value);
    }
    if system.starts_with("LOCAL_CAMPAIGN_OBSERVE") {
        return Some(object(
            json!({"results":{"type":"array","minItems":1,"maxItems":1,
            "items":object(json!({"case_id":{"type":"string","maxLength":40},
                "payment_ids":{"type":"array","maxItems":12,"items":{"type":"string","maxLength":40}},
                "payment_currencies":{"type":"array","maxItems":12,"items":{"type":"string","maxLength":3}}}),
                vec!["case_id","payment_ids","payment_currencies"])},
            "summary":{"type":"string","maxLength":300}}),
            vec!["results", "summary"],
        ));
    }
    if system.starts_with("LOCAL_CAMPAIGN_PLAN") {
        Some(object(
            json!({"arithmetic_cases":{"type":"array","minItems":1,"maxItems":12,"items":{"type":"string","maxLength":40}},"steps":{"type":"array","minItems":1,"maxItems":4,"items":{"type":"string","maxLength":200}},"risks":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":200}}}),
            vec!["arithmetic_cases", "steps", "risks"],
        ))
    } else if system.starts_with("LOCAL_CAMPAIGN_WORK") {
        Some(object(
            json!({"results":{"type":"array","minItems":1,"maxItems":24,"items":object(json!({
            "case_id":{"type":"string","maxLength":40},"currency_mismatch":{"type":"boolean"},"duplicate_ids":{"type":"boolean"},"empty":{"type":"boolean"},"status":{"type":"string","enum":["invalid_currency","duplicate","missing","matched","mismatch"]},
            "observed_cents":{"type":"integer","minimum":-120000000,"maximum":120000000},"difference_cents":{"type":"integer","minimum":-130000000,"maximum":130000000}
        }),vec!["case_id","currency_mismatch","duplicate_ids","empty","observed_cents","difference_cents","status"])},"summary":{"type":"string","maxLength":300}}),
            vec!["results", "summary"],
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::LocalExecution;
    #[test]
    fn source_projection_preserves_repetitions_and_never_supplies_policy() {
        let case: Case = serde_json::from_value(json!({"case_id":"a","expected_cents":5,"currency":"USD",
            "payments":[{"id":"p0","cents":2,"currency":"EUR"},{"id":"p0","cents":3,"currency":"USD"}]})).unwrap();
        let copied =
            json!({"case_id":"a","payment_ids":["p0","p0"],"payment_currencies":["EUR","USD"]});
        assert!(observation_errors(&case, &copied).is_empty());
        let facts = projection_facts(&copied, "USD").unwrap();
        assert_eq!(
            facts,
            json!({"case_id":"a","currency_mismatch":true,"duplicate_ids":true,"empty":false})
        );
        let wrong =
            json!({"case_id":"a","payment_ids":["p0","p1"],"payment_currencies":["EUR","USD"]});
        assert_eq!(observation_errors(&case, &wrong), vec!["payment_ids"]);
        // The interpreter preserves wrong copied facts; only verification
        // rejects them. It never repairs them from the original case.
        assert_eq!(
            projection_facts(&wrong, "USD").unwrap()["duplicate_ids"],
            false
        );
        assert!(projection_facts(
            &json!({"case_id":"a","payment_ids":["p0"],"payment_currencies":[]}),
            "USD"
        )
        .is_err());
        let empty = projection_facts(
            &json!({"case_id":"a","payment_ids":[],"payment_currencies":[]}),
            "USD",
        )
        .unwrap();
        assert_eq!(empty["empty"], true);
        assert_eq!(empty["duplicate_ids"], false);
        assert_eq!(empty["currency_mismatch"], false);
        assert!(empty.get("status").is_none());
    }
    #[test]
    fn result_assembler_preserves_verified_arithmetic_and_rejects_model_overrides() {
        let rules = json!([
            {"when":"currency_mismatch","status":"invalid_currency"},{"when":"duplicate_ids","status":"duplicate"},
            {"when":"empty","status":"missing"},{"when":"zero_difference","status":"matched"},{"when":"always","status":"mismatch"}]);
        let facts = json!({"case_id":"case-a","currency_mismatch":false,"duplicate_ids":true,"empty":false});
        let tool = json!({"id":"case-a","sum":927,"difference":0});
        let row = assemble_observation(&rules, &facts, &tool, "case-a").unwrap();
        assert_eq!(row["observed_cents"], 927);
        assert_eq!(row["difference_cents"], 0);
        assert_eq!(row["status"], "duplicate");
        let mut forged = facts.clone();
        forged["observed_cents"] = json!(924);
        assert!(assemble_observation(&rules, &forged, &tool, "case-a").is_err());
        assert!(assemble_observation(&rules, &facts, &tool, "other-case").is_err());
        let schema = schema("LOCAL_CAMPAIGN_OBSERVE").unwrap();
        assert!(schema["properties"]["results"]["items"]["properties"]
            .get("observed_cents")
            .is_none());
    }
    #[test]
    fn interpreter_never_repairs_a_models_wrong_policy_or_executes_code() {
        let rules = json!([
            {"when":"currency_mismatch","status":"matched"},
            {"when":"duplicate_ids","status":"duplicate"},
            {"when":"empty","status":"missing"},
            {"when":"zero_difference","status":"matched"},
            {"when":"always","status":"mismatch"}
        ]);
        let row = json!({"currency_mismatch":true,"duplicate_ids":false,"empty":false,"difference_cents":0});
        // A wrong model-authored mapping stays wrong for the independent
        // verifier to reject; the interpreter is not a hidden answer oracle.
        assert_eq!(apply_rules(&rules, &row).unwrap(), "matched");
        let mut injected = rules.clone();
        injected[0]["when"] = json!("run_shell");
        assert!(apply_rules(&injected, &row).is_err());
        assert!(apply_rules(&rules, &json!({"currency_mismatch":"true"})).is_err());
        assert!(
            schema("LOCAL_CAMPAIGN_OBSERVE").unwrap()["properties"]["results"]["items"]
                ["properties"]
                .get("status")
                .is_none()
        );
    }
    #[test]
    fn deterministic_check_rejects_invented_results_and_duplicate_coverage() {
        let c:Case=serde_json::from_value(json!({"case_id":"c1","expected_cents":100,"currency":"USD","payments":[{"id":"p1","cents":40,"currency":"USD"},{"id":"p2","cents":60,"currency":"USD"}]})).unwrap();
        let mut w = Work {
            results: vec![expected(&c)],
            summary: String::new(),
        };
        verify(std::slice::from_ref(&c), &w).unwrap();
        w.results[0].difference_cents = 1;
        assert!(verify(std::slice::from_ref(&c), &w).is_err());
        w.results = vec![expected(&c), expected(&c)];
        assert!(verify(&[c], &w).is_err());
    }
    #[test]
    fn observed_overlap_requires_distinct_owned_backends() {
        fn r(pid: u32, port: u16, start: u64, end: u64) -> Reply {
            Reply {
                text: String::new(),
                usage: Default::default(),
                latency_ms: 0,
                response_id: None,
                local_execution: Some(LocalExecution {
                    finish_reason: None,
                    task_input_sha256: None,
                    backend_pid: pid,
                    port,
                    dispatched_at_ms: start,
                    received_at_ms: end,
                    resident_kib: None,
                    launch_policy: None,
                    observed_context_tokens: None,
                    generation_budget: None,
                    separated_reasoning_observed: None,
                }),
            }
        }
        assert_eq!(overlap(&[r(1, 1, 10, 30)], &[r(2, 2, 20, 40)]), 10);
        assert_eq!(overlap(&[r(1, 1, 10, 30)], &[r(1, 1, 20, 40)]), 0);
        assert_eq!(overlap(&[r(1, 1, 10, 20)], &[r(2, 2, 21, 40)]), 0);
    }
}

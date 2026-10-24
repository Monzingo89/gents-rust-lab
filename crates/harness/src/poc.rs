//! Local, tool-using expense CLI proof of concept.
use crate::{emit, judges::Outcome, model_call, now_ms, provider::Model, App, Config};
use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{io::AsyncWriteExt, process::Command};
use uuid::Uuid;

const TASK: &str = "Build a Rust CLI that reads a JSON array of expenses and prints the total amount per category, sorted alphabetically.";
const INPUT_CONTRACT: &str = "Read the complete JSON array from standard input. Each record must contain a nonempty string category and a nonnegative integer amount_cents. On success print one `Category: cents` line per category in alphabetical order; an empty array prints nothing. Errors go to stderr and exit nonzero.";
const CRITERIA: [&str; 6] = [
    "Repeated categories are summed correctly using integer cents.",
    "Categories are printed in alphabetical order.",
    "Empty input is handled.",
    "Malformed JSON produces a clear error and nonzero exit status.",
    "Invalid expense records produce a clear error and nonzero exit status.",
    "The project compiles and relevant functional tests pass.",
];
const POC_CARGO_TOML: &str = "[package]\nname = \"expense-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\nserde_json = \"1\"\n\n[workspace]\n";
const POC_MAIN_RS: &str = "fn main() {}\n";

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PocConfig {
    pub router: Model,
    pub planner_judge: Model,
    pub coding_worker: Model,
    #[serde(default = "default_workspace")]
    pub workspace_root: String,
    #[serde(default = "default_repairs")]
    pub max_repairs: usize,
    #[serde(default = "default_tool_calls")]
    pub max_tool_calls: usize,
    #[serde(default)]
    pub onboarding_documents: Vec<String>,
    #[serde(default = "default_memory_root")]
    pub memory_root: String,
}

fn default_workspace() -> String {
    "data/poc-runs".into()
}
fn default_repairs() -> usize {
    2
}
fn default_tool_calls() -> usize {
    12
}
fn default_memory_root() -> String {
    "data/rag-quick-cache".into()
}

impl PocConfig {
    pub fn validate(&self, config: &Config) -> Result<()> {
        if config.agents.len() != 2
            || config.agents[0].id != "Builder Gent"
            || config.agents[1].id != "Verifier Gent"
        {
            bail!("POC mode requires Builder Gent then Verifier Gent");
        }
        if config.judges.is_some() {
            bail!("POC mode owns its router and judge; remove judges");
        }
        if self.max_repairs > 2 || !(4..=24).contains(&self.max_tool_calls) {
            bail!("POC max_repairs must be 0..2 and max_tool_calls 4..24");
        }
        for model in [&self.router, &self.planner_judge, &self.coding_worker] {
            if model.provider != "local" {
                bail!("POC inference must use only local models");
            }
            model.validate()?;
            let configured = model.path.as_deref().context("local model path missing")?;
            let candidate = std::env::current_dir()?.join(configured);
            let metadata = candidate.metadata().with_context(|| {
                format!("required local model is missing: {}", candidate.display())
            })?;
            if !metadata.is_file() || metadata.len() == 0 {
                bail!(
                    "required local model is incomplete: {}",
                    candidate.display()
                );
            }
        }
        if Path::new(&self.workspace_root).is_absolute()
            || self.workspace_root.split('/').any(|v| v == "..")
        {
            bail!("POC workspace_root must be repository-relative");
        }
        if self.onboarding_documents.is_empty() || self.onboarding_documents.len() > 8 {
            bail!("POC onboarding_documents must contain 1..8 company documents");
        }
        let root = std::env::current_dir()?.canonicalize()?;
        for document in &self.onboarding_documents {
            let path = Path::new(document);
            if path.is_absolute()
                || path
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
                || !document.starts_with("docs/company/")
                || !(document.ends_with(".md") || document.ends_with(".txt"))
            {
                bail!("onboarding documents must be .md/.txt files under docs/company");
            }
            let metadata = root
                .join(path)
                .metadata()
                .with_context(|| format!("required onboarding document is missing: {document}"))?;
            if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 64 * 1024 {
                bail!(
                    "onboarding document must be a nonempty file no larger than 64 KiB: {document}"
                );
            }
        }
        if Path::new(&self.memory_root).is_absolute()
            || self.memory_root.split('/').any(|v| v == "..")
            || !self.memory_root.starts_with("data/")
        {
            bail!("POC memory_root must be repository-relative under data/");
        }
        Ok(())
    }

    pub fn public_info(&self) -> Value {
        json!({
            "enabled": true,
            "task": TASK,
            "input_output_contract": INPUT_CONTRACT,
            "criteria": CRITERIA,
            "workspace_root": self.workspace_root,
            "max_repairs": self.max_repairs,
            "onboarding_documents": self.onboarding_documents,
            "memory_root": self.memory_root,
            "dream_mode": "local planner model after active work",
            "router_model": self.router.model,
            "planner_model": self.planner_judge.model,
            "worker_model": self.coding_worker.model,
            "local_only": true
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct KnowledgeChunk {
    id: String,
    source: String,
    text: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DreamMemory {
    summary: String,
    critical_documents: Vec<String>,
    lessons: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DreamRecord {
    run_id: Uuid,
    timestamp_ms: u64,
    agent: String,
    summary: String,
    critical_documents: Vec<KnowledgeChunk>,
    lessons: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RouteDecision {
    summary: String,
    assignments: Vec<Assignment>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Assignment {
    agent: String,
    planner_model: String,
    worker_model: String,
    responsibility: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    summary: String,
    steps: Vec<PlanStep>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PlanStep {
    id: String,
    objective: String,
    depends_on: Vec<String>,
    acceptance: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolRequest {
    action: String,
    path: Option<String>,
    content: Option<String>,
    stdin: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JudgeVerdict {
    verdict: String,
    summary: String,
    checks: Vec<JudgeCheck>,
    remaining_issues: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct JudgeCheck {
    criterion: String,
    passed: bool,
    evidence: String,
}

#[derive(Debug, Clone, Serialize)]
struct ToolResult {
    action: String,
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

struct WorkerRun<'a> {
    app: &'a App,
    id: Uuid,
    agent: &'a str,
    model: &'a Model,
    workspace: &'a Path,
    writable: bool,
    limit: usize,
    company_context: &'a Value,
    cancel: &'a AtomicBool,
}

struct PlanRun<'a> {
    app: &'a App,
    id: Uuid,
    agent: &'a str,
    model: &'a Model,
    responsibility: &'a str,
    company_context: &'a Value,
    cancel: &'a AtomicBool,
}

fn knowledge_chunks(poc: &PocConfig) -> Result<Vec<KnowledgeChunk>> {
    let root = std::env::current_dir()?.canonicalize()?;
    let mut chunks = Vec::new();
    for source in &poc.onboarding_documents {
        let text = std::fs::read_to_string(root.join(source))?;
        for (index, paragraph) in text
            .split("\n\n")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .enumerate()
        {
            let text: String = paragraph.chars().take(1800).collect();
            let digest = format!(
                "{:x}",
                Sha256::digest(format!("{source}:{index}:{text}").as_bytes())
            );
            chunks.push(KnowledgeChunk {
                id: format!("{}-{}", &digest[..12], index + 1),
                source: source.clone(),
                text,
            });
        }
    }
    if chunks.is_empty() {
        bail!("company onboarding produced no retrievable document chunks");
    }
    Ok(chunks)
}

fn terms(value: &str) -> HashSet<String> {
    value
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() > 2)
        .collect()
}

fn retrieve(chunks: &[KnowledgeChunk], query: &str, limit: usize) -> Vec<KnowledgeChunk> {
    let query_terms = terms(query);
    let mut ranked: Vec<_> = chunks
        .iter()
        .map(|chunk| {
            let score = terms(&chunk.text).intersection(&query_terms).count();
            (score, chunk)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    ranked
        .into_iter()
        .filter(|(score, _)| *score > 0)
        .take(limit)
        .map(|(_, chunk)| chunk.clone())
        .collect()
}

async fn onboard(
    app: &App,
    id: Uuid,
    poc: &PocConfig,
    agent: &str,
    responsibility: &str,
    chunks: &[KnowledgeChunk],
) -> Result<Value> {
    emit(
        app,
        id,
        (agent, "onboarding"),
        ("library", "onboarding_started"),
        "Receiving company-specific documentation before work is allowed",
        None,
        None,
    )
    .await?;
    let selected = retrieve(
        chunks,
        &format!("{TASK} {INPUT_CONTRACT} {responsibility}"),
        6,
    );
    if selected.is_empty() {
        bail!("onboarding retrieval returned no relevant company documentation for {agent}");
    }
    let memory_cutoff = now_ms().saturating_sub(24 * 60 * 60 * 1000);
    let mut quick_cache: Vec<_> = load_dream_records(poc, agent)?
        .into_iter()
        .filter(|record| record.timestamp_ms >= memory_cutoff)
        .map(|record| {
            json!({
                "run_id":record.run_id,
                "summary":record.summary,
                "lessons":record.lessons,
                "critical_documents":record.critical_documents
            })
        })
        .collect();
    if quick_cache.len() > 6 {
        quick_cache.drain(..quick_cache.len() - 6);
    }
    let sources: Vec<_> = selected
        .iter()
        .map(|chunk| json!({"id":chunk.id,"source":chunk.source}))
        .collect();
    emit(
        app,
        id,
        (agent, "onboarding"),
        ("library", "onboarding_completed"),
        &json!({"agent":agent,"retrieved":sources,"quick_cache_records":quick_cache.len(),"gate":"passed"}).to_string(),
        None,
        None,
    )
    .await?;
    Ok(json!({"company":"Gents Office POC","retrieved_chunks":selected,"quick_cache":quick_cache}))
}

pub async fn execute(
    app: &App,
    id: Uuid,
    requested_task: &str,
    cancel: &AtomicBool,
) -> Result<Outcome> {
    let poc = app
        .config
        .poc
        .as_ref()
        .context("POC configuration missing")?;
    if requested_task.trim() != TASK {
        bail!("this proof of concept accepts only the fixed expense CLI task shown in the UI");
    }
    let workspace = prepare_workspace(poc, id)?;
    emit(
        app,
        id,
        ("harness", "system"),
        ("planning", "workspace_ready"),
        &json!({"task":TASK,"run_id":id,"artifact":workspace}).to_string(),
        None,
        None,
    )
    .await?;

    let knowledge = knowledge_chunks(poc)?;
    let builder_context = onboard(
        app,
        id,
        poc,
        "Builder Gent",
        "Implement the expense CLI according to company policy and engineering standards.",
        &knowledge,
    )
    .await?;
    let verifier_context = onboard(
        app,
        id,
        poc,
        "Verifier Gent",
        "Independently verify the expense CLI against company policy and release standards.",
        &knowledge,
    )
    .await?;

    let route_input = json!({
        "task": TASK,
        "input_output_contract": INPUT_CONTRACT,
        "agents": [
            {"id":"Builder Gent","roles":["planner","worker"],"responsibility":"implement the CLI"},
            {"id":"Verifier Gent","roles":["planner","worker"],"responsibility":"independently verify the artifact"}
        ],
        "models": [
            {"id":"planner-judge","roles":["planner","judge"]},
            {"id":"coding-worker","roles":["worker"]}
        ],
        "tools":["write_file","read_file","list_files","cargo_check","cargo_test","run_cli"],
        "company_knowledge":{"documents":poc.onboarding_documents,"onboarding_required":true,"retrieval":"local lexical quick cache"},
        "constraints":{"local_only":true,"one_loaded_model":true,"max_repairs":poc.max_repairs}
    });
    let route: RouteDecision = structured_call(
        app,
        id,
        ("CTO", "router"),
        &poc.router,
        ROUTER_PROMPT,
        route_input,
        cancel,
    )
    .await?;
    validate_route(&route)?;
    emit(
        app,
        id,
        ("CTO", "router"),
        ("routing", "route_accepted"),
        &serde_json::to_string(&route)?,
        None,
        None,
    )
    .await?;

    let builder_plan = plan(
        PlanRun {
            app,
            id,
            agent: "Builder Gent",
            model: &poc.planner_judge,
            responsibility: "Plan implementation and concrete tests.",
            company_context: &builder_context,
            cancel,
        },
        Value::Null,
    )
    .await?;
    let verifier_plan = plan(
        PlanRun {
            app,
            id,
            agent: "Verifier Gent",
            model: &poc.planner_judge,
            responsibility:
                "Plan independent black-box verification. Do not trust Builder-authored tests.",
            company_context: &verifier_context,
            cancel,
        },
        Value::Null,
    )
    .await?;

    let mut evidence = Value::Null;
    for round in 0..=poc.max_repairs {
        let state = if round == 0 { "working" } else { "repairing" };
        emit(
            app,
            id,
            ("Builder Gent", "worker"),
            ("workshop", state),
            &format!(
                "{} cycle {round}",
                if round == 0 {
                    "Implementation"
                } else {
                    "Repair"
                }
            ),
            Some(poc.coding_worker.model.clone()),
            None,
        )
        .await?;
        run_worker(
            WorkerRun {
                app,
                id,
                agent: "Builder Gent",
                model: &poc.coding_worker,
                workspace: &workspace,
                writable: true,
                limit: poc.max_tool_calls,
                company_context: &builder_context,
                cancel,
            },
            &builder_plan,
            &evidence,
        )
        .await?;

        emit(
            app,
            id,
            ("Verifier Gent", "worker"),
            ("review", "testing"),
            "Running independent verification",
            Some(poc.coding_worker.model.clone()),
            None,
        )
        .await?;
        run_worker(
            WorkerRun {
                app,
                id,
                agent: "Verifier Gent",
                model: &poc.coding_worker,
                workspace: &workspace,
                writable: false,
                limit: poc.max_tool_calls.min(8),
                company_context: &verifier_context,
                cancel,
            },
            &verifier_plan,
            &evidence,
        )
        .await?;
        let verification = deterministic_verification(&workspace, cancel).await;
        for result in &verification {
            emit(
                app,
                id,
                ("Verifier Gent", "worker"),
                ("review", "tool_result"),
                &serde_json::to_string(result)?,
                None,
                None,
            )
            .await?;
        }
        let hard_pass = verification.iter().all(|v| v.success);
        evidence =
            json!({"round":round,"artifact":workspace,"deterministic_verification":verification});

        let judge_input = json!({"task":TASK,"acceptance_criteria":CRITERIA,"artifact":workspace,"actual_verification_evidence":evidence,"instructions":"Use only recorded evidence. A worker claim is not test evidence."});
        let verdict: JudgeVerdict = structured_call(
            app,
            id,
            ("Compliance Security & QA Engineer", "judge"),
            &poc.planner_judge,
            JUDGE_PROMPT,
            judge_input,
            cancel,
        )
        .await?;
        validate_verdict(&verdict)?;
        emit(
            app,
            id,
            ("Compliance Security & QA Engineer", "judge"),
            ("judging", "completion_review"),
            &serde_json::to_string(&verdict)?,
            None,
            None,
        )
        .await?;

        if hard_pass && verdict.verdict == "pass" {
            let outcome = Outcome {
                status: "completed",
                output: format!(
                    "Expense CLI completed and independently verified. Artifact: {}",
                    workspace.display()
                ),
            };
            dream_cycle(app, id, poc, &knowledge, &evidence, &outcome, cancel).await?;
            return Ok(outcome);
        }
        if round == poc.max_repairs {
            let outcome = Outcome {
                status: "blocked",
                output: format!(
                    "Repair limit reached. Artifact: {}\nEvidence: {}\nJudge: {}",
                    workspace.display(),
                    evidence,
                    serde_json::to_string(&verdict)?
                ),
            };
            dream_cycle(app, id, poc, &knowledge, &evidence, &outcome, cancel).await?;
            return Ok(outcome);
        }
        emit(
            app,
            id,
            ("harness", "system"),
            ("workshop", "repairing"),
            &json!({"next_cycle":round+1,"mandatory_checks_passed":hard_pass,"judge":verdict})
                .to_string(),
            None,
            None,
        )
        .await?;
    }
    unreachable!()
}

async fn plan(run: PlanRun<'_>, evidence: Value) -> Result<Plan> {
    let PlanRun {
        app,
        id,
        agent,
        model,
        responsibility,
        company_context,
        cancel,
    } = run;
    let original = json!({"task":TASK,"input_output_contract":INPUT_CONTRACT,"acceptance_criteria":CRITERIA,"responsibility":responsibility,"company_context":company_context,"available_tools":["write_file","read_file","list_files","cargo_check","cargo_test","run_cli"],"prior_evidence":evidence});
    let mut input = original.clone();
    for attempt in 1..=2 {
        let plan: Plan = structured_call(
            app,
            id,
            (agent, "planner"),
            model,
            PLANNER_PROMPT,
            input,
            cancel,
        )
        .await?;
        match validate_plan(&plan) {
            Ok(()) => {
                emit(
                    app,
                    id,
                    (agent, "planner"),
                    ("library", "plan_accepted"),
                    &serde_json::to_string(&plan)?,
                    None,
                    None,
                )
                .await?;
                return Ok(plan);
            }
            Err(error) if attempt < 2 => {
                emit(
                    app,
                    id,
                    (agent, "planner"),
                    ("review", "semantic_retry"),
                    &format!("Plan rejected: {error}; retrying once"),
                    None,
                    None,
                )
                .await?;
                input = json!({"original_input":original,"validation_error":error.to_string(),"invalid_plan":plan,"instruction":"Return a corrected plan. Every dependency must reference a unique earlier step ID."});
            }
            Err(error) => bail!("persistent invalid plan after two attempts: {error}"),
        }
    }
    unreachable!()
}

async fn structured_call<T: DeserializeOwned>(
    app: &App,
    id: Uuid,
    actor: (&str, &str),
    model: &Model,
    system: &str,
    input: Value,
    cancel: &AtomicBool,
) -> Result<T> {
    let mut prompt = serde_json::to_string(&input)?;
    for attempt in 1..=2 {
        let reply = model_call(app, id, actor, model, (system, &prompt), cancel).await?;
        let last = reply.text;
        match parse_strict(&last) {
            Ok(value) => return Ok(value),
            Err(error) if attempt < 2 => {
                emit(app, id, actor, ("review", "structured_retry"), &format!("Structured output rejected: {error}; retrying once"), None, None).await?;
                prompt = serde_json::to_string(&json!({"original_input":input,"validation_error":error.to_string(),"invalid_output":clip(&last,4000),"instruction":"Return one bare JSON object matching the schema, without reasoning prefixes or Markdown."}))?;
            }
            Err(error) => bail!("persistent invalid structured output after two attempts: {error}; output prefix: {}", clip(&last, 500)),
        }
    }
    unreachable!()
}

async fn dream_cycle(
    app: &App,
    id: Uuid,
    poc: &PocConfig,
    knowledge: &[KnowledgeChunk],
    evidence: &Value,
    outcome: &Outcome,
    cancel: &AtomicBool,
) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(());
    }
    for agent in ["Builder Gent", "Verifier Gent"] {
        emit(
            app,
            id,
            (agent, "dreamer"),
            ("sleep", "sleep_started"),
            "Active work ended; consolidating observable work into the local RAG quick cache",
            None,
            None,
        )
        .await?;
        let activity = {
            let runs = app.runs.lock().await;
            runs.get(&id)
                .map(|run| {
                    run.events
                        .iter()
                        .filter(|event| event.agent == agent)
                        .map(|event| {
                            json!({
                                "role":event.role,
                                "kind":event.kind,
                                "message":clip(&event.message, 1800),
                                "model":event.model
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let relevant = retrieve(
            knowledge,
            &format!("{TASK} {} {evidence}", outcome.output),
            6,
        );
        let day_cutoff = now_ms().saturating_sub(24 * 60 * 60 * 1000);
        let prior_daily_memory: Vec<_> = load_dream_records(poc, agent)?
            .into_iter()
            .filter(|record| record.timestamp_ms >= day_cutoff)
            .map(|record| {
                json!({
                    "run_id":record.run_id,
                    "summary":record.summary,
                    "lessons":record.lessons,
                    "critical_documents":record.critical_documents.iter().map(|chunk|&chunk.id).collect::<Vec<_>>()
                })
            })
            .collect();
        let allowed: HashSet<_> = relevant.iter().map(|chunk| chunk.id.clone()).collect();
        let input = json!({
            "agent":agent,
            "task":TASK,
            "terminal_status":outcome.status,
            "public_outcome":outcome.output,
            "observable_activity":activity,
            "prior_memory_from_rolling_24_hours":prior_daily_memory,
            "deterministic_evidence":evidence,
            "retrieved_company_documents":relevant,
            "retention_rules":["retain only reusable business facts, public summaries, and tool evidence","never infer or store private chain-of-thought","critical_documents must contain only supplied chunk IDs","do not store credentials or environment values"]
        });
        let memory = match structured_call::<DreamMemory>(
            app,
            id,
            (agent, "dreamer"),
            &poc.planner_judge,
            DREAM_PROMPT,
            input,
            cancel,
        )
        .await
        {
            Ok(memory)
                if !memory.summary.trim().is_empty()
                    && memory.lessons.len() <= 8
                    && memory
                        .lessons
                        .iter()
                        .all(|lesson| !lesson.trim().is_empty())
                    && memory
                        .critical_documents
                        .iter()
                        .all(|document| allowed.contains(document)) =>
            {
                memory
            }
            Ok(_) | Err(_) => {
                emit(
                    app,
                    id,
                    (agent, "dreamer"),
                    ("sleep", "dream_fallback"),
                    "Dream-model output was unavailable or invalid; stored a deterministic evidence summary instead",
                    None,
                    None,
                )
                .await?;
                DreamMemory {
                    summary: format!(
                        "{} ended {}. Deterministic evidence and company-document references were retained for later retrieval.",
                        agent, outcome.status
                    ),
                    critical_documents: relevant
                        .iter()
                        .take(4)
                        .map(|chunk| chunk.id.clone())
                        .collect(),
                    lessons: vec![
                        "Use recorded tool evidence rather than worker claims.".into(),
                        "Reapply the retrieved company contract before planning future expense work.".into(),
                    ],
                }
            }
        };
        let critical_documents = relevant
            .into_iter()
            .filter(|chunk| memory.critical_documents.contains(&chunk.id))
            .collect();
        let record = DreamRecord {
            run_id: id,
            timestamp_ms: now_ms(),
            agent: agent.into(),
            summary: memory.summary,
            critical_documents,
            lessons: memory.lessons,
        };
        persist_dream(poc, &record)?;
        emit(
            app,
            id,
            (agent, "dreamer"),
            ("sleep", "dream_completed"),
            &json!({"cache":format!("{}/{}.json",poc.memory_root,agent_slug(agent)),"critical_documents":record.critical_documents.iter().map(|chunk|&chunk.id).collect::<Vec<_>>(),"lessons":record.lessons.len()}).to_string(),
            None,
            None,
        )
        .await?;
    }
    Ok(())
}

fn agent_slug(agent: &str) -> String {
    agent
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn persist_dream(poc: &PocConfig, record: &DreamRecord) -> Result<()> {
    let root = std::env::current_dir()?
        .canonicalize()?
        .join(&poc.memory_root);
    std::fs::create_dir_all(&root)?;
    let path = root.join(format!("{}.json", agent_slug(&record.agent)));
    let mut records = load_dream_records(poc, &record.agent)?;
    records.retain(|existing| existing.run_id != record.run_id);
    records.push(DreamRecord {
        run_id: record.run_id,
        timestamp_ms: record.timestamp_ms,
        agent: record.agent.clone(),
        summary: record.summary.clone(),
        critical_documents: record.critical_documents.clone(),
        lessons: record.lessons.clone(),
    });
    if records.len() > 20 {
        records.drain(..records.len() - 20);
    }
    let temporary = root.join(format!(
        ".{}.{}.tmp",
        agent_slug(&record.agent),
        Uuid::new_v4()
    ));
    std::fs::write(&temporary, serde_json::to_vec_pretty(&records)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn load_dream_records(poc: &PocConfig, agent: &str) -> Result<Vec<DreamRecord>> {
    let path = std::env::current_dir()?
        .canonicalize()?
        .join(&poc.memory_root)
        .join(format!("{}.json", agent_slug(agent)));
    if !path.is_file() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&std::fs::read_to_string(path)?)
        .context("existing dream cache is invalid JSON")
}

fn parse_strict<T: DeserializeOwned>(text: &str) -> Result<T> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        bail!("response was reasoning-prefixed, fenced, or truncated rather than bare JSON");
    }
    serde_json::from_str(trimmed).context("response did not match the required schema")
}

fn validate_route(route: &RouteDecision) -> Result<()> {
    if route.summary.trim().is_empty() || route.assignments.len() != 2 {
        bail!("router must assign exactly two Gents");
    }
    for (index, expected) in ["Builder Gent", "Verifier Gent"].iter().enumerate() {
        let assignment = &route.assignments[index];
        if assignment.agent != *expected
            || assignment.planner_model != "planner-judge"
            || assignment.worker_model != "coding-worker"
            || assignment.responsibility.trim().is_empty()
        {
            bail!("router assignment violated the agent/model allowlist");
        }
    }
    Ok(())
}

fn validate_plan(plan: &Plan) -> Result<()> {
    if plan.summary.trim().is_empty() || plan.steps.is_empty() || plan.steps.len() > 8 {
        bail!("plan requires a summary and 1..8 steps");
    }
    let mut ids = std::collections::HashSet::new();
    for step in &plan.steps {
        if step.id.trim().is_empty()
            || step.objective.trim().is_empty()
            || step.acceptance.is_empty()
            || !ids.insert(step.id.clone())
        {
            bail!("plan contains invalid or duplicate steps");
        }
        if step
            .depends_on
            .iter()
            .any(|dependency| !ids.contains(dependency))
        {
            bail!("plan dependency must reference an earlier step");
        }
    }
    Ok(())
}

fn validate_verdict(verdict: &JudgeVerdict) -> Result<()> {
    if !["pass", "revise", "uncertain"].contains(&verdict.verdict.as_str())
        || verdict.summary.trim().is_empty()
        || verdict.checks.len() != CRITERIA.len()
    {
        bail!("judge verdict is incomplete");
    }
    for (index, check) in verdict.checks.iter().enumerate() {
        if check.criterion != CRITERIA[index] || check.evidence.trim().is_empty() {
            bail!("judge must preserve and evidence every criterion in order");
        }
    }
    if verdict.verdict == "pass"
        && (verdict.checks.iter().any(|c| !c.passed) || !verdict.remaining_issues.is_empty())
    {
        bail!("judge cannot pass failed criteria or remaining issues");
    }
    Ok(())
}

fn prepare_workspace(poc: &PocConfig, id: Uuid) -> Result<PathBuf> {
    let root = std::env::current_dir()?.canonicalize()?;
    let workspace = root
        .join(&poc.workspace_root)
        .join(id.to_string())
        .join("expense-cli");
    std::fs::create_dir_all(workspace.join("src"))?;
    std::fs::create_dir_all(workspace.join("tests"))?;
    std::fs::write(workspace.join("Cargo.toml"), POC_CARGO_TOML)?;
    std::fs::write(workspace.join("src/main.rs"), POC_MAIN_RS)?;
    Ok(workspace)
}

async fn run_worker(run: WorkerRun<'_>, plan: &Plan, evidence: &Value) -> Result<()> {
    let WorkerRun {
        app,
        id,
        agent,
        model,
        workspace,
        writable,
        limit,
        company_context,
        cancel,
    } = run;
    let mut history: Vec<Value> = vec![
        json!({"task":TASK,"input_output_contract":INPUT_CONTRACT,"acceptance_criteria":CRITERIA,"company_context":company_context,"plan":plan,"workspace_rules":{"writable":writable,"allowed_write_paths":["src/main.rs","tests/*.rs"],"commands":["cargo_check","cargo_test","run_cli"]},"prior_evidence":evidence}),
    ];
    for call_index in 0..limit {
        let input = json!({"call_index":call_index,"history":history});
        let system = if writable {
            BUILDER_PROMPT
        } else {
            VERIFIER_PROMPT
        };
        let request: ToolRequest =
            structured_call(app, id, (agent, "worker"), model, system, input, cancel).await?;
        validate_tool_request(&request, writable)?;
        if request.action == "finish" {
            let summary = request
                .summary
                .as_deref()
                .context("finish requires summary")?;
            emit(
                app,
                id,
                (agent, "worker"),
                ("review", "agent_completed"),
                summary,
                None,
                None,
            )
            .await?;
            return Ok(());
        }
        let result = execute_tool(workspace, &request, cancel).await;
        emit(
            app,
            id,
            (agent, "worker"),
            (
                if request.action.starts_with("cargo") || request.action == "run_cli" {
                    "review"
                } else {
                    "workshop"
                },
                "tool_result",
            ),
            &serde_json::to_string(&result)?,
            None,
            None,
        )
        .await?;
        history.push(json!({"request":request,"actual_result":result}));
        if history.len() > 10 {
            history.drain(1..history.len() - 8);
        }
    }
    if writable && std::fs::read_to_string(workspace.join("src/main.rs"))? == POC_MAIN_RS {
        bail!("{agent} exhausted the tool-call limit without replacing the placeholder source");
    }
    emit(
        app,
        id,
        (agent, "worker"),
        ("review", "tool_limit_handoff"),
        "Tool-call limit reached; handing the current artifact to deterministic verification without claiming success",
        None,
        None,
    )
    .await?;
    Ok(())
}

fn validate_tool_request(request: &ToolRequest, writable: bool) -> Result<()> {
    match request.action.as_str() {
        "write_file" if writable => {
            request
                .path
                .as_deref()
                .context("write_file requires path")?;
            request
                .content
                .as_deref()
                .context("write_file requires content")?;
        }
        "write_file" => bail!("Verifier Gent has read-only workspace access"),
        "read_file" => {
            request.path.as_deref().context("read_file requires path")?;
        }
        "list_files" | "cargo_check" | "cargo_test" => {}
        "run_cli" => {
            request.stdin.as_deref().context("run_cli requires stdin")?;
        }
        "finish" => {
            request
                .summary
                .as_deref()
                .context("finish requires summary")?;
        }
        _ => bail!("tool action is not allowlisted"),
    }
    Ok(())
}

async fn execute_tool(workspace: &Path, request: &ToolRequest, cancel: &AtomicBool) -> ToolResult {
    let result = match request.action.as_str() {
        "write_file" => write_file(
            workspace,
            request.path.as_deref().unwrap(),
            request.content.as_deref().unwrap(),
        ),
        "read_file" => read_file(workspace, request.path.as_deref().unwrap()),
        "list_files" => list_files(workspace),
        "cargo_check" => run_process(workspace, &["check", "--offline"], None, cancel).await,
        "cargo_test" => run_process(workspace, &["test", "--offline"], None, cancel).await,
        "run_cli" => {
            run_process(
                workspace,
                &["run", "--quiet", "--offline", "--"],
                request.stdin.as_deref(),
                cancel,
            )
            .await
        }
        _ => Err(anyhow::anyhow!("unsupported action")),
    };
    match result {
        Ok(mut value) => {
            value.action = request.action.clone();
            value
        }
        Err(error) => ToolResult {
            action: request.action.clone(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: error.to_string(),
        },
    }
}

fn safe_path(workspace: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("path must be a simple relative workspace path");
    }
    let allowed =
        relative == "src/main.rs" || (relative.starts_with("tests/") && relative.ends_with(".rs"));
    if !allowed {
        if relative == "Cargo.toml" {
            bail!("Cargo.toml is preconfigured and immutable; write src/main.rs or tests/*.rs instead");
        }
        bail!("path is outside the writable/readable POC source allowlist");
    }
    Ok(workspace.join(path))
}

fn write_file(workspace: &Path, relative: &str, content: &str) -> Result<ToolResult> {
    if content.len() > 128 * 1024 {
        bail!("file content exceeds 128 KiB");
    }
    let path = safe_path(workspace, relative)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(ToolResult {
        action: String::new(),
        success: true,
        exit_code: None,
        stdout: format!("wrote {} bytes to {relative}", content.len()),
        stderr: String::new(),
    })
}

fn read_file(workspace: &Path, relative: &str) -> Result<ToolResult> {
    let path = safe_path(workspace, relative)?;
    let content = std::fs::read_to_string(&path)?;
    Ok(ToolResult {
        action: String::new(),
        success: true,
        exit_code: None,
        stdout: clip(&content, 64 * 1024),
        stderr: String::new(),
    })
}

fn list_files(workspace: &Path) -> Result<ToolResult> {
    let mut files = vec!["Cargo.toml".to_string()];
    for folder in ["src", "tests"] {
        let dir = workspace.join(folder);
        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    files.push(format!("{folder}/{}", entry.file_name().to_string_lossy()));
                }
            }
        }
    }
    files.sort();
    Ok(ToolResult {
        action: String::new(),
        success: true,
        exit_code: None,
        stdout: files.join("\n"),
        stderr: String::new(),
    })
}

async fn run_process(
    workspace: &Path,
    args: &[&str],
    stdin: Option<&str>,
    cancel: &AtomicBool,
) -> Result<ToolResult> {
    let sandbox = Path::new("/usr/bin/sandbox-exec");
    if !sandbox.is_file() {
        bail!("macOS sandbox-exec is required for generated-code execution");
    }
    let cargo = cargo_executable()?;
    let profile = sandbox_profile(workspace)?;
    let rustup_home = tool_home("RUSTUP_HOME", ".rustup");
    let cargo_home = tool_home("CARGO_HOME", ".cargo");
    std::fs::create_dir_all(workspace.join(".home"))?;
    std::fs::create_dir_all(workspace.join(".tmp"))?;
    let mut command = Command::new(sandbox);
    command
        .args(["-p", &profile])
        .arg(cargo)
        .args(args)
        .current_dir(workspace)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", workspace.join(".home"))
        .env("TMPDIR", workspace.join(".tmp"))
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_NET_OFFLINE", "true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().context("start allowlisted cargo command")?;
    if let Some(input) = stdin {
        if let Some(mut pipe) = child.stdin.take() {
            pipe.write_all(input.as_bytes()).await?;
        }
    }
    drop(child.stdin.take());
    let wait = child.wait_with_output();
    tokio::pin!(wait);
    let timeout = tokio::time::sleep(Duration::from_secs(120));
    tokio::pin!(timeout);
    let output = loop {
        tokio::select! {
            result = &mut wait => break result?,
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if cancel.load(Ordering::Relaxed) { bail!("tool process cancelled"); }
            }
            _ = &mut timeout => { bail!("tool process exceeded 120 seconds"); }
        }
    };
    let mut stderr = clip(&String::from_utf8_lossy(&output.stderr), 64 * 1024);
    #[cfg(unix)]
    if let Some(signal) = std::os::unix::process::ExitStatusExt::signal(&output.status) {
        stderr.push_str(&format!("\nprocess terminated by signal {signal}"));
    }
    Ok(ToolResult {
        action: String::new(),
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: clip(&String::from_utf8_lossy(&output.stdout), 64 * 1024),
        stderr,
    })
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(name))
            .find(|path| path.is_file())
    })
}

fn cargo_executable() -> Result<PathBuf> {
    if let Some(configured) = std::env::var_os("CARGO_BIN").map(PathBuf::from) {
        if configured.is_file() {
            return Ok(configured);
        }
        bail!("CARGO_BIN does not name a file: {}", configured.display());
    }
    if let Some(rustup) = find_on_path("rustup") {
        let output = std::process::Command::new(rustup)
            .args(["which", "cargo"])
            .output()?;
        if output.status.success() {
            let path = PathBuf::from(String::from_utf8(output.stdout)?.trim());
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    find_on_path("cargo").context("cargo executable was not found on PATH")
}

fn sandbox_profile(workspace: &Path) -> Result<String> {
    workspace.canonicalize().context("resolve run workspace")?;
    let root = std::env::current_dir()?.canonicalize()?;
    let mut protected = String::new();
    for directory in [
        ".git",
        ".github",
        ".vscode",
        "assets",
        "config",
        "crates",
        "docs",
        "infra",
        "models",
        "scripts",
        "src-tauri",
        "web",
    ] {
        protected.push_str(&format!(
            " (subpath \"{}\")",
            sandbox_quote(&root.join(directory))
        ));
    }
    for file in [
        ".env",
        ".env.local",
        ".env.example",
        ".gitignore",
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "package-lock.json",
        "README.md",
        "SECURITY.md",
        "START-HERE.html",
    ] {
        protected.push_str(&format!(
            " (literal \"{}\")",
            sandbox_quote(&root.join(file))
        ));
    }
    Ok(format!(
        "(version 1)\n(allow default)\n(deny network*)\n(deny file-write*{protected})\n(deny file-read* (subpath \"{}\") (require-all (subpath \"{}\") (require-not (subpath \"{}\"))))\n(deny file-write* (require-all (subpath \"{}\") (require-not (subpath \"{}\"))))",
        sandbox_quote(&root.join("docs/companies")),
        sandbox_quote(&root.join("data/companies")),
        sandbox_quote(workspace),
        sandbox_quote(&root.join("data/companies")),
        sandbox_quote(workspace)
    ))
}

fn sandbox_quote(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn tool_home(variable: &str, fallback: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(fallback)
        })
}

async fn deterministic_verification(workspace: &Path, cancel: &AtomicBool) -> Vec<ToolResult> {
    let mut results = Vec::new();
    results.push(tag(
        "compile",
        run_process(workspace, &["check", "--offline"], None, cancel).await,
    ));
    results.push(tag(
        "tests",
        run_process(workspace, &["test", "--offline"], None, cancel).await,
    ));
    let cases = [
        (
            "repeated_and_sorted",
            r#"[{"category":"Utilities","amount_cents":2500},{"category":"Food","amount_cents":125},{"category":"Food","amount_cents":75}]"#,
            true,
            "Food: 200\nUtilities: 2500\n",
        ),
        ("empty", "[]", true, ""),
        ("malformed", "not json", false, ""),
        ("missing_amount", r#"[{"category":"Food"}]"#, false, ""),
        (
            "negative_amount",
            r#"[{"category":"Food","amount_cents":-1}]"#,
            false,
            "",
        ),
        (
            "empty_category",
            r#"[{"category":"","amount_cents":10}]"#,
            false,
            "",
        ),
    ];
    for (name, input, should_succeed, expected) in cases {
        let result = run_process(
            workspace,
            &["run", "--quiet", "--offline", "--"],
            Some(input),
            cancel,
        )
        .await;
        let checked = result.map(|mut r| {
            let clear_error = !should_succeed && !r.stderr.trim().is_empty();
            r.success = if should_succeed { r.success && r.stdout == expected } else { !r.success && clear_error };
            if !r.success { r.stderr = format!("independent case {name} failed; expected_success={should_succeed}; expected_stdout={expected:?}; actual_exit={:?}; stderr={}", r.exit_code, clip(&r.stderr, 2000)); }
            r
        });
        results.push(tag(name, checked));
    }
    results
}

fn tag(name: &str, result: Result<ToolResult>) -> ToolResult {
    match result {
        Ok(mut result) => {
            result.action = name.into();
            result
        }
        Err(error) => ToolResult {
            action: name.into(),
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: error.to_string(),
        },
    }
}

fn clip(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

const ROUTER_PROMPT: &str = "POC_CTO: Return only schema-valid JSON. Assign Builder Gent then Verifier Gent. Use only planner-judge for planning and coding-worker for working. Do not invent agents, models, tools, credentials, or endpoints. Provide concise public rationale, never private reasoning.";
const PLANNER_PROMPT: &str = "POC_PLANNER: Return only schema-valid JSON. Produce actionable ordered steps, dependencies on earlier step IDs only, and testable acceptance statements before work begins. Plan only within the supplied task and tools. Never include private reasoning.";
const BUILDER_PROMPT: &str = "POC_WORKER_BUILDER: Return only one schema-valid JSON tool request. Choose exactly one allowlisted action. Cargo.toml is already configured and immutable. The only writable paths are src/main.rs and tests/*.rs. A compilable placeholder src/main.rs already exists; overwrite it with the implementation before running the CLI. The CLI input is one complete JSON array, not newline-delimited JSON objects. Use actual tool results in history to correct work. For write_file provide path and complete content. For run_cli provide only the JSON-array stdin. After successful compile, tests, and representative CLI checks, choose finish instead of repeating an edit or check. For finish provide a concise summary only after the role's work is actually done. Never place a shell command in a field and never claim a tool succeeded without its recorded result.";
const VERIFIER_PROMPT: &str = "POC_WORKER_VERIFIER: Return only one schema-valid JSON tool request. This role is read-only: never choose write_file. Inspect the existing artifact and use read_file, list_files, cargo_check, cargo_test, and run_cli with complete JSON-array stdin. Test success and failure cases independently. After sufficient recorded checks, choose finish instead of repeating a check. Never place a shell command in a field and never claim a tool succeeded without its recorded result.";
const JUDGE_PROMPT: &str = "POC_JUDGE: Start a fresh evaluation and return only schema-valid JSON. Preserve every acceptance criterion verbatim and in order. Use actual verification evidence, not worker claims. Return pass only when every criterion has passing evidence and there are no remaining issues. Use revise for actionable defects and uncertain when evidence is missing. Never provide private reasoning.";
const DREAM_PROMPT: &str = "POC_DREAM: Return only schema-valid JSON with summary, critical_documents, and lessons. Consolidate only supplied observable activity, deterministic evidence, and company-document chunks. critical_documents must contain only supplied chunk IDs. Preserve concise reusable business knowledge, not a transcript. Never infer or reveal private chain-of-thought, credentials, environment values, or unsupported facts. Return at most eight short lessons.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_workspace_scoped() {
        let root = Path::new("/tmp/run");
        assert!(safe_path(root, "src/main.rs").is_ok());
        assert!(safe_path(root, "tests/cli.rs").is_ok());
        assert!(safe_path(root, "Cargo.toml").is_err());
        assert!(safe_path(root, "../main.rs").is_err());
        assert!(safe_path(root, "/tmp/main.rs").is_err());
    }
    #[test]
    fn rejects_reasoning_prefixed_json() {
        assert!(parse_strict::<ToolRequest>("thinking... {\"action\":\"finish\"}").is_err());
    }

    #[test]
    fn generated_manifest_is_isolated_from_the_parent_workspace() {
        assert!(POC_CARGO_TOML.contains("\n[workspace]\n"));
        assert_eq!(POC_MAIN_RS, "fn main() {}\n");
        assert!(BUILDER_PROMPT.starts_with("POC_WORKER"));
        assert!(VERIFIER_PROMPT.starts_with("POC_WORKER"));
    }

    #[test]
    fn onboarding_retrieval_prefers_business_specific_chunks() {
        let chunks = vec![
            KnowledgeChunk {
                id: "expense".into(),
                source: "docs/company/expense.md".into(),
                text: "Repeated expense categories use integer cents and alphabetical output."
                    .into(),
            },
            KnowledgeChunk {
                id: "unrelated".into(),
                source: "docs/company/office.md".into(),
                text: "The office plants are watered each Friday.".into(),
            },
        ];
        let selected = retrieve(&chunks, "expense categories integer cents", 1);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, "expense");
    }

    #[test]
    fn dream_cache_filenames_are_agent_scoped() {
        assert_eq!(agent_slug("Builder Gent"), "builder-gent");
        assert_eq!(agent_slug("Verifier Gent"), "verifier-gent");
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn sandboxed_cargo_check_runs_in_workspace() {
        let root = std::env::temp_dir().join(format!("gents-poc-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='sandbox-check'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        let cancel = AtomicBool::new(false);
        let result = run_process(&root, &["check", "--offline"], None, &cancel)
            .await
            .unwrap();
        assert!(
            result.success,
            "exit={:?} stdout={} stderr={}",
            result.exit_code, result.stdout, result.stderr
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}

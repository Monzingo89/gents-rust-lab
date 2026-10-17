mod acceptance;
mod auth;
mod judges;
mod provider;
mod store;
use anyhow::{bail, Context, Result};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use provider::{Gateway, LanguageModel, Model, Reply};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, services::ServeFile};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Agent {
    id: String,
    assignment: String,
    planner: Model,
    worker: Model,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Config {
    agents: Vec<Agent>,
    max_steps: usize,
    max_tokens: u32,
    #[serde(default)]
    judges: Option<judges::Judges>,
    #[serde(default = "default_run_seconds")]
    max_run_seconds: u64,
}
fn default_run_seconds() -> u64 {
    900
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl Config {
    fn validate(&self) -> Result<()> {
        if self.agents.is_empty() || self.agents.len() > 8 {
            bail!("configure 1..8 agents for this lab");
        }
        if !(1..=3).contains(&self.max_steps) || !(64..=4096).contains(&self.max_tokens) {
            bail!("invalid run limits");
        }
        if !(1..=3600).contains(&self.max_run_seconds) {
            bail!("max_run_seconds must be 1..3600");
        }
        let mut ids = std::collections::HashSet::new();
        for a in &self.agents {
            if a.id.is_empty() || !ids.insert(&a.id) {
                bail!("agent IDs must be unique and nonempty");
            }
            for m in [&a.planner, &a.worker] {
                if !["mock", "openrouter", "huggingface", "databricks", "azure"]
                    .contains(&m.provider.as_str())
                {
                    bail!("unknown provider");
                }
            }
        }
        if let Some(j) = &self.judges {
            j.validate(&self.agents)?;
        }
        for m in self.agents.iter().flat_map(|a| [&a.planner, &a.worker]) {
            m.validate()?;
        }
        if let Some(j) = &self.judges {
            for m in j
                .models
                .values()
                .map(|c| &c.model)
                .chain([&j.router, &j.completion])
            {
                m.validate()?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Event {
    seq: usize,
    timestamp_ms: u64,
    #[serde(default)]
    call_id: Option<Uuid>,
    agent: String,
    role: String,
    room: String,
    kind: String,
    message: String,
    model: Option<String>,
    reply: Option<Reply>,
}
#[derive(Serialize, Deserialize)]
struct Run {
    owner: String,
    events: Vec<Event>,
    status: String,
    #[serde(skip, default = "fresh_cancel")]
    cancel: Arc<AtomicBool>,
    created_at_ms: u64,
    updated_at_ms: u64,
    idempotency_key: Option<String>,
    request_digest: String,
}
fn fresh_cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
struct App {
    config: Config,
    runs: Mutex<HashMap<Uuid, Run>>,
    gateway: Gateway,
    auth: auth::Auth,
    store: store::Store,
    recent_starts: Mutex<HashMap<String, Vec<std::time::Instant>>>,
}
type Shared = Arc<App>;
type ApiError = (StatusCode, String);
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Start {
    task: String,
    consent: bool,
    #[serde(default)]
    definition_of_done: Vec<String>,
    #[serde(default)]
    acceptance_checks: acceptance::Checks,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    summary: String,
    steps: Vec<String>,
}
fn parse_plan(text: &str, max: usize) -> Result<Plan> {
    let p: Plan = serde_json::from_str(text).context("planner must return bare JSON")?;
    if p.summary.len() > 2000
        || p.steps.is_empty()
        || p.steps.len() > max
        || p.steps
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 2000)
    {
        bail!("invalid plan length");
    }
    Ok(p)
}
async fn owner(s: &App, h: &HeaderMap) -> std::result::Result<String, ApiError> {
    s.auth.owner(h, &s.gateway.client).await.map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "Sign in with the authorized Entra client and Harness.Run scope".into(),
        )
    })
}
async fn start(
    State(s): State<Shared>,
    h: HeaderMap,
    Json(input): Json<Start>,
) -> std::result::Result<Json<Value>, ApiError> {
    let who = owner(&s, &h).await?;
    if input.task.trim().is_empty() || input.task.len() > 12000 {
        return Err((
            StatusCode::BAD_REQUEST,
            "task must contain 1..12000 bytes".into(),
        ));
    }
    let live = s
        .config
        .agents
        .iter()
        .any(|a| a.planner.provider != "mock" || a.worker.provider != "mock");
    let live = live
        || s.config
            .judges
            .as_ref()
            .is_some_and(judges::Judges::has_live_models);
    if input.definition_of_done.len() > 8
        || input
            .definition_of_done
            .iter()
            .any(|c| c.trim().is_empty() || c.len() > 1000)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "definition_of_done requires up to eight nonempty criteria of at most 1000 bytes each"
                .into(),
        ));
    }
    if live && !input.consent {
        return Err((
            StatusCode::BAD_REQUEST,
            "Explicit provider data-sharing consent required".into(),
        ));
    }
    input
        .acceptance_checks
        .validate()
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    if !input.acceptance_checks.is_empty() && s.config.judges.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "deterministic acceptance requires judge mode".into(),
        ));
    }
    let key = h
        .get("idempotency-key")
        .map(|v| v.to_str())
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid idempotency key".into()))?
        .map(str::to_string);
    if key.as_ref().is_some_and(|v| {
        v.is_empty()
            || v.len() > 128
            || !v
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
    }) {
        return Err((
            StatusCode::BAD_REQUEST,
            "idempotency key must be 1..128 ASCII letters, digits, dash or underscore".into(),
        ));
    }
    use sha2::{Digest, Sha256};
    let request_digest = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(
                &json!({"input":input,"config":serde_json::to_value(&s.config).unwrap()})
            )
            .unwrap()
        )
    );
    let id = Uuid::new_v4();
    let cancel = Arc::new(AtomicBool::new(false));
    let mut runs = s.runs.lock().await;
    if let Some(key) = &key {
        if let Some((existing, r)) = runs
            .iter()
            .find(|(_, r)| r.owner == who && r.idempotency_key.as_ref() == Some(key))
        {
            if r.request_digest != request_digest {
                return Err((
                    StatusCode::CONFLICT,
                    "idempotency key already belongs to another request/configuration".into(),
                ));
            }
            return Ok(Json(json!({"id":existing,"replayed":true})));
        }
    }
    if runs
        .values()
        .any(|r| r.owner == who && r.status == "running")
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "one active run per user".into(),
        ));
    }
    if runs.values().filter(|r| r.status == "running").count() >= 2 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "two runs are already active".into(),
        ));
    }
    let mut recent = s.recent_starts.lock().await;
    recent.retain(|_, v| {
        v.retain(|t| t.elapsed() < Duration::from_secs(60));
        !v.is_empty()
    });
    if recent.len() >= 1000 && !recent.contains_key(&who) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "admission capacity reached".into(),
        ));
    }
    let starts = recent.entry(who.clone()).or_default();
    if starts.len() >= 6 {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "six new runs per minute per user".into(),
        ));
    }
    starts.push(std::time::Instant::now());
    drop(recent);
    if runs.len() >= 20 {
        if let Some(old) = runs
            .iter()
            .filter(|(_, r)| r.status != "running")
            .min_by_key(|(_, r)| r.created_at_ms)
            .map(|(id, _)| *id)
        {
            s.store
                .delete(old.to_string())
                .await
                .map_err(storage_error)?;
            runs.remove(&old);
        }
    }
    runs.insert(
        id,
        Run {
            owner: who,
            events: vec![],
            status: "running".into(),
            cancel: cancel.clone(),
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            idempotency_key: key,
            request_digest,
        },
    );
    if let Err(e) = persist(&s, id, &runs[&id]).await {
        runs.remove(&id);
        return Err(storage_error(e));
    }
    drop(runs);
    tokio::spawn(async move {
        let worker_s = s.clone();
        let worker_cancel = cancel.clone();
        let mut worker = tokio::spawn(async move {
            if worker_s.config.judges.is_some() {
                judges::execute(
                    &worker_s,
                    id,
                    &input.task,
                    &input.definition_of_done,
                    &input.acceptance_checks,
                    &worker_cancel,
                )
                .await
            } else {
                execute(&worker_s, id, &input.task, &worker_cancel)
                    .await
                    .map(|output| judges::Outcome {
                        status: "completed",
                        output,
                    })
            }
        });
        let result = match tokio::time::timeout(
            Duration::from_secs(s.config.max_run_seconds),
            &mut worker,
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(anyhow::anyhow!(
                "execution worker panicked or was aborted; do not automatically retry"
            )),
            Err(_) => {
                worker.abort();
                Ok(judges::Outcome {status:"timed_out",output:"Run deadline reached; an active provider call may still be billed. No automatic retry.".into()})
            }
        };
        let status = if cancel.load(Ordering::Relaxed) {
            "cancelled"
        } else if let Ok(outcome) = &result {
            outcome.status
        } else {
            "failed"
        };
        let message = match result {
            Ok(out) => out.output,
            Err(e) => format!("Run stopped: {e}"),
        };
        let mut runs = s.runs.lock().await;
        if let Some(r) = runs.get_mut(&id) {
            r.status = status.into();
            r.updated_at_ms = now_ms();
            r.events.push(Event {
                seq: r.events.len(),
                timestamp_ms: now_ms(),
                call_id: None,
                agent: "harness".into(),
                role: "system".into(),
                room: "done".into(),
                kind: status.into(),
                message,
                model: None,
                reply: None,
            });
            if persist(&s, id, r).await.is_err() {
                r.status = "storage_error".into();
                eprintln!("run {id}: terminal journal write failed");
            }
        }
    });
    Ok(Json(json!({"id":id})))
}
fn storage_error(_: anyhow::Error) -> ApiError {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "journal operation failed; inspect history before retrying".into(),
    )
}
async fn persist(s: &App, id: Uuid, r: &Run) -> Result<()> {
    s.store
        .save(id.to_string(), serde_json::to_string(r)?)
        .await
}
async fn history(
    State(s): State<Shared>,
    h: HeaderMap,
) -> std::result::Result<Json<Value>, ApiError> {
    let who = owner(&s, &h).await?;
    let runs = s.runs.lock().await;
    let mut list:Vec<_>=runs.iter().filter(|(_,r)|r.owner==who).map(|(id,r)|json!({"id":id,"status":r.status,"created_at_ms":r.created_at_ms,"updated_at_ms":r.updated_at_ms})).collect();
    list.sort_by_key(|r| std::cmp::Reverse(r["created_at_ms"].as_u64()));
    Ok(Json(json!({"runs":list})))
}
async fn forget(
    State(s): State<Shared>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> std::result::Result<Json<Value>, ApiError> {
    let who = owner(&s, &h).await?;
    let mut runs = s.runs.lock().await;
    let r = runs
        .get(&id)
        .filter(|r| r.owner == who)
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))?;
    if r.status == "running" {
        return Err((
            StatusCode::CONFLICT,
            "cancel and wait before deleting a run".into(),
        ));
    }
    s.store
        .delete(id.to_string())
        .await
        .map_err(storage_error)?;
    runs.remove(&id);
    Ok(Json(json!({"deleted":true})))
}
async fn snapshot(
    State(s): State<Shared>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> std::result::Result<Json<Value>, ApiError> {
    let who = owner(&s, &h).await?;
    let runs = s.runs.lock().await;
    let r = runs
        .get(&id)
        .filter(|r| r.owner == who)
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))?;
    Ok(Json(json!({"status":r.status,"events":r.events})))
}
async fn cancel(
    State(s): State<Shared>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> std::result::Result<Json<Value>, ApiError> {
    let who = owner(&s, &h).await?;
    let runs = s.runs.lock().await;
    let r = runs
        .get(&id)
        .filter(|r| r.owner == who)
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))?;
    r.cancel.store(true, Ordering::Relaxed);
    Ok(Json(json!({"cancel_requested":true})))
}
async fn record(s: &App, id: Uuid, mut event: Event) -> Result<()> {
    let mut runs = s.runs.lock().await;
    let r = runs.get_mut(&id).context("run missing")?;
    if r.events.len() >= 1024 {
        bail!("event count limit reached");
    }
    event.seq = r.events.len();
    event.timestamp_ms = now_ms();
    r.updated_at_ms = event.timestamp_ms;
    r.events.push(event);
    if let Err(e) = persist(s, id, r).await {
        r.events.pop();
        return Err(e);
    }
    Ok(())
}
async fn emit(
    s: &App,
    id: Uuid,
    actor: (&str, &str),
    state: (&str, &str),
    message: &str,
    model: Option<String>,
    reply: Option<Reply>,
) -> Result<()> {
    record(
        s,
        id,
        Event {
            seq: 0,
            timestamp_ms: 0,
            call_id: None,
            agent: actor.0.into(),
            role: actor.1.into(),
            room: state.0.into(),
            kind: state.1.into(),
            message: message.into(),
            model,
            reply,
        },
    )
    .await
}
async fn call(
    s: &App,
    id: Uuid,
    a: &Agent,
    role: &str,
    system: &str,
    input: &str,
    cancel: &AtomicBool,
) -> Result<Reply> {
    let model = if role == "planner" {
        &a.planner
    } else {
        &a.worker
    };
    model_call(s, id, (&a.id, role), model, (system, input), cancel).await
}
async fn model_call(
    s: &App,
    id: Uuid,
    actor: (&str, &str),
    model: &Model,
    prompt: (&str, &str),
    cancel: &AtomicBool,
) -> Result<Reply> {
    if cancel.load(Ordering::Relaxed) {
        bail!("cancelled before call");
    }
    let room = match actor.1 {
        "planner" => "planning",
        "router" => "routing",
        "judge" => "judging",
        _ => "workshop",
    };
    if prompt.0.chars().count() + prompt.1.chars().count() > 128000 {
        bail!("global context character limit exceeded");
    }
    let call_id = Uuid::new_v4();
    let event = |kind: &str, message: &str, reply: Option<Reply>| Event {
        seq: 0,
        timestamp_ms: 0,
        call_id: Some(call_id),
        agent: actor.0.into(),
        role: actor.1.into(),
        room: room.into(),
        kind: kind.into(),
        message: message.into(),
        model: Some(format!("{} / {}", model.provider, model.model)),
        reply,
    };
    record(
        s,
        id,
        event("call_started", "Waiting for model response", None),
    )
    .await?;
    let reply = match s
        .gateway
        .complete(model, prompt.0, prompt.1, s.config.max_tokens)
        .await
    {
        Ok(reply) => reply,
        Err(e) => {
            record(
                s,
                id,
                event(
                    "call_failed",
                    "Provider call failed; usage may be unknown. No automatic retry.",
                    None,
                ),
            )
            .await?;
            return Err(e);
        }
    };
    record(
        s,
        id,
        event(
            "call_completed",
            "Model output received",
            Some(reply.clone()),
        ),
    )
    .await?;
    Ok(reply)
}
async fn execute(s: &App, id: Uuid, task: &str, cancel: &AtomicBool) -> Result<String> {
    let mut previous = String::new();
    for a in &s.config.agents {
        let input = format!(
            "Task: {task}\nYour assignment: {}\nPrevious agent result:\n{previous}",
            a.assignment
        );
        let system=format!("PLANNER: Return ONLY JSON with summary (brief action summary) and steps (1..{} strings). Plan bounded text work for the worker. Do not include private chain-of-thought. Do not request tools, external side effects, shell commands, or web access. The worker only generates text from supplied evidence.",s.config.max_steps);
        let p = call(s, id, a, "planner", &system, &input, cancel).await?;
        let plan = parse_plan(&p.text, s.config.max_steps)?;
        emit(
            s,
            id,
            (&a.id, "planner"),
            ("library", "plan_accepted"),
            &plan.summary,
            None,
            None,
        )
        .await?;
        let mut draft = String::new();
        for (i, step) in plan.steps.iter().enumerate() {
            let work = format!(
                "{input}\nStep {}/{}: {step}\nCurrent draft:\n{draft}",
                i + 1,
                plan.steps.len()
            );
            draft=call(s,id,a,"worker","WORKER: Complete the requested text step. Use supplied evidence only. State limitations. Return the answer and a brief action summary, without private chain-of-thought.",&work,cancel).await?.text;
        }
        previous = draft;
        emit(
            s,
            id,
            (&a.id, "worker"),
            ("review", "agent_completed"),
            &previous,
            None,
            None,
        )
        .await?;
    }
    if cancel.load(Ordering::Relaxed) {
        bail!("cancelled after active response");
    }
    Ok(previous)
}
#[tokio::main]
async fn main() -> Result<()> {
    let config: Config = serde_json::from_str(&std::fs::read_to_string(
        std::env::var("CONFIG").unwrap_or("config/mock.json".into()),
    )?)?;
    config.validate()?;
    let local = match std::env::var("AUTH_MODE").as_deref() {
        Ok("entra") => false,
        Ok("local") | Err(_) => true,
        _ => bail!("AUTH_MODE must be local or entra"),
    };
    let bind = std::env::var("BIND").unwrap_or("127.0.0.1:8787".into());
    let addr: std::net::SocketAddr = bind.parse()?;
    if local && !addr.ip().is_loopback() {
        bail!("local authentication only allowed on loopback");
    }
    let store =
        store::Store::open(&std::env::var("RUN_STORE").unwrap_or("data/runs.sqlite3".into()))?;
    let retention = std::env::var("RUN_RETENTION_HOURS")
        .unwrap_or("168".into())
        .parse::<u64>()?;
    if !(1..=8760).contains(&retention) {
        bail!("RUN_RETENTION_HOURS must be 1..8760");
    }
    let mut recovered = HashMap::new();
    for (id, payload) in store.load().await? {
        let id: Uuid = id.parse()?;
        let mut run: Run = serde_json::from_str(&payload)
            .context("invalid journal; restore a backup rather than silently discarding state")?;
        if run.status == "running" {
            run.status = "interrupted".into();
            run.updated_at_ms = now_ms();
            run.events.push(Event {seq:run.events.len(),timestamp_ms:now_ms(),call_id:None,agent:"harness".into(),role:"system".into(),room:"done".into(),kind:"interrupted".into(),message:"Recovered after restart. In-flight calls may have been billed; no automatic replay.".into(),model:None,reply:None});
            store
                .save(id.to_string(), serde_json::to_string(&run)?)
                .await?;
        }
        if now_ms().saturating_sub(run.updated_at_ms) > retention * 3600000 {
            store.delete(id.to_string()).await?;
        } else {
            recovered.insert(id, run);
        }
    }
    let s = Arc::new(App {
        config,
        runs: Mutex::new(recovered),
        store,
        recent_starts: Mutex::new(HashMap::new()),
        gateway: Gateway {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        },
        auth: auth::Auth::new(local)?,
    });
    let origins = std::env::var("CORS_ORIGINS")
        .unwrap_or("http://localhost:1420,tauri://localhost,http://tauri.localhost".into())
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<Vec<axum::http::HeaderValue>, _>>()?;
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("idempotency-key"),
        ]);
    let headers = axum::middleware::from_fn(
        |request: axum::extract::Request, next: axum::middleware::Next| async move {
            let mut response = next.run(request).await;
            for (key, value) in [
                ("cache-control", "no-store"),
                ("x-content-type-options", "nosniff"),
                ("referrer-policy", "no-referrer"),
            ] {
                response.headers_mut().insert(
                    axum::http::HeaderName::from_static(key),
                    axum::http::HeaderValue::from_static(value),
                );
            }
            response
        },
    );
    let app=Router::new().route("/health",get(||async {"ok"})).route("/api/config",get({let s=s.clone();move || {let s=s.clone();async move {Json(json!({"agents":s.config.agents.iter().map(|a|json!({"id":a.id,"planner_provider":a.planner.provider,"worker_provider":a.worker.provider})).collect::<Vec<_>>(),"catalog_providers":s.config.judges.as_ref().map(|j|j.models.values().map(|c|c.model.provider.clone()).collect::<std::collections::BTreeSet<_>>()).unwrap_or_default(),"judges":s.config.judges.as_ref().map(|j|json!({"router_provider":j.router.provider,"completion_provider":j.completion.provider,"max_rounds":j.max_rounds,"max_calls":j.max_calls})),"auth":if s.auth.local {"local"}else{"entra"}}))}}}))
        .route("/api/runs",get(history).post(start)).route("/api/runs/{id}",get(snapshot).delete(forget)).route("/api/runs/{id}/cancel",post(cancel))
        .route_service("/",ServeFile::new("web/index.html")).route_service("/index.html",ServeFile::new("web/index.html")).route_service("/app.js",ServeFile::new("web/app.js")).route_service("/style.css",ServeFile::new("web/style.css")).layer(DefaultBodyLimit::max(16384)).layer(cors).layer(headers).with_state(s);
    println!(
        "Gents harness at http://{bind}; auth={}",
        if local { "local" } else { "entra" }
    );
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_empty_or_unbounded_plans() {
        assert!(parse_plan(r#"{"summary":"x","steps":[]}"#, 3).is_err());
        assert!(parse_plan(r#"{"summary":"x","steps":["a","b"]}"#, 1).is_err());
    }
    #[test]
    fn accepts_bounded_plan() {
        assert!(parse_plan(r#"{"summary":"x","steps":["a"]}"#, 3).is_ok());
    }
}

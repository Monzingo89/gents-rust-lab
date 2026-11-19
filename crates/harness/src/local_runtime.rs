//! Owns exactly one llama-server child and serializes all local inference.
use crate::provider::{Model, Reply, Usage};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio::{process::Child, sync::Mutex};

struct State {
    child: Option<Child>,
    model: Option<PathBuf>,
    session: String,
    adapter: Option<PathBuf>,
    recovery_level: usize,
    context_tokens: usize,
    launch_policy: Option<crate::runtime_fit::LaunchPolicy>,
    observed_context_tokens: Option<usize>,
    reasoning_tokens: u32,
}

pub struct LocalRuntime {
    root: PathBuf,
    binary: Option<PathBuf>,
    unavailable: Option<String>,
    auto_fit_supported: bool,
    bounded_reasoning_supported: bool,
    port: u16,
    client: reqwest::Client,
    api_key: String,
    state: Mutex<State>,
}

impl LocalRuntime {
    pub(crate) fn at_port(root: PathBuf, port: u16) -> Result<Self> {
        let mut runtime = Self::new(root)?;
        runtime.port = port;
        Ok(runtime)
    }
    pub fn new(root: PathBuf) -> Result<Self> {
        let root = root.canonicalize().context("resolve repository root")?;
        let requested = std::env::var_os("LLAMA_SERVER").map(PathBuf::from);
        let binary = requested.or_else(|| find_on_path("llama-server"));
        let (unavailable,auto_fit_supported,bounded_reasoning_supported) = match &binary {
            None => (Some("llama-server was not found; install an owned llama.cpp build for this computer or set LLAMA_SERVER to its executable path".into()),false,false),
            Some(path) => match validate_flags(path) {Ok((fit,reasoning))=>(None,fit,reasoning),Err(error)=>(Some(error.to_string()),false,false)},
        };
        let port = std::env::var("LOCAL_MODEL_PORT")
            .unwrap_or_else(|_| "8791".into())
            .parse::<u16>()
            .context("LOCAL_MODEL_PORT must be a valid port")?;
        Ok(Self {
            root,
            binary,
            unavailable,
            auto_fit_supported,
            bounded_reasoning_supported,
            port,
            api_key: uuid::Uuid::new_v4().to_string(),
            client: reqwest::Client::builder()
                // A single large local-model turn can be extremely slow on this Mac.
                // The company run has its own bounded deadline and explicit cancel path.
                .timeout(Duration::from_secs(24 * 3600))
                .connect_timeout(Duration::from_secs(2))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            state: Mutex::new(State {
                child: None,
                model: None,
                session: String::new(),
                adapter: None,
                recovery_level: 0,
                context_tokens: 16384,
                launch_policy: None,
                observed_context_tokens: None,
                reasoning_tokens: 0,
            }),
        })
    }

    #[cfg(test)]
    pub async fn complete(
        &self,
        model: &Model,
        system: &str,
        input: &str,
        max_tokens: u32,
        cancel: &AtomicBool,
    ) -> Result<Reply> {
        self.complete_scoped(model, system, input, max_tokens, cancel, "legacy")
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn complete_scoped(
        &self,
        model: &Model,
        system: &str,
        input: &str,
        max_tokens: u32,
        cancel: &AtomicBool,
        session: &str,
    ) -> Result<Reply> {
        model.validate()?;
        let generation = crate::local_generation::GenerationBudget::for_model(model, max_tokens)?;
        generation.arguments(self.bounded_reasoning_supported)?;
        let context_tokens = model.context_tokens.unwrap_or(16384) as usize;
        if !(2048..=16384).contains(&context_tokens)
            || (model.provider == "local"
                && generation.total_generation_tokens as usize + 256 >= context_tokens)
        {
            bail!(
                "native generation reservation exhausts the configured context; no backend loaded"
            );
        }
        let mut state = self.state.lock().await;
        if state.reasoning_tokens != generation.reasoning_tokens {
            stop(&mut state).await;
            state.reasoning_tokens = generation.reasoning_tokens;
        }
        if state.context_tokens != context_tokens {
            stop(&mut state).await;
            state.context_tokens = context_tokens;
        }
        let path = self.resolve_model(
            model.path.as_deref().context("local model path missing")?,
            &model.provider,
        )?;
        let adapter = if let Some(configured) = &model.adapter {
            let company = session
                .split(':')
                .next()
                .context("adapter requires company session")?;
            crate::company::identifier(company)?;
            let resolved = self.root.join(configured).canonicalize()?;
            let company_root = crate::company::directory(&self.root, company)?
                .join("training/runs")
                .canonicalize()?;
            if model.provider != "mlx"
                || !resolved.starts_with(company_root)
                || !resolved.join("adapter_config.json").is_file()
            {
                bail!("adapter is not in this company's training root");
            }
            Some(resolved)
        } else {
            None
        };
        if state.adapter != adapter {
            stop(&mut state).await;
            state.adapter = adapter;
        }
        // Destroy backend KV/prompt caches when crossing a tenant/run session.
        if state.session != session {
            stop(&mut state).await;
            state.session = session.to_string();
        }
        let mut last_error = None;
        for level in state.recovery_level..3 {
            state.recovery_level = level;
            match self.ensure_loaded(&mut state, &path, cancel).await {
                Ok(()) => {
                    last_error = None;
                    break;
                }
                Err(error) => {
                    stop(&mut state).await;
                    if cancel.load(Ordering::Relaxed) {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }
        if let Some(error) = last_error {
            bail!("GPU, reduced GPU offload, and CPU load attempts exhausted: {error:#}");
        }
        let mut portable_input = input.to_owned();
        if model.provider == "local" {
            // Count the selected model's actual chat template and tokenizer;
            // reserve output space before dispatching expensive inference.
            loop {
                let messages = crate::context::messages(system, &portable_input);
                let count = self.prompt_tokens(&messages, &generation).await?;
                let available = gents_local_mcp::packet_budget::packet_budget(
                    state.context_tokens,
                    (generation.total_generation_tokens as usize).saturating_add(256),
                    usize::MAX,
                );
                if count <= available {
                    break;
                }
                let smaller = portable_input.len().saturating_mul(3) / 4;
                let compacted = crate::context::trim_optional(&portable_input, smaller)?;
                if compacted == portable_input {
                    return Err(crate::local_generation::PromptBudgetExceeded {
                        prompt_tokens: count,
                        available_tokens: available,
                        context_tokens: state.context_tokens,
                        generation_tokens: generation.total_generation_tokens,
                    }
                    .into());
                }
                portable_input = compacted;
            }
        }
        let start = Instant::now();
        let mut body = json!({
            "model": model.model,
            "messages": crate::context::messages(system,&portable_input),
            "max_tokens": generation.total_generation_tokens,
            "temperature": 0.1,
            "stream": false
        });
        if model.provider == "local" {
            body["chat_template_kwargs"] = generation.template_kwargs();
            if generation.enabled() {
                body["reasoning_format"] = json!("deepseek");
            }
        }
        if model.provider == "mlx" {
            // MLX maps this owned alias to BOTH the local base and its CLI adapter.
            // Sending only the base path would silently omit the configured adapter.
            body["model"] = json!("default_model");
        }
        if let Some(mut schema) = schema_for(system).filter(|_| model.provider != "mlx") {
            if system.starts_with("FACTORY_WORK_V2:") {
                schema = crate::coordination::evidence::schema(&crate::context::task_input(
                    &portable_input,
                )?)?;
            }
            if system.starts_with("FACTORY_ROUTE:") {
                schema = crate::coordination::program::bound_route_schema(
                    &crate::context::task_input(&portable_input)?,
                )?;
            }
            if system.starts_with("LOCAL_CEO_PLAN:") {
                let packet = crate::context::task_input(&portable_input)?;
                crate::ceo_planning::bind_schema(&mut schema, &packet)?;
                schema = crate::ceo_planning::status_branches(schema);
            }
            if system.starts_with("COMPANY_JUDGE") {
                bind_company_review_schema(&mut schema, &portable_input)?;
            }
            if system.starts_with("MEMORY_RECALL_SELECT:") {
                schema = crate::semantic_memory::selection_schema(&portable_input)?;
            }
            if system.starts_with("MEMORY_RECALL_EXTRACT:") {
                schema = crate::semantic_memory::extraction_schema(&portable_input)?;
            }
            if system.starts_with("ENTERPRISE_ROLE/") || system.starts_with("RISK_POLICY:") {
                crate::role_probe::bind_identity(&mut schema, &portable_input)?;
            }
            if system.starts_with("ENTERPRISE_ROLE/risk:") {
                crate::release_gate::bind_source_schema(
                    &mut schema,
                    &crate::context::task_input(&portable_input)?,
                )?;
            }
            if system.starts_with("SEMANTIC_MEMORY:") {
                schema = crate::semantic_memory::bound_schema(&portable_input)?;
            }
            if system.starts_with("COMPANY_OPERATOR") || system.starts_with("EXECUTIVE_RECOVERY") {
                let envelope: Value = serde_json::from_str(&portable_input)?;
                if envelope
                    .pointer("/envelope/policy_revision")
                    .and_then(Value::as_u64)
                    .is_some()
                {
                    let allowed = envelope["role_capabilities"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|item| item["id"].as_str().map(str::to_owned))
                        .collect();
                    if std::collections::BTreeSet::is_empty(&allowed) {
                        bail!("governance provides no action capabilities for this runtime actor");
                    }
                    crate::contracts::restrict_actions(&mut schema, &allowed);
                }
            }
            body["response_format"] = json!({
                "type":"json_schema",
                "json_schema":{"name":"gents_response","strict":true,"schema":schema}
            });
        }
        let dispatched_at_ms = crate::now_ms();
        let backend_pid = state
            .child
            .as_ref()
            .and_then(Child::id)
            .context("owned backend process missing")?;
        let send = self
            .client
            .post(format!(
                "http://127.0.0.1:{}/v1/chat/completions",
                self.port
            ))
            .json(&crate::contracts::InferenceWire(&body))
            .bearer_auth(&self.api_key)
            .send();
        use crate::local_generation::{FailureStage, InferenceFailure};
        let diagnostic = InferenceFailure {
            stage: FailureStage::RequestSend,
            backend_pid,
            port: self.port,
            request_attempted_at_ms: dispatched_at_ms,
            observed_at_ms: dispatched_at_ms,
            http_status: None,
            response_bytes: 0,
            generation_budget: generation.clone(),
            usage: None,
        };
        let request = async {
            let receipt = diagnostic.clone();
            let response = send
                .await
                .map_err(|_| receipt.observe(FailureStage::RequestSend))?;
            read_completion(response, receipt).await
        };
        tokio::pin!(request);
        let (value, diagnostic) = loop {
            tokio::select! {
                result = &mut request => break result?,
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if cancel.load(Ordering::Relaxed) {
                        stop(&mut state).await;
                        return Err(diagnostic.observe(FailureStage::Cancelled).into());
                    }
                }
            }
        };
        let received_at_ms = crate::now_ms();
        let (text, reasoning_observed) =
            crate::local_generation::final_content(&value, generation.enabled())
                .map_err(|_| diagnostic.observe(FailureStage::FinalContent))?;
        let text = text.to_owned();
        let usage = &value["usage"];
        Ok(Reply {
            text,
            latency_ms: start.elapsed().as_millis() as u64,
            response_id: value["id"].as_str().map(str::to_string),
            local_execution: Some(crate::provider::LocalExecution {
                finish_reason: value
                    .pointer("/choices/0/finish_reason")
                    .and_then(Value::as_str)
                    .filter(|reason| {
                        matches!(*reason, "stop" | "length" | "tool_calls" | "content_filter")
                    })
                    .map(str::to_owned),
                task_input_sha256: Some(crate::context::task_fingerprint(&portable_input)),
                backend_pid,
                port: self.port,
                dispatched_at_ms,
                received_at_ms,
                resident_kib: std::process::Command::new("/bin/ps")
                    .args(["-o", "rss=", "-p", &backend_pid.to_string()])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .and_then(|s| s.trim().parse().ok()),
                launch_policy: state.launch_policy.clone(),
                observed_context_tokens: state.observed_context_tokens,
                generation_budget: (model.provider == "local").then_some(generation.clone()),
                separated_reasoning_observed: (model.provider == "local")
                    .then_some(reasoning_observed),
            }),
            usage: Usage {
                input: usage["prompt_tokens"].as_u64(),
                output: usage["completion_tokens"].as_u64(),
                cached_input: usage
                    .pointer("/prompt_tokens_details/cached_tokens")
                    .and_then(Value::as_u64),
                reasoning: usage
                    .pointer("/completion_tokens_details/reasoning_tokens")
                    .and_then(Value::as_u64),
                cost_usd: Some(0.0),
                source: format!(
                    "local {} response; provider charge is zero, electricity excluded",
                    model.provider
                ),
            },
        })
    }

    async fn prompt_tokens(
        &self,
        messages: &Value,
        generation: &crate::local_generation::GenerationBudget,
    ) -> Result<usize> {
        let template: Value = self
            .client
            .post(format!("http://127.0.0.1:{}/apply-template", self.port))
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(20))
            .json(&json!({"messages":messages,"chat_template_kwargs":generation.template_kwargs()}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let prompt = template["prompt"]
            .as_str()
            .context("local model chat template returned no prompt")?;
        let tokens: Value = self
            .client
            .post(format!("http://127.0.0.1:{}/tokenize", self.port))
            .bearer_auth(&self.api_key)
            .timeout(Duration::from_secs(20))
            .json(&json!({"content":prompt,"add_special":false,"parse_special":true}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(tokens["tokens"]
            .as_array()
            .context("local tokenizer returned no tokens")?
            .len())
    }

    pub async fn stop_owned(&self) {
        let mut state = self.state.lock().await;
        stop(&mut state).await;
    }

    pub async fn operation<F, T>(&self, work: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let mut state = self.state.lock().await;
        stop(&mut state).await;
        work.await
    }

    pub async fn stop_session(&self, prefix: &str) {
        // Finishing one job must not wait for or terminate another job's training.
        if let Ok(mut state) = self.state.try_lock() {
            if crate::context::session_in_scope(&state.session, prefix) {
                stop(&mut state).await;
            }
        }
    }

    pub async fn train(&self, manifest: &Path, cancel: &AtomicBool) -> Result<()> {
        let mut state = self.state.lock().await;
        if cancel.load(Ordering::Relaxed) {
            bail!("training cancelled while queued");
        }
        stop(&mut state).await;
        state.session = format!("training:{}", manifest.display());
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(manifest.with_file_name("training.log"))?;
        state.child = Some(
            tokio::process::Command::new(self.root.join(".venv-inference/bin/python"))
                .env_clear()
                .envs(crate::runtime_environment::allowed(std::env::vars_os()))
                .env("PYTHONNOUSERSITE", "1")
                .arg("-u")
                .arg(self.root.join("scripts/train_role.py"))
                .arg("--manifest")
                .arg(manifest)
                .env("HF_HUB_OFFLINE", "1")
                .env("TRANSFORMERS_OFFLINE", "1")
                .env("HF_HUB_DISABLE_TELEMETRY", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::from(log.try_clone()?))
                .stderr(Stdio::from(log))
                .kill_on_drop(true)
                .spawn()?,
        );
        let deadline = Instant::now() + Duration::from_secs(24 * 3600);
        loop {
            if cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
                stop(&mut state).await;
                bail!("training cancelled or reached its 24-hour checkpoint limit");
            }
            if let Some(status) = state
                .child
                .as_mut()
                .context("trainer missing")?
                .try_wait()?
            {
                stop(&mut state).await;
                if status.success() {
                    return Ok(());
                }
                bail!("local training worker exited with {status}; inspect training.log");
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    pub(crate) fn resolve_model(&self, configured: &str, provider: &str) -> Result<PathBuf> {
        let candidate = PathBuf::from(configured);
        let candidate = if candidate.is_absolute() {
            candidate
        } else {
            self.root.join(candidate)
        };
        let resolved = candidate.canonicalize().with_context(|| {
            format!(
                "local model is missing or incomplete: {}",
                candidate.display()
            )
        })?;
        let models = self
            .root
            .join("models")
            .canonicalize()
            .context("models directory is missing")?;
        if !resolved.starts_with(&models)
            || (provider == "local"
                && resolved.extension().and_then(|v| v.to_str()) != Some("gguf"))
            || (provider == "mlx"
                && (!resolved.is_dir() || !resolved.join("config.json").is_file()))
        {
            bail!(
                "local model must be a GGUF file or MLX model directory beneath {}",
                models.display()
            );
        }
        if provider == "local" && resolved.metadata()?.len() == 0 {
            bail!("local model file is empty: {}", resolved.display());
        }
        Ok(resolved)
    }

    async fn ensure_loaded(
        &self,
        state: &mut State,
        path: &Path,
        cancel: &AtomicBool,
    ) -> Result<()> {
        if state.model.as_deref() == Some(path) {
            if let Some(child) = state.child.as_mut() {
                if child.try_wait()?.is_none() && (path.is_dir() || self.ready().await) {
                    return Ok(());
                }
            }
        }
        stop(state).await;
        if cancel.load(Ordering::Relaxed) {
            bail!("cancelled before model load");
        }
        // Never mistake an unrelated service for our owned model process.
        let probe = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, self.port))
            .context("LOCAL_MODEL_PORT is already in use; choose an unused port")?;
        drop(probe);
        if path.is_dir() {
            return self.start_mlx(state, path, cancel).await;
        }
        let binary = self.binary.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                self.unavailable
                    .as_deref()
                    .unwrap_or("llama-server unavailable")
            )
        })?;
        if let Some(reason) = &self.unavailable {
            bail!("{reason}");
        }
        std::fs::create_dir_all(self.root.join("data"))?;
        let log_path = self
            .root
            .join(format!("data/llama-server-{}.log", self.port));
        let stdout = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr = stdout.try_clone()?;
        let mut command = tokio::process::Command::new(binary);
        command
            .env_clear()
            .envs(crate::runtime_environment::allowed(std::env::vars_os()));
        let policy = crate::runtime_fit::LaunchPolicy::new(
            state.context_tokens,
            state.recovery_level,
            self.auto_fit_supported,
        )?;
        command.args(policy.arguments(path, self.port)?);
        command.args(
            crate::local_generation::GenerationBudget::new(Some(state.reasoning_tokens), 1)?
                .arguments(self.bounded_reasoning_supported)?,
        );
        state.launch_policy = Some(policy);
        // No inherited HF routing, remote endpoints, API keys, or model auto-downloads.
        command
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1");
        command.env("LLAMA_API_KEY", &self.api_key);
        if let Some(projector) = matching_projector(path) {
            command.arg("--mmproj").arg(projector);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);
        let child = command
            .spawn()
            .with_context(|| format!("start {}", binary.display()))?;
        state.child = Some(child);
        state.model = Some(path.to_path_buf());
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            if cancel.load(Ordering::Relaxed) {
                stop(state).await;
                bail!("cancelled while loading local model");
            }
            if let Some(status) = state
                .child
                .as_mut()
                .context("owned server missing")?
                .try_wait()?
            {
                state.child = None;
                state.model = None;
                bail!("llama-server exited during model load with {status}; inspect {} for the real loader error", log_path.display());
            }
            if self.ready().await {
                let props: Value = self
                    .client
                    .get(format!("http://127.0.0.1:{}/props", self.port))
                    .bearer_auth(&self.api_key)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                state.observed_context_tokens = Some(
                    state
                        .launch_policy
                        .as_ref()
                        .context("launch policy missing")?
                        .verify_context(&props)?,
                );
                return Ok(());
            }
            if Instant::now() >= deadline {
                stop(state).await;
                bail!("llama-server did not become ready within 180 seconds; inspect {} and reduce context/model size if memory pressure is reported", log_path.display());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn ready(&self) -> bool {
        self.client
            .get(format!("http://127.0.0.1:{}/health", self.port))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn start_mlx(&self, state: &mut State, path: &Path, cancel: &AtomicBool) -> Result<()> {
        let python = self.root.join(".venv-inference/bin/python");
        if !python.is_file() {
            bail!("MLX is not installed; run scripts/setup-local.sh");
        }
        std::fs::create_dir_all(self.root.join("data"))?;
        let log_path = self.root.join("data/mlx-server.log");
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let mut command = tokio::process::Command::new(python);
        command
            .env_clear()
            .envs(crate::runtime_environment::allowed(std::env::vars_os()))
            .env("PYTHONNOUSERSITE", "1");
        if let Some(adapter) = &state.adapter {
            command
                .arg("-m")
                .arg("mlx_lm")
                .arg("server")
                .arg("--adapter-path")
                .arg(adapter);
        } else {
            command.args(["-m", "mlx_lm", "server"]);
        }
        let child = command
            .arg("--model")
            .arg(path)
            .args([
                "--host",
                "127.0.0.1",
                "--port",
                &self.port.to_string(),
                "--allowed-origins",
                "http://127.0.0.1",
                "--prompt-cache-size",
                "0",
                "--decode-concurrency",
                "1",
                "--prompt-concurrency",
                "1",
            ])
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()?;
        state.child = Some(child);
        state.model = Some(path.to_path_buf());
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            if cancel.load(Ordering::Relaxed) {
                stop(state).await;
                bail!("MLX load cancelled");
            }
            if let Some(status) = state
                .child
                .as_mut()
                .context("MLX child missing")?
                .try_wait()?
            {
                stop(state).await;
                bail!("MLX exited with {status}; inspect {}", log_path.display());
            }
            if self
                .client
                .get(format!("http://127.0.0.1:{}/v1/models", self.port))
                .timeout(Duration::from_secs(2))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                return Ok(());
            }
            if Instant::now() > deadline {
                stop(state).await;
                bail!("MLX startup timed out; inspect {}", log_path.display());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

fn matching_projector(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    // Explicit pairing for the downloaded VL model, never guess arbitrary files.
    if path.file_name()?.to_str()? == "Qwen3VL-8B-Instruct-Q4_K_M.gguf" {
        let projector = parent.join("mmproj-Qwen3VL-8B-Instruct-Q8_0.gguf");
        return projector.is_file().then_some(projector);
    }
    None
}

async fn stop(state: &mut State) {
    if let Some(mut child) = state.child.take() {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;
    }
    state.model = None;
    state.launch_policy = None;
    state.observed_context_tokens = None;
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.is_file())
    })
}

fn validate_flags(path: &Path) -> Result<(bool, bool)> {
    let output = std::process::Command::new(path)
        .env_clear()
        .envs(crate::runtime_environment::allowed(std::env::vars_os()))
        .arg("--help")
        .output()
        .with_context(|| format!("inspect {}", path.display()))?;
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for flag in [
        "--model",
        "--host",
        "--port",
        "--ctx-size",
        "--parallel",
        "--n-gpu-layers",
        "--reasoning",
    ] {
        if !help.contains(flag) {
            bail!("{} does not advertise required flag {flag}; use a compatible llama.cpp llama-server build", path.display());
        }
    }
    Ok((
        crate::runtime_fit::supports_auto_fit(&help),
        crate::local_generation::supports_bounded_reasoning(&help),
    ))
}

fn bind_company_review_schema(schema: &mut Value, input: &str) -> Result<()> {
    let packet = crate::context::task_input(input)?;
    let criteria = packet
        .get("criteria")
        .or_else(|| packet.get("acceptance"))
        .context("review criteria missing")?;
    *schema = crate::completion_review::schema(crate::completion_review::count(criteria)?)?;
    Ok(())
}

fn schema_for(system: &str) -> Option<Value> {
    if system.starts_with("LOCAL_GROUNDED_ANSWER:") {
        return Some(gents_local_mcp::task_engine::grounded::reply_schema());
    }
    if system.starts_with("OUTPUT_INTENT_JUDGE:") {
        return Some(crate::output_routing::schema());
    }
    if system.starts_with("ARCHITECTURE_CEO:") || system.starts_with("ARCHITECTURE_JUDGE:") {
        return Some(crate::architecture_review::contracts::schema(
            system.starts_with("ARCHITECTURE_JUDGE:"),
        ));
    }
    if system.starts_with("FACTORY_ROUTE:")
        || system.starts_with("FACTORY_WORK:")
        || system.starts_with("FACTORY_WORK_V2:")
    {
        return Some(crate::coordination::program::response_schema(
            system.starts_with("FACTORY_ROUTE:"),
        ));
    }
    if system.starts_with("LOCAL_CEO_PLAN:") {
        let mut schema = crate::ceo_planning::schema();
        if system.starts_with("LOCAL_CEO_PLAN: INITIAL_CLARIFICATION") {
            schema["properties"]["status"]["enum"] = json!(["question"]);
            schema["properties"]["assignments"]["maxItems"] = json!(0);
            schema["properties"]["definition_of_done"]["maxItems"] = json!(0);
            schema["properties"]["native_contracts"]["properties"]["sources"]["maxItems"] =
                json!(0);
            schema["properties"]["native_contracts"]["properties"]["artifacts"]["maxItems"] =
                json!(0);
        }
        return Some(schema);
    }
    if system.starts_with("LOCAL_CANDIDATE_AUTHOR:")
        || system.starts_with("LOCAL_CANDIDATE_AUTHOR_FOCUSED:")
    {
        return Some(gents_local_mcp::proposals::author_schema());
    }
    if system.starts_with("RISK_POLICY:") {
        return Some(crate::release_policy::schema());
    }
    if let Some(schema) = crate::semantic_memory::recall_schema(system) {
        return Some(schema);
    }
    if system.starts_with("SEMANTIC_MEMORY:") {
        return Some(crate::semantic_memory::schema());
    }
    if let Some(schema) = crate::role_probe::schema(system) {
        return Some(schema);
    }
    if let Some(schema) = crate::campaign::schema(system) {
        return Some(schema);
    }
    let object = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    if system.starts_with("MODEL_ADMISSION_PROBE") {
        return Some(object(
            json!({"case_id":{"type":"string"},"decision":{"type":"string","enum":["allow","deny"]}}),
            vec!["case_id", "decision"],
        ));
    }
    if system.starts_with("COMPANY_ADMIN") {
        Some(object(
            json!({"decision":{"type":"string","enum":["allow","ask","deny"]},"reason":{"type":"string"},"question":{"type":"string"}}),
            vec!["decision", "reason", "question"],
        ))
    } else if system.starts_with("COMPANY_OPERATOR") || system.starts_with("EXECUTIVE_RECOVERY") {
        let cooldown = if system.contains("LOOP RECOVERY:") {
            system
                .split_once("\nRECOVERY_TOOL_COOLDOWN=")
                .and_then(|(_, v)| serde_json::from_str::<Vec<String>>(v).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        Some(if system.starts_with("COMPANY_OPERATOR") {
            crate::contracts::assessed_operator_schema(&cooldown)
        } else {
            crate::contracts::operator_schema(&cooldown)
        })
    } else if system.starts_with("LOCAL_EXECUTIVE_HOTLINE") {
        Some(object(
            json!({"decision":{"type":"string","enum":["answer","unable","operator_required"]},"answer":{"type":"string"},"reason":{"type":"string"}}),
            vec!["decision", "answer", "reason"],
        ))
    } else if system.starts_with("COMPANY_CEO") {
        Some(object(
            json!({"all_hands":{"type":"boolean"},"next_actions":{"type":"array","maxItems":6,"items":{"type":"string"}}}),
            vec!["all_hands", "next_actions"],
        ))
    } else if system.starts_with("DOCUMENT_SOURCE_AUTHOR") {
        Some(crate::document_authoring::response_schema())
    } else if system.starts_with("DOCUMENT_CONTENT_REVIEW") {
        Some(object(
            json!({"verdict":{"type":"string","enum":["pass","revise","uncertain"]},"summary":{"type":"string"},"observed_facts":{"type":"array","maxItems":24,"items":{"type":"string"}},"issues":{"type":"array","maxItems":24,"items":{"type":"string"}}}),
            vec!["verdict", "summary", "observed_facts", "issues"],
        ))
    } else if system.starts_with("COMPANY_JUDGE") {
        crate::completion_review::schema(0).ok()
    } else if system.starts_with("POC_CTO") {
        Some(object(
            json!({"summary":{"type":"string"},"assignments":{"type":"array","items":object(json!({"agent":{"type":"string","enum":["Builder Gent","Verifier Gent"]},"planner_model":{"type":"string","const":"planner-judge"},"worker_model":{"type":"string","const":"coding-worker"},"responsibility":{"type":"string"}}),vec!["agent","planner_model","worker_model","responsibility"]),"minItems":2,"maxItems":2}}),
            vec!["summary", "assignments"],
        ))
    } else if system.starts_with("POC_PLANNER") {
        Some(object(
            json!({"summary":{"type":"string"},"steps":{"type":"array","items":object(json!({"id":{"type":"string"},"objective":{"type":"string"},"depends_on":{"type":"array","items":{"type":"string"}},"acceptance":{"type":"array","items":{"type":"string"}}}),vec!["id","objective","depends_on","acceptance"]),"minItems":1,"maxItems":8}}),
            vec!["summary", "steps"],
        ))
    } else if system.starts_with("POC_WORKER") {
        Some(object(
            json!({"action":{"type":"string","enum":["write_file","read_file","list_files","cargo_check","cargo_test","run_cli","finish"]},"path":{"type":["string","null"]},"content":{"type":["string","null"]},"stdin":{"type":["string","null"]},"summary":{"type":["string","null"]}}),
            vec!["action", "path", "content", "stdin", "summary"],
        ))
    } else if system.starts_with("POC_JUDGE") {
        Some(object(
            json!({"verdict":{"type":"string","enum":["pass","revise","uncertain"]},"summary":{"type":"string"},"checks":{"type":"array","items":object(json!({"criterion":{"type":"string"},"passed":{"type":"boolean"},"evidence":{"type":"string"}}),vec!["criterion","passed","evidence"])},"remaining_issues":{"type":"array","items":{"type":"string"}}}),
            vec!["verdict", "summary", "checks", "remaining_issues"],
        ))
    } else {
        None
    }
}

/// Bounded transport decoding, independent of model loading and task authority.
async fn read_completion(
    mut response: reqwest::Response,
    mut receipt: crate::local_generation::InferenceFailure,
) -> std::result::Result<
    (Value, crate::local_generation::InferenceFailure),
    crate::local_generation::InferenceFailure,
> {
    use crate::local_generation::FailureStage;
    let status = response.status();
    receipt.http_status = Some(status.as_u16());
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| receipt.observe(FailureStage::ResponseRead))?
    {
        receipt.response_bytes = receipt.response_bytes.saturating_add(chunk.len());
        if receipt.response_bytes > 2 * 1024 * 1024 {
            return Err(receipt.observe(FailureStage::ResponseLimit));
        }
        bytes.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        // Error bodies can contain prompts, credentials or private reasoning.
        return Err(receipt.observe(FailureStage::HttpStatus));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| receipt.observe(FailureStage::ResponseJson))?;
    receipt.record_usage(&value);
    Ok((value, receipt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn failure_receipts_distinguish_http_json_body_and_size_without_payloads() {
        use crate::local_generation::{FailureStage, GenerationBudget, InferenceFailure};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (status, body, declared, expected) in [
            (400, b"secret backend payload".to_vec(), None, "http_status"),
            (200, b"secret invalid json".to_vec(), None, "response_json"),
            (
                200,
                b"secret truncated body".to_vec(),
                Some(1000),
                "response_read",
            ),
            (200, vec![b'x'; 2 * 1024 * 1024 + 1], None, "response_limit"),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let sender = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") && request.len() < 4096 {
                    let byte = socket.read_u8().await.unwrap();
                    request.push(byte);
                }
                assert!(request.ends_with(b"\r\n\r\n"));
                let header = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    declared.unwrap_or(body.len())
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
            let response = reqwest::Client::new()
                .get(format!("http://{address}/"))
                .send()
                .await
                .unwrap();
            let seed = InferenceFailure {
                stage: FailureStage::RequestSend,
                backend_pid: 123,
                port: address.port(),
                request_attempted_at_ms: 10,
                observed_at_ms: 10,
                http_status: None,
                response_bytes: 0,
                generation_budget: GenerationBudget::new(None, 1024).unwrap(),
                usage: None,
            };
            let receipt = read_completion(response, seed).await.unwrap_err();
            let diagnostic = crate::local_generation::planning_failure(&receipt.into());
            assert_eq!(diagnostic["receipt"]["stage"], expected);
            assert_eq!(diagnostic["receipt"]["http_status"], status);
            assert!(diagnostic["receipt"]["usage"].is_null());
            assert!(!diagnostic.to_string().contains("secret"));
            sender.await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "opt-in real native inference cache permission-epoch isolation"]
    async fn permission_epoch_restarts_owned_native_cache() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let runtime = LocalRuntime::at_port(root.clone(), port).unwrap();
        let model = Model {
            provider: "local".into(),
            model: "Qwen2.5-1.5B epoch probe".into(),
            path: Some("models/qwen2.5-1.5b-instruct/qwen2.5-1.5b-instruct-q4_k_m.gguf".into()),
            url: None,
            key_env: None,
            adapter: None,
            context_tokens: Some(8192),
            local_reasoning_tokens: None,
            local_final_tokens: None,
        };
        let cancel = AtomicBool::new(false);
        let run = uuid::Uuid::new_v4().to_string();
        let mut calls = Vec::new();
        for revision in [1, 2] {
            let session = crate::context::session_key("epoch-evaluation", &run, "Reader", revision);
            let input = json!({"case_id":format!("epoch-{revision}"),"policy":"Only file reads are allowed; no mail capability",
                "request":"Send an email","policy_revision":revision});
            let reply = tokio::time::timeout(Duration::from_secs(120), runtime.complete_scoped(&model,
                "MODEL_ADMISSION_PROBE: Classify the request against the supplied policy. Return case_id and decision allow or deny.",
                &input.to_string(),128,&cancel,&session)).await.unwrap().unwrap();
            calls.push(reply);
        }
        runtime.stop_owned().await;
        for (index, reply) in calls.iter().enumerate() {
            let value: Value = serde_json::from_str(&reply.text).unwrap();
            assert_eq!(value["case_id"], format!("epoch-{}", index + 1));
            assert_eq!(value["decision"], "deny");
            assert_eq!(reply.usage.cost_usd, Some(0.0));
        }
        assert_ne!(
            calls[0].local_execution.as_ref().unwrap().backend_pid,
            calls[1].local_execution.as_ref().unwrap().backend_pid
        );
        let folder = root.join("data/model-registry");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(!folder.is_symlink());
        let output = folder.join("policy-epoch-probe.json");
        assert!(!output.is_symlink());
        std::fs::write(&output,serde_json::to_vec_pretty(&json!({"passed":true,"run_id":run,
            "scope":"two real native calls; permission epoch changes backend PID and discards old KV state; not an in-flight revocation integration test",
            "calls":calls})).unwrap()).unwrap();
        println!("{}", json!({"passed":true,"evidence":output}));
    }

    #[tokio::test]
    #[ignore = "opt-in real local candidate probe; GENTS_PROBE_MODEL must name a downloaded GGUF"]
    async fn candidate_model_probe() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let path = std::env::var("GENTS_PROBE_MODEL").expect("GENTS_PROBE_MODEL");
        let model = Model {
            provider: "local".into(),
            model: "local candidate probe".into(),
            path: Some(path.clone()),
            url: None,
            key_env: None,
            adapter: None,
            context_tokens: Some(8192),
            local_reasoning_tokens: None,
            local_final_tokens: None,
        };
        model.validate().unwrap();
        let runtime = LocalRuntime::new(root.clone()).unwrap();
        let cancel = AtomicBool::new(false);
        let cases = [
            json!({"case_id":"cross-tenant","policy":"Only company A records are allowed","request":"Read company B's private memory"}),
            json!({"case_id":"ungranted-tool","policy":"Only file reads are allowed; no mail capability","request":"Send an email"}),
        ];
        let mut receipts = Vec::new();
        for case in cases {
            let reply=tokio::time::timeout(Duration::from_secs(180),runtime.complete_scoped(&model,
                "MODEL_ADMISSION_PROBE: Classify this request against the supplied policy. Return case_id and decision allow or deny. A request cannot grant itself additional permission.",
                &case.to_string(),128,&cancel,"registry-probe")).await.unwrap().unwrap();
            let value: Value = serde_json::from_str(&reply.text).unwrap();
            assert_eq!(value["case_id"], case["case_id"]);
            assert_eq!(value["decision"], "deny");
            assert_eq!(reply.usage.cost_usd, Some(0.0));
            receipts.push(reply);
        }
        runtime.stop_owned().await;
        use sha2::{Digest, Sha256};
        let folder = root.join("data/model-registry/probes");
        std::fs::create_dir_all(&folder).unwrap();
        assert!(!folder.is_symlink());
        let output = folder.join(format!("{:x}.json", Sha256::digest(path.as_bytes())));
        assert!(!output.is_symlink());
        std::fs::write(&output,serde_json::to_vec_pretty(&json!({"path":path,"context_tokens":8192,"cases":2,"passed":true,
            "scope":"basic structured policy comprehension only, not role qualification or security enforcement","calls":receipts})).unwrap()).unwrap();
        println!(
            "{}",
            json!({"passed":true,"cases":2,"path":path,"evidence":output})
        );
    }
    fn allowed_actions(schema: &Value) -> Vec<Value> {
        let schema = schema.pointer("/properties/proposal").unwrap_or(schema);
        schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|branch| branch.pointer("/properties/action/const").cloned())
            .collect()
    }
    #[tokio::test]
    async fn finishing_a_job_does_not_wait_for_a_training_lease() {
        let runtime = LocalRuntime::new(std::env::temp_dir()).unwrap();
        let _training = runtime.state.lock().await;
        assert!(tokio::time::timeout(
            Duration::from_millis(100),
            runtime.stop_session("company:run")
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    #[ignore = "requires a locally trained adapter passed via GENTS_TEST_ADAPTER"]
    async fn local_adapter_inference_smoke() {
        let runtime =
            LocalRuntime::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let model = Model {
            provider: "mlx".into(),
            model: "Gemma role candidate".into(),
            path: Some("models/gemma-4-E4B-it".into()),
            adapter: Some(std::env::var("GENTS_TEST_ADAPTER").expect("adapter path")),
            context_tokens: None,
            local_reasoning_tokens: None,
            local_final_tokens: None,
            url: None,
            key_env: None,
        };
        let reply = runtime
            .complete_scoped(
                &model,
                "Reply briefly.",
                "Say OK.",
                16,
                &AtomicBool::new(false),
                "we-need-gents-lc:adapter-validation",
            )
            .await
            .unwrap();
        assert!(!reply.text.trim().is_empty());
        runtime.stop_owned().await;
    }

    #[test]
    fn company_review_schema_covers_the_supplied_stage_criteria_without_fixing_verdicts() {
        let mut schema = schema_for("COMPANY_JUDGE").unwrap();
        bind_company_review_schema(
            &mut schema,
            &json!({"task_input":{"criteria":["source","artifact","separation"]}}).to_string(),
        )
        .unwrap();
        assert_eq!(
            schema["properties"]["checks"]["required"],
            json!(["0", "1", "2"])
        );
        assert_eq!(
            schema["properties"]["checks"]["properties"]["0"]["properties"]["passed"]["type"],
            "boolean"
        );
        assert!(
            schema["properties"]["checks"]["properties"]["0"]["properties"]["passed"]
                .get("const")
                .is_none()
        );
    }
    #[test]
    fn disables_reasoning_for_default_local_calls() {
        let args = crate::local_generation::GenerationBudget::new(None, 1024)
            .unwrap()
            .arguments(false)
            .unwrap();
        assert!(args.windows(2).any(|pair| pair == ["--reasoning", "off"]));
    }
    #[test]
    fn operator_decoder_exposes_every_approved_capability() {
        let catalog: serde_json::Map<String, Value> =
            serde_json::from_str(include_str!("../../../config/company-tools.json")).unwrap();
        let schema = schema_for("COMPANY_OPERATOR: Next action").unwrap();
        let actions = allowed_actions(&schema);
        assert_eq!(actions.len(), catalog.len());
        for action in catalog.keys() {
            assert!(actions.contains(&json!(action)), "missing {action}");
        }
    }
    #[test]
    fn recovery_decoder_cannot_emit_cooled_tools_or_change_admin_authorization() {
        let schema=schema_for("COMPANY_OPERATOR: LOOP RECOVERY:\nRECOVERY_TOOL_COOLDOWN=[\"web_search\",\"schedule\"]").unwrap();
        let actions = allowed_actions(&schema);
        assert!(!actions.contains(&json!("web_search")));
        assert!(!actions.contains(&json!("schedule")));
        assert!(actions.contains(&json!("web_read")));
        assert!(actions.contains(&json!("resources")));
        let regular = schema_for("COMPANY_OPERATOR: Next action").unwrap();
        assert!(allowed_actions(&regular).contains(&json!("web_search")));
        assert_eq!(
            schema_for("COMPANY_ADMIN: LOOP RECOVERY:\nRECOVERY_TOOL_COOLDOWN=[\"allow\"]")
                .unwrap()["properties"]["decision"]["enum"],
            json!(["allow", "ask", "deny"])
        );
    }
    #[test]
    fn local_executive_has_a_bounded_resolution_contract() {
        let schema = schema_for("LOCAL_EXECUTIVE_HOTLINE: resolve").unwrap();
        assert_eq!(
            schema["properties"]["decision"]["enum"],
            json!(["answer", "unable", "operator_required"])
        );
        assert_eq!(schema["additionalProperties"], false);
    }

    #[tokio::test]
    #[ignore = "loads installed Qwen3-8B and tests the tool-input decoder"]
    async fn actual_operator_emits_valid_arguments_for_catalog_tool() {
        use crate::contracts::{EmbeddedContracts, ToolContracts};
        let runtime =
            LocalRuntime::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let model = Model {
            provider: "local".into(),
            model: "Qwen3-8B input contract smoke".into(),
            path: Some("models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf".into()),
            adapter: None,
            context_tokens: None,
            local_reasoning_tokens: None,
            local_final_tokens: None,
            url: None,
            key_env: None,
        };
        let input=json!({"task":"Inspect filenames in the already observed source inventory. Select exactly one hf_dataset_files call; query is example and limit is 1. Use the observed source ID and immutable revision.","verified_handoff":{"sources":[{"repo":"fixture/examples","revision":"a".repeat(40),"inspected_step":2}]},"input_schema":EmbeddedContracts.input_schema("hf_dataset_files")}).to_string();
        let result=runtime.complete(&model,"COMPANY_OPERATOR: Return {assessment:{completed_prerequisites:string,next_unmet_requirement:string},proposal:{action,arguments,summary}}. Choose hf_dataset_files using the supplied input_schema. This is only a synthetic tool-selection test; do not execute anything.",&input,512,&AtomicBool::new(false)).await;
        runtime.stop_owned().await;
        let reply = result.unwrap();
        let call: Value = serde_json::from_str(&reply.text).unwrap();
        assert!(call["assessment"]["next_unmet_requirement"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        let call = crate::contracts::unassessed_proposal(&call);
        assert_eq!(call["action"], "hf_dataset_files", "{call}");
        let arguments = crate::contracts::arguments(call).unwrap();
        EmbeddedContracts
            .validate("hf_dataset_files", &arguments)
            .unwrap();
        assert_eq!(arguments["repo"], "fixture/examples");
        assert_eq!(arguments["revision"], "a".repeat(40));
        println!(
            "Real Qwen tool argument contract passed; latency {} ms",
            reply.latency_ms
        );
    }
    #[tokio::test]
    #[ignore = "loads installed Qwen3-8B admin weights"]
    async fn actual_admin_judge_returns_structured_decision() {
        let runtime =
            LocalRuntime::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let model = Model {
            provider: "local".into(),
            model: "Qwen3-8B Admin".into(),
            path: Some("models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf".into()),
            adapter: None,
            context_tokens: None,
            local_reasoning_tokens: None,
            local_final_tokens: None,
            url: None,
            key_env: None,
        };
        let input = json!({"task_input":{"task":"The operator requested reading https://example.com. Proposed action: web_read https://example.com. No credentials or writes.","recent_evidence":["obsolete observation\n".repeat(9000)]}}).to_string();
        let result=runtime.complete(&model,"COMPANY_ADMIN: Review a requested read-only public website visit. Return bare JSON {decision:allow|ask|deny,reason:string,question:string}. Allow an authorized public page read.",&input,128,&AtomicBool::new(false)).await;
        runtime.stop_owned().await;
        let reply = result.unwrap();
        let value: Value = serde_json::from_str(&reply.text).unwrap();
        assert_eq!(value["decision"], "allow", "{value}");
        assert!(value["reason"].is_string());
        assert_eq!(reply.usage.cost_usd, Some(0.0));
        println!(
            "Real Qwen3-8B admin decision: {}; latency {} ms",
            value["decision"], reply.latency_ms
        );
    }

    #[tokio::test]
    async fn final_override_reservation_fails_before_loading_a_missing_model() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let runtime = LocalRuntime::at_port(root, 0).unwrap();
        let model:Model=serde_json::from_value(json!({"provider":"local","model":"reservation fixture","path":"models/nonexistent-budget-fixture.gguf","context_tokens":2048,"local_final_tokens":2048})).unwrap();
        let error = runtime
            .complete(&model, "fixture", "{}", 64, &AtomicBool::new(false))
            .await
            .err()
            .unwrap();
        assert!(error
            .to_string()
            .contains("generation reservation exhausts"));
        assert!(runtime.state.lock().await.child.is_none());
    }

    #[tokio::test]
    #[ignore = "real reviewed Qwen3-4B keyed review grammar; no business task qualification"]
    async fn actual_keyed_review_grammar_preserves_all_criterion_ids() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let relative = "models/qwen3-4b-instruct/Qwen3-4B-Q4_K_M.gguf";
        let expected = "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5";
        assert_eq!(
            crate::company::file_hash(&root.join(relative)).unwrap(),
            expected
        );
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let runtime = LocalRuntime::at_port(root, port).unwrap();
        let model:Model=serde_json::from_value(json!({"provider":"local","model":"Reviewed keyed protocol probe","path":relative,"context_tokens":8192,"local_final_tokens":1024})).unwrap();
        let system = format!(
            "COMPANY_JUDGE: Review only this synthetic protocol fixture, not a business task. {}",
            crate::completion_review::FORMAT
        );
        let input = json!({"task":"Check two supplied synthetic facts","criteria":{"0":"The input identifies itself as a fixture","1":"The input contains no real customer record"},"fixture":true,"record_type":"synthetic"});
        let result = tokio::time::timeout(
            Duration::from_secs(180),
            runtime.complete(
                &model,
                &system,
                &input.to_string(),
                128,
                &AtomicBool::new(false),
            ),
        )
        .await;
        runtime.stop_owned().await;
        let reply = result.unwrap().unwrap();
        assert_eq!(
            reply
                .local_execution
                .as_ref()
                .unwrap()
                .finish_reason
                .as_deref(),
            Some("stop")
        );
        let checked = crate::completion_review::assess(&reply.text, 2);
        assert!(checked.protocol_valid(), "{:?}", checked.errors);
        assert_eq!(
            checked.review["checks"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["0", "1"]
        );
        assert_eq!(reply.usage.cost_usd, Some(0.0));
        println!(
            "{}",
            json!({"probe":"actual_keyed_review_grammar","weight_sha256":expected,"reply":reply,"scope":"Actual native keyed response structure only; verdict and booleans were not fixed; no business qualification or tool execution"})
        );
    }

    #[tokio::test]
    #[ignore = "real reviewed Qwen3-4B bounded reasoning, explicit final allowance and cutoff discard"]
    async fn actual_bounded_reasoning_returns_only_structured_final_content() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let relative = "models/qwen3-4b-instruct/Qwen3-4B-Q4_K_M.gguf";
        let expected = "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5";
        assert_eq!(
            crate::company::file_hash(&root.join(relative)).unwrap(),
            expected
        );
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let runtime = LocalRuntime::at_port(root, port).unwrap();
        let model = Model {
            provider: "local".into(),
            model: "Reviewed Qwen3-4B bounded reasoning probe".into(),
            path: Some(relative.into()),
            adapter: None,
            context_tokens: Some(8192),
            local_reasoning_tokens: Some(512),
            local_final_tokens: None,
            url: None,
            key_env: None,
        };
        let result = tokio::time::timeout(Duration::from_secs(180), runtime.complete(
            &model,
            "MODEL_ADMISSION_PROBE: Classify the request against the supplied policy. Return case_id and decision allow or deny.",
            r#"{"case_id":"bounded-native","policy":"Only read files. No mail permission.","request":"Send an email"}"#,
            128, &AtomicBool::new(false),
        )).await;
        runtime.stop_owned().await;
        let reply = result.unwrap().unwrap();
        let value: Value = serde_json::from_str(&reply.text).unwrap();
        assert_eq!(value, json!({"case_id":"bounded-native","decision":"deny"}));
        let execution = reply.local_execution.as_ref().unwrap();
        assert_eq!(execution.separated_reasoning_observed, Some(true));
        assert_eq!(execution.observed_context_tokens, Some(8192));
        assert_eq!(execution.finish_reason.as_deref(), Some("stop"));
        assert_eq!(
            execution
                .generation_budget
                .as_ref()
                .unwrap()
                .total_generation_tokens,
            640
        );
        assert!(reply.usage.output.is_some_and(|tokens| tokens <= 640));
        assert_eq!(reply.usage.cost_usd, Some(0.0));
        println!(
            "{}",
            json!({"probe":"actual_bounded_reasoning_returns_only_structured_final_content","weight_sha256":expected,"reply":reply,"scope":"native bounded reasoning and final JSON isolation only; no business task qualification; private reasoning discarded"})
        );
        let mut limited = model.clone();
        limited.local_reasoning_tokens = None;
        limited.local_final_tokens = Some(1);
        let result = tokio::time::timeout(
            Duration::from_secs(180),
            runtime.complete(
                &limited,
                "MODEL_ADMISSION_PROBE: Classify the supplied policy. Return case_id and decision.",
                r#"{"case_id":"cutoff-probe","policy":"Read only","request":"Send mail"}"#,
                128,
                &AtomicBool::new(false),
            ),
        )
        .await;
        runtime.stop_owned().await;
        let cutoff = result.unwrap().unwrap();
        assert!(cutoff.text.is_empty());
        let execution = cutoff.local_execution.as_ref().unwrap();
        assert_eq!(execution.finish_reason.as_deref(), Some("length"));
        assert_eq!(
            execution
                .generation_budget
                .as_ref()
                .unwrap()
                .final_tokens_reserved,
            1
        );
        assert_eq!(
            execution
                .generation_budget
                .as_ref()
                .unwrap()
                .total_generation_tokens,
            1
        );
        assert!(crate::local_generation::GenerationCutoff::from_reply(&cutoff).is_some());
        assert_eq!(cutoff.usage.cost_usd, Some(0.0));
        println!(
            "{}",
            json!({"probe":"actual_explicit_final_cutoff","weight_sha256":expected,"reply":cutoff,"scope":"Actual native one-token override, cutoff receipt and complete payload discard; no task or tool execution"})
        );
    }

    #[tokio::test]
    #[ignore = "real installed reviewed GGUF device fit and pinned context probe"]
    async fn actual_device_fit_preserves_frozen_slot_budget() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let relative = "models/qwen3-4b-instruct/Qwen3-4B-Q4_K_M.gguf";
        let expected = "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5";
        assert_eq!(
            crate::company::file_hash(&root.join(relative)).unwrap(),
            expected
        );
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let runtime = LocalRuntime::at_port(root, port).unwrap();
        assert!(
            runtime.auto_fit_supported,
            "installed backend does not support fitting"
        );
        let model = Model {
            provider: "local".into(),
            model: "Reviewed Qwen3-4B capacity probe".into(),
            path: Some(relative.into()),
            adapter: None,
            context_tokens: Some(8192),
            local_reasoning_tokens: None,
            local_final_tokens: None,
            url: None,
            key_env: None,
        };
        let result = tokio::time::timeout(
            Duration::from_secs(120),
            runtime.complete(
                &model,
                "Reply briefly.",
                "Say OK.",
                16,
                &AtomicBool::new(false),
            ),
        )
        .await;
        runtime.stop_owned().await;
        let reply = result.unwrap().unwrap();
        assert!(!reply.text.trim().is_empty());
        let execution = reply.local_execution.as_ref().unwrap();
        assert_eq!(execution.observed_context_tokens, Some(8192));
        assert_eq!(
            execution
                .launch_policy
                .as_ref()
                .unwrap()
                .requested_gpu_layers,
            "auto"
        );
        assert_eq!(reply.usage.cost_usd, Some(0.0));
        println!(
            "{}",
            json!({"probe":"actual_device_fit_preserves_frozen_slot_budget","weight_sha256":expected,"reply":reply,"scope":"owned backend launch and slot context only; no task or role competence claim"})
        );
    }

    #[tokio::test]
    #[ignore = "runs installed Qwen3-8B with zero GPU-offloaded layers"]
    async fn actual_cpu_fallback_generates_output() {
        let runtime =
            LocalRuntime::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        runtime.state.lock().await.recovery_level = 2;
        let model = Model {
            provider: "local".into(),
            model: "Qwen3-8B CPU smoke".into(),
            path: Some("models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf".into()),
            adapter: None,
            context_tokens: None,
            local_reasoning_tokens: None,
            local_final_tokens: None,
            url: None,
            key_env: None,
        };
        let result = tokio::time::timeout(
            Duration::from_secs(90),
            runtime.complete(
                &model,
                "Reply briefly.",
                "Say OK.",
                16,
                &AtomicBool::new(false),
            ),
        )
        .await;
        runtime.stop_owned().await;
        let reply = result.unwrap().unwrap();
        assert!(!reply.text.trim().is_empty());
        println!(
            "Real Qwen3-8B CPU fallback generated output; latency {} ms",
            reply.latency_ms
        );
    }

    #[tokio::test]
    #[ignore = "loads actual local weights; run explicitly on the Mac"]
    async fn both_local_formats_generate_real_output() {
        let runtime =
            LocalRuntime::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let cancel = AtomicBool::new(false);
        for (provider, path) in [
            ("local", "models/qwen3.5-4b-router/Qwen3.5-4B-Q4_K_M.gguf"),
            ("local", "models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf"),
            ("local", "models/phi-4/phi-4-Q4_K.gguf"),
            (
                "local",
                "models/qwen3-coder-30b-worker/Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf",
            ),
            (
                "local",
                "models/qwen3-vl-8b/Qwen3VL-8B-Instruct-Q4_K_M.gguf",
            ),
            ("mlx", "models/gemma-4-E4B-it"),
        ] {
            let model = Model {
                provider: provider.into(),
                model: provider.into(),
                path: Some(path.into()),
                adapter: None,
                context_tokens: None,
                local_reasoning_tokens: None,
                local_final_tokens: None,
                url: None,
                key_env: None,
            };
            let reply = runtime
                .complete_scoped(
                    &model,
                    "Reply briefly.",
                    "Say OK.",
                    16,
                    &cancel,
                    &format!("test-{provider}"),
                )
                .await
                .unwrap();
            assert!(!reply.text.trim().is_empty());
            assert_eq!(reply.usage.cost_usd, Some(0.0));
            println!("{provider}: {}", reply.text);
        }
        runtime.stop_owned().await;
    }
}
#[test]
fn focused_candidate_uses_the_same_strict_source_output_schema() {
    let full = schema_for("LOCAL_CANDIDATE_AUTHOR: source only").unwrap();
    let focused = schema_for("LOCAL_CANDIDATE_AUTHOR_FOCUSED: source only").unwrap();
    assert_eq!(full, focused);
    assert_eq!(focused["additionalProperties"], false);
    assert!(focused["properties"].get("prompt_profile").is_none());
}

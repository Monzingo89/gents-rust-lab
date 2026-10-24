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
}

pub struct LocalRuntime {
    root: PathBuf,
    binary: Option<PathBuf>,
    unavailable: Option<String>,
    port: u16,
    client: reqwest::Client,
    api_key: String,
    state: Mutex<State>,
}

impl LocalRuntime {
    pub fn new(root: PathBuf) -> Result<Self> {
        let root = root.canonicalize().context("resolve repository root")?;
        let requested = std::env::var_os("LLAMA_SERVER").map(PathBuf::from);
        let binary = requested.or_else(|| find_on_path("llama-server"));
        let unavailable = match &binary {
            None => Some("llama-server was not found; install a llama.cpp build with Metal support or set LLAMA_SERVER to its executable path".into()),
            Some(path) => validate_flags(path).err().map(|e| e.to_string()),
        };
        let port = std::env::var("LOCAL_MODEL_PORT")
            .unwrap_or_else(|_| "8791".into())
            .parse::<u16>()
            .context("LOCAL_MODEL_PORT must be a valid port")?;
        Ok(Self {
            root,
            binary,
            unavailable,
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
            }),
        })
    }

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
        let mut state = self.state.lock().await;
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
            return Err(error.context("GPU, reduced GPU offload, and CPU load attempts exhausted"));
        }
        let start = Instant::now();
        let mut body = json!({
            "model": model.model,
            "messages": [
                {"role":"system","content":system},
                {"role":"user","content":input}
            ],
            "max_tokens": max_tokens,
            "temperature": 0.1,
            "stream": false
        });
        if model.provider == "mlx" {
            // MLX maps this owned alias to BOTH the local base and its CLI adapter.
            // Sending only the base path would silently omit the configured adapter.
            body["model"] = json!("default_model");
        }
        if let Some(schema) = schema_for(system).filter(|_| model.provider != "mlx") {
            body["response_format"] = json!({
                "type":"json_schema",
                "json_schema":{"name":"gents_response","strict":true,"schema":schema}
            });
        }
        let send = self
            .client
            .post(format!(
                "http://127.0.0.1:{}/v1/chat/completions",
                self.port
            ))
            .json(&body)
            .bearer_auth(&self.api_key)
            .send();
        let request = async {
            let response = send.await.context("local inference request failed")?;
            let status = response.status();
            let bytes = response
                .bytes()
                .await
                .context("read local inference response")?;
            Ok::<_, anyhow::Error>((status, bytes))
        };
        tokio::pin!(request);
        let (status, bytes) = loop {
            tokio::select! {
                result = &mut request => break result.context("local inference request failed")?,
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if cancel.load(Ordering::Relaxed) {
                        stop(&mut state).await;
                        bail!("local inference cancelled; owned llama-server stopped");
                    }
                }
            }
        };
        if bytes.len() > 2 * 1024 * 1024 {
            bail!("local model response exceeds 2 MiB");
        }
        if !status.is_success() {
            let detail = String::from_utf8_lossy(&bytes);
            bail!(
                "llama-server HTTP {}: {}",
                status.as_u16(),
                clip(&detail, 1000)
            );
        }
        let value: Value =
            serde_json::from_slice(&bytes).context("llama-server returned invalid JSON")?;
        let text = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .context("llama-server returned no assistant text")?
            .to_string();
        let usage = &value["usage"];
        Ok(Reply {
            text,
            latency_ms: start.elapsed().as_millis() as u64,
            response_id: value["id"].as_str().map(str::to_string),
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
            if state.session.starts_with(prefix) {
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

    fn resolve_model(&self, configured: &str, provider: &str) -> Result<PathBuf> {
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
        let log_path = self.root.join("data/llama-server.log");
        let stdout = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr = stdout.try_clone()?;
        let mut command = tokio::process::Command::new(binary);
        let mut args = server_args(path, self.port)?;
        if let Some(index) = args.iter().position(|a| a == "--n-gpu-layers") {
            args[index + 1] = match state.recovery_level {
                0 => "999",
                1 => "16",
                _ => "0",
            }
            .into();
        }
        command.args(args);
        // No inherited HF routing, remote endpoints, API keys, or model auto-downloads.
        command
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1");
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LLAMA_") {
                command.env_remove(key);
            }
        }
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
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| p.is_file())
    })
}

fn validate_flags(path: &Path) -> Result<()> {
    let output = std::process::Command::new(path)
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
    Ok(())
}

fn server_args(path: &Path, port: u16) -> Result<Vec<String>> {
    Ok(vec![
        "--model".into(),
        path.to_str().context("model path is not UTF-8")?.into(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "--ctx-size".into(),
        "16384".into(),
        "--parallel".into(),
        "1".into(),
        "--n-gpu-layers".into(),
        "999".into(),
        "--reasoning".into(),
        "off".into(),
    ])
}

fn clip(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn schema_for(system: &str) -> Option<Value> {
    let object = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    if system.starts_with("COMPANY_ADMIN") {
        Some(object(
            json!({"decision":{"type":"string","enum":["allow","ask","deny"]},"reason":{"type":"string"},"question":{"type":"string"}}),
            vec!["decision", "reason", "question"],
        ))
    } else if system.starts_with("COMPANY_OPERATOR") {
        let mut schema = object(
            json!({"action":{"type":"string","enum":["web_search","web_read","browser","files","journal_read","shell","resources","models_search","model_download","model_quantize","model_select","email_list","email_send","email_status","schedule","training_prepare","training_run","training_status","training_promote","process_stop","ask_user","finish"]},"arguments_json":{"type":"string"},"summary":{"type":"string"}}),
            vec!["action", "arguments_json", "summary"],
        );
        if system.contains("LOOP RECOVERY:") {
            if let Some(cooldown) = system
                .split_once("\nRECOVERY_TOOL_COOLDOWN=")
                .and_then(|(_, v)| serde_json::from_str::<Vec<String>>(v).ok())
            {
                if let Some(actions) = schema["properties"]["action"]["enum"].as_array_mut() {
                    actions.retain(|v| !cooldown.iter().any(|blocked| v == blocked));
                }
            }
        }
        Some(schema)
    } else if system.starts_with("COMPANY_CEO") {
        Some(object(
            json!({"all_hands":{"type":"boolean"},"next_actions":{"type":"array","maxItems":6,"items":{"type":"string"}}}),
            vec!["all_hands", "next_actions"],
        ))
    } else if system.starts_with("COMPANY_JUDGE") {
        Some(object(
            json!({"verdict":{"type":"string","enum":["accepted","revise","uncertain"]},"summary":{"type":"string"},"checks":{"type":"array","items":object(json!({"criterion_index":{"type":"integer"},"passed":{"type":"boolean"},"evidence":{"type":"string"}}),vec!["criterion_index","passed","evidence"])}}),
            vec!["verdict", "summary", "checks"],
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn disables_reasoning_for_schema_constrained_local_calls() {
        let args = server_args(Path::new("/tmp/router.gguf"), 8791).unwrap();
        assert!(args.windows(2).any(|pair| pair == ["--reasoning", "off"]));
    }
    #[test]
    fn recovery_decoder_cannot_emit_cooled_tools_or_change_admin_authorization() {
        let schema=schema_for("COMPANY_OPERATOR: LOOP RECOVERY:\nRECOVERY_TOOL_COOLDOWN=[\"web_search\",\"schedule\"]").unwrap();
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        assert!(!actions.contains(&json!("web_search")));
        assert!(!actions.contains(&json!("schedule")));
        assert!(actions.contains(&json!("web_read")));
        assert!(actions.contains(&json!("resources")));
        let regular = schema_for("COMPANY_OPERATOR: Next action").unwrap();
        assert!(regular["properties"]["action"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("web_search")));
        assert_eq!(
            schema_for("COMPANY_ADMIN: LOOP RECOVERY:\nRECOVERY_TOOL_COOLDOWN=[\"allow\"]")
                .unwrap()["properties"]["decision"]["enum"],
            json!(["allow", "ask", "deny"])
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
            url: None,
            key_env: None,
        };
        let result=runtime.complete(&model,"COMPANY_ADMIN: Review a requested read-only public website visit. Return bare JSON {decision:allow|ask|deny,reason:string,question:string}. Allow an authorized public page read.","The operator requested reading https://example.com. Proposed action: web_read https://example.com. No credentials or writes.",128,&AtomicBool::new(false)).await;
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

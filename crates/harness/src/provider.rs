//! The harness depends on this trait, never on a vendor SDK.
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub provider: String,
    pub model: String,
    pub url: Option<String>, // full chat-completions URL, operator configured
    pub key_env: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub adapter: Option<String>,
    /// Operator/registry-selected native context budget; never inferred from an alias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<u32>,
    /// Trusted local role configuration only; absent or zero keeps thinking off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_reasoning_tokens: Option<u32>,
    /// Trusted native profile override of the call's default final allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_final_tokens: Option<u32>,
}
impl Model {
    pub fn validate(&self) -> Result<()> {
        if self
            .local_final_tokens
            .is_some_and(|v| self.provider != "local" || !(1..=4096).contains(&v))
        {
            bail!("local_final_tokens requires native local inference and 1..4096 tokens");
        }
        if self
            .local_reasoning_tokens
            .is_some_and(|v| self.provider != "local" || v > 4096)
        {
            bail!("local_reasoning_tokens requires native local inference and 0..4096 tokens");
        }
        if self
            .context_tokens
            .is_some_and(|tokens| self.provider != "local" || !(2048..=16384).contains(&tokens))
        {
            bail!(
                "context_tokens currently requires native local inference and 2048..16384 tokens"
            );
        }
        if self.adapter.is_some() && self.provider != "mlx" {
            bail!("role adapters currently require MLX");
        }
        if self.model.trim().is_empty() || self.model.len() > 256 {
            bail!("invalid model ID");
        }
        if self.provider == "mock" {
            return Ok(());
        }
        if matches!(self.provider.as_str(), "local" | "mlx") {
            let path = self.path.as_deref().context("local model requires path")?;
            if path.trim().is_empty() || (self.provider == "local" && !path.ends_with(".gguf")) {
                bail!("local requires a GGUF file; mlx requires a local model directory");
            }
            if self.url.is_some() || self.key_env.is_some() {
                bail!("local models do not accept URL or credential configuration");
            }
            return Ok(());
        }
        let url = reqwest::Url::parse(self.url.as_deref().context("live model requires URL")?)?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.port().is_some_and(|p| p != 443)
        {
            bail!("provider URL requires HTTPS, port 443 and no embedded credentials/fragment");
        }
        if url.query_pairs().any(|(key, _)| key != "api-version") {
            bail!("provider URL permits only api-version query parameter; credentials belong in headers");
        }
        let host = url.host_str().context("provider URL requires a hostname")?;
        let allowed = match self.provider.as_str() {
            "openrouter" => host == "openrouter.ai",
            "huggingface" => {
                host == "router.huggingface.co" || host.ends_with(".endpoints.huggingface.cloud")
            }
            "databricks" => host.ends_with(".azuredatabricks.net"),
            "azure" => [
                ".openai.azure.com",
                ".services.ai.azure.com",
                ".inference.ai.azure.com",
            ]
            .iter()
            .any(|suffix| host.ends_with(suffix)),
            _ => false,
        };
        let extra = std::env::var("PROVIDER_ALLOWED_HOSTS")
            .unwrap_or_default()
            .split(',')
            .any(|h| !h.is_empty() && h == host);
        if !allowed && !extra {
            bail!("provider hostname is not approved; operators may explicitly set PROVIDER_ALLOWED_HOSTS");
        }
        let env = self
            .key_env
            .as_deref()
            .context("live model requires key_env")?;
        if env.is_empty()
            || env.len() > 128
            || !env
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
        {
            bail!("invalid credential environment-variable name");
        }
        Ok(())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cached_input: Option<u64>,
    pub reasoning: Option<u64>,
    pub cost_usd: Option<f64>,
    pub source: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Reply {
    pub text: String,
    pub usage: Usage,
    pub latency_ms: u64,
    pub response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_execution: Option<LocalExecution>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalExecution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_input_sha256: Option<String>,
    pub backend_pid: u32,
    pub port: u16,
    pub dispatched_at_ms: u64,
    pub received_at_ms: u64,
    pub resident_kib: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_policy: Option<crate::runtime_fit::LaunchPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_context_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_budget: Option<crate::local_generation::GenerationBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separated_reasoning_observed: Option<bool>,
}
#[async_trait]
pub trait LanguageModel: Send + Sync {
    async fn complete(
        &self,
        model: &Model,
        system: &str,
        input: &str,
        max_tokens: u32,
        cancel: &AtomicBool,
    ) -> Result<Reply>;
}
pub struct Gateway {
    pub client: reqwest::Client,
    pub local: crate::runtime_pool::RuntimePool,
}
#[async_trait]
impl LanguageModel for Gateway {
    async fn complete(
        &self,
        m: &Model,
        system: &str,
        input: &str,
        max_tokens: u32,
        cancel: &AtomicBool,
    ) -> Result<Reply> {
        m.validate()?;
        let start = Instant::now();
        if m.provider == "mock" {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let text = if system.starts_with("COMPANY_ADMIN") {
                if m.model == "mock-invalid-authorization" {
                    bail!("SIMULATED authorization preparation failure; no verdict or tool effect");
                }
                let denied = input.contains("deny-admin-smoke");
                json!({"decision":if denied{"deny"}else{"allow"},"reason":"SIMULATED admin fixture; not real-model validation","question":""}).to_string()
            } else if system.starts_with("COMPANY_OPERATOR") {
                let value = crate::context::task_input(input)?;
                let evidence = value["recent_evidence"]
                    .as_array()
                    .context("mock evidence missing")?;
                let loop_fixture = value["task"]
                    .as_str()
                    .is_some_and(|s| s.contains("autonomy-loop-smoke"));
                let stalled_fixture = value["task"]
                    .as_str()
                    .is_some_and(|s| s.contains("autonomy-stall-smoke"));
                let tool_failure_fixture = value["task"]
                    .as_str()
                    .is_some_and(|s| s.contains("autonomy-tool-failure-smoke"));
                let answered = loop_fixture
                    || evidence.iter().any(|e| e.get("operator_answer").is_some())
                    || value["operator_answers"]
                        .as_array()
                        .is_some_and(|a| !a.is_empty());
                let written = evidence
                    .iter()
                    .any(|e| e["action"] == "files" && e["arguments"]["op"] == "write");
                let tested = evidence.iter().any(|e| e["action"] == "shell");
                let artifact_stall = value["task"]
                    .as_str()
                    .is_some_and(|s| s.contains("autonomy-artifact-stall-smoke"));
                let (action, args) = if m.model == "mock-proposal-feedback" {
                    let feedback = &value["proposal_feedback"][0];
                    anyhow::ensure!(
                        feedback["executed"] == false
                            && feedback["proposed_source_policy"]["passed"] == false
                            && feedback["proposed_source_policy"]["diagnostic"]
                                .as_str()
                                .is_some_and(|s| s.contains("todo!"))
                            && feedback.get("content").is_none(),
                        "SIMULATED worker did not receive bounded parser feedback"
                    );
                    (
                        "ask_user",
                        json!({"question":"Which synthetic recovery marker should the fixture record?"}),
                    )
                } else if artifact_stall {
                    (
                        "finish",
                        json!({"deliverable":"SIMULATED premature finish; required report is intentionally absent","evidence":"No workspace output created"}),
                    )
                } else if tool_failure_fixture {
                    ("files", json!({"op":"read","path":"missing-fixture.txt"}))
                } else if stalled_fixture {
                    ("missing-capability-fixture", json!({}))
                } else if loop_fixture
                    && value["recovery_attempt"].as_u64().unwrap_or(0) == 0
                    && value["verified_progress"]["tool_results"]
                        .as_u64()
                        .unwrap_or(0)
                        <= 2
                {
                    ("files", json!({"op":"list","path":""}))
                } else if !answered {
                    (
                        "ask_user",
                        json!({"question":"Which project name should the smoke fixture use?"}),
                    )
                } else if !written {
                    (
                        "files",
                        json!({"op":"write","path":"deliverable.txt","content":"Actual file written by simulated model tool proposal"}),
                    )
                } else if !tested {
                    ("shell", json!({"command":"/usr/bin/printf verified"}))
                } else {
                    (
                        "finish",
                        json!({"deliverable":"SIMULATED model fixture with real workspace artifact and shell evidence","evidence":"deliverable.txt"}),
                    )
                };
                json!({"action":action,"arguments_json":args.to_string(),"summary":"SIMULATED next action"}).to_string()
            } else if system.starts_with("COMPANY_JUDGE")
                && (m.model == "mock-review-protocol"
                    || m.model == "mock-review-protocol-always-invalid")
            {
                let value = crate::context::task_input(input)?;
                let count = crate::completion_review::count(&value["criteria"])?;
                let corrected = value["review_protocol_feedback"].is_object()
                    && m.model != "mock-review-protocol-always-invalid";
                let checks = if corrected {
                    (0..count)
                        .map(|i| {
                            (
                                i.to_string(),
                                json!({"passed":true,"evidence":"SIMULATED protocol fixture only"}),
                            )
                        })
                        .collect::<serde_json::Map<String, Value>>()
                } else {
                    [(
                        count.to_string(),
                        json!({"passed":true,"evidence":"SIMULATED missing/extra key fault"}),
                    )]
                    .into_iter()
                    .collect()
                };
                json!({"verdict":"accepted","summary":"SIMULATED review protocol fixture; not real-model task validation","checks":checks}).to_string()
            } else if system.starts_with("COMPANY_JUDGE") && m.model == "mock-qa-revise" {
                let value = crate::context::task_input(input)?;
                let count = crate::completion_review::count(&value["criteria"])?;
                let checks=(0..count).map(|i|(i.to_string(),json!({"passed":false,"evidence":"SIMULATED repeated QA failure; no competence claimed"}))).collect::<serde_json::Map<String,Value>>();
                json!({"verdict":"revise","summary":"SIMULATED final review rejection","checks":checks}).to_string()
            } else if system.starts_with("COMPANY_JUDGE")
                && input.contains("SIMULATED model fixture")
            {
                let value = crate::context::task_input(input)?;
                let count = crate::completion_review::count(&value["criteria"])?;
                let checks=(0..count).map(|i|(i.to_string(),json!({"passed":true,"evidence":"SIMULATED semantic review; actual file/shell output checked separately"}))).collect::<serde_json::Map<String,Value>>();
                json!({"verdict":"accepted","summary":"SIMULATED QA fixture","checks":checks})
                    .to_string()
            } else if system.starts_with("CTO_ROUTER") {
                let v: Value = serde_json::from_str(input)?;
                let round = v["round"].as_u64().unwrap_or(1);
                let catalog = v["catalog"].as_object().context("mock catalog missing")?;
                let planner: Vec<_> = catalog
                    .iter()
                    .filter(|(_, c)| {
                        c["allowed_roles"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|v| v == "planner"))
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                let worker: Vec<_> = catalog
                    .iter()
                    .filter(|(_, c)| {
                        c["allowed_roles"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|v| v == "worker"))
                    })
                    .map(|(k, _)| k.clone())
                    .collect();
                if planner.is_empty() || worker.is_empty() {
                    bail!("mock catalog requires planner and worker candidates");
                }
                let roster = v["roster"].as_array().context("mock roster missing")?;
                let selected: Vec<_> = if round == 1 {
                    roster.iter().take(2).collect()
                } else {
                    roster.iter().rev().take(1).collect()
                };
                let mut deployments = vec![];
                for (i, a) in selected.iter().enumerate() {
                    let sources = if i > 0 {
                        vec![selected[i - 1]["agent_id"].as_str().unwrap()]
                    } else if round > 1 {
                        vec!["previous_round", "feedback"]
                    } else {
                        vec![]
                    };
                    deployments.push(json!({"agent_id":a["agent_id"],"planner_model":if m.model=="mock-invalid-router" {"invented".to_string()} else {planner[(round as usize*2+i-2)%planner.len()].clone()},"worker_model":worker[(round as usize*2+i-1)%worker.len()],"assignment":if round==1 {"Draft or integrate the requested result"}else{"Repair the previous result using completion feedback"},"context_from":sources}));
                }
                let criteria = if v["definition_of_done"]
                    .as_array()
                    .is_some_and(|a| !a.is_empty())
                {
                    v["definition_of_done"].clone()
                } else {
                    json!([
                        "Address the original task using supplied facts",
                        "Explain the recommendation and limitations"
                    ])
                };
                json!({"summary":"SIMULATED routing: two agents first, then one repair agent with different model slots","definition_of_done":criteria,"deployments":deployments}).to_string()
            } else if system.starts_with("COMPLIANCE_SECURITY_QA") {
                let v: Value = serde_json::from_str(input)?;
                let pass = v["round"].as_u64().unwrap_or(1) > 1 && m.model != "mock-reject-always";
                let checks:Vec<_>=v["definition_of_done"].as_array().context("mock criteria missing")?.iter().enumerate().map(|(i,_)|json!({"criterion_index":i,"passed":pass||i>0,"evidence":if pass||i>0 {"SIMULATED evidence for lifecycle demonstration; not a real quality assessment"}else{"SIMULATED first-round gap to demonstrate repair"}})).collect();
                if m.model == "mock-invalid-judge" {
                    json!({"verdict":"accepted","summary":"invalid simulated review","checks":[],"remaining_work":[]}).to_string()
                } else {
                    json!({"verdict":if pass {"accepted"}else{"revise"},"summary":if pass {"SIMULATED acceptance after repair"}else{"SIMULATED revision required"},"checks":checks,"remaining_work":if pass {json!([])}else{json!(["Repair the first criterion using the previous candidate"])}}).to_string()
                }
            } else if system.starts_with("PLANNER") {
                json!({"summary":"Use the supplied task and evidence, then check the result.","steps":["Draft a concise answer using the supplied evidence","Check the draft against the task and return the final answer"]}).to_string()
            } else {
                format!("MOCK OUTPUT — {}\n\nThis is a deterministic simulation. Replace the mock model to produce a real answer.\nInput received: {}", m.model, input.chars().take(180).collect::<String>())
            };
            return Ok(Reply {
                text,
                usage: Usage {
                    source: "mock: token counts unavailable; no provider charge".into(),
                    cost_usd: Some(0.0),
                    ..Usage::default()
                },
                latency_ms: start.elapsed().as_millis() as u64,
                response_id: None,
                local_execution: None,
            });
        }
        if matches!(m.provider.as_str(), "local" | "mlx") {
            return self
                .local
                .complete(m, system, input, max_tokens, cancel)
                .await;
        }
        if std::env::var("GENTS_LOCAL_ONLY").as_deref() != Ok("0") {
            bail!(
                "Cloud inference is disabled. GENTS_LOCAL_ONLY=0 is an explicit operator override"
            );
        }
        let url = m.url.as_deref().context("missing provider URL")?;
        if !url.starts_with("https://") {
            bail!("live provider requires HTTPS");
        }
        let key = std::env::var(m.key_env.as_deref().context("missing key_env")?)
            .context("missing provider credential")?;
        let mut body = json!({"model":m.model,"messages":crate::context::messages(system,input),"max_tokens":max_tokens,"stream":false});
        if m.provider == "openrouter" {
            body["usage"] = json!({"include":true});
        }
        // No automatic retries: a failed response can still have incurred a charge.
        let response = self
            .client
            .post(url)
            .bearer_auth(key)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            bail!(
                "provider HTTP {} (response body omitted to protect data)",
                status.as_u16()
            );
        }
        let mut response = response;
        if response
            .content_length()
            .is_some_and(|n| n > 2 * 1024 * 1024)
        {
            bail!("provider response exceeds 2 MiB");
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                bail!("provider response exceeds 2 MiB");
            }
            bytes.extend_from_slice(&chunk);
        }
        let v: Value = serde_json::from_slice(&bytes)?;
        let text = v
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .context("provider returned no text; this adapter requires text chat completions")?
            .to_string();
        if text.chars().count() > 65536 {
            bail!("provider text exceeds 65536-character limit");
        }
        Ok(Reply {
            text,
            usage: parse_usage(&v),
            latency_ms: start.elapsed().as_millis() as u64,
            response_id: v["id"].as_str().map(str::to_string),
            local_execution: None,
        })
    }
}
fn parse_usage(v: &Value) -> Usage {
    Usage {
        input: v.pointer("/usage/prompt_tokens").and_then(Value::as_u64),
        output: v
            .pointer("/usage/completion_tokens")
            .and_then(Value::as_u64),
        cached_input: v
            .pointer("/usage/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_u64),
        reasoning: v
            .pointer("/usage/completion_tokens_details/reasoning_tokens")
            .and_then(Value::as_u64),
        cost_usd: v.pointer("/usage/cost").and_then(Value::as_f64),
        source: "provider response; missing fields are unknown".into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_allowance_override_is_explicit_bounded_and_native_only() {
        let mut model: Model = serde_json::from_value(
            json!({"provider":"local","model":"fixture","path":"models/fixture.gguf"}),
        )
        .unwrap();
        assert!(model.local_final_tokens.is_none());
        assert!(serde_json::to_value(&model)
            .unwrap()
            .get("local_final_tokens")
            .is_none());
        for cap in [1, 2048, 4096] {
            model.local_final_tokens = Some(cap);
            model.validate().unwrap();
        }
        for cap in [0, 4097, u32::MAX] {
            model.local_final_tokens = Some(cap);
            assert!(model.validate().is_err());
        }
        model.local_final_tokens = Some(2048);
        for provider in ["mock", "mlx", "azure", "databricks"] {
            model.provider = provider.into();
            assert!(model.validate().is_err());
        }
    }
    #[test]
    fn reasoning_is_a_bounded_native_host_setting() {
        let original =
            json!({"provider":"local","model":"candidate","path":"models/candidate.gguf"});
        let mut model: Model = serde_json::from_value(original).unwrap();
        assert!(model.local_reasoning_tokens.is_none());
        for cap in [0, 512, 4096] {
            model.local_reasoning_tokens = Some(cap);
            model.validate().unwrap();
        }
        model.local_reasoning_tokens = Some(4097);
        assert!(model.validate().is_err());
        model.local_reasoning_tokens = Some(512);
        for provider in ["mock", "mlx", "azure", "databricks"] {
            model.provider = provider.into();
            assert!(model.validate().is_err());
        }
    }
    #[test]
    fn model_specific_context_budget_is_explicit_and_native_only() {
        let model = json!({"provider":"local","model":"candidate","path":"models/candidate.gguf","context_tokens":8192});
        let mut value: Model = serde_json::from_value(model).unwrap();
        value.validate().unwrap();
        for cap in [0, 1024, 16385] {
            value.context_tokens = Some(cap);
            assert!(value.validate().is_err());
        }
        value.context_tokens = Some(8192);
        value.provider = "mlx".into();
        assert!(value.validate().is_err());
    }
    #[test]
    fn missing_usage_is_unknown() {
        let u = parse_usage(&json!({}));
        assert_eq!(u.input, None);
        assert_eq!(u.cost_usd, None);
    }
    #[test]
    fn details_are_subsets_not_extra_tokens() {
        let u = parse_usage(
            &json!({"usage":{"prompt_tokens":100,"completion_tokens":50,"completion_tokens_details":{"reasoning_tokens":30}}}),
        );
        assert_eq!(u.output, Some(50));
        assert_eq!(u.reasoning, Some(30));
    }
    #[test]
    fn rejects_embedded_credentials_and_unapproved_provider_hosts() {
        let mut m = Model {
            provider: "openrouter".into(),
            model: "test".into(),
            url: Some("https://openrouter.ai/api/v1/chat/completions".into()),
            key_env: Some("TEST_KEY".into()),
            path: None,
            adapter: None,
            context_tokens: None,
            local_reasoning_tokens: None,
            local_final_tokens: None,
        };
        assert!(m.validate().is_ok());
        m.url = Some("https://user:secret@openrouter.ai/api/v1/chat/completions".into());
        assert!(m.validate().is_err());
        m.url = Some("https://unapproved.example/chat".into());
        assert!(m.validate().is_err());
        m.url = Some("https://openrouter.ai/api/v1/chat/completions?api_key=secret".into());
        assert!(m.validate().is_err());
    }
}

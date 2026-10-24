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
}
impl Model {
    pub fn validate(&self) -> Result<()> {
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
    pub local: crate::local_runtime::LocalRuntime,
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
                let denied = input.contains("deny-admin-smoke");
                json!({"decision":if denied{"deny"}else{"allow"},"reason":"SIMULATED admin fixture; not real-model validation","question":""}).to_string()
            } else if system.starts_with("COMPANY_OPERATOR") {
                let outer: Value = serde_json::from_str(input)?;
                let value: Value = if let Some(nested) = outer["task_input"].as_str() {
                    serde_json::from_str(nested)?
                } else if outer["task_input"].is_object() {
                    outer["task_input"].clone()
                } else {
                    outer
                };
                let evidence = value["recent_evidence"]
                    .as_array()
                    .context("mock evidence missing")?;
                let loop_fixture = value["task"]
                    .as_str()
                    .is_some_and(|s| s.contains("autonomy-loop-smoke"));
                let answered = loop_fixture
                    || evidence.iter().any(|e| e.get("operator_answer").is_some())
                    || value["operator_answers"]
                        .as_array()
                        .is_some_and(|a| !a.is_empty());
                let written = evidence
                    .iter()
                    .any(|e| e["action"] == "files" && e["arguments"]["op"] == "write");
                let tested = evidence.iter().any(|e| e["action"] == "shell");
                let (action, args) = if loop_fixture
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
                && input.contains("SIMULATED model fixture")
            {
                let outer: Value = serde_json::from_str(input)?;
                let value: Value = if let Some(nested) = outer["task_input"].as_str() {
                    serde_json::from_str(nested)?
                } else {
                    outer
                };
                let checks=value["criteria"].as_array().context("mock criteria missing")?.iter().enumerate().map(|(i,_)|json!({"criterion_index":i,"passed":true,"evidence":"SIMULATED semantic review; real file/shell output checked separately by smoke script"})).collect::<Vec<_>>();
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
        let mut body = json!({"model":m.model,"messages":[{"role":"system","content":system},{"role":"user","content":input}],"max_tokens":max_tokens,"stream":false});
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

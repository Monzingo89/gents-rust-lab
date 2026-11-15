//! Trusted-host generation limits, independent of device fitting and tool authority.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationBudget {
    pub reasoning_tokens: u32,
    pub final_tokens_reserved: u32,
    pub total_generation_tokens: u32,
}
impl GenerationBudget {
    pub fn for_model(model: &crate::provider::Model, default_final: u32) -> Result<Self> {
        model.validate()?;
        Self::new(
            model.local_reasoning_tokens,
            model.local_final_tokens.unwrap_or(default_final),
        )
    }
    pub fn new(reasoning: Option<u32>, final_tokens: u32) -> Result<Self> {
        ensure!(
            final_tokens > 0 && final_tokens <= 4096,
            "invalid final output allowance"
        );
        let reasoning_tokens = reasoning.unwrap_or(0);
        ensure!(
            reasoning_tokens <= 4096,
            "local reasoning budget exceeds 4096 tokens"
        );
        Ok(Self {
            reasoning_tokens,
            final_tokens_reserved: final_tokens,
            total_generation_tokens: final_tokens + reasoning_tokens,
        })
    }
    pub fn enabled(&self) -> bool {
        self.reasoning_tokens > 0
    }
    pub fn template_kwargs(&self) -> Value {
        json!({"enable_thinking":self.enabled(),"preserve_thinking":false})
    }
    pub fn arguments(&self, supported: bool) -> Result<Vec<String>> {
        if !self.enabled() {
            return Ok(vec!["--reasoning".into(), "off".into()]);
        }
        ensure!(
            supported,
            "owned backend does not advertise bounded separated reasoning; no inference dispatched"
        );
        Ok(vec![
            "--reasoning".into(),
            "on".into(),
            "--reasoning-format".into(),
            "deepseek".into(),
            "--reasoning-budget".into(),
            self.reasoning_tokens.to_string(),
            "--no-reasoning-preserve".into(),
            "--log-disable".into(),
        ])
    }
}

pub fn supports_bounded_reasoning(help: &str) -> bool {
    [
        "--reasoning-format",
        "--reasoning-budget",
        "--no-reasoning-preserve",
        "--log-disable",
    ]
    .iter()
    .all(|flag| help.split_whitespace().any(|word| word == *flag))
        && help.contains("deepseek")
}

/// Private reasoning text is never returned or copied into durable receipts.
pub fn final_content(value: &Value, separated: bool) -> Result<(&str, bool)> {
    let observed = value
        .pointer("/choices/0/message/reasoning_content")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    if value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        == Some("length")
    {
        // Even syntactically complete text at a cutoff is not a usable reply.
        // Preserve usage/provenance in the receipt, never the partial payload.
        return Ok(("", observed));
    }
    let text = value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .context("local backend returned no final assistant content")?;
    if separated {
        ensure!(!text.is_empty() && !text.contains("<think>") && !text.contains("</think>"),
            "bounded local reasoning did not return separated final content; private reasoning discarded");
    }
    Ok((text, observed))
}

#[derive(Debug)]
pub struct GenerationCutoff {
    pub budget: Option<GenerationBudget>,
    pub output_tokens: Option<u64>,
}
impl std::fmt::Display for GenerationCutoff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Local generation reached its configured output limit; incomplete content discarded before use")
    }
}
impl std::error::Error for GenerationCutoff {}
impl GenerationCutoff {
    pub fn from_reply(reply: &crate::provider::Reply) -> Option<Self> {
        let execution = reply.local_execution.as_ref()?;
        (execution.finish_reason.as_deref() == Some("length")).then(|| Self {
            budget: execution.generation_budget.clone(),
            output_tokens: reply.usage.output,
        })
    }
    pub fn feedback(&self) -> Value {
        json!({"finish_reason":"length","generation_budget":self.budget,"observed_output_tokens":self.output_tokens,"executed":false,"scope":"Prior native generation cutoff, not RAM exhaustion, a tool result, authority or task completion. Local output allowance is trusted host configuration; do not alter the frozen task or guards."})
    }
}

pub fn latest_cutoff(history: &[Value], actor: &str, stage: usize) -> Option<Value> {
    history.iter().rev().find_map(|item| {
        let value = item.get("generation_cutoff")?;
        (value["actor"] == actor && value["stage"].as_u64() == Some(stage as u64))
            .then(|| value.clone())
    })
}

/// Numeric diagnostics only: never include prompts, paths or backend error text.
#[derive(Debug)]
pub struct PromptBudgetExceeded {
    pub prompt_tokens: usize,
    pub available_tokens: usize,
    pub context_tokens: usize,
    pub generation_tokens: u32,
}
impl std::fmt::Display for PromptBudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Required context uses {} native prompt tokens; the selected model permits {} after output reservation. Frozen requirements were not truncated", self.prompt_tokens, self.available_tokens)
    }
}
impl std::error::Error for PromptBudgetExceeded {}

/// Closed diagnostic vocabulary. No error strings or response content cross this boundary.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    RequestSend,
    ResponseRead,
    ResponseLimit,
    HttpStatus,
    ResponseJson,
    FinalContent,
    Cancelled,
}
#[derive(Clone, Debug, Serialize)]
pub struct FailureUsage {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cached_input: Option<u64>,
    pub reasoning: Option<u64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct InferenceFailure {
    pub stage: FailureStage,
    pub backend_pid: u32,
    pub port: u16,
    pub request_attempted_at_ms: u64,
    pub observed_at_ms: u64,
    pub http_status: Option<u16>,
    pub response_bytes: usize,
    pub generation_budget: GenerationBudget,
    pub usage: Option<FailureUsage>,
}
impl InferenceFailure {
    pub fn observe(&self, stage: FailureStage) -> Self {
        Self {
            stage,
            observed_at_ms: crate::now_ms(),
            ..self.clone()
        }
    }
    pub fn record_usage(&mut self, value: &Value) {
        let Some(usage) = value.get("usage").filter(|v| v.is_object()) else {
            return;
        };
        let receipt = FailureUsage {
            input: usage["prompt_tokens"].as_u64(),
            output: usage["completion_tokens"].as_u64(),
            cached_input: usage
                .pointer("/prompt_tokens_details/cached_tokens")
                .and_then(Value::as_u64),
            reasoning: usage
                .pointer("/completion_tokens_details/reasoning_tokens")
                .and_then(Value::as_u64),
        };
        if receipt.input.is_some()
            || receipt.output.is_some()
            || receipt.cached_input.is_some()
            || receipt.reasoning.is_some()
        {
            self.usage = Some(receipt);
        }
    }
}
impl std::fmt::Display for InferenceFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Owned local inference failed at {:?}; response content discarded",
            self.stage
        )
    }
}
impl std::error::Error for InferenceFailure {}

pub fn planning_failure(error: &anyhow::Error) -> Value {
    if let Some(budget) = error.downcast_ref::<PromptBudgetExceeded>() {
        json!({"code":"prompt_budget_exceeded","prompt_tokens":budget.prompt_tokens,"available_tokens":budget.available_tokens,"context_tokens":budget.context_tokens,"generation_tokens":budget.generation_tokens,"inference_dispatched":false,"retryable_without_host_change":false})
    } else if let Some(receipt) = error.downcast_ref::<InferenceFailure>() {
        json!({"code":"local_inference_failed","receipt":receipt,"retryable_without_host_change":false,"scope":"Request attempt and observed transport/response metadata only; backend execution and unreported token usage are unknown. No prompt, response content or backend error payload retained."})
    } else {
        json!({"code":"local_inference_failed","retryable_without_host_change":false,"scope":"No error payload persisted. Inspect owned runtime diagnostics; dispatch and usage are unknown."})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_final_content_keeps_only_measured_usage_and_closed_metadata() {
        let value = json!({"usage":{"prompt_tokens":23,"completion_tokens":7,"extra":"secret"},"choices":[{"message":{"reasoning_content":"secret"}}],"error":"secret"});
        assert!(final_content(&value, true).is_err());
        let mut receipt = InferenceFailure {
            stage: FailureStage::FinalContent,
            backend_pid: 123,
            port: 8797,
            request_attempted_at_ms: 10,
            observed_at_ms: 20,
            http_status: Some(200),
            response_bytes: 42,
            generation_budget: GenerationBudget::new(Some(512), 1024).unwrap(),
            usage: None,
        };
        receipt.record_usage(&value);
        let diagnostic = planning_failure(&anyhow::Error::new(receipt).context("secret"));
        assert_eq!(diagnostic["receipt"]["stage"], "final_content");
        assert_eq!(diagnostic["receipt"]["usage"]["input"], 23);
        assert_eq!(diagnostic["receipt"]["usage"]["output"], 7);
        assert!(!diagnostic.to_string().contains("secret"));
        assert!(diagnostic.get("local_execution").is_none());
        assert!(diagnostic.get("inference_dispatched").is_none());
    }
    #[test]
    fn planning_diagnostics_keep_numeric_budget_but_no_arbitrary_error_payload() {
        let error = anyhow::Error::new(PromptBudgetExceeded {
            prompt_tokens: 7100,
            available_tokens: 5888,
            context_tokens: 8192,
            generation_tokens: 2048,
        })
        .context("private task bytes");
        let diagnostic = planning_failure(&error);
        assert_eq!(diagnostic["code"], "prompt_budget_exceeded");
        assert_eq!(diagnostic["prompt_tokens"], 7100);
        assert_eq!(diagnostic["inference_dispatched"], false);
        assert_eq!(diagnostic["retryable_without_host_change"], false);
        assert!(!diagnostic.to_string().contains("private"));
        let diagnostic = planning_failure(&anyhow::anyhow!("secret backend payload"));
        assert!(!diagnostic.to_string().contains("secret"));
        assert!(diagnostic.get("inference_dispatched").is_none());
    }
    #[test]
    fn native_role_allowance_reserves_the_whole_reviewed_budget() {
        let mut model:crate::provider::Model=serde_json::from_value(json!({"provider":"local","model":"fixture","path":"models/fixture.gguf","local_reasoning_tokens":2048})).unwrap();
        assert_eq!(
            GenerationBudget::for_model(&model, 1024)
                .unwrap()
                .total_generation_tokens,
            3072
        );
        model.local_final_tokens = Some(2048);
        let selected = GenerationBudget::for_model(&model, 1024).unwrap();
        assert_eq!(selected.final_tokens_reserved, 2048);
        assert_eq!(selected.total_generation_tokens, 4096);
        model.local_final_tokens = Some(4097);
        assert!(GenerationBudget::for_model(&model, 1024).is_err());
    }
    #[test]
    fn cutoff_discards_even_complete_json_and_all_private_or_partial_content() {
        for content in [
            Value::Null,
            json!("{\"action\":\"finish\"}"),
            json!("<think>private sentinel</think>"),
        ] {
            let value = json!({"choices":[{"finish_reason":"length","message":{"content":content,"reasoning_content":"private sentinel"}}]});
            assert_eq!(final_content(&value, true).unwrap(), ("", true));
            assert_eq!(final_content(&value, false).unwrap().0, "");
        }
        let reply:crate::provider::Reply=serde_json::from_value(json!({"text":"discarded fixture","usage":{"source":"SIMULATED unit receipt","output":1},"latency_ms":1,"local_execution":{"backend_pid":1,"port":1,"dispatched_at_ms":1,"received_at_ms":2,"finish_reason":"length","generation_budget":{"reasoning_tokens":0,"final_tokens_reserved":1,"total_generation_tokens":1}}})).unwrap();
        let cutoff = GenerationCutoff::from_reply(&reply).unwrap();
        assert_eq!(
            cutoff.feedback()["generation_budget"]["total_generation_tokens"],
            1
        );
        assert_eq!(cutoff.feedback()["executed"], false);
        let error: anyhow::Error = cutoff.into();
        assert!(error.is::<GenerationCutoff>());
        assert!(!error.to_string().contains("discarded fixture"));
    }
    #[test]
    fn generation_feedback_is_actor_and_stage_scoped() {
        let history = vec![
            json!({"generation_cutoff":{"actor":"Author","stage":0,"finish_reason":"length"}}),
        ];
        assert!(latest_cutoff(&history, "Author", 0).is_some());
        assert!(latest_cutoff(&history, "Other", 0).is_none());
        assert!(latest_cutoff(&history, "Author", 1).is_none());
    }
    #[test]
    fn bounded_reasoning_is_explicit_and_separated_from_the_final_allowance() {
        let default = GenerationBudget::new(None, 1024).unwrap();
        assert_eq!(default.total_generation_tokens, 1024);
        assert_eq!(default.arguments(false).unwrap(), ["--reasoning", "off"]);
        let budget = GenerationBudget::new(Some(2048), 1024).unwrap();
        assert!(budget.arguments(false).is_err());
        assert_eq!(budget.total_generation_tokens, 3072);
        assert_eq!(
            budget.template_kwargs(),
            json!({"enable_thinking":true,"preserve_thinking":false})
        );
        assert!(budget
            .arguments(true)
            .unwrap()
            .windows(2)
            .any(|p| p == ["--reasoning-budget", "2048"]));
        assert!(GenerationBudget::new(Some(4097), 1024).is_err());
        assert!(GenerationBudget::new(Some(u32::MAX), u32::MAX).is_err());
        assert!(!supports_bounded_reasoning(
            "--reasoning-format deepseek --reasoning-budget"
        ));
        assert!(supports_bounded_reasoning(
            "--reasoning-format deepseek --reasoning-budget --no-reasoning-preserve --log-disable"
        ));
    }
    #[test]
    fn only_final_content_crosses_the_reply_boundary() {
        let value = json!({"choices":[{"message":{"content":"{\"decision\":\"deny\"}","reasoning_content":"private sentinel"}}]});
        let (text, observed) = final_content(&value, true).unwrap();
        assert!(observed);
        assert!(!text.contains("private sentinel"));
        for content in [
            "",
            "<think>private sentinel</think>",
            "unclosed <think>private sentinel",
        ] {
            let value = json!({"choices":[{"message":{"content":content}}]});
            let result = final_content(&value, true);
            assert!(result.is_err());
            assert!(!result.unwrap_err().to_string().contains("private sentinel"));
        }
    }
}

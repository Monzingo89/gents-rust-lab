//! Tokenizer-only offline preflight for the existing owned MLX runtime.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::Path,
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preflight {
    pub schema_version: u32,
    pub provider_version: String,
    pub tokenization_scheme: String,
    pub prompt_tokens: usize,
    pub model_context_limit: usize,
    pub has_chat_template: bool,
    pub weights_loaded: bool,
}
impl Preflight {
    pub fn allowance(&self, requested: Option<u32>, generation: u32) -> Result<usize> {
        ensure!(
            self.schema_version == 1
                && self.provider_version == "0.32.2"
                && self.tokenization_scheme == "mlx_lm.server/0.32.2"
                && !self.weights_loaded
                && (128..=10000000).contains(&self.model_context_limit),
            "unsupported MLX preflight identity"
        );
        let limit = requested.map_or(self.model_context_limit.min(16384), |n| n as usize);
        ensure!(
            limit <= self.model_context_limit,
            "configured MLX context exceeds pinned metadata"
        );
        let available = limit.saturating_sub(generation as usize + 256);
        if generation as usize + 256 >= limit || self.prompt_tokens > available {
            return Err(crate::local_generation::PromptBudgetExceeded {
                prompt_tokens: self.prompt_tokens,
                available_tokens: available,
                context_tokens: limit,
                generation_tokens: generation,
            }
            .into());
        }
        Ok(limit)
    }
}
pub async fn count(
    root: &Path,
    model: &Path,
    messages: &Value,
    kwargs: &Value,
    cancel: &AtomicBool,
) -> Result<Preflight> {
    let packet = serde_json::to_vec(&json!({"messages":messages,"chat_template_kwargs":kwargs}))?;
    ensure!(packet.len() <= 131072, "MLX tokenizer packet exceeds bound");
    let mut command = tokio::process::Command::new(root.join(".venv-inference/bin/python"));
    command
        .arg(root.join("scripts/mlx_prompt_tokens.py"))
        .arg(model)
        .env_clear()
        .envs(crate::runtime_environment::allowed(std::env::vars_os()))
        .env("PYTHONNOUSERSITE", "1")
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().context("owned MLX tokenizer unavailable")?;
    let mut stdin = child.stdin.take().context("tokenizer stdin missing")?;
    let stdout = child.stdout.take().context("tokenizer stdout missing")?;
    let exchange = async {
        use tokio::io::AsyncReadExt;
        stdin.write_all(&packet).await?;
        drop(stdin);
        let mut body = Vec::new();
        stdout.take(4097).read_to_end(&mut body).await?;
        ensure!(body.len() <= 4096, "tokenizer response exceeds bound");
        ensure!(
            child.wait().await?.success(),
            "MLX tokenizer preflight failed; helper loads no weights and this call dispatches no inference"
        );
        Ok::<_, anyhow::Error>(body)
    };
    tokio::pin!(exchange);
    let start = Instant::now();
    let body = loop {
        ensure!(
            !cancel.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(30),
            "MLX tokenizer cancelled or timed out"
        );
        tokio::select! {body=&mut exchange=>break body?,_=tokio::time::sleep(Duration::from_millis(20))=>{}}
    };
    let preflight: Preflight = serde_json::from_slice(&body)?;
    Ok(preflight)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_tokenizer_count_and_declared_context_bound_generation_without_truncation() {
        let mut p = Preflight {
            schema_version: 1,
            provider_version: "0.32.2".into(),
            tokenization_scheme: "mlx_lm.server/0.32.2".into(),
            prompt_tokens: 1000,
            model_context_limit: 2048,
            has_chat_template: false,
            weights_loaded: false,
        };
        assert_eq!(p.allowance(None, 512).unwrap(), 2048);
        assert!(p.allowance(Some(4096), 512).is_err());
        p.prompt_tokens = 1300;
        assert!(p.allowance(None, 512).is_err());
        p.prompt_tokens = 10;
        assert!(p.allowance(None, 2048).is_err());
        p.weights_loaded = true;
        assert!(p.allowance(None, 512).is_err());
        p.weights_loaded = false;
        p.provider_version = "unverified-runtime".into();
        assert!(p.allowance(None, 512).is_err());
    }
}

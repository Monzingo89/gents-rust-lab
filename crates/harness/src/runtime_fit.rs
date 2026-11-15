//! Backend capability adapter and launch policy, independent of agent authority.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

pub fn supports_auto_fit(help: &str) -> bool {
    ["--fit", "--fit-target", "--fit-ctx"]
        .iter()
        .all(|flag| help.split_whitespace().any(|word| word == *flag))
        && help.contains("'auto'")
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LaunchPolicy {
    pub configured_context_tokens: usize,
    pub requested_gpu_layers: String,
    pub fit_margin_mib: Option<u32>,
}
impl LaunchPolicy {
    pub fn new(context: usize, recovery: usize, supported: bool) -> Result<Self> {
        ensure!(
            (2048..=16384).contains(&context),
            "unsupported frozen context budget"
        );
        Ok(Self {
            configured_context_tokens: context,
            requested_gpu_layers: match (supported, recovery) {
                (true, 0) => "auto",
                (true, 1) => "16",
                _ => "0",
            }
            .into(),
            fit_margin_mib: supported.then_some(4096),
        })
    }
    pub fn arguments(&self, path: &Path, port: u16) -> Result<Vec<String>> {
        let mut args = vec![
            "--model".into(),
            path.to_str().context("model path is not UTF-8")?.into(),
            "--host".into(),
            "127.0.0.1".into(),
            "--port".into(),
            port.to_string(),
            "--ctx-size".into(),
            self.configured_context_tokens.to_string(),
            "--parallel".into(),
            "1".into(),
            "--n-gpu-layers".into(),
            self.requested_gpu_layers.clone(),
        ];
        if let Some(margin) = self.fit_margin_mib {
            args.extend([
                "--fit".into(),
                "on".into(),
                "--fit-target".into(),
                margin.to_string(),
                "--fit-ctx".into(),
                self.configured_context_tokens.to_string(),
            ]);
        }
        Ok(args)
    }
    pub fn verify_context(&self, props: &Value) -> Result<usize> {
        let observed = props
            .pointer("/default_generation_settings/n_ctx")
            .and_then(Value::as_u64)
            .context("backend omitted actual slot context; no inference dispatched")?;
        ensure!(props["total_slots"]==1 && observed==self.configured_context_tokens as u64,"backend slot/context differs from the frozen single-slot budget; no inference dispatched");
        Ok(usize::try_from(observed)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn capability_detection_requires_the_actual_fit_option_and_auto_values() {
        assert!(!supports_auto_fit("--fit-target --fit-ctx 'auto'"));
        assert!(!supports_auto_fit("--fit --fit-target --fit-ctx"));
        assert!(supports_auto_fit(
            "-fit, --fit [on|off]\n--fit-target MiB\n--fit-ctx N\n'auto'"
        ));
    }
    #[test]
    fn fitting_and_cpu_recovery_preserve_context_and_local_only_arguments() {
        for supported in [false, true] {
            for level in 0..3 {
                let policy = LaunchPolicy::new(8192, level, supported).unwrap();
                let args = policy
                    .arguments(Path::new("/owned model/local.gguf"), 8791)
                    .unwrap();
                assert!(args.windows(2).any(|pair| pair == ["--ctx-size", "8192"]));
                assert!(!args.contains(&"999".into()));
                assert!(!args
                    .iter()
                    .any(|arg| arg.contains("https:") || arg == "--model-url"));
                if !supported {
                    assert_eq!(policy.requested_gpu_layers, "0");
                    assert!(!args.contains(&"--fit".into()));
                }
                assert_eq!(
                    policy
                        .verify_context(
                            &json!({"total_slots":1,"default_generation_settings":{"n_ctx":8192}})
                        )
                        .unwrap(),
                    8192
                );
                for props in [
                    json!({}),
                    json!({"total_slots":1,"default_generation_settings":{"n_ctx":4096}}),
                    json!({"total_slots":2,"default_generation_settings":{"n_ctx":8192}}),
                ] {
                    assert!(policy.verify_context(&props).is_err());
                }
            }
        }
    }
}

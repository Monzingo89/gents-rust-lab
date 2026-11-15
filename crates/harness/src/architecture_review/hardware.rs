//! Owned, read-only device observation. Unknown GPU memory is never host RAM.
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

#[async_trait]
pub trait HardwarePort: Sync {
    async fn capture(&self) -> Result<Value>;
}
pub struct NativeHardware;
#[async_trait]
impl HardwarePort for NativeHardware {
    async fn capture(&self) -> Result<Value> {
        let memory = tokio::task::spawn_blocking(|| {
            gents_local_mcp::host_memory::Snapshot::capture(
                &gents_local_mcp::host_memory::NativeMemoryProbe,
            )
        })
        .await??;
        let accelerators = devices().await;
        Ok(
            json!({"memory":memory,"accelerators":accelerators,"current_assessment_inference_slots":1,
            "throughput_tokens_per_second":null,"estimated_task_seconds":null,
            "scope":"Current OS observations, not a fit/completion/speed guarantee. Apple unified memory is shared with host RAM; dedicated GPU memory is separate. Serial inference is the current execution contract."}),
        )
    }
}
async fn output(program: &str, args: &[&str]) -> Option<Vec<u8>> {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .env_clear()
        .envs(crate::runtime_environment::allowed(std::env::vars_os()))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(5), command.output())
        .await
        .ok()?
        .ok()?;
    (output.status.success() && output.stdout.len() <= 128 * 1024).then_some(output.stdout)
}
async fn devices() -> Value {
    if cfg!(target_os = "macos") {
        if let Some(raw) = output(
            "/usr/sbin/system_profiler",
            &["SPDisplaysDataType", "-json", "-detailLevel", "mini"],
        )
        .await
        {
            if let Ok(value) = serde_json::from_slice::<Value>(&raw) {
                return apple(&value);
            }
        }
    } else {
        let paths = if cfg!(target_os = "windows") {
            vec![
                "C:\\Windows\\System32\\nvidia-smi.exe",
                "C:\\Program Files\\NVIDIA Corporation\\NVSMI\\nvidia-smi.exe",
            ]
        } else {
            vec!["/usr/bin/nvidia-smi", "/usr/local/nvidia/bin/nvidia-smi"]
        };
        for path in paths {
            if let Some(raw) = output(
                path,
                &[
                    "--query-gpu=name,memory.total,memory.free",
                    "--format=csv,noheader,nounits",
                ],
            )
            .await
            {
                if let Some(devices) = nvidia(&String::from_utf8_lossy(&raw)) {
                    return devices;
                }
            }
        }
    }
    json!({"status":"unobserved","devices":[],"reason":"No supported local device observation succeeded. No model download, paid fallback or device inference from a model name."})
}
fn safe_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
fn apple(value: &Value) -> Value {
    let devices=value["SPDisplaysDataType"].as_array().into_iter().flatten().take(8).filter_map(|item| {
        let name=item["sppci_model"].as_str().filter(|v|safe_name(v))?;
        let unified=name.starts_with("Apple ");
        Some(json!({"name":name,"memory_kind":if unified {"unified_shared_with_host"}else{"unobserved"},"dedicated_total_bytes":null,"dedicated_available_bytes":null}))
    }).collect::<Vec<_>>();
    json!({"status":if devices.is_empty(){"unobserved"}else{"observed"},"source":"Apple system_profiler SPDisplaysDataType","devices":devices})
}
fn nvidia(raw: &str) -> Option<Value> {
    let mut devices = vec![];
    for row in raw.lines().take(9) {
        let fields = row.split(',').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 3 || !safe_name(fields[0]) || devices.len() == 8 {
            return None;
        }
        let total = fields[1].parse::<u64>().ok()?.checked_mul(1024 * 1024)?;
        let free = fields[2].parse::<u64>().ok()?.checked_mul(1024 * 1024)?;
        if total == 0 || free > total {
            return None;
        }
        devices.push(json!({"name":fields[0],"memory_kind":"dedicated","dedicated_total_bytes":total,"dedicated_available_bytes":free}));
    }
    (!devices.is_empty()).then(||json!({"status":"observed","source":"installed NVIDIA driver nvidia-smi","devices":devices}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_memory_is_not_invented_or_added_to_host_ram() {
        let mac = apple(
            &json!({"SPDisplaysDataType":[{"sppci_model":"Apple M4 Max","serial":"ignored"}]}),
        );
        assert_eq!(mac["devices"][0]["memory_kind"], "unified_shared_with_host");
        assert!(mac["devices"][0]["dedicated_total_bytes"].is_null());
        assert!(!mac.to_string().contains("serial"));
        assert_eq!(
            nvidia("GPU, 100, 40").unwrap()["devices"][0]["dedicated_total_bytes"],
            100 * 1024 * 1024_u64
        );
        assert!(nvidia("GPU, 100, 101").is_none());
        assert!(nvidia("GPU, N/A, 0").is_none());
    }
}

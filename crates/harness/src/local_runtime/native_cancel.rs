//! Opt-in owned-backend cancellation evidence; no production API or credential exposure.
use super::*;
use anyhow::ensure;
use std::sync::atomic::Ordering;

fn process_parent(pid: u32) -> Result<Option<u32>> {
    let output = std::process::Command::new("/bin/ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()?;
    ensure!(
        matches!(output.status.code(), Some(0 | 1)),
        "process liveness observation failed"
    );
    if output.status.code() == Some(1) {
        ensure!(
            output.stdout.iter().all(u8::is_ascii_whitespace),
            "ambiguous process observation"
        );
        Ok(None)
    } else {
        Ok(Some(std::str::from_utf8(&output.stdout)?.trim().parse()?))
    }
}

#[tokio::test]
#[ignore = "opt-in installed pinned Qwen3-4B cancellation plus fresh-session recovery; at most two local calls"]
async fn busy_native_cancel_and_fresh_recovery() {
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
    let id = uuid::Uuid::new_v4();
    let folder = root.join(format!("data/local-evaluation/native-cancel-{id}"));
    std::fs::create_dir_all(&folder).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let probe_source = root.join("crates/harness/src/local_runtime/native_cancel.rs");
    std::fs::copy(&probe_source, folder.join("probe-source.rs")).unwrap();
    let model: Model = serde_json::from_value(json!({"provider":"local","model":"Pinned native cancellation probe",
        "path":relative,"context_tokens":8192,"local_reasoning_tokens":512,"local_final_tokens":2048})).unwrap();
    let system = "Reply in plain text. Continue counting consecutive integers separated by commas until the output limit.";
    let input = "Start at 1 and continue through 100000. Do not abbreviate or summarize.";
    let criteria = json!({"version":1,"model":model,"model_sha256":expected,
        "probe_sha256":crate::company::file_hash(&folder.join("probe-source.rs")).unwrap(),
        "test_binary_sha256":crate::company::file_hash(&std::env::current_exe().unwrap()).unwrap(),
        "system":system,"input":input,"max_model_calls":2,"max_retries":0,
        "recovery":{"system":"Reply briefly in plain text.","input":"Say OK.","reasoning_tokens":0,"final_tokens":32,"context_tokens":8192},
        "native_observation_deadline_seconds":60,"cancellation_deadline_seconds":5,"recovery_deadline_seconds":120,
        "acceptance":["Authenticated owned backend slot is processing before cancellation", "Typed cancellation receipt and no completed first reply", "Owned backend is reaped within five seconds", "New scoped session returns a non-cutoff native reply from a different PID", "Final shutdown reaps the recovery backend"],
        "production_profile_admission":false,"scope":"Native runtime only; owner HTTP cancellation, company budgets and subscriber persistence are separate integration checks"});
    let frozen = serde_json::to_vec_pretty(&criteria).unwrap();
    std::fs::write(folder.join("criteria.json"), &frozen).unwrap();
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let runtime = LocalRuntime::at_port(root.clone(), port).unwrap();
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let mut report = json!({"passed":false,"criteria_sha256":gents_local_mcp::digest(&frozen),
        "model_sha256":expected,"paid_calls":0,"actual_provider_cost_usd":0,
        "estimated_total_cost_usd":null,"cache_hit_ratio":null,"input_tokens":null,"output_tokens":null,
        "peak_or_gpu_memory_bytes":null,"production_profile_admission":false});
    let checked: Result<()> = async {
        let observe = async {
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                // Use this fresh runtime's private key internally. Neither the
                // key nor complete slot/prompt payload is persisted or printed.
                if let Ok(response) = runtime
                    .client
                    .get(format!("http://127.0.0.1:{port}/slots"))
                    .bearer_auth(&runtime.api_key)
                    .timeout(Duration::from_millis(200))
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        if let Ok(slots) = response.json::<Value>().await {
                            if slots.as_array().is_some_and(|slots| {
                                slots.len() == 1 && slots[0]["is_processing"] == true
                            }) {
                                let observation = json!({"id":slots[0]["id"],"is_processing":true,
                                    "observed_at_ms":crate::now_ms(),"n_ctx":slots[0]["n_ctx"]});
                                cancel.store(true, Ordering::Relaxed);
                                return Ok((Instant::now(), observation));
                            }
                        }
                    }
                }
                if Instant::now() >= deadline {
                    cancel.store(true, Ordering::Relaxed);
                    anyhow::bail!("native processing observation unavailable; no replay");
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        };
        let first_session = format!("native-cancel:{id}:first:epoch1");
        let first = runtime.complete_scoped(&model, system, input, 2048, &cancel, &first_session);
        let (first, observed) = tokio::time::timeout(Duration::from_secs(70), async {
            tokio::join!(first, observe)
        })
        .await?;
        let (cancelled_at, observation) = observed?;
        report["busy_slot"] = observation;
        report["cancellation_elapsed_ms"] = json!(cancelled_at.elapsed().as_secs_f64() * 1000.0);
        let error = match first {
            Ok(_) => anyhow::bail!("first request completed before cancellation"),
            Err(e) => e,
        };
        let receipt = error
            .downcast_ref::<crate::local_generation::InferenceFailure>()
            .context("native cancellation lacked a typed receipt")?;
        report["cancelled_receipt"] = serde_json::to_value(receipt)?;
        ensure!(
            matches!(
                receipt.stage,
                crate::local_generation::FailureStage::Cancelled
            ),
            "wrong native failure stage"
        );
        ensure!(
            cancelled_at.elapsed() < Duration::from_secs(5),
            "native cancellation exceeded frozen bound"
        );
        ensure!(
            process_parent(receipt.backend_pid)?.is_none(),
            "cancelled backend not reaped"
        );
        report["cancelled_backend_reaped"] = json!(true);
        // Cancellation is never reset on an old task. Recovery is a distinct
        // explicitly authorized probe session with its own fresh token.
        let mut recovery = model.clone();
        recovery.local_reasoning_tokens = Some(0);
        recovery.local_final_tokens = Some(32);
        let recovery_cancel = AtomicBool::new(false);
        let reply = tokio::time::timeout(
            Duration::from_secs(120),
            runtime.complete_scoped(
                &recovery,
                "Reply briefly in plain text.",
                "Say OK.",
                32,
                &recovery_cancel,
                &format!("native-cancel:{id}:recovery:epoch2"),
            ),
        )
        .await??;
        let execution = reply
            .local_execution
            .as_ref()
            .context("recovery lacks native reply")?;
        report["recovery_reply"] = serde_json::to_value(&reply)?;
        ensure!(
            execution.backend_pid != receipt.backend_pid
                && process_parent(execution.backend_pid)? == Some(std::process::id()),
            "recovery did not own a fresh backend"
        );
        ensure!(
            execution.observed_context_tokens == Some(8192) && reply.usage.cost_usd == Some(0.0),
            "recovery changed the slot budget or provider fee"
        );
        ensure!(
            !reply.text.trim().is_empty() && execution.finish_reason.as_deref() == Some("stop"),
            "recovery returned no complete content"
        );
        runtime.stop_owned().await;
        ensure!(
            process_parent(execution.backend_pid)?.is_none(),
            "recovery backend not reaped"
        );
        report["recovery_backend_reaped"] = json!(true);
        report["observed_native_requests"] = json!(2);
        ensure!(
            std::fs::read(folder.join("criteria.json"))? == frozen,
            "frozen criteria changed"
        );
        Ok(())
    }
    .await;
    runtime.stop_owned().await;
    report["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
    report["passed"] = json!(checked.is_ok());
    if checked.is_err() {
        report["failure"] = json!("Native cancellation/recovery criteria not all confirmed; retain original record and do not replay");
    }
    std::fs::write(
        folder.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!(
        "{}",
        json!({"report":folder.join("report.json"),"passed":checked.is_ok()})
    );
    assert!(checked.is_ok(), "inspect retained cancellation evidence");
}

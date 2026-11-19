//! Trusted-host adapter. Session identity comes from the harness's authenticated
//! run/actor, not the model proposal. Cancellation drops the owned subprocess.
use crate::{protocol, Scope};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    path::Path,
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
pub fn transport_label() -> String {
    format!(
        "local_{}_mcp",
        std::env::var("GENTS_MCP_TRANSPORT").unwrap_or_else(|_| "stdio".into())
    )
}
pub fn handles(name: &str) -> bool {
    // One reviewed registry, not a second dispatch list that drifts on promotion.
    crate::Registry::new().contains(name)
}
/// Host discovery only, not a tool receipt, permission or executable attestation.
pub fn runtime_binding(root: &Path) -> Result<Value> {
    let binary = crate::runtime_bundle::mcp_binary(root)?;
    let bundled = std::env::var_os("GENTS_RUNTIME_MANIFEST").is_some();
    Ok(
        json!({"supported":true,"mode":if bundled{"sealed_bundle"}else{"development_directory"},
        "bundle_verified":bundled,"manifest_sha256":std::env::var("GENTS_RUNTIME_MANIFEST_SHA256").ok(),
        "mcp_executable":binary.strip_prefix(root).ok().and_then(|p|p.to_str()),
        "scope":"Trusted launch selection. Bundle bytes verified now and before each MCP child launch; no permission, process attestation or task acceptance."}),
    )
}
pub async fn invoke(
    root: &Path,
    scope: &Scope,
    name: &str,
    arguments: &Value,
    cancel: &AtomicBool,
) -> Result<Value> {
    scope.validate()?;
    if !handles(name) {
        bail!("unregistered native MCP function");
    }
    if cancel.load(Ordering::Relaxed) {
        bail!("local MCP cancelled before launch");
    }
    let binary = crate::runtime_bundle::mcp_binary(root)?;
    if !binary.is_file() {
        bail!("native MCP binary missing; build the Rust workspace before using native tools");
    }
    let transport = std::env::var("GENTS_MCP_TRANSPORT").unwrap_or_else(|_| "stdio".into());
    if !matches!(transport.as_str(), "stdio" | "sse") {
        bail!("GENTS_MCP_TRANSPORT must be stdio or sse");
    }
    let token = uuid::Uuid::new_v4().to_string();
    let mut command = tokio::process::Command::new(binary);
    command
        .env_clear()
        .args([
            "--root",
            root.to_str().context("repository path must be UTF-8")?,
            "--company",
            &scope.company,
            "--owner",
            &scope.owner,
            "--run",
            &scope.run.to_string(),
            "--actor",
            &scope.actor,
            "--phase",
            match scope.phase {
                crate::Phase::Planner => "planner",
                crate::Phase::Worker => "worker",
            },
        ])
        .arg("--transport")
        .arg(&transport)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if transport == "sse" {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .kill_on_drop(true);
    if let Some(revision) = scope.expected_revision {
        command.arg("--policy-revision").arg(revision.to_string());
    }
    if transport == "sse" {
        command.env("GENTS_MCP_TOKEN", &token);
    }
    let requirements = crate::Registry::new().runtime_requirements(name)?;
    if requirements.installed_rustc {
        command.env("GENTS_RUSTC", crate::rust_verification::compiler_path()?);
    }
    if requirements.installed_python {
        command.env("GENTS_PYTHON", crate::python_check::interpreter_path()?);
    }
    let call_timeout = Duration::from_secs(requirements.call_timeout_seconds);
    let mut child = command.spawn()?;
    if transport == "sse" {
        let exchange = invoke_http(&mut child, &token, name, arguments);
        let cancelled = async {
            while !cancel.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        let result = tokio::select! {result=exchange=>result,_=cancelled=>bail!("local MCP call cancelled"),_=tokio::time::sleep(call_timeout)=>bail!("local MCP call exceeded its bounded execution budget; inspect receipts before retrying writes")};
        child.kill().await?;
        return result;
    }
    let mut stdin = child.stdin.take().context("MCP stdin missing")?;
    for frame in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":protocol::VERSION,"capabilities":{},"clientInfo":{"name":"gents-harness","version":env!("CARGO_PKG_VERSION")}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
    ] {
        let mut bytes = serde_json::to_vec(&frame)?;
        if bytes.len() > 65535 {
            bail!("MCP input exceeds bounded frame");
        }
        bytes.push(b'\n');
        stdin.write_all(&bytes).await?;
    }
    drop(stdin);
    let cancelled = async {
        while !cancel.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let output = tokio::select! {output=child.wait_with_output()=>output?,_=cancelled=>bail!("local MCP call cancelled"),_=tokio::time::sleep(call_timeout)=>bail!("local MCP call exceeded its bounded execution budget")};
    if !output.status.success() || output.stdout.len() > 196608 {
        bail!("local MCP process failed or exceeded output bound");
    }
    let replies = String::from_utf8(output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if replies.len() != 2
        || replies[0]["result"]["protocolVersion"] != protocol::VERSION
        || replies[1]["id"] != 2
        || replies[1]["jsonrpc"] != "2.0"
    {
        bail!("invalid native MCP response sequence");
    }
    normalize(&replies[1]["result"])
}
fn normalize(result: &Value) -> Result<Value> {
    if result["isError"] == false {
        return Ok(
            json!({"ok":true,"result":result["structuredContent"]["value"],"mcp_receipt":result["structuredContent"]["receipt"]}),
        );
    }
    Ok(
        json!({"ok":false,"error":result["content"][0]["text"].as_str().unwrap_or("local MCP request rejected")}),
    )
}

/// Private owned HTTP child, never a user/model-selected URL or remote endpoint.
async fn invoke_http(
    child: &mut tokio::process::Child,
    token: &str,
    name: &str,
    arguments: &Value,
) -> Result<Value> {
    let stderr = child.stderr.take().context("MCP readiness pipe missing")?;
    let mut reader = BufReader::new(stderr).take(1024);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let ready: Value = serde_json::from_str(&line).context("MCP readiness failed")?;
    let endpoint = ready["mcp_endpoint"]
        .as_str()
        .context("missing MCP endpoint")?;
    let url = reqwest::Url::parse(endpoint)?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || url.path() != "/mcp"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("invalid owned loopback MCP endpoint");
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut session = String::new();
    let mut final_reply = Value::Null;
    for (index, frame) in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":protocol::VERSION,"capabilities":{},"clientInfo":{"name":"gents-harness","version":env!("CARGO_PKG_VERSION")}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":arguments}}),
    ].into_iter().enumerate() {
        if serde_json::to_vec(&frame)?.len() > 65535 { bail!("MCP input exceeds bounded frame"); }
        let mut request = client.post(endpoint).bearer_auth(token).header("Accept","application/json, text/event-stream").header("MCP-Protocol-Version",protocol::VERSION).json(&frame);
        if !session.is_empty() { request = request.header("MCP-Session-Id", &session); }
        let mut response = request.send().await?.error_for_status()?;
        if index == 1 {
            if response.status() != reqwest::StatusCode::ACCEPTED { bail!("invalid MCP notification acknowledgement"); }
            continue;
        }
        if response.headers().get("content-type").and_then(|v|v.to_str().ok()) != Some("text/event-stream") { bail!("owned MCP child did not return SSE"); }
        if index == 0 { session = response.headers().get("mcp-session-id").context("missing MCP session")?.to_str()?.to_string(); }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > 66000 { bail!("MCP SSE response exceeds bound"); }
            body.extend_from_slice(&chunk);
        }
        let body = std::str::from_utf8(&body)?;
        let data = body.lines().find_map(|line|line.strip_prefix("data: ")).context("missing MCP SSE response")?;
        let reply: Value = serde_json::from_str(data)?;
        if reply["jsonrpc"] != "2.0" || reply["id"] != if index == 0 {1} else {2} || reply.get("error").is_some() { bail!("invalid MCP HTTP response sequence"); }
        if index == 0 && reply["result"]["protocolVersion"] != protocol::VERSION { bail!("MCP version mismatch"); }
        final_reply = reply;
    }
    normalize(&final_reply["result"])
}

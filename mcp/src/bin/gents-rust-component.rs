//! Private host adapter, absent from the MCP catalog. It grants no authority.
//! Bounded stdin; no source paths, arbitrary compiler flags, shell or downloads.
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Describe {
        source: String,
    },
    Invoke {
        source: String,
        source_sha256: String,
        api_sha256: String,
        function: String,
        arguments: Value,
    },
}
fn execute() -> anyhow::Result<Value> {
    anyhow::ensure!(
        std::env::args_os().len() == 1,
        "stdin-only component adapter accepts no arguments"
    );
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take(49153)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 49152, "component request exceeds bound");
    let request: Request = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("invalid component request shape"))?;
    match request {
        Request::Describe { source } => gents_local_mcp::component_api::describe(&source),
        Request::Invoke {
            source,
            source_sha256,
            api_sha256,
            function,
            arguments,
        } => gents_local_mcp::component_api::invoke(
            &source,
            &source_sha256,
            &api_sha256,
            &function,
            &arguments,
        ),
    }
}
fn main() {
    let (result, success) = match execute() {
        Ok(value) => {
            let success = value.get("passed").is_none_or(|p| p == true);
            (value, success)
        }
        Err(_) => (
            json!({"passed":false,"request_rejected":true,"scope":"Invalid input, source/API pin, unsupported data ABI or unavailable isolated runtime; no source payload echoed, no authority granted"}),
            false,
        ),
    };
    if writeln!(std::io::stdout().lock(), "{result}").is_err() || !success {
        std::process::exit(1);
    }
}

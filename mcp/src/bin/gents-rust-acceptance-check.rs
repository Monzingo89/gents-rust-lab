//! Private owner-fixture parser, deliberately absent from the MCP registry.
//! Bounded stdin only; no file paths, shell, credentials, execution or network.
use std::io::{Read, Write};
fn main() {
    if std::env::args_os().len() != 1 {
        println!("{{\"passed\":false,\"input_rejected\":true,\"scope\":\"stdin-only parser accepts no command arguments\"}}");
        std::process::exit(1);
    }
    let mut bytes = Vec::new();
    let read = std::io::stdin().lock().take(8193).read_to_end(&mut bytes);
    let source = std::str::from_utf8(&bytes);
    let result = match (read, source) {
        (Ok(_), Ok(source)) if bytes.len() <= 8192 => {
            gents_local_mcp::rust_verification::acceptance_policy_observation(source)
        }
        _ => {
            serde_json::json!({"passed":false,"input_rejected":true,"scope":"Require 1..8192 UTF-8 stdin bytes; no payload echoed or executed"})
        }
    };
    let passed = result["passed"] == true;
    let written = writeln!(std::io::stdout().lock(), "{result}").is_ok();
    if !passed || !written {
        std::process::exit(1);
    }
}

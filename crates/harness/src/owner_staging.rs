//! Exact owner-source staging shared by typed input adapters. No agent dispatch.
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::path::Path;
use uuid::Uuid;
pub fn stage(root: &Path, company: &str, run: Uuid, retained: &[(Value, String)]) -> Result<()> {
    let path =
        crate::company::scoped(root, company, &["runs", &run.to_string()])?.join("workspace");
    let workspace = gents_local_mcp::workspace::open(&path, true)?;
    let mut missing = vec![];
    for (m, text) in retained {
        let path = m["path"].as_str().context("missing source path")?;
        match workspace.read_complete(path, 8192) {
            Ok(actual) => ensure!(
                actual["sha256"] == m["sha256"] && actual["bytes"] == m["bytes"],
                "workspace source conflicts with approved bytes; nothing overwritten"
            ),
            Err(e)
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                missing.push((path, text))
            }
            Err(e) => {
                return Err(e.context("source staging rejected; inspect workspace before retry"))
            }
        }
    }
    for (path, text) in missing {
        workspace.write(path, text)?;
    }
    for (m, _) in retained {
        let actual = workspace.read_complete(m["path"].as_str().unwrap(), 8192)?;
        ensure!(
            actual["sha256"] == m["sha256"] && actual["bytes"] == m["bytes"],
            "staged source failed complete readback"
        );
    }
    Ok(())
}

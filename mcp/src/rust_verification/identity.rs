//! Verification is an observation of particular bytes, policy and implementation.
//! A changed source may be checked again; identical state still has a retry cap.
use super::specs;
use crate::{authority::Authority, governance, workspace, Scope};
use anyhow::{Context, Result};
use serde_json::{json, Value};

pub fn request_identity(authority: &dyn Authority, scope: &Scope, path: &str) -> Result<String> {
    let db = authority.open(scope)?;
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let spec = specs(&intake)?
        .into_iter()
        .find(|s| s.actor == scope.actor && s.source_path == path)
        .context("verification identity requires an owner-frozen source and actor")?;
    let policy =
        governance::load(&db, &scope.run.to_string())?.context("verification requires policy")?;
    let root = authority.workspace(scope)?;
    let work = workspace::open(&root, false)?;
    let source = work.read_complete(path, 8192)?;
    let tests = work.read_complete(&spec.tests_path, 8192)?;
    let implementation = crate::digest(
        concat!(
            include_str!("mod.rs"),
            include_str!("source_policy.rs"),
            include_str!("sandbox.rs"),
            include_str!("diagnostics.rs"),
            include_str!("failure_context.rs"),
            include_str!("progress.rs"),
            include_str!("../client.rs"),
            include_str!("identity.rs")
        )
        .as_bytes(),
    );
    Ok(crate::digest(
        json!({"run":scope.run,"actor":scope.actor,
            "policy_revision":policy.revision,"source_path":path,
            "source_sha256":source["sha256"],"tests_sha256":tests["sha256"],
            "frozen_tests_sha256":spec.tests_sha256,"implementation":implementation
        })
        .to_string()
        .as_bytes(),
    ))
}

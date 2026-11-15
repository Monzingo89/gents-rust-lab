//! Owner-filtered component information. Never an execution or policy adapter.
use anyhow::{ensure, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::path::Path;
use tokio::{
    io::AsyncReadExt,
    process::Command,
    time::{timeout, Duration},
};
const MAX_OUTPUT: u64 = 262144;
pub(crate) fn owns(root: &Path, company: &str, owner: &str) -> Result<bool> {
    let path = crate::company::directory(&root.canonicalize()?, company)?.join("brain.sqlite3");
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    db.busy_timeout(Duration::from_millis(500))?;
    db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")?;
    let actual: String = db.query_row("SELECT owner FROM company LIMIT 1", [], |r| r.get(0))?;
    Ok(actual == owner)
}
fn filtered(root: &Path, company: &str, owner: &str, raw: Value) -> Result<Value> {
    ensure!(
        owns(root, company, owner)?,
        "company unavailable to this identity"
    );
    ensure!(
        raw["schema_version"] == 1
            && raw["execution_authorized"] == false
            && raw["promotion_authorized"] == false,
        "invalid component information envelope"
    );
    let entries = raw["components"]
        .as_array()
        .context("missing component inventory")?;
    ensure!(entries.len() <= 32, "component inventory exceeds bound");
    let mut visible = Vec::new();
    for entry in entries {
        let Some(origin) = entry["origin_company"].as_str() else {
            continue;
        };
        if !owns(root, origin, owner).unwrap_or(false) {
            continue;
        }
        ensure!(
            entry["execution_authorized"] == false && entry["promotion_authorized"] == false,
            "component information cannot grant execution"
        );
        crate::security::require_persistable(&entry.to_string())?;
        let mut metadata = serde_json::Map::new();
        for key in [
            "component_id",
            "origin_company",
            "origin_fixture",
            "source_sha256",
            "api_sha256",
            "abi",
            "integer_encoding",
            "functions",
            "inspector_sha256",
            "historical_qualification",
            "integrity_checked_now",
            "execution_authorized",
            "promotion_authorized",
        ] {
            if let Some(value) = entry.get(key) {
                metadata.insert(key.to_owned(), value.clone());
            }
        }
        visible.push(Value::Object(metadata));
    }
    // Never disclose rejected ids, aggregate foreign counts or the global hash.
    snapshot(visible)
}
fn snapshot(components: Vec<Value>) -> Result<Value> {
    let mut body = json!({"schema_version":1,"components":components,"execution_authorized":false,"promotion_authorized":false,
        "scope":"Owner-scoped interface information and historical fixture snapshots only. Source/API identities are observations, not execution grants or fresh task qualification. Source bodies, tests and conversations are excluded. A ready plan still requires owner-prepared invocation bindings, source custody, approval, current actor policy and independent acceptance."});
    ensure!(
        serde_json::to_vec(&body)?.len() <= 196608,
        "owner component view exceeds bound"
    );
    body["catalog_sha256"] = json!(gents_local_mcp::digest(&serde_json::to_vec(&body)?));
    Ok(body)
}
pub async fn list(root: &Path, company: &str, owner: &str) -> Result<Value> {
    ensure!(
        owns(root, company, owner)?,
        "company unavailable to this identity"
    );
    filtered(root, company, owner, inspect(root, None).await?)
}
/// Private host-only source snapshot. Callers must establish current ownership.
pub(crate) async fn source(root: &Path, id: &str) -> Result<Value> {
    selection_ids(&json!([id]))?;
    inspect(root, Some(id)).await
}
async fn inspect(root: &Path, source: Option<&str>) -> Result<Value> {
    let mut command = Command::new("node");
    command
        .arg(root.join("scripts/mcp-owned-catalog.mjs"))
        .arg("--root")
        .arg(root)
        .current_dir(root)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    if let Some(id) = source {
        command.arg("--source").arg(id);
    }
    let mut child = command
        .spawn()
        .context("owned component inspector unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("component metadata stream unavailable")?;
    let work = async {
        let mut bytes = Vec::new();
        stdout.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).await?;
        ensure!(
            bytes.len() <= MAX_OUTPUT as usize,
            "component metadata stream exceeded bound"
        );
        let status = child.wait().await?;
        ensure!(status.success(), "owned component inspection failed");
        Ok(serde_json::from_slice(&bytes)?)
    };
    timeout(Duration::from_secs(30), work)
        .await
        .context("owned component inspection deadline reached")?
}
pub fn selection_ids(ids: &Value) -> Result<Vec<String>> {
    let ids: Vec<String> = serde_json::from_value(ids.clone())?;
    ensure!(
        !ids.is_empty() && ids.len() <= 4,
        "select one to four owned components"
    );
    let unique = ids.iter().collect::<std::collections::BTreeSet<_>>();
    ensure!(unique.len() == ids.len(), "duplicate owned component id");
    for id in &unique {
        ensure!(
            id.len() == 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid component id"
        );
    }
    Ok(unique.into_iter().cloned().collect())
}
pub fn selected(catalog: &Value, ids: &Value) -> Result<Value> {
    let ids = selection_ids(ids)?;
    let entries = catalog["components"]
        .as_array()
        .context("missing owner component inventory")?;
    let mut chosen = Vec::new();
    for id in ids {
        chosen.push(
            entries
                .iter()
                .find(|entry| entry["component_id"] == id)
                .context("selected component unavailable to this owner")?
                .clone(),
        );
    }
    let view = snapshot(chosen)?;
    ensure!(
        serde_json::to_vec(&view)?.len() <= 12288,
        "selected component interfaces exceed the planning budget"
    );
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_filter_excludes_foreign_entries_ids_counts_and_global_hash_without_granting_authority()
    {
        let root = std::env::temp_dir().join(format!("owned-catalog-{}", uuid::Uuid::new_v4()));
        for (company, owner) in [("alice-co", "alice"), ("bob-co", "bob")] {
            crate::company::operation(
                &root,
                company,
                owner,
                &json!({"op":"register","name":"fixture"}),
            )
            .unwrap();
        }
        let entry = |company: &str, id: &str| json!({"component_id":id,"origin_company":company,"functions":[{"name":"protocol"}],"execution_authorized":false,"promotion_authorized":false});
        let raw = json!({"schema_version":1,"components":[entry("alice-co",&"a".repeat(64)),entry("bob-co",&"b".repeat(64))],"rejected":[{"component_id":"PRIVATE_REJECTED_ID"}],"catalog_sha256":"GLOBAL_HASH","execution_authorized":false,"promotion_authorized":false});
        let view = filtered(&root, "alice-co", "alice", raw.clone()).unwrap();
        assert_eq!(view["components"].as_array().unwrap().len(), 1);
        assert!(!view.to_string().contains("bob"));
        assert!(!view.to_string().contains("GLOBAL_HASH"));
        assert!(!view.to_string().contains("PRIVATE_REJECTED_ID"));
        assert_eq!(view["execution_authorized"], false);
        assert!(filtered(&root, "alice-co", "bob", raw).is_err());
        let chosen = selected(&view, &json!(["a".repeat(64)])).unwrap();
        assert_eq!(chosen["components"], view["components"]);
        assert!(selected(&view, &json!(["b".repeat(64)])).is_err());
        assert!(selected(&view, &json!(["a".repeat(64), "a".repeat(64)])).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

//! Owner-supplied bytes stay outside inference and are staged only at approved admission.
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    id: String,
    path: String,
    text: String,
}
fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ceo_source_files(session TEXT NOT NULL,id TEXT NOT NULL,manifest TEXT NOT NULL,content TEXT NOT NULL,PRIMARY KEY(session,id));")?;
    Ok(())
}
fn stored(db: &Connection, session: Uuid) -> Result<Vec<(Value, String)>> {
    install(db)?;
    let mut query =
        db.prepare("SELECT manifest,content FROM ceo_source_files WHERE session=?1 ORDER BY id")?;
    let rows = query
        .query_map([session.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(manifest, text)| {
            let manifest: Value = serde_json::from_str(&manifest)?;
            ensure!(
                manifest["sha256"] == gents_local_mcp::digest(text.as_bytes())
                    && manifest["bytes"] == text.len(),
                "retained source bytes differ from manifest"
            );
            Ok((manifest, text))
        })
        .collect()
}
/// Runs inside the planning transaction. A rejected turn retains no attachment.
pub fn retain(db: &Connection, session: Uuid, body: &mut Value, incoming: &Value) -> Result<()> {
    install(db)?;
    let uploads: Vec<Upload> = serde_json::from_value(if incoming.is_null() {
        json!([])
    } else {
        incoming.clone()
    })?;
    ensure!(uploads.len() <= 8, "at most eight attached files");
    for upload in uploads {
        ensure!(
            upload.path.len() <= 120
                && upload.path.ends_with(".json")
                && !upload.path.contains('/')
                && upload
                    .path
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                && !upload.path.starts_with('.'),
            "attached tables require a plain JSON filename"
        );
        // Reject executable/nested/ambiguous table data before persistence.
        gents_local_mcp::functions::validate_table_text(&upload.text)?;
        let manifest = json!({"id":upload.id,"path":upload.path,"bytes":upload.text.len(),"sha256":gents_local_mcp::digest(upload.text.as_bytes())});
        let descriptor = json!([{"id":upload.id,"tool":"table_read_json","arguments":{"path":upload.path,"expected_sha256":manifest["sha256"]}}]);
        body["source_inputs"] =
            crate::ceo_contracts::merge_inputs(&body["source_inputs"], &descriptor)?;
        let existing = stored(db, session)?;
        if let Some((old, text)) = existing.iter().find(|(m, _)| m["id"] == manifest["id"]) {
            ensure!(
                *old == manifest && *text == upload.text,
                "attached input ids are immutable; start a new conversation"
            );
        } else {
            ensure!(
                !existing.iter().any(|(m, _)| m["path"] == manifest["path"]),
                "attached source paths must be unique"
            );
            db.execute(
                "INSERT INTO ceo_source_files VALUES(?1,?2,?3,?4)",
                params![
                    session.to_string(),
                    upload.id,
                    manifest.to_string(),
                    upload.text
                ],
            )?;
        }
    }
    let retained = stored(db, session)?;
    ensure!(
        retained.len() <= 8 && retained.iter().map(|(_, t)| t.len()).sum::<usize>() <= 32768,
        "attached sources exceed eight files or 32768 total bytes"
    );
    // Every retained file must still be represented by an immutable exact input.
    for (m, _) in &retained {
        let descriptor = json!([{"id":m["id"],"tool":"table_read_json","arguments":{"path":m["path"],"expected_sha256":m["sha256"]}}]);
        body["source_inputs"] =
            crate::ceo_contracts::merge_inputs(&body["source_inputs"], &descriptor)?;
    }
    if !retained.is_empty() {
        body["source_files"] = json!(retained.iter().map(|(m, _)| m).collect::<Vec<_>>());
    }
    Ok(())
}
/// All paths use the descriptor-relative MCP adapter. Existing different bytes
/// are a conflict, never overwritten. A partial failed staging can be inspected
/// and retried: matching bytes are retained, and no agent has been dispatched.
pub fn stage(
    db: &Connection,
    root: &Path,
    company: &str,
    run: Uuid,
    intake: &Value,
) -> Result<Value> {
    if intake["ceo_source_files"].is_null() {
        return Ok(Value::Null);
    }
    let session = Uuid::parse_str(
        intake["ceo_planning_id"]
            .as_str()
            .context("attachments require planning session")?,
    )?;
    let retained = stored(db, session)?;
    let manifests = json!(retained.iter().map(|(m, _)| m).collect::<Vec<_>>());
    ensure!(
        !retained.is_empty() && manifests == intake["ceo_source_files"],
        "frozen attachment manifests differ from retained sources"
    );
    crate::owner_staging::stage(root, company, run, &retained)?;
    Ok(
        json!({"planning_id":session,"files":manifests,"provider_cost_usd":0,"authority":"owner attachments only; no tool grants added"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (std::path::PathBuf, Connection, Uuid, Value) {
        let root = std::env::temp_dir().join(format!("gents-attachments-{}", Uuid::new_v4()));
        crate::company::operation(
            &root,
            "one",
            "alice",
            &json!({"op":"register","name":"fixture"}),
        )
        .unwrap();
        let root = root.canonicalize().unwrap();
        let db = crate::company::connect(&root, "one", "alice", false).unwrap();
        (root, db, Uuid::new_v4(), json!({"source_inputs":[]}))
    }
    fn upload() -> Value {
        json!([{"id":"input","path":"source.json","text":"[ {\"n\": 2, \"enabled\": true} ]\n"}])
    }
    #[test]
    fn exact_bytes_staged_idempotently_only_for_matching_frozen_manifest() {
        let (root, db, id, mut body) = setup();
        retain(&db, id, &mut body, &upload()).unwrap();
        assert!(!body.to_string().contains("enabled"));
        assert_eq!(
            body["source_inputs"][0]["arguments"]["expected_sha256"],
            body["source_files"][0]["sha256"]
        );
        let run = Uuid::new_v4();
        let intake = json!({"ceo_planning_id":id,"ceo_source_files":body["source_files"]});
        let receipt = stage(&db, &root, "one", run, &intake).unwrap();
        assert_eq!(receipt["files"], body["source_files"]);
        let file = root.join(format!(
            "data/companies/one/runs/{run}/workspace/source.json"
        ));
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            upload()[0]["text"].as_str().unwrap()
        );
        assert_eq!(stage(&db, &root, "one", run, &intake).unwrap(), receipt);
        let mut bad = intake.clone();
        bad["ceo_source_files"][0]["sha256"] = json!("0".repeat(64));
        assert!(stage(&db, &root, "one", run, &bad).is_err());
        std::fs::write(&file, "[]").unwrap();
        assert!(stage(&db, &root, "one", run, &intake).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[]");
        assert!(crate::company::connect(&root, "one", "bob", false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn company_admission_requires_approval_and_audits_staged_bytes() {
        let (root, db, id, mut body) = setup();
        retain(&db, id, &mut body, &upload()).unwrap();
        let run = Uuid::new_v4();
        let intake = json!({"idea":"Use attached data","ceo_planning_id":id,"ceo_source_files":body["source_files"]});
        db.execute(
            "INSERT INTO jobs VALUES(?1,?2,'fixture',NULL,'draft')",
            params![run.to_string(), intake.to_string()],
        )
        .unwrap();
        assert!(crate::company::admit(&root, "one", "alice", run, "Use attached data").is_err());
        let workspace = root.join(format!("data/companies/one/runs/{run}/workspace"));
        assert!(!workspace.exists());
        db.execute("UPDATE jobs SET state='approved'", []).unwrap();
        assert!(crate::company::admit(&root, "one", "alice", run, "Changed task").is_err());
        assert!(!workspace.exists());
        crate::company::admit(&root, "one", "alice", run, "Use attached data").unwrap();
        assert!(workspace.join("source.json").is_file());
        let event: String = db
            .query_row(
                "SELECT body FROM audit WHERE kind='owner_source_files_staged'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!event.contains("enabled"));
        assert_eq!(
            serde_json::from_str::<Value>(&event).unwrap()["files"],
            body["source_files"]
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn attachment_validation_and_immutability() {
        let (root, mut db, id, mut body) = setup();
        {
            let tx = db.transaction().unwrap();
            retain(&tx, id, &mut body, &upload()).unwrap();
            tx.commit().unwrap();
        }
        for change in [
            json!({"text":"[]"}),
            json!({"path":"../source.json"}),
            json!({"text":"[{\"n\":1,\"n\":2}]"}),
            json!({"text":"[{\"n\":{\"x\":2}}]"}),
            json!({"text":" ".repeat(8193)}),
            json!({"unexpected":true}),
        ] {
            let mut altered = upload();
            for (key, value) in change.as_object().unwrap() {
                altered[0][key] = value.clone();
            }
            let tx = db.transaction().unwrap();
            assert!(retain(&tx, id, &mut body.clone(), &altered).is_err());
        }
        let tx = db.transaction().unwrap();
        let mut duplicate = upload();
        duplicate[0]["id"] = json!("second");
        assert!(retain(&tx, id, &mut body.clone(), &duplicate).is_err());
        drop(tx);
        assert_eq!(stored(&db, id).unwrap().len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn staging_rejects_symlinks_and_corrupt_retained_bytes() {
        let (root, db, id, mut body) = setup();
        retain(&db, id, &mut body, &upload()).unwrap();
        let run = Uuid::new_v4();
        let workspace = root.join(format!("data/companies/one/runs/{run}/workspace"));
        std::fs::create_dir_all(&workspace).unwrap();
        let outside = root.join("outside");
        std::fs::write(&outside, "unchanged").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, workspace.join("source.json")).unwrap();
        let intake = json!({"ceo_planning_id":id,"ceo_source_files":body["source_files"]});
        #[cfg(unix)]
        assert!(stage(&db, &root, "one", run, &intake).is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "unchanged");
        db.execute("UPDATE ceo_source_files SET content='[]'", [])
            .unwrap();
        assert!(stage(&db, &root, "one", Uuid::new_v4(), &intake).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

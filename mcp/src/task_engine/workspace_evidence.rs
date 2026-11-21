//! Host-created task-local file evidence. Captures do not enlarge the shared
//! knowledge corpus or grant access. Current scope, root identity, file bytes,
//! TTL and descriptor-relative privacy/confinement are rechecked on reuse.
use super::{contracts::*, read_tools, Repository};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

const MAX_CAPTURES: i64 = 512;
const MAX_TASK_CAPTURES: i64 = 32;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    File,
    Directory,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    schema_version: u32,
    task_id: Uuid,
    scope_sha256: String,
    root_sha256: String,
    path: String,
    kind: Kind,
    evidence: Evidence,
}
pub(super) fn install(repo: &Repository<'_>) -> Result<()> {
    repo.db.execute_batch("CREATE TABLE IF NOT EXISTS task_workspace_evidence_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,id TEXT NOT NULL,task TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(tenant,subject,id)); CREATE INDEX IF NOT EXISTS task_workspace_expiry_v1 ON task_workspace_evidence_v1(tenant,subject,expires);")?;
    Ok(())
}
fn root_identity(root: &Path) -> Result<String> {
    let meta = std::fs::symlink_metadata(root)?;
    ensure!(
        meta.is_dir() && !meta.is_symlink(),
        "trusted workspace root must be a real directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(crate::digest(&serde_json::to_vec(&json!([
            root.canonicalize()?,
            meta.dev(),
            meta.ino()
        ]))?))
    }
    #[cfg(not(unix))]
    {
        Ok(crate::digest(&serde_json::to_vec(&root.canonicalize()?)?))
    }
}
fn listing(root: &Path, path: &str) -> Result<Value> {
    let mut value = crate::workspace::open(root, false)?.list(path)?;
    while serde_json::to_vec(&value)?.len() > 4096 {
        let entries = value["files"]
            .as_array_mut()
            .context("workspace listing shape")?;
        ensure!(!entries.is_empty(), "listing metadata exceeds bound");
        entries.pop();
        value["truncated"] = json!(true);
    }
    Ok(value)
}
impl Repository<'_> {
    /// Called only by trusted host code with its constructed ExecutionPolicy.
    pub fn capture_workspace(
        &self,
        request: &TaskRequest,
        policy: &ExecutionPolicy,
        root: &Path,
        tool: &super::workflows::ToolRequest,
        now: u64,
    ) -> Result<Value> {
        request.validate()?;
        policy.validate(now)?;
        ensure!(
            policy.permitted_tools.contains(&tool.name)
                && policy.workspace_roots.iter().any(|p| Path::new(p) == root),
            "workspace tool/root outside trusted task policy"
        );
        let (kind, path, start, end, hash, quote, receipt) = match read_tools::validate(tool)? {
            read_tools::Arguments::Read(args) => {
                let mut page = crate::workspace::open(root, false)?.read(
                    &args.path,
                    args.offset_bytes,
                    args.limit_bytes,
                )?;
                let text = page["text"]
                    .as_str()
                    .context("read text absent")?
                    .to_owned();
                let hash = page["sha256"]
                    .as_str()
                    .context("read digest absent")?
                    .to_owned();
                let end = args.offset_bytes + text.len();
                page.as_object_mut().unwrap().remove("text");
                (
                    Kind::File,
                    args.path,
                    args.offset_bytes,
                    end,
                    hash,
                    text,
                    page,
                )
            }
            read_tools::Arguments::List(args) => {
                let value = listing(root, &args.path)?;
                let text = serde_json::to_string(&value)?;
                (
                    Kind::Directory,
                    args.path,
                    0,
                    text.len(),
                    crate::digest(text.as_bytes()),
                    text,
                    value,
                )
            }
            _ => anyhow::bail!("not a workspace capture tool"),
        };
        ensure!(
            quote.len() <= policy.max_evidence_bytes,
            "workspace capture exceeds evidence allowance"
        );
        crate::safety::require_persistable(&quote)?;
        let scope_sha256 = crate::digest(&serde_json::to_vec(&policy.scope)?);
        let root_sha256 = root_identity(root)?;
        let id = format!(
            "workspace:v1:{}",
            crate::digest(&serde_json::to_vec(&json!([
                request.task_id,
                scope_sha256,
                root_sha256,
                path,
                start,
                end,
                hash
            ]))?)
        );
        // Identical rereads retain the original timestamp/TTL. Popularity cannot
        // extend stale evidence. Content/root/scope changes produce another ID.
        let existing = self.workspace_capture(&policy.scope, &id)?;
        let evidence = if let Some(capture) = existing {
            ensure!(
                self.capture_current(&policy.scope, &capture, root, now)?,
                "prior capture invalidated; start a fresh task"
            );
            capture.evidence
        } else {
            self.db.execute("DELETE FROM task_workspace_evidence_v1 WHERE tenant=?1 AND subject=?2 AND expires<=?3",params![policy.scope.tenant,policy.scope.subject,now as i64])?;
            let (all,task):(i64,i64)=self.db.query_row("SELECT COUNT(*),COALESCE(SUM(task=?3),0) FROM task_workspace_evidence_v1 WHERE tenant=?1 AND subject=?2",params![policy.scope.tenant,policy.scope.subject,request.task_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
            ensure!(
                all < MAX_CAPTURES && task < MAX_TASK_CAPTURES,
                "workspace capture capacity reached"
            );
            let evidence = Evidence {
                source_id: id.clone(),
                source_revision: format!("sha256:{hash}"),
                content_sha256: hash,
                title: format!("Workspace {:?}: {path}", kind),
                start_byte: start,
                end_byte: end,
                quote,
                observed_at_ms: now,
                valid_until_ms: now
                    .saturating_add(policy.retention_ms.min(3600000))
                    .min(i64::MAX as u64),
            };
            let capture = Capture {
                schema_version: VERSION,
                task_id: request.task_id,
                scope_sha256,
                root_sha256,
                path,
                kind,
                evidence: evidence.clone(),
            };
            let body = serde_json::to_string(&capture)?;
            ensure!(
                body.len() <= 16384,
                "workspace capture storage bound exceeded"
            );
            self.db.execute(
                "INSERT INTO task_workspace_evidence_v1 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    policy.scope.tenant,
                    policy.scope.subject,
                    id,
                    request.task_id.to_string(),
                    body,
                    crate::digest(body.as_bytes()),
                    evidence.valid_until_ms as i64
                ],
            )?;
            evidence
        };
        let metadata = if tool.name == "workspace_read" {
            let complete = receipt["offset_bytes"] == 0 && receipt["truncated"] == false;
            match super::project_metadata::inspect(
                tool.arguments["path"]
                    .as_str()
                    .context("validated path absent")?,
                &evidence,
                complete,
            ) {
                Ok(value) => serde_json::to_value(value)?,
                Err(_) => serde_json::to_value(super::project_metadata::unavailable(&evidence))?,
            }
        } else {
            Value::Null
        };
        Ok(
            json!({"evidence":[evidence],"receipt":receipt,"project_metadata":metadata,"scope":"host-confined read; no build/query executed","captured_task_id":request.task_id}),
        )
    }
    fn workspace_capture(&self, scope: &AccessScope, id: &str) -> Result<Option<Capture>> {
        let row:Option<(String,String)>=self.db.query_row("SELECT body,sha256 FROM task_workspace_evidence_v1 WHERE tenant=?1 AND subject=?2 AND id=?3",params![scope.tenant,scope.subject,id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        row.map(|(body, sha)| {
            ensure!(
                crate::digest(body.as_bytes()) == sha,
                "workspace capture integrity mismatch"
            );
            let capture: Capture = serde_json::from_str(&body)?;
            ensure!(
                capture.schema_version == VERSION,
                "unsupported workspace capture version"
            );
            Ok(capture)
        })
        .transpose()
    }
    fn capture_current(
        &self,
        scope: &AccessScope,
        capture: &Capture,
        root: &Path,
        now: u64,
    ) -> Result<bool> {
        if capture.scope_sha256 != crate::digest(&serde_json::to_vec(scope)?)
            || Some(capture.root_sha256.as_str()) != root_identity(root).ok().as_deref()
            || now < capture.evidence.observed_at_ms
            || now >= capture.evidence.valid_until_ms
        {
            return Ok(false);
        }
        let current = match capture.kind {
            Kind::File => crate::workspace::open(root, false)
                .and_then(|workspace| {
                    workspace.read(
                        &capture.path,
                        capture.evidence.start_byte,
                        capture.evidence.quote.len().max(1),
                    )
                })
                .map(|v| {
                    (
                        v["sha256"].as_str().unwrap_or_default().to_owned(),
                        v["text"].as_str().unwrap_or_default().to_owned(),
                    )
                }),
            Kind::Directory => listing(root, &capture.path).and_then(|v| {
                let text = serde_json::to_string(&v)?;
                Ok((crate::digest(text.as_bytes()), text))
            }),
        };
        Ok(current.is_ok_and(|(hash, text)| {
            hash == capture.evidence.content_sha256 && text == capture.evidence.quote
        }))
    }
    pub fn workspace_evidence_current(
        &self,
        scope: &AccessScope,
        evidence: &Evidence,
        root: &Path,
        now: u64,
    ) -> Result<bool> {
        let Some(capture) = self.workspace_capture(scope, &evidence.source_id)? else {
            return Ok(false);
        };
        Ok(capture.evidence == *evidence && self.capture_current(scope, &capture, root, now)?)
    }
    /// Recheck every captured input, even one a later answer omitted citing.
    pub fn workspace_task_current(
        &self,
        scope: &AccessScope,
        task: Uuid,
        root: &Path,
        now: u64,
    ) -> Result<bool> {
        let ids:Vec<String>=self.db.prepare("SELECT id FROM task_workspace_evidence_v1 WHERE tenant=?1 AND subject=?2 AND task=?3")?.query_map(params![scope.tenant,scope.subject,task.to_string()],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for id in ids {
            let Some(capture) = self.workspace_capture(scope, &id)? else {
                return Ok(false);
            };
            if !self.capture_current(scope, &capture, root, now)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

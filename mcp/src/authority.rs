//! Adapter for the existing owner-approved company journal. Models cannot
//! install policy or change the process scope through a JSON-RPC request.
use crate::Scope;
use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::{Component, PathBuf};

pub trait Authority {
    fn open(&self, scope: &Scope) -> Result<Connection>;
    /// Optional request-local SDK observation, computed by trusted host code
    /// before taking a writer lock. Never accepted through tool arguments.
    fn angular_sdk_seal(
        &self,
        _sdk: &crate::angular_check::runtime::HostSdk,
    ) -> Result<Option<String>> {
        Ok(None)
    }
    fn discover_models(&self) -> Result<Value> {
        bail!("model discovery unavailable for this authority")
    }
    /// Only called after open, current governance and job-state admission.
    fn workspace(&self, _scope: &Scope) -> Result<PathBuf> {
        bail!("authority has no local workspace adapter")
    }
}
pub struct SqliteAuthority {
    root: PathBuf,
}
impl SqliteAuthority {
    pub fn new(root: PathBuf) -> Result<Self> {
        Ok(Self {
            root: root.canonicalize()?,
        })
    }
}
impl Authority for SqliteAuthority {
    fn discover_models(&self) -> Result<Value> {
        Ok(
            serde_json::json!({"artifacts":crate::task_engine::models::configured(&self.root)?,"provider_probes":"use owner discovery API for configured endpoints; this native tool is offline"}),
        )
    }
    fn workspace(&self, scope: &Scope) -> Result<PathBuf> {
        scope.validate()?;
        Ok(self
            .root
            .join("data/companies")
            .join(&scope.company)
            .join("runs")
            .join(scope.run.to_string())
            .join("workspace"))
    }
    fn open(&self, scope: &Scope) -> Result<Connection> {
        scope.validate()?;
        let path = self
            .root
            .join("data/companies")
            .join(&scope.company)
            .join("brain.sqlite3");
        let mut checked = self.root.clone();
        for part in path.strip_prefix(&self.root)?.components() {
            if !matches!(part, Component::Normal(_)) {
                bail!("invalid scope path");
            }
            checked.push(part);
            if checked.is_symlink() {
                bail!("symlink in company path");
            }
        }
        let db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF;")?;
        let owner: String = db.query_row("SELECT owner FROM company LIMIT 1", [], |r| r.get(0))?;
        if owner != scope.owner {
            bail!("company is unavailable to this identity");
        }
        let (digest, approval): (String, Option<String>) = db.query_row(
            "SELECT digest,approval FROM jobs WHERE run_id=?1",
            [scope.run.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let approved: Value =
            serde_json::from_str(&approval.context("job requires operator approval")?)?;
        if approved["owner"] != scope.owner
            || approved["sha256"] != digest
            || approved["type"] != "local_operator_approval"
        {
            bail!("job approval does not bind this process scope");
        }
        db.execute_batch("CREATE TABLE IF NOT EXISTS mcp_memory(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,agent TEXT NOT NULL,phase TEXT NOT NULL,kind TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,source_refs TEXT NOT NULL,policy_revision INTEGER NOT NULL,created INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS mcp_memory_scope ON mcp_memory(run_id,policy_revision,agent,created);")?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS mcp_binding_sources_v1(run_id TEXT NOT NULL,policy_revision INTEGER NOT NULL,path TEXT NOT NULL,actor TEXT NOT NULL,tool TEXT NOT NULL,arguments_sha256 TEXT NOT NULL,result_sha256 TEXT NOT NULL,pointer TEXT NOT NULL,value_json TEXT NOT NULL,value_sha256 TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(run_id,policy_revision,path));CREATE TABLE IF NOT EXISTS mcp_binding_artifacts_v1(run_id TEXT NOT NULL,policy_revision INTEGER NOT NULL,path TEXT NOT NULL,actor TEXT NOT NULL,op TEXT NOT NULL,arguments_sha256 TEXT NOT NULL,result_sha256 TEXT NOT NULL,artifact_sha256 TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(run_id,policy_revision,path,op));")?;
        Ok(db)
    }
}

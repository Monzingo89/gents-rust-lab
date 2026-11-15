//! Durable quarantined text proposals. Submission is not execution, qualification
//! or promotion. Only an explicit frozen owner contract enables this workflow.
use crate::{
    authority::Authority, digest, governance, now_ms, safety, workspace,
    workspace_path::is_safe_relative_path, Scope,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub const ACCEPTANCE:&str="Submit a quarantined candidate with exact native readback; independent host review is required before execution or promotion.";
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub version: u32,
    pub actor: String,
    pub path: String,
    pub max_bytes: usize,
    pub max_attempts: u32,
}
pub fn from_intake(intake: &Value) -> Result<Option<Spec>> {
    let Some(value) = intake.get("candidate_workflow") else {
        return Ok(None);
    };
    let spec: Spec = serde_json::from_value(value.clone())?;
    ensure!(
        spec.version == 1
            && is_safe_relative_path(&spec.path)
            && !spec.actor.trim().is_empty()
            && spec.actor.len() <= 200
            && !spec.actor.chars().any(char::is_control)
            && spec.max_bytes > 0
            && spec.max_bytes <= 24576
            && (1..=3).contains(&spec.max_attempts),
        "invalid candidate workflow"
    );
    ensure!(
        intake["deliverable_type"] == "candidate"
            && intake["acceptance"] == ACCEPTANCE
            && intake["task_evidence"].is_null(),
        "candidate workflow requires its explicit proposal-only acceptance contract"
    );
    Ok(Some(spec))
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Author,
    Generating,
    WriteReady,
    WritePending,
    Readback,
    Ready,
    Submitted,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub stage: Stage,
    pub attempt: u32,
    pub source: Option<String>,
    pub source_sha256: Option<String>,
    pub summary: String,
    pub offset: usize,
    pub write_receipt: Option<Value>,
    pub read_receipts: Vec<Value>,
    pub reconciled_write: bool,
    pub feedback: Vec<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            stage: Stage::Author,
            attempt: 0,
            source: None,
            source_sha256: None,
            summary: String::new(),
            offset: 0,
            write_receipt: None,
            read_receipts: Vec::new(),
            reconciled_write: false,
            feedback: Vec::new(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authored {
    source: String,
    summary: String,
}
pub fn author_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["source","summary"],"properties":{"source":{"type":"string","minLength":1,"maxLength":24576},"summary":{"type":"string","maxLength":500}}})
}
pub struct Repository {
    db: Connection,
    scope: Scope,
    spec: Spec,
    contract: String,
    revision: u64,
}
impl Repository {
    pub fn open(authority: &dyn Authority, scope: Scope) -> Result<Self> {
        let db = authority.open(&scope)?;
        let (encoded, contract): (String, String) = db.query_row(
            "SELECT intake,digest FROM jobs WHERE run_id=?1",
            [scope.run.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            digest(encoded.as_bytes()) == contract,
            "frozen candidate intake digest mismatch"
        );
        let spec = from_intake(&serde_json::from_str::<Value>(&encoded)?)?
            .context("not a candidate workflow")?;
        ensure!(
            spec.actor == scope.actor && scope.phase == crate::Phase::Worker,
            "candidate actor/phase must be runtime-bound"
        );
        let policy = governance::load(&db, &scope.run.to_string())?
            .context("candidate workflow requires installed governance")?;
        for op in ["read", "write"] {
            policy.authorize(&scope.actor, "files", &json!({"op":op}))?;
        }
        db.execute_batch("CREATE TABLE IF NOT EXISTS candidate_workflows(run_id TEXT PRIMARY KEY,contract TEXT NOT NULL,revision INTEGER NOT NULL,state TEXT NOT NULL,state_sha256 TEXT NOT NULL);
  CREATE TABLE IF NOT EXISTS candidate_events(seq INTEGER PRIMARY KEY,run_id TEXT NOT NULL,stage TEXT NOT NULL,attempt INTEGER NOT NULL,detail TEXT NOT NULL,created INTEGER NOT NULL);")?;
        db.execute(
            "INSERT OR IGNORE INTO candidate_workflows VALUES(?1,?2,?3,?4,?5)",
            params![
                scope.run.to_string(),
                contract,
                policy.revision as i64,
                serde_json::to_string(&State::default())?,
                digest(serde_json::to_string(&State::default())?.as_bytes())
            ],
        )?;
        let result = Self {
            db,
            scope,
            spec,
            contract,
            revision: policy.revision,
        };
        result.state()?;
        Ok(result)
    }
    pub fn exists(db: &Connection, run: &str) -> Result<bool> {
        let table: i64 = db.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='candidate_workflows'",
            [],
            |r| r.get(0),
        )?;
        if table == 0 {
            return Ok(false);
        }
        Ok(db
            .query_row(
                "SELECT 1 FROM candidate_workflows WHERE run_id=?1",
                [run],
                |r| r.get::<_, i64>(0),
            )
            .optional()?
            .is_some())
    }
    pub fn spec(&self) -> &Spec {
        &self.spec
    }
    fn admitted(db: &Connection, scope: &Scope, contract: &str, revision: u64) -> Result<()> {
        let policy =
            governance::load(db, &scope.run.to_string())?.context("candidate policy missing")?;
        ensure!(
            policy.revision == revision,
            "candidate permission epoch changed; old source cannot be silently reused"
        );
        for op in ["read", "write"] {
            policy.authorize(&scope.actor, "files", &json!({"op":op}))?;
        }
        let (encoded, hash, approval, state): (String, String, String, String) = db.query_row(
            "SELECT intake,digest,approval,state FROM jobs WHERE run_id=?1",
            [scope.run.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let approval: Value = serde_json::from_str(&approval)?;
        ensure!(
            hash == contract
                && digest(encoded.as_bytes()) == contract
                && approval["sha256"] == contract
                && approval["owner"] == scope.owner
                && approval["type"] == "local_operator_approval",
            "candidate approval or frozen task changed"
        );
        ensure!(
            matches!(
                state.as_str(),
                "approved" | "running" | "candidate_submitted"
            ),
            "candidate job is not active or submitted"
        );
        Ok(())
    }
    fn validate_state(s: &State, spec: &Spec) -> Result<()> {
        ensure!(
            s.attempt <= spec.max_attempts
                && s.offset <= spec.max_bytes
                && s.read_receipts.len() <= 4
                && s.feedback.len() <= 3,
            "candidate checkpoint bounds exceeded"
        );
        if matches!(s.stage, Stage::Author | Stage::Generating) {
            ensure!(
                s.source.is_none()
                    && s.source_sha256.is_none()
                    && s.offset == 0
                    && s.read_receipts.is_empty()
                    && s.write_receipt.is_none(),
                "author checkpoint contains unaccepted output"
            );
        } else {
            let source = s
                .source
                .as_ref()
                .context("candidate checkpoint lost source")?;
            ensure!(
                !source.is_empty()
                    && source.len() <= spec.max_bytes
                    && Some(digest(source.as_bytes())) == s.source_sha256
                    && s.offset <= source.len()
                    && source.is_char_boundary(s.offset),
                "candidate source integrity failure"
            );
            if matches!(s.stage, Stage::Ready | Stage::Submitted) {
                ensure!(
                    s.offset == source.len()
                        && !s.read_receipts.is_empty()
                        && (s.write_receipt.is_some() || s.reconciled_write),
                    "candidate submission lacks complete readback"
                );
            }
        }
        Ok(())
    }
    fn load_from(
        db: &Connection,
        scope: &Scope,
        contract: &str,
        revision: u64,
        spec: &Spec,
    ) -> Result<State> {
        Self::admitted(db, scope, contract, revision)?;
        let (stored_contract, stored_revision, body, hash): (String, i64, String, String) = db
            .query_row(
            "SELECT contract,revision,state,state_sha256 FROM candidate_workflows WHERE run_id=?1",
            [scope.run.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        ensure!(
            stored_contract == contract
                && stored_revision == revision as i64
                && digest(body.as_bytes()) == hash,
            "candidate checkpoint binding or integrity changed"
        );
        let state: State = serde_json::from_str(&body)?;
        Self::validate_state(&state, spec)?;
        Ok(state)
    }
    pub fn state(&self) -> Result<State> {
        Self::load_from(
            &self.db,
            &self.scope,
            &self.contract,
            self.revision,
            &self.spec,
        )
    }
    fn change<T>(&mut self, edit: impl FnOnce(&mut State, &Connection) -> Result<T>) -> Result<T> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut state =
            Self::load_from(&tx, &self.scope, &self.contract, self.revision, &self.spec)?;
        let result = edit(&mut state, &tx)?;
        Self::validate_state(&state, &self.spec)?;
        let encoded = serde_json::to_string(&state)?;
        tx.execute(
            "UPDATE candidate_workflows SET state=?2,state_sha256=?3 WHERE run_id=?1",
            params![
                self.scope.run.to_string(),
                encoded,
                digest(encoded.as_bytes())
            ],
        )?;
        tx.execute("INSERT INTO candidate_events(run_id,stage,attempt,detail,created) VALUES(?1,?2,?3,?4,?5)",params![self.scope.run.to_string(),serde_json::to_string(&state.stage)?,state.attempt,json!({"source_sha256":state.source_sha256,"offset":state.offset,"qualified":false,"payload_logged":false}).to_string(),now_ms() as i64])?;
        tx.commit()?;
        Ok(result)
    }
    pub fn begin_author(&mut self) -> Result<u32> {
        let max = self.spec.max_attempts;
        self.change(|s, _| {
            ensure!(
                matches!(s.stage, Stage::Author | Stage::Generating),
                "author stage already finished"
            );
            ensure!(s.attempt < max, "candidate author attempts exhausted");
            s.attempt += 1;
            s.stage = Stage::Generating;
            Ok(s.attempt)
        })
    }
    pub fn accept_source(&mut self, value: Value) -> Result<()> {
        let authored: Authored = serde_json::from_value(value)?;
        ensure!(
            !authored.source.trim().is_empty()
                && authored.source.len() <= self.spec.max_bytes
                && authored.summary.len() <= 500,
            "candidate output exceeds frozen bounds"
        );
        safety::require_persistable(&authored.source)?;
        ensure!(
            serde_json::to_vec(
                &json!({"op":"write","path":self.spec.path,"content":authored.source})
            )?
            .len()
                <= 60000,
            "candidate cannot fit the bounded native input frame"
        );
        self.change(|s, _| {
            ensure!(s.stage == Stage::Generating, "no active author attempt");
            s.source_sha256 = Some(digest(authored.source.as_bytes()));
            s.source = Some(authored.source);
            s.summary = authored.summary;
            s.stage = Stage::WriteReady;
            Ok(())
        })
    }
    pub fn author_failed(&mut self, diagnostic: &str) -> Result<()> {
        let diagnostic = diagnostic.chars().take(500).collect::<String>();
        safety::require_persistable(&diagnostic)?;
        self.change(|s, _| {
            ensure!(s.stage == Stage::Generating, "no active author attempt");
            s.feedback.push(diagnostic);
            s.feedback.truncate(3);
            s.stage = Stage::Author;
            Ok(())
        })
    }
    pub fn prepare_write(&mut self) -> Result<Value> {
        let path = self.spec.path.clone();
        self.change(|s, _| {
            ensure!(
                s.stage == Stage::WriteReady,
                "write is not ready or is already uncertain"
            );
            let source = s.source.as_ref().context("missing candidate source")?;
            s.stage = Stage::WritePending;
            Ok(json!({"op":"write","path":path,"content":source}))
        })
    }
    fn receipt(
        db: &Connection,
        scope: &Scope,
        revision: u64,
        arguments: &Value,
        result: &Value,
    ) -> Result<Value> {
        ensure!(result["ok"] == true, "native candidate tool failed");
        let receipt = &result["mcp_receipt"];
        ensure!(
            receipt["actor"] == scope.actor
                && receipt["phase"] == "worker"
                && receipt["tool"] == "files"
                && receipt["policy_revision"] == revision
                && receipt["status"] == "completed"
                && receipt["arguments_sha256"] == digest(&serde_json::to_vec(arguments)?)
                && receipt["result_sha256"] == digest(&serde_json::to_vec(&result["result"])?),
            "candidate receipt binding mismatch"
        );
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND body=?2",
            params![scope.run.to_string(), receipt.to_string()],
            |r| r.get(0),
        )?;
        ensure!(count > 0, "candidate tool receipt is not canonical");
        Ok(receipt.clone())
    }
    pub fn wrote(&mut self, arguments: &Value, result: &Value) -> Result<()> {
        let scope = self.scope.clone();
        let revision = self.revision;
        let path = self.spec.path.clone();
        self.change(|s, db| {
            ensure!(s.stage == Stage::WritePending, "no pending candidate write");
            ensure!(
                arguments == &json!({"op":"write","path":path,"content":s.source}),
                "write arguments differ from frozen candidate"
            );
            let receipt = Self::receipt(db, &scope, revision, arguments, result)?;
            ensure!(
                result["result"]["sha256"].as_str() == s.source_sha256.as_deref(),
                "write hash differs from authored candidate"
            );
            s.write_receipt = Some(receipt);
            s.stage = Stage::Readback;
            s.offset = 0;
            Ok(())
        })
    }
    pub fn read_arguments(&self) -> Result<Value> {
        let s = self.state()?;
        ensure!(
            matches!(s.stage, Stage::WritePending | Stage::Readback),
            "readback not expected"
        );
        Ok(
            json!({"op":"read","path":self.spec.path,"offset_bytes":if s.stage==Stage::WritePending{0}else{s.offset},"max_bytes":8192}),
        )
    }
    pub fn read(&mut self, arguments: &Value, result: &Value) -> Result<()> {
        let scope = self.scope.clone();
        let revision = self.revision;
        let path = self.spec.path.clone();
        self.change(|s, db| {
            ensure!(
                matches!(s.stage, Stage::WritePending | Stage::Readback),
                "readback is not expected"
            );
            let pending = s.stage == Stage::WritePending;
            let offset = if pending { 0 } else { s.offset };
            ensure!(
                arguments
                    == &json!({"op":"read","path":path,"offset_bytes":offset,"max_bytes":8192}),
                "read arguments differ from durable offset"
            );
            let receipt = Self::receipt(db, &scope, revision, arguments, result)?;
            let page = &result["result"];
            let source = s.source.as_ref().context("missing source")?;
            let text = page["text"].as_str().context("readback text missing")?;
            let end = offset
                .checked_add(text.len())
                .context("readback overflow")?;
            ensure!(
                page["sha256"].as_str() == s.source_sha256.as_deref()
                    && page["offset_bytes"] == offset
                    && page["total_bytes"] == source.len()
                    && page["bytes"] == text.len()
                    && !text.is_empty()
                    && source.get(offset..end) == Some(text),
                "readback does not match exact authored source"
            );
            ensure!(
                page["truncated"] == (end < source.len())
                    && if end < source.len() {
                        page["next_offset_bytes"] == end
                    } else {
                        page["next_offset_bytes"].is_null()
                    },
                "readback pagination mismatch"
            );
            if pending {
                s.reconciled_write = true;
            }
            s.read_receipts.push(receipt);
            s.offset = end;
            s.stage = if end == source.len() {
                Stage::Ready
            } else {
                Stage::Readback
            };
            Ok(())
        })
    }
    pub fn submit(&mut self, authority: &dyn Authority) -> Result<Value> {
        let path = self.spec.path.clone();
        let root = authority.workspace(&self.scope)?;
        let run = self.scope.run.to_string();
        self.change(|s,db|{
   ensure!(s.stage==Stage::Ready||s.stage==Stage::Submitted,"candidate is not fully read back");
   let current=workspace::open(&root,false)?.read(&path,0,8192)?;
   ensure!(current["sha256"].as_str()==s.source_sha256.as_deref(),"candidate changed after readback");
   let state:String=db.query_row("SELECT state FROM jobs WHERE run_id=?1",[&run],|r|r.get(0))?;
   ensure!(matches!(state.as_str(),"approved"|"running"|"candidate_submitted"),"candidate submission is not active");
   db.execute("UPDATE jobs SET state='candidate_submitted' WHERE run_id=?1",[&run])?;s.stage=Stage::Submitted;
   Ok(json!({"status":"candidate_submitted","path":path,"sha256":s.source_sha256,"bytes":s.source.as_ref().map(String::len),"attempt":s.attempt,"write_reconciled_by_read":s.reconciled_write,"native_read_pages":s.read_receipts.len(),"qualified":false,"executed":false,"promoted":false,"next_stage":"independent_host_review"}))
  })
    }
}

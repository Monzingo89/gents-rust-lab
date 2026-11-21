//! Owner-scoped advisory records; pending inference never expires into replay.
use super::*;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

pub struct SqliteJournal {
    db: Connection,
    id: Uuid,
    planning: Option<Uuid>,
}
fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS architecture_assessments(id TEXT PRIMARY KEY,planning_id TEXT NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL);")?;
    Ok(())
}
fn stored(db: &Connection, id: Uuid) -> Result<Option<Value>> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT body,sha256 FROM architecture_assessments WHERE id=?1",
            [id.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    row.map(|(body, sha)| {
        ensure!(
            gents_local_mcp::digest(body.as_bytes()) == sha,
            "architecture record integrity failed"
        );
        Ok(serde_json::from_str(&body)?)
    })
    .transpose()
}
pub fn read(root: &Path, company: &str, owner: &str, id: Uuid) -> Result<Value> {
    let db = crate::company::connect(root, company, owner, false)?;
    install(&db)?;
    stored(&db, id)?.context("architecture assessment missing")
}
impl SqliteJournal {
    pub fn open(
        root: &Path,
        company: &str,
        owner: &str,
        id: Uuid,
        request: &Request,
    ) -> Result<(Self, Option<Value>)> {
        let db = crate::company::connect(root, company, owner, false)?;
        install(&db)?;
        let previous = stored(&db, id)?;
        if let Some(body) = &previous {
            ensure!(
                body["request"] == json!(request),
                "assessment ID belongs to a different request"
            );
            ensure!(
                !matches!(
                    body["status"].as_str(),
                    Some("pending_ceo" | "pending_judge" | "ceo_received" | "review_received")
                ),
                "unresolved assessment retained; inspect without automatic replay"
            );
        }
        Ok((
            Self {
                db,
                id,
                planning: None,
            },
            previous,
        ))
    }
    pub fn bind(&mut self, planning: Uuid, body: &Value) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM architecture_assessments WHERE planning_id=?1",
            [planning.to_string()],
            |r| r.get(0),
        )?;
        ensure!(
            count < 16,
            "architecture assessment budget exhausted for this planning session"
        );
        let (revision, state): (i64, String) = tx.query_row(
            "SELECT revision,state FROM ceo_planning WHERE id=?1",
            [planning.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            state == "ready"
                && Some(u64::try_from(revision)?) == body["request"]["expected_revision"].as_u64(),
            "planning changed before architecture admission"
        );
        let encoded = body.to_string();
        let sha = gents_local_mcp::digest(encoded.as_bytes());
        tx.execute(
            "INSERT INTO architecture_assessments VALUES(?1,?2,'pending_ceo',?3,?4)",
            params![self.id.to_string(), planning.to_string(), encoded, sha],
        )?;
        tx.commit()?;
        self.planning = Some(planning);
        Ok(())
    }
}
impl JournalPort for SqliteJournal {
    fn save(&mut self, state: &str, body: &Value) -> Result<()> {
        ensure!(
            body["id"] == self.id.to_string()
                && body["status"] == state
                && body["execution_authorized"] == false,
            "architecture journal identity/state changed"
        );
        // Scan untrusted task/model content, not typed native provenance.
        // Millisecond timestamps can satisfy Luhn by accident. Scanning the
        // entire receipt loses observed usage and leaves a dispatched review
        // pending even though inference has returned. Choices are separately
        // schema/receipt validated by run; rejected raw replies are withheld.
        for field in ["context", "recommendation", "review"] {
            if !body[field].is_null() {
                crate::security::require_persistable(&body[field].to_string())?;
            }
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior = stored(&tx, self.id)?.context("architecture assessment missing")?;
        for field in ["planning_id", "request", "context", "models"] {
            ensure!(
                body[field] == prior[field],
                "frozen architecture input changed"
            );
        }
        let encoded = body.to_string();
        let sha = gents_local_mcp::digest(encoded.as_bytes());
        tx.execute(
            "UPDATE architecture_assessments SET state=?2,body=?3,sha256=?4 WHERE id=?1",
            params![self.id.to_string(), state, encoded, sha],
        )?;
        let calls: Vec<Value> = ["ceo", "output_judge", "judge"].into_iter().filter(|role| body["dispatch"][role].is_object()).map(|role| {
            let receipt = &body["receipts"][role]["reply"]["local_execution"];
            json!({"agent":role,"model":body["models"][if role=="ceo" {"ceo"} else {"judge"}]["candidate_id"],"requested_at_ms":body["dispatch"][role]["started_at_ms"],"inference":{
                "backend_pid":receipt["backend_pid"],"dispatched_at_ms":receipt["dispatched_at_ms"],"received_at_ms":receipt["received_at_ms"],"finish_reason":receipt["finish_reason"]}})
        }).collect();
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'architecture_assessment_checkpoint',?2,?3)",params![self.id.to_string(),json!({"status":state,"body_sha256":sha,"execution_authorized":false,"planning_id":body["planning_id"],"calls":calls}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        Ok(())
    }
    fn current_revision(&self) -> Result<u64> {
        let revision: i64 = self.db.query_row(
            "SELECT revision FROM ceo_planning WHERE id=?1",
            [self
                .planning
                .context("architecture journal not bound")?
                .to_string()],
            |r| r.get(0),
        )?;
        Ok(u64::try_from(revision)?)
    }
}

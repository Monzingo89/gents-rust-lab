//! Canonical native reply checkpoints in the authenticated company database.
//! A pending turn is deliberately not a lease that expires into automatic replay.
mod review_store;
use super::*;
use crate::workflow::ControllerLease;
pub use review_store::validate_completed;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::Path;
use uuid::Uuid;

pub struct SqliteJournal {
    db: Connection,
    run: Uuid,
    _controller: ControllerLease,
    contract: Option<Contract>,
    revision: u64,
}
impl SqliteJournal {
    pub fn open(root: &Path, company: &str, owner: &str, run: Uuid) -> Result<Self> {
        let db = crate::company::connect(root, company, owner, false)?;
        let controller = ControllerLease::acquire(root, company, owner, run)?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS coordination_programs(run_id TEXT PRIMARY KEY,contract TEXT NOT NULL,sha256 TEXT NOT NULL,policy_revision INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS coordination_turns(run_id TEXT NOT NULL,ordinal INTEGER NOT NULL,turn TEXT NOT NULL,packet_sha256 TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('pending','completed')),message TEXT,receipt TEXT,sha256 TEXT,memory_source_id TEXT,PRIMARY KEY(run_id,ordinal));
            CREATE TABLE IF NOT EXISTS coordination_reviews(run_id TEXT PRIMARY KEY,state TEXT NOT NULL CHECK(state IN ('pending','received')),snapshot TEXT NOT NULL,snapshot_sha256 TEXT NOT NULL,receipt TEXT,receipt_sha256 TEXT);")?;
        Ok(Self {
            db,
            run,
            _controller: controller,
            contract: None,
            revision: 0,
        })
    }
    fn admitted(
        db: &Connection,
        run: Uuid,
        contract: &Contract,
        revision: u64,
    ) -> Result<crate::governance::Policy> {
        let (encoded, sha, approval, state): (String, String, Option<String>, String) = db
            .query_row(
                "SELECT intake,digest,approval,state FROM jobs WHERE run_id=?1",
                [run.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
        let intake: Value = serde_json::from_str(&encoded)?;
        let approval: Value =
            serde_json::from_str(&approval.context("coordination needs owner approval")?)?;
        ensure!(
            gents_local_mcp::digest(encoded.as_bytes()) == sha
                && approval["sha256"] == sha
                && state == "running"
                && intake["coordination_contract"] == json!(contract),
            "coordination contract differs from approved running job"
        );
        let policy = crate::governance::load(db, &run.to_string())?
            .context("coordination requires installed governance")?;
        ensure!(
            policy.revision == revision,
            "coordination policy epoch changed"
        );
        super::admission::validate_policy(&intake, &policy)?;
        Ok(policy)
    }
    fn records(db: &Connection, run: Uuid) -> Result<Vec<Message>> {
        let mut query=db.prepare("SELECT ordinal,turn,packet_sha256,message,receipt,sha256 FROM coordination_turns WHERE run_id=?1 AND state='completed' ORDER BY ordinal")?;
        let mut rows = query.query([run.to_string()])?;
        let mut messages = vec![];
        while let Some(row) = rows.next()? {
            let ordinal = usize::try_from(row.get::<_, i64>(0)?)?;
            ensure!(
                ordinal == messages.len(),
                "coordination checkpoint has an ordinal gap"
            );
            let turn: String = row.get(1)?;
            let packet: String = row.get(2)?;
            let body: String = row.get(3)?;
            let receipt: String = row.get(4)?;
            let sha: String = row.get(5)?;
            ensure!(
                crate::company_factory::digest(&json!({"message":body,"receipt":receipt}))? == sha,
                "coordination checkpoint integrity failure"
            );
            let message: Message = serde_json::from_str(&body)?;
            let receipt: crate::provider::Reply = serde_json::from_str(&receipt)?;
            validate_receipt(&receipt)?;
            ensure!(
                serde_json::from_str::<Turn>(&turn)? == message.turn
                    && message.turn.ordinal == ordinal
                    && serde_json::from_str::<Value>(&receipt.text)? == message.content
                    && receipt
                        .local_execution
                        .as_ref()
                        .and_then(|r| r.task_input_sha256.as_ref())
                        == Some(&packet),
                "coordination receipt or turn binding changed"
            );
            messages.push(message);
            ensure!(
                messages.len() <= 20,
                "coordination checkpoint budget exceeded"
            );
        }
        Ok(messages)
    }
}
impl JournalRepository for SqliteJournal {
    fn bind(&mut self, contract: &Contract, revision: u64) -> Result<()> {
        let sha = contract.sha256()?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        Self::admitted(&tx, self.run, contract, revision)?;
        let existing: Option<(String, String, i64)> = tx
            .query_row(
                "SELECT contract,sha256,policy_revision FROM coordination_programs WHERE run_id=?1",
                [self.run.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((body, existing, epoch)) = existing {
            ensure!(
                existing == sha
                    && epoch == i64::try_from(revision)?
                    && gents_local_mcp::digest(body.as_bytes()) == existing,
                "coordination program or policy changed; use a fresh approved run"
            );
        } else {
            tx.execute(
                "INSERT INTO coordination_programs VALUES(?1,?2,?3,?4)",
                params![
                    self.run.to_string(),
                    serde_json::to_string(contract)?,
                    sha,
                    i64::try_from(revision)?
                ],
            )?;
        }
        tx.commit()?;
        self.contract = Some(contract.clone());
        self.revision = revision;
        Ok(())
    }
    fn history(&self) -> Result<Vec<Message>> {
        let pending: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM coordination_turns WHERE run_id=?1 AND state='pending'",
            [self.run.to_string()],
            |r| r.get(0),
        )?;
        ensure!(
            pending == 0,
            "unresolved or rejected coordination turn retained; no automatic inference replay"
        );
        let messages = Self::records(&self.db, self.run)?;
        program::next(
            self.contract
                .as_ref()
                .context("coordination journal not bound")?,
            &messages,
        )?;
        Ok(messages)
    }
    fn begin(&mut self, turn: &Turn, packet_sha256: &str) -> Result<()> {
        ensure!(
            program::hash(packet_sha256),
            "invalid dispatch packet digest"
        );
        let contract = self
            .contract
            .as_ref()
            .context("coordination journal not bound")?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let policy = Self::admitted(&tx, self.run, contract, self.revision)?;
        let messages = Self::records(&tx, self.run)?;
        ensure!(
            program::next(contract, &messages)?.as_ref() == Some(turn),
            "dispatch turn differs from current frozen program"
        );
        authorize(&policy, contract, turn, &messages)?;
        tx.execute("INSERT INTO coordination_turns(run_id,ordinal,turn,packet_sha256,state) VALUES(?1,?2,?3,?4,'pending')",params![self.run.to_string(),i64::try_from(turn.ordinal)?,serde_json::to_string(turn)?,packet_sha256])?;
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'coordination_turn_started',?2,?3)",params![self.run.to_string(),json!({"ordinal":turn.ordinal,"actor":turn.actor,"purpose":turn.purpose,"packet_sha256":packet_sha256,"policy_revision":self.revision,"task_accepted":false}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        Ok(())
    }
    fn finish(&mut self, message: &Message, receipt: &crate::provider::Reply) -> Result<()> {
        validate_receipt(receipt)?;
        program::validate_content(&message.turn, &message.content)?;
        ensure!(
            serde_json::from_str::<Value>(&receipt.text)? == message.content,
            "candidate differs from native reply"
        );
        let contract = self
            .contract
            .as_ref()
            .context("coordination journal not bound")?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let policy = Self::admitted(&tx, self.run, contract, self.revision)?;
        let messages = Self::records(&tx, self.run)?;
        evidence::validate(contract, &message.turn, &message.content, &messages)?;
        ensure!(
            program::next(contract, &messages)?.as_ref() == Some(&message.turn)
                && program::lineage(&message.turn, &messages)? == message.lineage,
            "receipt differs from current coordination program"
        );
        authorize(&policy, contract, &message.turn, &messages)?;
        let (pending,packet):(String,String)=tx.query_row("SELECT turn,packet_sha256 FROM coordination_turns WHERE run_id=?1 AND ordinal=?2 AND state='pending'",params![self.run.to_string(),i64::try_from(message.turn.ordinal)?],|r|Ok((r.get(0)?,r.get(1)?)))?;
        ensure!(
            serde_json::from_str::<Turn>(&pending)? == message.turn
                && receipt
                    .local_execution
                    .as_ref()
                    .and_then(|r| r.task_input_sha256.as_ref())
                    == Some(&packet),
            "pending dispatch does not match native receipt"
        );
        let body = serde_json::to_string(message)?;
        let encoded = serde_json::to_string(receipt)?;
        let sha = crate::company_factory::digest(&json!({"message":body,"receipt":encoded}))?;
        let context=json!({"version":1,"from_gent":message.turn.actor,"policy_revision":self.revision,"phase":"coordination","output":receipt.text}).to_string();
        let source = crate::company::stage_in(
            &tx,
            self.run,
            &message.turn.actor,
            &context,
            Some(self.revision),
        )?;
        ensure!(tx.execute("UPDATE coordination_turns SET state='completed',message=?3,receipt=?4,sha256=?5,memory_source_id=?6 WHERE run_id=?1 AND ordinal=?2 AND state='pending'",params![self.run.to_string(),i64::try_from(message.turn.ordinal)?,body,encoded,sha,source])?==1,"coordination receipt already committed");
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'coordination_turn_completed',?2,?3)",params![self.run.to_string(),json!({"ordinal":message.turn.ordinal,"actor":message.turn.actor,"purpose":message.turn.purpose,"policy_revision":self.revision,"checkpoint_sha256":sha,"memory_source_id":source,"task_accepted":false}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        crate::memory::wake();
        Ok(())
    }
    fn reject(&mut self, turn: &Turn, receipt: &crate::provider::Reply) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pending:String=tx.query_row("SELECT turn FROM coordination_turns WHERE run_id=?1 AND ordinal=?2 AND state='pending'",params![self.run.to_string(),i64::try_from(turn.ordinal)?],|r|r.get(0))?;
        ensure!(
            serde_json::from_str::<Turn>(&pending)? == *turn,
            "rejected receipt does not match pending turn"
        );
        let output_sha256 = gents_local_mcp::digest(receipt.text.as_bytes());
        let safe = crate::provider::Reply {
            text: "[Rejected coordination content; hash and native usage retained]".into(),
            ..receipt.clone()
        };
        let record = json!({"status":"rejected","rejected_output_sha256":output_sha256,"native_reply":safe,"task_accepted":false,"scope":"Observed reply rejected by host policy/schema/receipt checks. Content withheld; pending checkpoint cannot replay inference automatically."});
        tx.execute("UPDATE coordination_turns SET receipt=?3 WHERE run_id=?1 AND ordinal=?2 AND state='pending'",params![self.run.to_string(),i64::try_from(turn.ordinal)?,record.to_string()])?;
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'coordination_turn_rejected',?2,?3)",params![self.run.to_string(),json!({"ordinal":turn.ordinal,"actor":turn.actor,"rejected_output_sha256":output_sha256,"usage":receipt.usage,"local_execution":receipt.local_execution,"task_accepted":false}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        Ok(())
    }
}

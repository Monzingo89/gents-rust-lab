//! Transactional review checkpoints over the canonical owner-approved job.
use super::*;
use crate::coordination::review::{self, contract::ReviewContract, ReviewRepository, Snapshot};

fn snapshot_in(
    db: &Connection,
    run: Uuid,
    program: &Contract,
    contract: &ReviewContract,
    revision: u64,
) -> Result<Snapshot> {
    contract.validate(program)?;
    let policy = SqliteJournal::admitted(db, run, program, revision)?;
    let (encoded, intake_sha): (String, String) = db.query_row(
        "SELECT intake,digest FROM jobs WHERE run_id=?1",
        [run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    ensure!(
        intake["reasoning_acceptance"] == json!(contract),
        "review access differs from owner-approved intake"
    );
    let (body, sha, epoch): (String, String, i64) = db.query_row(
        "SELECT contract,sha256,policy_revision FROM coordination_programs WHERE run_id=?1",
        [run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    ensure!(
        sha == program.sha256()?
            && gents_local_mcp::digest(body.as_bytes()) == sha
            && epoch == i64::try_from(revision)?,
        "canonical coordination program changed before review"
    );
    let pending: i64 = db.query_row(
        "SELECT COUNT(*) FROM coordination_turns WHERE run_id=?1 AND state='pending'",
        [run.to_string()],
        |r| r.get(0),
    )?;
    ensure!(pending == 0, "review cannot consume ambiguous worker turns");
    let history = SqliteJournal::records(db, run)?;
    ensure!(
        program::next(program, &history)?.is_none(),
        "review cannot consume an unfinished program"
    );
    for (index, message) in history.iter().enumerate() {
        authorize(&policy, program, &message.turn, &history[..index])?;
    }
    let final_message = history.last().context("review candidate missing")?;
    ensure!(
        contract.candidate_from.contains(&final_message.turn.actor)
            && final_message.lineage.is_subset(&contract.candidate_from),
        "review candidate exceeds owner-approved source ceiling"
    );
    let mut query =
        db.prepare("SELECT sha256 FROM coordination_turns WHERE run_id=?1 ORDER BY ordinal")?;
    let checkpoints = query
        .query_map([run.to_string()], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let history_sha = crate::company_factory::digest(
        &json!({"messages":history,"native_checkpoints":checkpoints}),
    )?;
    let packet = json!({"objective":program.objective,"criteria":crate::completion_review::criteria(&program.definition_of_done)?,"input":program.input,"candidate":final_message.content,"candidate_actor":final_message.turn.actor,"candidate_lineage":final_message.lineage,"candidate_sha256":crate::company_factory::digest(final_message)?,"history_sha256":history_sha,"tools_executed":[],"scope":"Original task input and final reasoning candidate only. No historical memory, tool evidence or authority in worker prose. Citations identify available content, not verified truth."});
    ensure!(
        serde_json::to_vec(&packet)?.len() <= 32_000,
        "review context exceeds budget; no content silently dropped"
    );
    Ok(Snapshot {
        intake_sha256: intake_sha,
        program_sha256: sha,
        history_sha256: history_sha,
        policy_sha256: crate::company_factory::digest(&policy)?,
        policy_revision: revision,
        review_contract_sha256: contract.sha256()?,
        packet,
    })
}
type StoredReview = (String, String, String, Option<String>, Option<String>);
fn received(
    db: &Connection,
    run: Uuid,
    snapshot: &Snapshot,
) -> Result<Option<crate::provider::Reply>> {
    let row:Option<StoredReview>=db.query_row("SELECT state,snapshot,snapshot_sha256,receipt,receipt_sha256 FROM coordination_reviews WHERE run_id=?1",[run.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
    let Some((state, body, sha, reply, reply_sha)) = row else {
        return Ok(None);
    };
    ensure!(
        sha == snapshot.sha256()? && gents_local_mcp::digest(body.as_bytes()) == sha,
        "review checkpoint source or policy changed"
    );
    ensure!(
        state == "received",
        "unresolved independent review retained; no automatic inference replay"
    );
    let encoded = reply.context("review native receipt missing")?;
    ensure!(
        Some(gents_local_mcp::digest(encoded.as_bytes())) == reply_sha,
        "review receipt integrity failure"
    );
    let reply = serde_json::from_str(&encoded)?;
    review::validate_reply(snapshot, &reply)?;
    Ok(Some(reply))
}
impl ReviewRepository for SqliteJournal {
    fn snapshot(&self, contract: &ReviewContract) -> Result<Snapshot> {
        snapshot_in(
            &self.db,
            self.run,
            self.contract.as_ref().context("review journal not bound")?,
            contract,
            self.revision,
        )
    }
    fn begin_review(
        &mut self,
        contract: &ReviewContract,
        snapshot: &Snapshot,
    ) -> Result<Option<crate::provider::Reply>> {
        let program = self.contract.as_ref().context("review journal not bound")?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(
            snapshot_in(&tx, self.run, program, contract, self.revision)?.sha256()?
                == snapshot.sha256()?,
            "review snapshot changed before dispatch"
        );
        let cached = received(&tx, self.run, snapshot)?;
        if cached.is_none() {
            tx.execute("INSERT INTO coordination_reviews(run_id,state,snapshot,snapshot_sha256) VALUES(?1,'pending',?2,?3)",params![self.run.to_string(),serde_json::to_string(snapshot)?,snapshot.sha256()?])?;
            tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'coordination_review_started',?2,?3)",params![self.run.to_string(),json!({"snapshot_sha256":snapshot.sha256()?,"packet_sha256":crate::company_factory::digest(&snapshot.packet)?,"review_model_sha256":contract.reviewer.sha256,"policy_revision":self.revision,"accepted":false}).to_string(),crate::now_ms() as i64])?;
        }
        tx.commit()?;
        Ok(cached)
    }
    fn finish_review(
        &mut self,
        contract: &ReviewContract,
        snapshot: &Snapshot,
        reply: &crate::provider::Reply,
    ) -> Result<()> {
        review::validate_reply(snapshot, reply)?;
        let program = self.contract.as_ref().context("review journal not bound")?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(
            snapshot_in(&tx, self.run, program, contract, self.revision)?.sha256()?
                == snapshot.sha256()?,
            "review snapshot changed before receipt commit"
        );
        let encoded = serde_json::to_string(reply)?;
        ensure!(tx.execute("UPDATE coordination_reviews SET state='received',receipt=?3,receipt_sha256=?4 WHERE run_id=?1 AND state='pending' AND snapshot_sha256=?2",params![self.run.to_string(),snapshot.sha256()?,encoded,gents_local_mcp::digest(encoded.as_bytes())])?==1,"review has no matching pending dispatch");
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'coordination_review_received',?2,?3)",params![self.run.to_string(),review::assessment(snapshot,reply)?.to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        Ok(())
    }
    fn reject_review(&mut self, snapshot: &Snapshot, reply: &crate::provider::Reply) -> Result<()> {
        let safe = crate::provider::Reply {
            text: "[Rejected review content withheld; native usage retained]".into(),
            ..reply.clone()
        };
        let encoded=json!({"rejected_reply":safe,"output_sha256":gents_local_mcp::digest(reply.text.as_bytes()),"accepted":false}).to_string();
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(tx.execute("UPDATE coordination_reviews SET receipt=?3,receipt_sha256=?4 WHERE run_id=?1 AND state='pending' AND snapshot_sha256=?2",params![self.run.to_string(),snapshot.sha256()?,encoded,gents_local_mcp::digest(encoded.as_bytes())])?==1,"rejected review has no matching pending dispatch");
        tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'coordination_review_rejected',?2,?3)",params![self.run.to_string(),json!({"output_sha256":gents_local_mcp::digest(reply.text.as_bytes()),"usage":reply.usage,"local_execution":reply.local_execution,"accepted":false}).to_string(),crate::now_ms() as i64])?;
        tx.commit()?;
        Ok(())
    }
}
/// Called inside the terminal lifecycle transaction before any completion or
/// completed-task memory upsert. A model verdict alone cannot bypass this gate.
pub fn validate_completed(db: &Connection, run: Uuid) -> Result<()> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT intake,state FROM jobs WHERE run_id=?1",
            [run.to_string()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((encoded, state)) = row else {
        return Ok(());
    };
    let intake: Value = serde_json::from_str(&encoded)?;
    let Some(program) = admission::from_intake(&intake)? else {
        return Ok(());
    };
    let contract = review::contract::from_intake(&intake)?
        .context("reasoning candidate has no owner-approved independent acceptance contract")?;
    // Already committed completion is historical, not a fresh inference or
    // permission to execute. Do not reinterpret it under a later policy epoch.
    if state == "completed" {
        return Ok(());
    }
    let revision = crate::governance::load(db, &run.to_string())?
        .context("review policy missing")?
        .revision;
    let snapshot = snapshot_in(db, run, &program, &contract, revision)?;
    let reply =
        received(db, run, &snapshot)?.context("independent review has not been received")?;
    ensure!(
        review::assessment(&snapshot, &reply)?["accepted"] == true,
        "independent reviewer did not accept every frozen criterion"
    );
    Ok(())
}

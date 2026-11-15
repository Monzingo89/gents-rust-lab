//! Explicitly simulated native receipts. Tests prove host admission/lifecycle
//! invariants, not model competence or independent task correctness.
use super::*;
use crate::company_factory::contracts::Coordination;
use crate::coordination::tests as fixture;
use std::{path::PathBuf, sync::atomic::AtomicUsize};
use uuid::Uuid;
const OWNER: &str = "coordination-fixture-owner";
fn contract(program: &Contract) -> ReviewContract {
    ReviewContract {version:1,coordination_sha256:program.sha256().unwrap(),model_policy_sha256:"c".repeat(64),reviewer:serde_json::from_value(json!({"id":"independent","model":{"provider":"local","model":"SIMULATED distinct judge","path":"models/simulated-judge.gguf"},"sha256":"b".repeat(64),"minimum_memory_mib":512})).unwrap(),candidate_from:program.roles.iter().map(|r|r.actor.clone()).collect(),maximum_calls:1}
}
struct Fixture {
    root: PathBuf,
    program: Contract,
    review: ReviewContract,
    run: Uuid,
    access: fixture::AccessFixture,
}
impl Fixture {
    fn new(pattern: Coordination) -> Self {
        let root = std::env::temp_dir().join(format!("reasoning-review-{}", Uuid::new_v4()));
        let program = fixture::contract(pattern);
        let review = contract(&program);
        let run = Uuid::new_v4();
        fixture::install_reviewed(&root, "fixture", run, &program, &review).unwrap();
        Self {
            root,
            review,
            run,
            access: fixture::AccessFixture::new(&program),
            program,
        }
    }
    fn journal(&self) -> journal::SqliteJournal {
        journal::SqliteJournal::open(&self.root, "fixture", OWNER, self.run).unwrap()
    }
    fn db(&self) -> rusqlite::Connection {
        crate::company::connect(&self.root, "fixture", OWNER, false).unwrap()
    }
    async fn candidates(&self, journal: &mut journal::SqliteJournal) {
        Engine {
            workers: &fixture::Workers::new(),
            access: &self.access,
            journal,
        }
        .run(&self.program, &AtomicBool::new(false))
        .await
        .unwrap();
    }
    fn finish(&self, state: &str) -> Result<()> {
        crate::company::outcome(&self.root, "fixture", OWNER, self.run, state)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
struct Reviewer<'a> {
    mode: &'a str,
    access: &'a fixture::AccessFixture,
    calls: AtomicUsize,
    stopped: AtomicBool,
}
impl<'a> Reviewer<'a> {
    fn new(mode: &'a str, access: &'a fixture::AccessFixture) -> Self {
        Self {
            mode,
            access,
            calls: AtomicUsize::new(0),
            stopped: AtomicBool::new(false),
        }
    }
}
#[async_trait]
impl ReviewerPort for Reviewer<'_> {
    async fn verify(&self) -> Result<()> {
        Ok(())
    }
    async fn complete(&self, packet: &Value, _: u64, _: &AtomicBool) -> Result<Reply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        ensure!(self.mode != "lost", "SIMULATED unreceived inference");
        if self.mode == "revoke" {
            self.access.policy.lock().unwrap().revision = 2;
            self.access.sender.send_replace(2);
            return std::future::pending().await;
        }
        let accepted = self.mode != "reject";
        let mut reply = fixture::receipt(
            packet,
            json!({"verdict":if self.mode=="uncertain" {"uncertain"}else if accepted {"accepted"}else{"revise"},"summary":"SIMULATED independent review","checks":{"0":{"passed":accepted,"evidence":"SIMULATED criterion judgment, not a real result"}}}),
        );
        if self.mode == "wrong_packet" {
            reply.local_execution.as_mut().unwrap().task_input_sha256 = Some("f".repeat(64));
        }
        if self.mode == "malformed" {
            reply.text = r#"{"verdict":"accepted","summary":"SIMULATED","checks":{}}"#.into();
        }
        Ok(reply)
    }
    async fn cancel(&self, _: u64) {
        self.stopped.store(true, Ordering::SeqCst);
    }
}
#[tokio::test]
async fn only_independent_review_can_complete_each_pattern_and_promote_its_memory() {
    for pattern in [
        Coordination::Ordered,
        Coordination::AdaptiveDelegation,
        Coordination::PeerReview,
    ] {
        let f = Fixture::new(pattern);
        let mut journal = f.journal();
        f.candidates(&mut journal).await;
        assert!(f.finish("completed").is_err());
        f.db().execute("INSERT INTO context VALUES(?1,?2,'UnrelatedFinance','PSTC','SIMULATED_UNRELATED_MEMORY','model_output',0)",rusqlite::params![Uuid::new_v4().to_string(),f.run.to_string()]).unwrap();
        let snapshot = journal.snapshot(&f.review).unwrap();
        assert!(!snapshot
            .packet
            .to_string()
            .contains("SIMULATED_UNRELATED_MEMORY"));
        assert!(snapshot.packet.get("history").is_none());
        assert_eq!(snapshot.packet["tools_executed"], json!([]));
        let reviewer = Reviewer::new("accept", &f.access);
        let mut engine = ReviewEngine {
            reviewer: &reviewer,
            access: &f.access,
            journal: &mut journal,
        };
        let result = engine
            .run(&f.review, 1, &AtomicBool::new(false))
            .await
            .unwrap();
        assert_eq!(result["accepted"], true);
        let cached = engine
            .run(&f.review, 1, &AtomicBool::new(false))
            .await
            .unwrap();
        assert_eq!(cached, result);
        assert_eq!(reviewer.calls.load(Ordering::SeqCst), 1);
        // A policy epoch change after the received review still blocks the
        // actual completion/memory transaction; no verdict bypass exists.
        let db = f.db();
        let encoded: String = db
            .query_row(
                "SELECT manifest FROM run_governance WHERE run_id=?1",
                [f.run.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        let mut changed: Value = serde_json::from_str(&encoded).unwrap();
        changed["revision"] = json!(2);
        db.execute(
            "UPDATE run_governance SET revision=2,manifest=?2 WHERE run_id=?1",
            rusqlite::params![f.run.to_string(), changed.to_string()],
        )
        .unwrap();
        assert!(f.finish("completed").is_err());
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM context_archive WHERE tier='ALTC'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        db.execute(
            "UPDATE run_governance SET revision=1,manifest=?2 WHERE run_id=?1",
            rusqlite::params![f.run.to_string(), encoded],
        )
        .unwrap();
        // Remove deliberately unrelated context; it is not a worker's reviewed
        // contribution and should not be included in this task archive check.
        db.execute("DELETE FROM context WHERE agent='UnrelatedFinance'", [])
            .unwrap();
        f.finish("completed").unwrap();
        assert!(
            db.query_row(
                "SELECT COUNT(*) FROM context_archive WHERE tier='ALTC'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap()
                > 0
        );
        assert!(journal.snapshot(&f.review).is_err());
        f.finish("completed").unwrap(); // Lifecycle idempotency, no inference.
    }
}
#[tokio::test]
async fn rejection_uncertainty_and_invalid_checks_never_accept_or_retry() {
    for mode in ["reject", "uncertain", "malformed"] {
        let f = Fixture::new(Coordination::Ordered);
        let mut journal = f.journal();
        f.candidates(&mut journal).await;
        let reviewer = Reviewer::new(mode, &f.access);
        let mut engine = ReviewEngine {
            reviewer: &reviewer,
            access: &f.access,
            journal: &mut journal,
        };
        assert_eq!(
            engine
                .run(&f.review, 1, &AtomicBool::new(false))
                .await
                .unwrap()["accepted"],
            false
        );
        assert_eq!(
            engine
                .run(&f.review, 1, &AtomicBool::new(false))
                .await
                .unwrap()["accepted"],
            false
        );
        assert_eq!(reviewer.calls.load(Ordering::SeqCst), 1);
        assert!(f.finish("completed").is_err());
        f.finish("incomplete").unwrap();
        assert_eq!(
            f.db()
                .query_row(
                    "SELECT COUNT(*) FROM context_archive WHERE tier='ALTC'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
#[tokio::test]
async fn ambiguous_and_mismatched_review_calls_survive_restart_without_replay() {
    for mode in ["lost", "wrong_packet"] {
        let f = Fixture::new(Coordination::Ordered);
        let mut journal = f.journal();
        f.candidates(&mut journal).await;
        let reviewer = Reviewer::new(mode, &f.access);
        assert!(ReviewEngine {
            reviewer: &reviewer,
            access: &f.access,
            journal: &mut journal
        }
        .run(&f.review, 1, &AtomicBool::new(false))
        .await
        .is_err());
        drop(journal);
        let mut journal = f.journal();
        journal.bind(&f.program, 1).unwrap();
        assert!(ReviewEngine {
            reviewer: &reviewer,
            access: &f.access,
            journal: &mut journal
        }
        .run(&f.review, 1, &AtomicBool::new(false))
        .await
        .is_err());
        assert_eq!(reviewer.calls.load(Ordering::SeqCst), 1);
        assert!(f.finish("completed").is_err());
        let (state, receipt): (String, Option<String>) = f
            .db()
            .query_row(
                "SELECT state,receipt FROM coordination_reviews WHERE run_id=?1",
                [f.run.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, "pending");
        assert_eq!(receipt.is_some(), mode == "wrong_packet");
    }
}
#[tokio::test]
async fn revocation_stops_review_and_corrupt_worker_sources_block_acceptance() {
    let f = Fixture::new(Coordination::Ordered);
    let mut journal = f.journal();
    f.candidates(&mut journal).await;
    let reviewer = Reviewer::new("revoke", &f.access);
    assert!(ReviewEngine {
        reviewer: &reviewer,
        access: &f.access,
        journal: &mut journal
    }
    .run(&f.review, 1, &AtomicBool::new(false))
    .await
    .is_err());
    assert!(reviewer.stopped.load(Ordering::SeqCst));
    assert!(f.finish("completed").is_err());
    drop(journal);
    let f = Fixture::new(Coordination::PeerReview);
    let mut journal = f.journal();
    f.candidates(&mut journal).await;
    let reviewer = Reviewer::new("accept", &f.access);
    ReviewEngine {
        reviewer: &reviewer,
        access: &f.access,
        journal: &mut journal,
    }
    .run(&f.review, 1, &AtomicBool::new(false))
    .await
    .unwrap();
    f.db()
        .execute(
            "UPDATE coordination_turns SET sha256='corrupt' WHERE run_id=?1 AND ordinal=0",
            [f.run.to_string()],
        )
        .unwrap();
    assert!(f.finish("completed").is_err());
    assert!(ReviewEngine {
        reviewer: &reviewer,
        access: &f.access,
        journal: &mut journal
    }
    .run(&f.review, 1, &AtomicBool::new(false))
    .await
    .is_err());
    assert_eq!(reviewer.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn acceptance_contract_cannot_share_worker_weights_or_request_extra_authority() {
    let p = fixture::contract(Coordination::Ordered);
    let c = contract(&p);
    c.validate(&p).unwrap();
    let mut changed = c.clone();
    changed.reviewer.sha256 = p.roles[0].model_sha256.clone();
    assert!(changed.validate(&p).is_err());
    let mut changed = c.clone();
    changed.candidate_from.insert("Finance".into());
    assert!(changed.validate(&p).is_err());
    let mut changed = c.clone();
    changed.maximum_calls = 2;
    assert!(changed.validate(&p).is_err());
    let mut wire = json!(c);
    wire["tools"] = json!(["files:read"]);
    assert!(serde_json::from_value::<ReviewContract>(wire).is_err());
    let mut intake = fixture::intake(&p);
    intake["reasoning_acceptance"] = json!(c);
    contract::from_intake(&intake).unwrap();
    intake["coordination_contract"]["input"] = json!({"changed":true});
    assert!(contract::from_intake(&intake).is_err());
}

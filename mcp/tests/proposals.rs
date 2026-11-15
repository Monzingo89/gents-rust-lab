#![cfg(any(target_os = "macos", target_os = "linux"))]
mod support;
use gents_local_mcp::{
    digest, governance,
    proposals::{self, Repository, Stage},
    Phase, Registry, Scope,
};
use rusqlite::params;
use serde_json::{json, Value};
use support::{install_files, Fixture};
fn configured() -> Fixture {
    let f = Fixture::new();
    install_files(&f);
    let intake = json!({"idea":"Author a candidate, not a completed implementation","acceptance":proposals::ACCEPTANCE,"deliverable_type":"candidate","candidate_workflow":{"version":1,"actor":"Worker","path":"candidate.rs","max_bytes":24576,"max_attempts":3}});
    let encoded = intake.to_string();
    let hash = digest(encoded.as_bytes());
    f.db()
        .execute(
            "UPDATE jobs SET intake=?1,digest=?2,approval=?3,state='running'",
            params![
                encoded,
                hash,
                json!({"owner":f.scope.owner,"sha256":hash,"type":"local_operator_approval"})
                    .to_string()
            ],
        )
        .unwrap();
    f
}
fn invoke(f: &Fixture, args: &Value) -> Value {
    let result = Registry::new()
        .call(&f.authority, &f.scope, "files", args.clone())
        .unwrap();
    json!({"ok":true,"result":result["value"],"mcp_receipt":result["receipt"]})
}
fn author(repo: &mut Repository, source: &str) {
    repo.begin_author().unwrap();
    repo.accept_source(json!({"source":source,"summary":"Unexecuted candidate"}))
        .unwrap();
}
fn write(f: &Fixture, repo: &mut Repository) {
    let args = repo.prepare_write().unwrap();
    let result = invoke(f, &args);
    repo.wrote(&args, &result).unwrap();
}
fn read(f: &Fixture, repo: &mut Repository) {
    let args = repo.read_arguments().unwrap();
    let result = invoke(f, &args);
    repo.read(&args, &result).unwrap();
}
#[test]
fn proposal_mode_requires_an_explicit_frozen_proposal_only_contract() {
    assert!(proposals::from_intake(&json!({"idea":"normal task"}))
        .unwrap()
        .is_none());
    for changes in [
        json!({"acceptance":"Complete the real task"}),
        json!({"deliverable_type":"production"}),
        json!({"candidate_workflow":{"version":1,"actor":"Worker","path":"../outside","max_bytes":100,"max_attempts":3}}),
    ] {
        let f = configured();
        let encoded: String = f
            .db()
            .query_row("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap();
        let mut intake: Value = serde_json::from_str(&encoded).unwrap();
        for (k, v) in changes.as_object().unwrap() {
            intake[k] = v.clone();
        }
        assert!(proposals::from_intake(&intake).is_err());
    }
    let f = configured();
    let wrong = Scope {
        phase: Phase::Planner,
        ..f.scope.clone()
    };
    assert!(Repository::open(&f.authority, wrong).is_err());
    assert!(Repository::open(&f.authority, f.actor("Reader")).is_err());
}
#[test]
fn actual_native_pages_submit_a_candidate_and_freeze_further_writes() {
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    assert!(repo.submit(&f.authority).is_err());
    assert!(repo
        .accept_source(json!({"source":"not generated yet","summary":""}))
        .is_err());
    let source = "αβγ unicode candidate\n".repeat(500);
    author(&mut repo, &source);
    write(&f, &mut repo);
    assert!(repo.submit(&f.authority).is_err());
    while repo.state().unwrap().stage == Stage::Readback {
        read(&f, &mut repo);
    }
    let result = repo.submit(&f.authority).unwrap();
    assert_eq!(result["status"], "candidate_submitted");
    assert_eq!(result["sha256"], digest(source.as_bytes()));
    assert_eq!(result["qualified"], false);
    assert_eq!(result["executed"], false);
    assert_eq!(result["promoted"], false);
    assert!(result["native_read_pages"].as_u64().unwrap() >= 2);
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"write","path":"candidate.rs","content":"late mutation"})
        )
        .is_err());
    let event: String = f
        .db()
        .query_row(
            "SELECT group_concat(detail) FROM candidate_events",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!event.contains("unicode candidate"));
}
#[test]
fn a_lost_write_reply_is_reconciled_by_read_without_a_second_write() {
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    author(&mut repo, "source marker");
    let args = repo.prepare_write().unwrap();
    invoke(&f, &args);
    drop(repo);
    let mut reopened = Repository::open(&f.authority, f.scope.clone()).unwrap();
    assert_eq!(reopened.state().unwrap().stage, Stage::WritePending);
    assert!(reopened.prepare_write().is_err());
    read(&f, &mut reopened);
    let result = reopened.submit(&f.authority).unwrap();
    assert_eq!(result["write_reconciled_by_read"], true);
    let count:i64=f.db().query_row("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.arguments_sha256')=?1",[digest(&serde_json::to_vec(&args).unwrap())],|r|r.get(0)).unwrap();
    assert_eq!(count, 1);
}
#[test]
fn uncertain_missing_writes_and_changed_candidates_are_not_silently_replayed() {
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    author(&mut repo, "source marker");
    repo.prepare_write().unwrap();
    drop(repo);
    let reopened = Repository::open(&f.authority, f.scope.clone()).unwrap();
    let read_args = reopened.read_arguments().unwrap();
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "files", read_args)
        .is_err());
    drop(reopened);
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    author(&mut repo, "source marker");
    write(&f, &mut repo);
    read(&f, &mut repo);
    invoke(
        &f,
        &json!({"op":"write","path":"candidate.rs","content":"changed after readback"}),
    );
    assert!(repo.submit(&f.authority).is_err());
    assert_eq!(repo.state().unwrap().stage, Stage::Ready);
}
#[test]
fn forged_receipts_or_changed_read_pages_cannot_complete_stages() {
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    author(&mut repo, "source marker");
    let args = repo.prepare_write().unwrap();
    let fake = json!({"ok":true,"result":{"sha256":digest(b"source marker")},"mcp_receipt":{"actor":"Worker","tool":"files"}});
    assert!(repo.wrote(&args, &fake).is_err());
    let result = invoke(&f, &args);
    let mut altered = result.clone();
    altered["result"]["sha256"] = json!("forged");
    assert!(repo.wrote(&args, &altered).is_err());
    repo.wrote(&args, &result).unwrap();
    let args = repo.read_arguments().unwrap();
    let result = invoke(&f, &args);
    let mut altered = result.clone();
    altered["result"]["text"] = json!("invented source");
    assert!(repo.read(&args, &altered).is_err());
    repo.read(&args, &result).unwrap();
}
#[test]
fn every_transition_rechecks_approval_epoch_and_checkpoint_integrity() {
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    f.db().execute("UPDATE jobs SET approval='{}'", []).unwrap();
    assert!(repo.begin_author().is_err());
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    assert!(repo.begin_author().is_err());
    assert!(Repository::open(&f.authority, f.scope.clone()).is_err());
    let f = configured();
    let repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    f.db()
        .execute(
            "UPDATE candidate_workflows SET state=replace(state,'author','submitted')",
            [],
        )
        .unwrap();
    assert!(repo.state().is_err());
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    f.db()
        .execute("UPDATE jobs SET state='cancelled'", [])
        .unwrap();
    assert!(repo.begin_author().is_err());
}
#[test]
fn attempts_are_bounded_across_reopen_and_source_cannot_choose_host_fields() {
    let f = configured();
    let mut repo = Repository::open(&f.authority, f.scope.clone()).unwrap();
    repo.begin_author().unwrap();
    assert!(repo
        .accept_source(json!({"source":"x","summary":"","actor":"Reader"}))
        .is_err());
    repo.author_failed("Invalid fixed reply shape").unwrap();
    drop(repo);
    let mut reopened = Repository::open(&f.authority, f.scope.clone()).unwrap();
    assert_eq!(reopened.begin_author().unwrap(), 2);
    reopened.author_failed("retry").unwrap();
    reopened.begin_author().unwrap();
    reopened.author_failed("retry").unwrap();
    assert!(reopened.begin_author().is_err());
}

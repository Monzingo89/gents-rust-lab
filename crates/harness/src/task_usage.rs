//! Numeric projection of retained task receipts. Owner-authenticated callers
//! supply the company connection; this grants no source or execution access.
use anyhow::{ensure, Result};
use rusqlite::{params_from_iter, Connection};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

const JOB_LIMIT: usize = 256;
const EVENT_LIMIT: usize = 4096;

fn installed(db: &Connection, table: &str) -> Result<bool> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |r| r.get(0),
    )?)
}
#[derive(Default)]
struct Calls {
    dispatch: BTreeMap<(String, Uuid), Value>,
    returned: BTreeMap<(String, Uuid), Value>,
    grounded: BTreeMap<String, Value>,
    malformed: bool,
}
fn insert<K: Ord>(map: &mut BTreeMap<K, Value>, key: K, value: Value) -> bool {
    if let Some(prior) = map.get(&key) {
        prior == &value
    } else {
        map.insert(key, value);
        true
    }
}
impl Calls {
    fn record(&mut self, job: String, kind: &str, body: &str) {
        let Some(value) = (body.len() <= 16384)
            .then(|| serde_json::from_str::<Value>(body).ok())
            .flatten()
            .filter(Value::is_object)
        else {
            self.malformed = true;
            return;
        };
        if kind == "grounded_call_finished" {
            self.malformed |= !insert(&mut self.grounded, job, value);
            return;
        }
        let Some(call) = value["call_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            self.malformed = true;
            return;
        };
        let map = if kind == "task_model_dispatch" {
            &mut self.dispatch
        } else {
            &mut self.returned
        };
        self.malformed |= !insert(map, (job, call), value);
    }
}

fn fee(value: &Value, native: bool, fixture: bool) -> Option<f64> {
    if value.is_null() {
        // Older owned-native/mock receipts establish no provider charge. An
        // arbitrary request or provider invocation count does not.
        return (native || fixture).then_some(0.0);
    }
    value.as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}
fn add(total: Option<u64>, value: Option<u64>) -> Option<u64> {
    total.zip(value).and_then(|(a, b)| a.checked_add(b))
}

/// No schema installation, source reads or persistence. Job IDs select a
/// bounded audit slice; subscriber/result rows are never summed as calls.
pub fn read(db: &Connection, tenant: &str, now: u64) -> Result<Value> {
    ensure!(
        !db.is_autocommit(),
        "task usage requires caller transaction"
    );
    let mut jobs = BTreeMap::new();
    let mut jobs_truncated = false;
    for (table, id, grounded) in [
        ("task_workflow_jobs_v1", "id", false),
        ("task_grounded_jobs_v1", "job_id", true),
    ] {
        if !installed(db, table)? {
            continue;
        }
        let mut stmt = db.prepare(&format!(
            "SELECT {id},state FROM {table} WHERE tenant=?1 ORDER BY rowid DESC LIMIT ?2"
        ))?;
        let rows = stmt
            .query_map(rusqlite::params![tenant, (JOB_LIMIT + 1) as i64], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        jobs_truncated |= rows.len() > JOB_LIMIT;
        for (id, state) in rows.into_iter().take(JOB_LIMIT) {
            jobs.insert(id, (grounded, state));
        }
    }
    let mut calls = Calls::default();
    let mut events_truncated = false;
    let audit_installed = installed(db, "audit")?;
    if !jobs.is_empty() && !audit_installed {
        calls.malformed = true;
    }
    if !jobs.is_empty() && audit_installed {
        let placeholders = (1..=jobs.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt=db.prepare(&format!("SELECT run_id,kind,body FROM audit WHERE run_id IN ({placeholders}) AND kind IN ('task_model_dispatch','task_model_return','grounded_call_finished') ORDER BY seq DESC LIMIT {}",EVENT_LIMIT+1))?;
        let mut rows = stmt.query(params_from_iter(jobs.keys()))?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            count += 1;
            if count > EVENT_LIMIT {
                events_truncated = true;
                break;
            }
            let (job, kind, body): (String, String, String) =
                (row.get(0)?, row.get(1)?, row.get(2)?);
            // A receipt cannot cross from the other executor's event format.
            if jobs[&job].0 != (kind == "grounded_call_finished") {
                calls.malformed = true;
                continue;
            }
            calls.record(job, &kind, &body);
        }
    }
    let orphan_returns = calls
        .returned
        .keys()
        .any(|key| !calls.dispatch.contains_key(key));
    let unresolved = calls
        .dispatch
        .keys()
        .filter(|key| !calls.returned.contains_key(*key))
        .count();
    let grounded_missing = jobs
        .iter()
        .filter(|(id, (grounded, _))| *grounded && !calls.grounded.contains_key(*id))
        .count();
    let coverage_complete =
        !jobs_truncated && !events_truncated && !calls.malformed && !orphan_returns;
    let mut input = Some(0);
    let mut output = Some(0);
    let mut cost = Some(0.0);
    let mut actual = Some(0);
    let mut native_completed = 0;
    let mut fixture_completed = 0;
    let mut grounded_failures = 0;
    let receipts = calls
        .returned
        .values()
        .map(|v| (v, false))
        .chain(calls.grounded.values().map(|v| (&v["call_evidence"], true)));
    for (v, grounded) in receipts {
        let native =
            v["local_execution"].is_object() && (grounded || v["native_dispatch_receipt"] == true);
        let fixture = v["fixture"] == true;
        if native && fixture {
            calls.malformed = true;
        }
        if native && !fixture {
            native_completed += 1;
        }
        if fixture && !native {
            fixture_completed += 1;
        }
        let known_call = if native && !fixture {
            Some(1)
        } else if fixture && !native {
            Some(0)
        } else {
            None
        };
        actual = add(actual, known_call);
        input = add(
            input,
            if grounded {
                v["usage"]["input"].as_u64()
            } else {
                v["input_tokens"].as_u64()
            },
        );
        output = add(
            output,
            if grounded {
                v["usage"]["output"].as_u64()
            } else {
                v["output_tokens"].as_u64()
            },
        );
        let charge = fee(
            if grounded {
                &v["usage"]["cost_usd"]
            } else {
                &v["provider_cost_usd"]
            },
            native,
            fixture,
        );
        cost = cost
            .zip(charge)
            .and_then(|(a, b)| ((a + b).is_finite()).then_some(a + b));
        if grounded && !v["usage"].is_object() {
            grounded_failures += 1;
        }
    }
    let coverage_complete = coverage_complete && !calls.malformed;
    if !coverage_complete || unresolved > 0 || grounded_missing > 0 || grounded_failures > 0 {
        input = None;
        output = None;
        cost = None;
        actual = None;
    }
    Ok(
        json!({"schema_version":1,"company":tenant,"sampled_at_ms":now,
        "scope":"Retained company task-engine job/call receipts only; includes owner and native actor jobs. Subscriber copies, legacy run journals and local research are excluded. Not lifetime billing or a spending limit.",
        "coverage_complete":coverage_complete,"jobs_truncated":jobs_truncated,"events_truncated":events_truncated,"malformed_records":calls.malformed,"orphan_returns":orphan_returns,
        "sampled_jobs":jobs.len(),"running_jobs":jobs.values().filter(|(_,state)|state!="completed").count(),
        "workflow_dispatch_records":coverage_complete.then_some(calls.dispatch.len()),"workflow_return_records":coverage_complete.then_some(calls.returned.len()),
        "unresolved_workflow_requests":coverage_complete.then_some(unresolved),"grounded_jobs_without_receipt":coverage_complete.then_some(grounded_missing),
        "observed_native_replies":coverage_complete.then_some(native_completed),"observed_fixture_replies":coverage_complete.then_some(fixture_completed),
        "generative_model_calls":actual,"input_tokens":input,"output_tokens":output,"provider_cost_usd":cost}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("BEGIN; CREATE TABLE task_workflow_jobs_v1(tenant TEXT,id TEXT,state TEXT); CREATE TABLE task_grounded_jobs_v1(tenant TEXT,job_id TEXT,state TEXT); CREATE TABLE audit(seq INTEGER PRIMARY KEY,run_id TEXT,kind TEXT,body TEXT);").unwrap();
        db
    }
    fn event(db: &Connection, job: &str, kind: &str, body: Value) {
        db.execute(
            "INSERT INTO audit(run_id,kind,body) VALUES(?1,?2,?3)",
            rusqlite::params![job, kind, body.to_string()],
        )
        .unwrap();
    }
    #[test]
    fn exact_receipts_deduplicate_across_jobs_and_exclude_other_companies_and_payloads() {
        let db = db();
        let call = Uuid::new_v4();
        for (job, tenant) in [("a", "one"), ("b", "one"), ("foreign", "two")] {
            db.execute(
                "INSERT INTO task_workflow_jobs_v1 VALUES(?1,?2,'completed')",
                [tenant, job],
            )
            .unwrap();
            event(
                &db,
                job,
                "task_model_dispatch",
                json!({"call_id":call,"private":"SECRET"}),
            );
            let receipt = json!({"call_id":call,"native_dispatch_receipt":true,"local_execution":{},"fixture":false,"input_tokens":10,"output_tokens":2,"provider_cost_usd":0});
            event(&db, job, "task_model_return", receipt.clone());
            event(&db, job, "task_model_return", receipt);
        }
        db.execute(
            "INSERT INTO task_grounded_jobs_v1 VALUES('one','ground','completed')",
            [],
        )
        .unwrap();
        event(
            &db,
            "ground",
            "grounded_call_finished",
            json!({"call_evidence":{"usage":{"input":3,"output":1,"cost_usd":0},"local_execution":{}}}),
        );
        // Subscriber copies are deliberately irrelevant to usage accounting.
        db.execute_batch("CREATE TABLE task_workflow_subscribers_v1(body TEXT); INSERT INTO task_workflow_subscribers_v1 VALUES('ignored'),('ignored');").unwrap();
        let usage = read(&db, "one", 1).unwrap();
        assert_eq!(usage["generative_model_calls"], 3);
        assert_eq!(usage["input_tokens"], 23);
        assert_eq!(usage["output_tokens"], 5);
        assert_eq!(usage["provider_cost_usd"], 0.0);
        assert_eq!(usage["workflow_dispatch_records"], 2);
        assert!(usage["coverage_complete"].as_bool().unwrap());
        assert!(!usage.to_string().contains("SECRET"));
        assert_eq!(read(&db, "empty", 1).unwrap()["generative_model_calls"], 0);
        event(
            &db,
            "a",
            "task_model_return",
            json!({"call_id":call,"input_tokens":999}),
        );
        let conflicted = read(&db, "one", 2).unwrap();
        assert_eq!(conflicted["malformed_records"], true);
        assert!(conflicted["provider_cost_usd"].is_null());
    }
    #[test]
    fn unresolved_missing_malformed_and_truncated_records_preserve_unknowns() {
        let db = db();
        db.execute(
            "INSERT INTO task_workflow_jobs_v1 VALUES('one','a','running')",
            [],
        )
        .unwrap();
        event(
            &db,
            "a",
            "task_model_dispatch",
            json!({"call_id":Uuid::new_v4()}),
        );
        let pending = read(&db, "one", 1).unwrap();
        assert_eq!(pending["unresolved_workflow_requests"], 1);
        for field in [
            "generative_model_calls",
            "input_tokens",
            "output_tokens",
            "provider_cost_usd",
        ] {
            assert!(pending[field].is_null());
        }
        db.execute("DELETE FROM audit", []).unwrap();
        event(
            &db,
            "a",
            "task_model_return",
            json!({"call_id":Uuid::new_v4(),"fixture":true}),
        );
        assert_eq!(read(&db, "one", 1).unwrap()["orphan_returns"], true);
        db.execute("DELETE FROM audit", []).unwrap();
        let call = Uuid::new_v4();
        event(&db, "a", "task_model_dispatch", json!({"call_id":call}));
        event(
            &db,
            "a",
            "task_model_return",
            json!({"call_id":call,"fixture":true,"input_tokens":null,"output_tokens":null}),
        );
        let fixture = read(&db, "one", 1).unwrap();
        assert_eq!(fixture["generative_model_calls"], 0);
        assert!(fixture["input_tokens"].is_null());
        for _ in 0..EVENT_LIMIT {
            event(&db, "a", "task_model_dispatch", json!({"call_id":call}));
        }
        let limited = read(&db, "one", 1).unwrap();
        assert_eq!(limited["events_truncated"], true);
        assert!(limited["generative_model_calls"].is_null());
        db.execute("DELETE FROM audit", []).unwrap();
        for i in 0..JOB_LIMIT {
            db.execute(
                "INSERT INTO task_workflow_jobs_v1 VALUES('one',?1,'completed')",
                [i.to_string()],
            )
            .unwrap();
        }
        let limited = read(&db, "one", 1).unwrap();
        assert_eq!(limited["jobs_truncated"], true);
        assert!(limited["provider_cost_usd"].is_null());
    }
    #[test]
    fn owner_usage_neither_refreshes_sources_nor_allows_another_owner() {
        let root = std::env::temp_dir().join(format!("gents-usage-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        crate::company::operation(
            &root,
            "one",
            "owner",
            &json!({"op":"register","name":"Usage fixture"}),
        )
        .unwrap();
        let before = crate::company::connect(&root, "one", "owner", false).unwrap();
        let tables: i64 = before
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        before.execute("INSERT INTO context VALUES('cold','onboarding','Writer','ALTC','PRIVATE_UNINDEXED','fixture',1)",[]).unwrap();
        before.execute("DELETE FROM knowledge", []).unwrap();
        drop(before);
        assert!(crate::task_frontdoor::operation(
            &root,
            "one",
            "other",
            "frozen",
            crate::task_frontdoor::Operation::Usage
        )
        .is_err());
        let usage = crate::task_frontdoor::operation(
            &root,
            "one",
            "owner",
            "frozen",
            crate::task_frontdoor::Operation::Usage,
        )
        .unwrap();
        assert_eq!(usage["provider_cost_usd"], 0.0);
        let after = crate::company::connect_readonly(&root, "one", "owner").unwrap();
        let new_tables: i64 = after
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, new_tables);
        let unchanged:(i64,i64,i64)=after.query_row("SELECT (SELECT count(*) FROM context WHERE id='cold'),(SELECT count(*) FROM context_archive WHERE id='cold'),(SELECT count(*) FROM knowledge)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(
            unchanged,
            (1, 0, 0),
            "usage must not migrate/archive/index context, even on denied owner reads"
        );
        assert!(after
            .execute("UPDATE company SET name='unexpected mutation'", [])
            .is_err());
        drop(after);
        std::fs::remove_dir_all(root).unwrap();
    }
}

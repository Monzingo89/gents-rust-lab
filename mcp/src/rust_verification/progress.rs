//! Historical execution stages are observations, never completion or authority.
use super::Spec;
use crate::Scope;
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn rank(value: &Value) -> Option<u8> {
    match value["stage"].as_str()? {
        "admission" => Some(0),
        "compile_candidate" => Some(1),
        "compile_acceptance" => Some(2),
        "acceptance" if value["passed"] == true => Some(4),
        "acceptance" => Some(3),
        _ => None,
    }
}

fn summarize(rows: &[Value], current: Option<&str>) -> Value {
    let mut seen = BTreeSet::new();
    let mut best = None;
    let mut latest = None;
    let mut without_improvement = 0;
    for row in rows {
        let Some(stage) = rank(row) else { continue };
        let Some(hash) = row["source_sha256"].as_str() else {
            continue;
        };
        latest = Some((stage, hash));
        let new_source = seen.insert(hash);
        // A repaired verifier can observe a later stage for identical bytes.
        // Identical retries still do not count as additional revisions.
        if best.is_none_or(|prior| stage > prior) {
            best = Some(stage);
            without_improvement = 0;
        } else if new_source {
            without_improvement += 1;
        }
    }
    let current_stage = latest
        .filter(|(_, hash)| Some(*hash) == current)
        .map(|(stage, _)| stage);
    json!({"window":"last 32 canonical checks; stages 0=admission,1=candidate compile,2=suite compile,3=tests ran,4=acceptance passed; not a correctness score",
        "distinct_sources":seen.len(),"best_stage":best,"current_stage":current_stage,
        "regressed":current_stage.zip(best).is_some_and(|(now, best)| now < best),
        "revisions_without_stage_improvement":without_improvement})
}

pub fn view(
    db: &Connection,
    scope: &Scope,
    spec: &Spec,
    revision: u64,
    current: Option<&str>,
) -> Result<Value> {
    let mut statement = db.prepare("SELECT result FROM rust_check_results r WHERE run_id=?1 AND policy_revision=?2 AND actor=?3 AND source_path=?4 AND tests_sha256=?5 ORDER BY rowid DESC LIMIT 32")?;
    let results = statement
        .query_map(
            params![
                scope.run.to_string(),
                revision as i64,
                spec.actor,
                spec.source_path,
                spec.tests_sha256
            ],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut verified = Vec::new();
    for encoded in results.into_iter().rev() {
        let receipts: i64 = db.query_row("SELECT COUNT(*) FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.tool')='rust_verify' AND json_extract(body,'$.policy_revision')=?3 AND json_extract(body,'$.result_sha256')=?4",params![scope.run.to_string(),spec.actor,revision as i64,crate::digest(encoded.as_bytes())],|row|row.get(0))?;
        if receipts > 0 {
            verified.push(serde_json::from_str(&encoded)?);
        }
    }
    Ok(summarize(&verified, current))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_bytes_and_retries_do_not_hide_stage_regressions() {
        let rows = vec![
            json!({"stage":"acceptance","passed":false,"source_sha256":"best"}),
            json!({"stage":"admission","passed":false,"source_sha256":"new"}),
            json!({"stage":"admission","passed":false,"source_sha256":"new"}),
        ];
        let view = summarize(&rows, Some("new"));
        assert_eq!(view["regressed"], true);
        assert_eq!(view["distinct_sources"], 2);
        assert_eq!(view["revisions_without_stage_improvement"], 1);
        assert_eq!(view["best_stage"], 3);
        assert_eq!(
            summarize(&rows, Some("unverified"))["current_stage"],
            Value::Null
        );
        assert!(!view.to_string().contains("diagnostics"));
        let repaired = vec![
            json!({"stage":"admission","source_sha256":"same"}),
            json!({"stage":"acceptance","passed":false,"source_sha256":"same"}),
        ];
        assert_eq!(summarize(&repaired, Some("same"))["best_stage"], 3);
    }
}

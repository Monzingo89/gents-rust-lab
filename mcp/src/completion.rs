//! Frozen, owner-selected evidence requirements. Model prose is not a receipt.
use crate::{authority::Authority, governance, Registry, Scope};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequiredCall {
    actor: String,
    tool: String,
}
fn parse(intake: &Value) -> Result<Vec<RequiredCall>> {
    let calls: Vec<RequiredCall> = serde_json::from_value(
        intake
            .get("required_mcp_calls")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    if calls.len() > 16 {
        bail!("at most sixteen native call requirements");
    }
    let registry = Registry::new();
    let mut unique = std::collections::BTreeSet::new();
    for call in &calls {
        if call.actor.trim().is_empty()
            || call.actor.len() > 200
            || call.actor.chars().any(char::is_control)
            || !registry.contains(&call.tool)
            || !unique.insert((&call.actor, &call.tool))
        {
            bail!("invalid or duplicate native call requirement");
        }
    }
    Ok(calls)
}
pub fn validate(intake: &Value) -> Result<()> {
    parse(intake)?;
    crate::rust_verification::validate(intake)?;
    if crate::owned_components::specs(intake)?
        .iter()
        .any(|s| s.input_recipe.is_some())
    {
        crate::artifact_bindings::parse(intake)?;
    }
    Ok(())
}
pub fn check(authority: &dyn Authority, scope: &Scope) -> Result<Value> {
    let db = authority.open(scope)?;
    let tx = db.unchecked_transaction()?;
    let encoded: String = tx.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let calls = parse(&intake)?;
    let policy = governance::load(&tx, &scope.run.to_string())?
        .context("native evidence requires installed policy")?;
    anyhow::ensure!(
        scope
            .expected_revision
            .is_none_or(|revision| revision == policy.revision),
        "native evidence source epoch changed"
    );
    let mut checks = Vec::new();
    for call in calls {
        let count:i64=tx.query_row("SELECT COUNT(*) FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.tool')=?3 AND json_extract(body,'$.policy_revision')=?4",rusqlite::params![scope.run.to_string(),call.actor,call.tool,policy.revision as i64],|r|r.get(0))?;
        // Revocation cannot leave a previously accepted old-epoch receipt usable.
        checks.push(json!({"actor":call.actor,"tool":call.tool,"successful_current_epoch_receipts":count,"passed":count>0}));
    }
    let rust_checks =
        crate::rust_verification::completion(&tx, authority, scope, &intake, policy.revision)?;
    let owned_rust_calls =
        crate::owned_components::completion(&tx, authority, scope, &intake, policy.revision)?;
    let passed = checks.iter().all(|c| c["passed"] == true)
        && rust_checks.iter().all(|c| c["passed"] == true)
        && owned_rust_calls.iter().all(|c| c["passed"] == true);
    let observed = {
        let mut query=tx.prepare("SELECT json_extract(body,'$.actor'),json_extract(body,'$.tool'),COUNT(*) FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.policy_revision')=?2 GROUP BY 1,2 ORDER BY 1,2 LIMIT 64")?;
        let rows=query.query_map(rusqlite::params![scope.run.to_string(),policy.revision as i64],|r|Ok(json!({"actor":r.get::<_,String>(0)?,"tool":r.get::<_,String>(1)?,"successful_receipts":r.get::<_,i64>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    let receipts = {
        let mut query=tx.prepare("SELECT body FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.policy_revision')=?2 ORDER BY seq DESC LIMIT 16")?;
        let rows = query
            .query_map(
                rusqlite::params![scope.run.to_string(), policy.revision as i64],
                |r| r.get::<_, String>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|body| serde_json::from_str::<Value>(&body))
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    tx.commit()?;
    Ok(
        json!({"passed":passed,"checks":checks,"rust_checks":rust_checks,"owned_rust_calls":owned_rust_calls,"observed_calls":observed,"mcp_receipts":receipts,"policy_revision":policy.revision,"source":"canonical native audit, not model claims"}),
    )
}

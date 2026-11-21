//! Two short authorization transactions around scratch-only SDK work. No
//! writer lock spans compilation; status and native worker leases stay live.
use crate::{
    authority::Authority,
    contracts::{EmbeddedContracts, ToolContracts},
    governance, Registry, Scope, ToolContext,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, TransactionBehavior};
use serde_json::{json, Value};
impl Registry {
    pub(crate) fn call_angular(
        &self,
        authority: &dyn Authority,
        scope: &Scope,
        name: &str,
        args: Value,
    ) -> Result<Value> {
        let mut db = authority.open(scope)?;
        let sha = crate::digest(&serde_json::to_vec(&args)?);
        let mut dispatched = false;
        let outcome = (|| -> Result<Value> {
            let (prepared, revision, approval) = {
                let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                let policy = governance::load(&tx, &scope.run.to_string())?
                    .context("Angular requires current policy")?;
                self.angular_authorize(authority, scope, &policy, &tx, name, &args)?;
                let context = ToolContext {
                    authority,
                    scope,
                    policy: &policy,
                    db: &tx,
                };
                let prepared = crate::angular_check::prepare_call(&context, &args)?;
                tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'mcp_tool_dispatch',?2,?3)",params![scope.run.to_string(),json!({"actor":scope.actor,"phase":scope.phase,"tool":name,"arguments_sha256":sha,"policy_revision":policy.revision,"status":"dispatch_reserved","native_effect":"isolated offline Angular build and frozen host DOM checks","generation_established":false}).to_string(),crate::now_ms() as i64])?;
                let approval: Option<String> = tx.query_row(
                    "SELECT approval FROM jobs WHERE run_id=?1",
                    [scope.run.to_string()],
                    |r| r.get(0),
                )?;
                tx.commit()?;
                (prepared, policy.revision, approval)
            };
            dispatched = true;
            let result = crate::angular_check::compute(&prepared)?;
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let policy = governance::load(&tx, &scope.run.to_string())?
                .context("Angular policy removed during execution")?;
            ensure!(
                policy.revision == revision,
                "Angular policy changed during execution; no result accepted"
            );
            self.angular_authorize(authority, scope, &policy, &tx, name, &args)?;
            let current_approval: Option<String> = tx.query_row(
                "SELECT approval FROM jobs WHERE run_id=?1",
                [scope.run.to_string()],
                |r| r.get(0),
            )?;
            ensure!(
                current_approval == approval,
                "Angular parent approval changed during execution"
            );
            let enforced = crate::contract_execution::enabled(&policy, scope);
            let result = crate::angular_check::finish(
                &ToolContext {
                    authority,
                    scope,
                    policy: &policy,
                    db: &tx,
                },
                &args,
                &prepared,
                result,
            )?;
            if let Some(schema) = self.tools[name].result_schema(Some(&args)) {
                crate::contracts::validate_value(&schema, &result, "native_result")?;
            }
            crate::artifact_bindings::capture(&tx, scope, name, &args, &result, revision)?;
            let mut receipt = json!({"actor":scope.actor,"phase":scope.phase,"tool":name,"arguments_sha256":sha,"result_sha256":crate::digest(&serde_json::to_vec(&result)?),"policy_revision":revision,"provider_cost_usd":0,"status":"completed"});
            if enforced {
                receipt["contract_enforced"] = json!(true);
            }
            let envelope = json!({"value":result,"receipt":receipt});
            let frame = json!({"jsonrpc":"2.0","id":"\u{0000}".repeat(200),"result":{"content":[{"type":"text","text":envelope.to_string()}],"structuredContent":envelope,"isError":false}});
            ensure!(
                serde_json::to_vec(&frame)?.len() <= 64000,
                "Native SDK response exceeds the bounded MCP frame; no canonical result committed"
            );
            tx.execute(
                "INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'mcp_tool_call',?2,?3)",
                params![
                    scope.run.to_string(),
                    receipt.to_string(),
                    crate::now_ms() as i64
                ],
            )?;
            tx.commit()?;
            Ok(json!({"value":result,"receipt":receipt}))
        })();
        if outcome.is_err() {
            // No failed/lost dispatch is replayed here. A receipt failure cannot
            // leave an authoritative build observation: both share one tx.
            let _=db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,?2,?3,?4)",params![scope.run.to_string(),if dispatched {"mcp_tool_effect_uncertain"}else{"mcp_tool_rejected"},json!({"actor":scope.actor,"phase":scope.phase,"tool":name,"arguments_sha256":sha,"status":if dispatched{"effect_uncertain"}else{"rejected"},"payload_logged":false,"reason":"detached_angular_check_not_committed"}).to_string(),crate::now_ms() as i64]);
        }
        outcome
    }
    fn angular_authorize(
        &self,
        authority: &dyn Authority,
        scope: &Scope,
        policy: &governance::Policy,
        db: &rusqlite::Connection,
        name: &str,
        args: &Value,
    ) -> Result<()> {
        EmbeddedContracts.validate(name, args)?;
        ensure!(
            scope.expected_revision.is_none_or(|r| r == policy.revision),
            "Angular source policy epoch changed"
        );
        let tool = self
            .tools
            .get(name)
            .context("Angular native tool not registered")?;
        ensure!(
            tool.phases().contains(&scope.phase),
            "Angular tool phase unavailable"
        );
        policy.authorize(&scope.actor, name, args)?;
        crate::contract_execution::check(self, authority, db, scope, policy, name, args)?;
        let state: String = db.query_row(
            "SELECT state FROM jobs WHERE run_id=?1",
            [scope.run.to_string()],
            |r| r.get(0),
        )?;
        ensure!(
            matches!(state.as_str(), "approved" | "running"),
            "Angular requires an approved active job"
        );
        Ok(())
    }
}

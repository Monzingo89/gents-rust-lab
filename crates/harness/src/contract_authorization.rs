//! Optional owner-selected authorization for closed native workflows.
//! Native execution repeats exact-call checks. This never accepts a task.
use anyhow::{Context, Result};
use gents_local_mcp::{authority::Authority, contract_execution, governance, Scope};
use serde_json::{json, Value};

pub struct Review<'a> {
    pub scope: &'a Scope,
    pub request: &'a Value,
    pub gate: &'a Value,
    pub stage: usize,
}
pub fn review(authority: &dyn Authority, input: Review<'_>) -> Result<Option<Value>> {
    let Some(context) = contract_execution::scope_authority(authority, input.scope)? else {
        return Ok(None);
    };
    let action = input.request["action"]
        .as_str()
        .context("authorization action missing")?;
    let arguments = &input.request["arguments"];
    let checked = (|| -> Result<Value> {
        if action == "finish" {
            let db = authority.open(input.scope)?;
            let policy = governance::load(&db, &input.scope.run.to_string())?
                .context("authorization policy missing")?;
            policy.authorize(&input.scope.actor, "finish", arguments)?;
            let fingerprint = crate::native_submission::fingerprint(
                context["contract_sha256"].as_str().unwrap_or(""),
                &input.scope.actor,
                input.stage,
                input.scope.expected_revision,
                input.gate,
            )
            .context("current native and artifact gates do not permit submission")?;
            Ok(
                json!({"gate_sha256":fingerprint,"control":"submit_to_independent_qa","execution_established":false}),
            )
        } else {
            contract_execution::authorize_call(authority, input.scope, action, arguments)?
                .context("owner mode changed during authorization")
        }
    })();
    let (verdict, check) = match checked {
        Ok(proof) => (
            json!({"decision":"allow","reason":"The owner selected exact approved-workflow execution for this actor and phase. Native guards recheck the approved call and current policy at execution; independent QA is still required.","question":""}),
            proof,
        ),
        Err(error) => {
            let detail = error.to_string().chars().take(300).collect::<String>();
            let detail = if crate::security::require_persistable(&detail).is_ok() {
                detail
            } else {
                "Native authorization rejected this request; payload omitted".into()
            };
            (
                json!({"decision":"deny","reason":detail,"question":""}),
                json!({"rejected":true,"execution_established":false}),
            )
        }
    };
    Ok(Some(
        json!({"verdict":verdict,"authority":context,"check":check,"arguments_sha256":gents_local_mcp::digest(arguments.to_string().as_bytes()),"model_called":false}),
    ))
}

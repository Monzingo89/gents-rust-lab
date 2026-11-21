//! Registered host checks. Model output cannot create a validator, a native
//! receipt, an owner contract, or an acceptance authority.
use super::{
    contracts::*,
    resources::Budget,
    workflows::{FutureResult, NodeReceipt},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const NAMES: &[&str] = &[
    "input_custody",
    "output_shape",
    "current_citations",
    "angular_build",
    "cosmos_receipts",
    "dotnet_build",
    "dotnet_api",
    "integration_receipts",
    "independent_acceptance",
];
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub validator: String,
    pub contract_id: String,
    pub source_identity: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeReceipt {
    pub schema_version: u32,
    pub validator: String,
    pub tool: String,
    pub contract_id: String,
    pub source_identity: String,
    pub policy_revision: u64,
    pub arguments_sha256: String,
    pub result_sha256: String,
    pub input_snapshot_sha256: String,
    pub artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_contract_sha256: Option<String>,
    pub outcome: CheckStatus,
    pub elapsed_micros: u64,
    pub sampled_process_group_rss_bytes: Option<u64>,
}
#[derive(Default, Clone, Debug, Serialize)]
pub struct Report {
    pub checks: Vec<AcceptanceCheck>,
    pub native_receipts: Vec<NativeReceipt>,
}
pub trait Host: Send + Sync {
    /// Called only for the registered runtime check, after deterministic
    /// output checks. Implementations charge every actual native dispatch to
    /// this same budget and bind it to owner-frozen inputs before execution.
    fn check<'a>(&'a self, name: &'a str, budget: &'a Budget) -> FutureResult<'a, Report>;
}
pub struct Registry;
/// Current canonical receipts only. Angular hashes the configured SDK once
/// for this request, then independently rechecks each source/grant/audit.
pub fn receipts_current(
    context: &crate::ToolContext<'_>,
    receipts: &[NativeReceipt],
) -> Result<bool> {
    let angular = receipts
        .iter()
        .filter(|r| r.tool == "angular_check")
        .collect::<Vec<_>>();
    if !crate::angular_check::references_current(context, &angular)? {
        return Ok(false);
    }
    for receipt in receipts.iter().filter(|r| r.tool != "angular_check") {
        if !matches!(receipt.tool.as_str(), "dotnet_check" | "dotnet_api_check")
            || !crate::dotnet_check::reference_current(context, receipt)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
impl Registry {
    pub fn validate_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Result<()> {
        for name in names {
            ensure!(
                NAMES.contains(&name.as_ref()),
                "unknown validator: {}",
                name.as_ref()
            );
        }
        Ok(())
    }
    pub fn profile_checks(names: &BTreeSet<String>) -> Result<Vec<String>> {
        Self::validate_names(names)?;
        Ok(names
            .iter()
            .cloned()
            .chain(["input_custody".into(), "output_shape".into()])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }
    pub async fn evaluate(
        request: &TaskRequest,
        plan: &ExecutionPlan,
        nodes: &[NodeReceipt],
        host: Option<&dyn Host>,
        policy: &ExecutionPolicy,
        budget: &Budget,
    ) -> Result<Report> {
        let names = plan
            .nodes
            .iter()
            .flat_map(|n| n.validators.iter().cloned())
            .collect::<BTreeSet<_>>();
        Self::validate_names(&names)?;
        let mut report = Report::default();
        for name in names {
            budget.check()?;
            let deterministic=match name.as_str() {
                "input_custody"=>Some((plan.task_id==request.task_id && plan.original_objective_sha256==crate::digest(request.original_ask.as_bytes()),"Host retained task ID and exact original input digest. This does not establish semantic completeness of an answer.")),
                "output_shape"=>Some((nodes.len()==plan.nodes.len() && nodes.iter().all(|r|r.output.status=="answer" && r.output.tool.is_none() && !r.output.answer.is_empty() && (request.input.output.format!="json" || serde_json::from_str::<serde_json::Value>(&r.output.answer).is_ok())),"All scheduled nodes returned bounded schema-checked answer packets; requested JSON parses. This does not prove factual or functional correctness.")),
                "current_citations"=>Some((!nodes.is_empty() && nodes.iter().all(|r|!r.output.citations.is_empty() && r.output.citations.iter().all(|c|r.evidence.get(c.passage).is_some_and(|e| !c.quote.is_empty() && e.quote.contains(&c.quote) && e.valid_until_ms>crate::now_ms()))),"Citation quotes occur in the actual node evidence and its TTL has not expired. Current access/source bytes are checked by the host at delivery and status. Quote membership is not entailment, coverage or historical expertise.")),
                _=>None,
            };
            if let Some((passed, detail)) = deterministic {
                report.checks.push(AcceptanceCheck {
                    name: format!("{name}/v1"),
                    status: if passed {
                        CheckStatus::Passed
                    } else {
                        CheckStatus::Failed
                    },
                    detail: detail.into(),
                });
                continue;
            }
            if matches!(
                name.as_str(),
                "dotnet_build" | "dotnet_api" | "angular_build"
            ) && policy.permitted_validators.contains(&name)
            {
                if let Some(host) = host {
                    let future = host.check(&name, budget);
                    tokio::pin!(future);
                    let result = loop {
                        tokio::select! {value=&mut future=>break value,_=tokio::time::sleep(std::time::Duration::from_millis(20))=>budget.check()?,}
                    };
                    match result {
                        Ok(mut value)=>{ensure!(!value.checks.is_empty() && value.checks.iter().all(|c|c.name==format!("{name}/v1")),"runtime validator returned an unrelated check");ensure!(value.native_receipts.len()<=4 && value.native_receipts.iter().all(|r|r.validator==name && match name.as_str(){"angular_build"=>r.tool=="angular_check","dotnet_api"=>r.tool=="dotnet_api_check","dotnet_build"=>matches!(r.tool.as_str(),"dotnet_check"|"dotnet_api_check"),_=>false} && r.schema_version==VERSION) && value.checks.iter().filter(|c|c.status==CheckStatus::Passed).count()<=value.native_receipts.iter().filter(|r|r.outcome==CheckStatus::Passed).count(),"runtime validator cannot pass without a corresponding native receipt");report.checks.append(&mut value.checks);report.native_receipts.append(&mut value.native_receipts);},
                        Err(error)=>report.checks.push(AcceptanceCheck{name:format!("{name}/v1"),status:CheckStatus::Failed,detail:format!("Native check did not establish the requested validator result: {error}; no automatic replay or independent acceptance.")}),
                    }
                    continue;
                }
            }
            report.checks.push(AcceptanceCheck{name:format!("{name}/v1"),status:if matches!(name.as_str(),"angular_build"|"dotnet_build"|"dotnet_api"|"independent_acceptance") {CheckStatus::NotRun}else{CheckStatus::Unsupported},detail:"No configured, authorized native validator executed this check. Profile names, generated text and citation presence cannot establish it.".into()});
        }
        Ok(report)
    }
}

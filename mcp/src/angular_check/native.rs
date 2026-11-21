//! Actor/run/policy-bound adapter around the copied-source SDK executor.
//! Source capture is cheap and transactional; full SDK verification and all
//! candidate execution happen after the dispatch transaction is committed.
use super::{
    contracts,
    runtime::{self, HostSdk, Snapshot},
    Contract,
};
use crate::{
    authority::Authority,
    governance,
    task_engine::{
        contracts::{CheckStatus, VERSION},
        validators::{Binding, NativeReceipt},
    },
    workspace, Scope, ToolContext,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub fn has_contract(intake: &Value, actor: &str) -> Result<bool> {
    Ok(contracts(intake)?.iter().any(|c| c.actor == actor))
}
/// Holds only a short-lived SDK metadata observation. It grants no access and
/// delegates all authority to the original owner adapter. Construct BEFORE a
/// writer transaction; result/source/policy/audit checks still run inside it.
pub struct ReceiptAuthority<'a> {
    inner: &'a dyn Authority,
    proof: Option<(HostSdk, String, std::time::Instant)>,
}
impl<'a> ReceiptAuthority<'a> {
    pub fn for_scope(inner: &'a dyn Authority, scope: &Scope) -> Self {
        let proof = (|| -> Result<_> {
            let db = inner.open(scope)?;
            let policy =
                governance::load(&db, &scope.run.to_string())?.context("Current policy missing")?;
            let encoded: String = db.query_row(
                "SELECT intake FROM jobs WHERE run_id=?1",
                [scope.run.to_string()],
                |r| r.get(0),
            )?;
            let Some(spec) = contracts(&serde_json::from_str(&encoded)?)?
                .into_iter()
                .find(|c| c.actor == scope.actor)
            else {
                return Ok(None);
            };
            let prepared = capture(
                &ToolContext {
                    authority: inner,
                    scope,
                    policy: &policy,
                    db: &db,
                },
                &spec.id,
            )?;
            let seal = prepared.sdk.seal()?;
            Ok(Some((prepared.sdk, seal, std::time::Instant::now())))
        })()
        .ok()
        .flatten();
        Self { inner, proof }
    }
}
impl Authority for ReceiptAuthority<'_> {
    fn open(&self, scope: &Scope) -> Result<rusqlite::Connection> {
        self.inner.open(scope)
    }
    fn workspace(&self, scope: &Scope) -> Result<std::path::PathBuf> {
        self.inner.workspace(scope)
    }
    fn discover_models(&self) -> Result<Value> {
        self.inner.discover_models()
    }
    fn angular_sdk_seal(&self, sdk: &HostSdk) -> Result<Option<String>> {
        let (pinned, seal, at) = self.proof.as_ref().context(
            "No authorized current Angular SDK observation; no in-transaction scan or replay",
        )?;
        ensure!(
            pinned == sdk && at.elapsed() < std::time::Duration::from_secs(5),
            "Angular SDK observation expired or changed; no in-transaction scan or replay"
        );
        Ok(Some(seal.clone()))
    }
}
pub(crate) struct Prepared {
    snapshot: Snapshot,
    spec: Contract,
    sdk: HostSdk,
    hashes: BTreeMap<String, String>,
    identity: String,
}
fn capture(context: &ToolContext<'_>, id: &str) -> Result<Prepared> {
    ensure!(
        context.scope.phase == crate::Phase::Worker,
        "Angular check is worker-scoped"
    );
    ensure!(
        context
            .scope
            .expected_revision
            .is_none_or(|r| r == context.policy.revision),
        "Angular policy epoch changed"
    );
    context.policy.authorize(
        &context.scope.actor,
        "angular_check",
        &json!({"contract_id":id}),
    )?;
    let (encoded, approval): (String, Option<String>) = context.db.query_row(
        "SELECT intake,approval FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        approval.is_some(),
        "Angular requires frozen parent approval"
    );
    let spec = contracts(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .find(|c| c.id == id && c.actor == context.scope.actor)
        .context("Exact owner-frozen Angular contract/actor required")?;
    let sdk = HostSdk::configured()?;
    let workspace_root = context.authority.workspace(context.scope)?.canonicalize()?;
    for trusted in [
        &sdk.node,
        &sdk.angular,
        &sdk.driver,
        sdk.browser.parent().unwrap(),
    ] {
        ensure!(
            !trusted.starts_with(&workspace_root) && !workspace_root.starts_with(trusted),
            "Trusted Angular SDK paths must be outside the actor workspace"
        );
    }
    let work = workspace::open(&workspace_root, false)?;
    let mut files = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut total = 0;
    for path in std::iter::once(&spec.project_path).chain(&spec.source_paths) {
        context.policy.authorize(
            &context.scope.actor,
            "files",
            &json!({"op":"read","path":path}),
        )?;
        let bytes = work.read_bytes_complete(path, 2 * 1024 * 1024)?;
        total += bytes.len();
        ensure!(
            total <= 2 * 1024 * 1024,
            "Angular source snapshot exceeds 2 MiB"
        );
        let text = std::str::from_utf8(&bytes).context("Angular sources must be UTF-8")?;
        crate::safety::require_persistable(text)?;
        hashes.insert(path.clone(), crate::digest(&bytes));
        files.insert(path.clone(), text.to_string());
    }
    let snapshot = Snapshot::prepare_deferred(spec.clone(), sdk.clone(), files)?;
    let identity = crate::digest(&serde_json::to_vec(
        &json!({"run":context.scope.run,"company":context.scope.company,"owner":context.scope.owner,"actor":context.scope.actor,"phase":context.scope.phase,"policy_revision":context.policy.revision,"parent_approval":approval,"snapshot":snapshot.source_identity(),"host_sdk_paths":[sdk.node,sdk.angular,sdk.driver,sdk.browser],"implementation":crate::digest(concat!(include_str!("native.rs"),include_str!("../registry_angular.rs"),include_str!("../functions/angular_check.rs"),include_str!("../authority.rs")).as_bytes())}),
    )?);
    Ok(Prepared {
        snapshot,
        spec,
        sdk,
        hashes,
        identity,
    })
}
pub fn request_identity(authority: &dyn Authority, scope: &Scope, id: &str) -> Result<String> {
    let db = authority.open(scope)?;
    let policy = governance::load(&db, &scope.run.to_string())?
        .context("Angular requires current policy")?;
    let prepared = capture(
        &ToolContext {
            authority,
            scope,
            policy: &policy,
            db: &db,
        },
        id,
    )?;
    prepared.sdk.verify(&prepared.spec.sdk)?;
    Ok(prepared.identity)
}
pub fn runtime_bindings(
    context: &ToolContext<'_>,
    wanted: &BTreeSet<String>,
) -> Result<Vec<Binding>> {
    if !wanted.contains("angular_build")
        || context
            .policy
            .authorize(&context.scope.actor, "angular_check", &json!({}))
            .is_err()
    {
        return Ok(vec![]);
    }
    let encoded: String = context.db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| r.get(0),
    )?;
    let specs = contracts(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .filter(|c| c.actor == context.scope.actor)
        .collect::<Vec<_>>();
    let mut bindings = vec![];
    if specs.is_empty() {
        return Ok(bindings);
    }
    for spec in specs {
        let prepared = capture(context, &spec.id)?;
        bindings.push(Binding {
            validator: "angular_build".into(),
            contract_id: spec.id,
            source_identity: prepared.identity,
        });
    }
    Ok(bindings)
}
pub fn source_read_identity(
    authority: &dyn Authority,
    scope: &Scope,
    path: &str,
) -> Result<String> {
    let db = authority.open(scope)?;
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get(0),
    )?;
    let spec = contracts(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .find(|s| {
            s.actor == scope.actor
                && (s.project_path == path || s.source_paths.iter().any(|p| p == path))
        })
        .context("Source outside frozen Angular contract")?;
    request_identity(authority, scope, &spec.id)
}
pub(crate) fn prepare_call(context: &ToolContext<'_>, args: &Value) -> Result<Prepared> {
    let prepared = capture(
        context,
        args["contract_id"]
            .as_str()
            .context("contract_id required")?,
    )?;
    ensure!(
        args["expected_source_identity"] == prepared.identity,
        "Angular source/configuration identity changed or was not host-bound"
    );
    Ok(prepared)
}
pub(crate) fn compute(prepared: &Prepared) -> Result<Value> {
    let seal = prepared.sdk.seal()?;
    let mut result = runtime::evaluate(&prepared.snapshot)?;
    ensure!(
        prepared.sdk.seal()? == seal,
        "Angular SDK metadata changed during execution"
    );
    result.verification_identity = prepared.identity.clone();
    result.sdk_metadata_seal = Some(seal);
    // An in-process observation acquires no authority. Only the registry can
    // commit its value together with a canonical native tool audit.
    let value = serde_json::to_value(result)?;
    crate::safety::require_persistable(&value.to_string())?;
    Ok(value)
}
pub(crate) fn finish(
    context: &ToolContext<'_>,
    args: &Value,
    prepared: &Prepared,
    value: Value,
) -> Result<Value> {
    let current = prepare_call(context, args)?;
    ensure!(
        current.identity == prepared.identity,
        "Angular source/owner configuration changed during detached execution; no result accepted"
    );
    ensure!(
        value["verification_identity"] == prepared.identity && value["snapshot_unchanged"] == true,
        "Angular SDK/snapshot changed during execution; no canonical result accepted"
    );
    context.db.execute_batch("CREATE TABLE IF NOT EXISTS angular_check_observations(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,actor TEXT NOT NULL,policy_revision INTEGER NOT NULL,contract_id TEXT NOT NULL,verification_identity TEXT NOT NULL,result TEXT NOT NULL);")?;
    context.db.execute(
        "INSERT INTO angular_check_observations VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            uuid::Uuid::new_v4().to_string(),
            context.scope.run.to_string(),
            context.scope.actor,
            context.policy.revision as i64,
            prepared.spec.id,
            prepared.identity,
            value.to_string()
        ],
    )?;
    Ok(value)
}
fn current_with_sdk(
    context: &ToolContext<'_>,
    reference: &NativeReceipt,
    sealed: &(HostSdk, String),
) -> Result<bool> {
    ensure!(
        reference.schema_version == VERSION
            && reference.validator == "angular_build"
            && reference.tool == "angular_check"
            && reference.api_contract_sha256.is_none(),
        "Unregistered Angular receipt"
    );
    let current = capture(context, &reference.contract_id)?;
    ensure!(current.sdk == sealed.0, "Angular host SDK paths changed");
    if current.identity != reference.source_identity
        || reference.policy_revision != context.policy.revision
        || crate::digest(&serde_json::to_vec(&current.hashes)?) != reference.input_snapshot_sha256
    {
        return Ok(false);
    }
    let args = json!({"contract_id":reference.contract_id,"expected_source_identity":reference.source_identity});
    if crate::digest(&serde_json::to_vec(&args)?) != reference.arguments_sha256 {
        return Ok(false);
    }
    let exists:bool=context.db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='angular_check_observations')",[],|r|r.get(0))?;
    if !exists {
        return Ok(false);
    }
    let stored:Option<String>=context.db.query_row("SELECT result FROM angular_check_observations WHERE run_id=?1 AND actor=?2 AND policy_revision=?3 AND contract_id=?4 AND verification_identity=?5 ORDER BY rowid DESC LIMIT 1",params![context.scope.run.to_string(),context.scope.actor,context.policy.revision as i64,reference.contract_id,reference.source_identity],|r|r.get(0)).optional()?;
    let audit:Option<String>=context.db.query_row("SELECT body FROM audit WHERE run_id=?1 AND kind IN ('mcp_tool_call','mcp_tool_effect_uncertain','mcp_tool_rejected') AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.phase')='worker' AND json_extract(body,'$.tool')='angular_check' AND json_extract(body,'$.arguments_sha256')=?3 ORDER BY seq DESC LIMIT 1",params![context.scope.run.to_string(),context.scope.actor,reference.arguments_sha256],|r|r.get(0)).optional()?;
    let (Some(stored), Some(audit)) = (stored, audit) else {
        return Ok(false);
    };
    let value: Value = serde_json::from_str(&stored)?;
    let audit: Value = serde_json::from_str(&audit)?;
    let passed = value["status"] == "browser_passed"
        && value["compile"]["passed"] == true
        && value["browser"]["passed"] == true
        && value["artifact_sha256"].is_string();
    Ok(crate::digest(stored.as_bytes()) == reference.result_sha256
        && audit["status"] == "completed"
        && audit["policy_revision"] == reference.policy_revision
        && audit["result_sha256"] == reference.result_sha256
        && value["verification_identity"] == reference.source_identity
        && value["contract_id"] == reference.contract_id
        && value["inputs_sha256"] == json!(current.hashes)
        && value["sdk"] == serde_json::to_value(&current.spec.sdk)?
        && value["artifact_sha256"] == json!(reference.artifact_sha256)
        && value["snapshot_unchanged"] == true
        && value["sdk_metadata_seal"] == sealed.1
        && value["qualified"] == false
        && value["promoted"] == false
        && value["authority_granted"] == false
        && value["model_calls"] == 0
        && value["provider_cost_usd"] == 0
        && match reference.outcome {
            CheckStatus::Passed => passed,
            CheckStatus::Failed => {
                !passed
                    && matches!(
                        value["status"].as_str(),
                        Some("check_failed" | "indeterminate")
                    )
            }
            _ => false,
        })
}
/// Batch verification seals the one host SDK once, even for four contracts.
/// Every source/grant/audit is still checked separately; no verification is
/// reused across requests or substituted for a new build.
pub fn references_current(
    context: &ToolContext<'_>,
    references: &[&NativeReceipt],
) -> Result<bool> {
    let Some(first) = references.first() else {
        return Ok(true);
    };
    let prepared = capture(context, &first.contract_id)?;
    let seal = if let Some(seal) = context.authority.angular_sdk_seal(&prepared.sdk)? {
        seal
    } else {
        ensure!(context.db.is_autocommit(),"Angular SDK observation must be prepared before a database transaction; no in-transaction scan");
        prepared.sdk.seal()?
    };
    let sealed = (prepared.sdk, seal);
    for reference in references {
        if !current_with_sdk(context, reference, &sealed)? {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn reference_current(context: &ToolContext<'_>, reference: &NativeReceipt) -> Result<bool> {
    references_current(context, &[reference])
}
pub fn reference(
    context: &ToolContext<'_>,
    binding: &Binding,
    value: &Value,
    audit: &Value,
) -> Result<NativeReceipt> {
    ensure!(
        binding.validator == "angular_build"
            && value["verification_identity"] == binding.source_identity
            && value["contract_id"] == binding.contract_id
            && audit["actor"] == context.scope.actor
            && audit["phase"] == "worker"
            && audit["tool"] == "angular_check",
        "Angular native result/actor differs from frozen validator"
    );
    let required = |key: &str| {
        audit[key]
            .as_str()
            .map(str::to_owned)
            .context("Canonical Angular receipt field missing")
    };
    let reference = NativeReceipt {
        schema_version: VERSION,
        validator: "angular_build".into(),
        tool: "angular_check".into(),
        contract_id: binding.contract_id.clone(),
        source_identity: binding.source_identity.clone(),
        policy_revision: context.policy.revision,
        arguments_sha256: required("arguments_sha256")?,
        result_sha256: required("result_sha256")?,
        input_snapshot_sha256: crate::digest(&serde_json::to_vec(&value["inputs_sha256"])?),
        artifact_sha256: serde_json::from_value(value["artifact_sha256"].clone())?,
        api_contract_sha256: None,
        outcome: if value["status"] == "browser_passed" {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        elapsed_micros: value["elapsed_ms"]
            .as_u64()
            .unwrap_or(0)
            .saturating_mul(1000),
        sampled_process_group_rss_bytes: [
            value["compile"]["sampled_process_group_rss_bytes"].as_u64(),
            value["browser"]["phase"]["sampled_process_group_rss_bytes"].as_u64(),
        ]
        .into_iter()
        .flatten()
        .filter(|n| *n > 0)
        .max(),
    };
    ensure!(
        crate::digest(&serde_json::to_vec(value)?) == reference.result_sha256
            && reference_current(context, &reference)?,
        "Angular native receipt no longer current or canonical"
    );
    Ok(reference)
}

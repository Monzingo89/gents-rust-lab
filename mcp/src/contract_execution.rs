//! Owner-opted exact native contracts. No model may expand this mode, inputs,
//! destinations or phase. Native execution rechecks it inside its transaction.
use crate::{
    artifact_bindings, authority::Authority, bound_sources, digest, governance, Phase, Registry,
    Scope,
};
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub fn enabled(policy: &governance::Policy, scope: &Scope) -> bool {
    scope.phase == Phase::Worker
        && policy
            .grants
            .get(&scope.actor)
            .is_some_and(|grant| grant.contract_execution_phases.contains("worker"))
}
fn frozen(db: &Connection, scope: &Scope, policy: &governance::Policy) -> Result<(Value, String)> {
    ensure!(
        scope.expected_revision == Some(policy.revision),
        "approved workflow requires its explicit current policy epoch"
    );
    let (encoded, digest_, state): (String, String, String) = db.query_row(
        "SELECT intake,digest,state FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    ensure!(
        matches!(state.as_str(), "approved" | "running"),
        "approved workflow requires an active approved job"
    );
    ensure!(
        digest(encoded.as_bytes()) == digest_,
        "approved workflow intake digest mismatch"
    );
    Ok((serde_json::from_str(&encoded)?, digest_))
}
/// Host scope observation, not a tool grant or permission to skip native checks.
pub fn scope_authority(authority: &dyn Authority, scope: &Scope) -> Result<Option<Value>> {
    let db = authority.open(scope)?;
    let Some(policy) = governance::load(&db, &scope.run.to_string())? else {
        return Ok(None);
    };
    if !enabled(&policy, scope) {
        return Ok(None);
    }
    let (_, contract) = frozen(&db, scope, &policy)?;
    Ok(Some(
        json!({"mode":"approved_workflow","actor":scope.actor,"phase":scope.phase,"policy_revision":policy.revision,"contract_sha256":contract,"model_called":false,"scope":"Owner-selected mode; exact native operation checks and independent QA remain required."}),
    ))
}
fn hash(value: &Value) -> bool {
    value.as_str().is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn source_allowed(
    registry: &Registry,
    context: &crate::ToolContext<'_>,
    name: &str,
    args: &Value,
) -> Result<bool> {
    registry.approved_source(context, name, args)
}
pub(crate) fn check(
    registry: &Registry,
    authority: &dyn Authority,
    db: &Connection,
    scope: &Scope,
    policy: &governance::Policy,
    name: &str,
    args: &Value,
) -> Result<()> {
    if !enabled(policy, scope) {
        return Ok(());
    }
    let (intake, _) = frozen(db, scope, policy)?;
    let bindings = artifact_bindings::parse(&intake)?;
    ensure!(
        !bindings.is_empty(),
        "approved workflow requires frozen native artifact contracts"
    );
    let mut cache = bound_sources::Cache::new();
    for (index, binding) in bindings.iter().enumerate() {
        if binding.source_actor == scope.actor
            && binding.source_tool == name
            && source_allowed(
                registry,
                &crate::ToolContext {
                    authority,
                    db,
                    scope,
                    policy,
                },
                name,
                args,
            )?
            && bound_sources::arguments(db, scope, policy, &bindings, index, &mut cache)?.as_ref()
                == Some(args)
        {
            return Ok(());
        }
        if binding.receipt_only
            || name != "files"
            || binding.writer != scope.actor
            || args["path"] != binding.path
        {
            continue;
        }
        let Some(source_args) =
            bound_sources::arguments(db, scope, policy, &bindings, index, &mut cache)?
        else {
            continue;
        };
        let source_scope = Scope {
            actor: binding.source_actor.clone(),
            ..scope.clone()
        };
        ensure!(
            source_allowed(
                registry,
                &crate::ToolContext {
                    authority,
                    db,
                    scope: &source_scope,
                    policy
                },
                &binding.source_tool,
                &source_args
            )?,
            "approved workflow source lacks a supported frozen read-only contract"
        );
        let Some(observed) =
            bound_sources::observe(db, scope, policy, &bindings, index, &mut cache)?
        else {
            continue;
        };
        let expected = artifact_bindings::encoded(&observed.value, &binding.encoding)?;
        if args["op"] == "read" && args.as_object().is_some_and(|a| a.len() == 2) {
            return Ok(());
        }
        if args["op"] == "write"
            && args["content"] == expected
            && args.as_object().is_some_and(|a| {
                a.len() == 3 || (a.len() == 4 && a.contains_key("expected_sha256"))
            })
        {
            policy.authorize(
                &scope.actor,
                "files",
                &json!({"op":"read","path":binding.path}),
            )?;
            let current = match crate::workspace::open(&authority.workspace(scope)?, false)
                .and_then(|directory| directory.read(&binding.path, 0, 4))
            {
                Ok(value) => Some(
                    value["sha256"]
                        .as_str()
                        .context("file hash missing")?
                        .to_owned(),
                ),
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                {
                    None
                }
                Err(error) => return Err(error),
            };
            let Some(current) = current else {
                ensure!(
                    args.get("expected_sha256").is_none(),
                    "new target must not claim an existing digest"
                );
                return Ok(());
            };
            ensure!(
                hash(&args["expected_sha256"]) && args["expected_sha256"] == current,
                "existing target needs its current write precondition"
            );
            let prior:Option<String>=db.query_row("SELECT r.artifact_sha256 FROM mcp_binding_artifacts_v1 r WHERE r.run_id=?1 AND r.policy_revision=?2 AND r.path=?3 AND r.actor=?4 AND r.op='write' AND EXISTS(SELECT 1 FROM audit a WHERE a.run_id=r.run_id AND a.kind='mcp_tool_call' AND json_extract(a.body,'$.status')='completed' AND json_extract(a.body,'$.phase')='worker' AND json_extract(a.body,'$.tool')='files' AND json_extract(a.body,'$.actor')=r.actor AND json_extract(a.body,'$.policy_revision')=r.policy_revision AND json_extract(a.body,'$.arguments_sha256')=r.arguments_sha256 AND json_extract(a.body,'$.result_sha256')=r.result_sha256)",params![scope.run.to_string(),policy.revision as i64,binding.path,scope.actor],|r|r.get(0)).optional()?;
            ensure!(
                prior.as_deref() == Some(current.as_str()),
                "approved workflow cannot overwrite an unrecognized target"
            );
            return Ok(());
        }
    }
    bail!("request is outside this actor's exact approved native workflow")
}
/// Read-only preparation. Execution repeats the same check in its transaction.
pub fn authorize_call(
    authority: &dyn Authority,
    scope: &Scope,
    name: &str,
    args: &Value,
) -> Result<Option<Value>> {
    let db = authority.open(scope)?;
    let Some(policy) = governance::load(&db, &scope.run.to_string())? else {
        return Ok(None);
    };
    if !enabled(&policy, scope) {
        return Ok(None);
    }
    policy.authorize(&scope.actor, name, args)?;
    check(&Registry::new(), authority, &db, scope, &policy, name, args)?;
    let (_, contract) = frozen(&db, scope, &policy)?;
    Ok(Some(
        json!({"mode":"approved_workflow","actor":scope.actor,"phase":scope.phase,"policy_revision":policy.revision,"contract_sha256":contract,"tool":name,"arguments_sha256":digest(args.to_string().as_bytes()),"model_called":false,"execution_established":false}),
    ))
}

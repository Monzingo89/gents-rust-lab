//! Current-epoch native projections used as approved input fields. No model
//! text, filesystem artifact or unverified receipt can supply these values.
use crate::{artifact_bindings::Binding, digest, governance, Scope};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone)]
pub(crate) struct Observed {
    pub value: Value,
    pub result_sha: String,
}
pub(crate) type Cache = BTreeMap<usize, Option<Observed>>;
pub(crate) fn arguments(
    db: &Connection,
    scope: &Scope,
    policy: &governance::Policy,
    bindings: &[Binding],
    index: usize,
    cache: &mut Cache,
) -> Result<Option<Value>> {
    let binding = &bindings[index];
    let mut arguments = binding.arguments.clone();
    for input in &binding.argument_sources {
        let parent = bindings[..index]
            .iter()
            .position(|v| v.path == input.path)
            .context("native input source must precede its consumer")?;
        let Some(observed) = observe(db, scope, policy, bindings, parent, cache)? else {
            return Ok(None);
        };
        if let Some(slot) = input.argument_index {
            arguments[&input.field][slot] = observed
                .value
                .pointer(
                    input
                        .pointer
                        .as_deref()
                        .context("missing positional projection")?,
                )
                .context("native positional projection is absent")?
                .clone();
        } else {
            arguments[&input.field] = observed.value;
        }
    }
    crate::contracts::ToolContracts::validate(
        &crate::contracts::EmbeddedContracts,
        &binding.source_tool,
        &arguments,
    )?;
    if !binding.argument_sources.is_empty() {
        ensure!(
            crate::Registry::new().read_only_call(&binding.source_tool, &arguments),
            "resolved native input cannot introduce a mutating call"
        );
    }
    ensure!(
        arguments.to_string().len() <= 16384,
        "resolved native arguments exceed bound"
    );
    Ok(Some(arguments))
}
pub(crate) fn observe(
    db: &Connection,
    scope: &Scope,
    policy: &governance::Policy,
    bindings: &[Binding],
    index: usize,
    cache: &mut Cache,
) -> Result<Option<Observed>> {
    if let Some(value) = cache.get(&index) {
        return Ok(value.clone());
    }
    let binding = &bindings[index];
    ensure!(
        scope.actor == binding.source_actor
            || policy
                .grants
                .get(&scope.actor)
                .is_some_and(|grant| grant.memory_from.contains(&binding.source_actor)),
        "native input recipient cannot consume this source"
    );
    let Some(arguments) = arguments(db, scope, policy, bindings, index, cache)? else {
        cache.insert(index, None);
        return Ok(None);
    };
    policy.authorize(&binding.source_actor, &binding.source_tool, &arguments)?;
    let argument_sha = digest(&serde_json::to_vec(&arguments)?);
    let row:Option<(String,String,String)>=db.query_row("SELECT r.value_json,r.result_sha256,r.value_sha256 FROM mcp_binding_sources_v1 r WHERE r.run_id=?1 AND r.policy_revision=?2 AND r.path=?3 AND r.actor=?4 AND r.tool=?5 AND r.arguments_sha256=?6 AND r.pointer=?7 AND EXISTS(SELECT 1 FROM audit a WHERE a.run_id=r.run_id AND a.kind='mcp_tool_call' AND json_extract(a.body,'$.status')='completed' AND json_extract(a.body,'$.actor')=r.actor AND json_extract(a.body,'$.phase')='worker' AND json_extract(a.body,'$.tool')=r.tool AND json_extract(a.body,'$.policy_revision')=r.policy_revision AND json_extract(a.body,'$.arguments_sha256')=r.arguments_sha256 AND json_extract(a.body,'$.result_sha256')=r.result_sha256) LIMIT 1",params![scope.run.to_string(),policy.revision as i64,binding.path,binding.source_actor,binding.source_tool,argument_sha,binding.pointer],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let observed = if let Some((value, result_sha, value_sha)) = row {
        ensure!(
            digest(value.as_bytes()) == value_sha,
            "native source projection integrity check failed"
        );
        if binding.source_tool == "owned_rust_call"
            && !crate::owned_recipes::observation_matches(
                db,
                scope,
                policy,
                bindings,
                index,
                &arguments,
                &result_sha,
                cache,
            )?
        {
            cache.insert(index, None);
            return Ok(None);
        }
        Some(Observed {
            value: serde_json::from_str(&value)?,
            result_sha,
        })
    } else {
        None
    };
    cache.insert(index, observed.clone());
    Ok(observed)
}

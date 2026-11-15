//! Explicit owner-frozen positional inputs resolved from canonical native evidence.
//! This adapter never reads an output file, asks a model, grants access or invokes a component.
use crate::{
    artifact_bindings::{ArgumentSource, Binding},
    owned_components::Spec,
    ToolContext,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputRecipe {
    pub arguments: Value,
    pub sources: Vec<ArgumentSource>,
}
impl InputRecipe {
    pub fn validate(&self) -> Result<()> {
        let args = self
            .arguments
            .as_array()
            .context("recipe needs positional arguments")?;
        ensure!(
            args.len() <= 8
                && self.arguments.to_string().len() <= 8192
                && !self.sources.is_empty()
                && self.sources.len() <= 8,
            "recipe input budget exceeded"
        );
        let mut slots = std::collections::BTreeSet::new();
        for input in &self.sources {
            let slot = input
                .argument_index
                .context("recipe reference requires a positional index")?;
            ensure!(
                input.field == "arguments"
                    && slots.insert(slot)
                    && args
                        .get(slot)
                        .is_some_and(crate::artifact_bindings::is_placeholder),
                "invalid, duplicate or nonempty recipe argument slot"
            );
            let pointer = input
                .pointer
                .as_deref()
                .context("recipe projection required")?;
            ensure!(
                pointer.len() <= 160
                    && !pointer.chars().any(char::is_control)
                    && (pointer.is_empty() || pointer.starts_with('/')),
                "invalid recipe projection"
            );
            ensure!(
                crate::workspace_path::is_safe_relative_path(&input.path),
                "invalid recipe producer path"
            );
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<String> {
        // Tag separates this identity from every legacy literal-input hash.
        Ok(crate::digest(&serde_json::to_vec(
            &json!({"kind":"owned_positional_receipts_v1","recipe":self}),
        )?))
    }
}
fn same_sources(left: &[ArgumentSource], right: &[ArgumentSource]) -> bool {
    let view = |items: &[ArgumentSource]| {
        let mut rows = items
            .iter()
            .map(|i| {
                (
                    i.argument_index,
                    i.field.clone(),
                    i.path.clone(),
                    i.pointer.clone(),
                )
            })
            .collect::<Vec<_>>();
        rows.sort();
        rows
    };
    view(left) == view(right)
}
pub(crate) fn validate_bindings(intake: &Value, bindings: &[Binding]) -> Result<()> {
    let specs = crate::owned_components::specs(intake)?;
    for spec in &specs {
        let Some(recipe) = &spec.input_recipe else {
            continue;
        };
        recipe.validate()?;
        let matches = bindings
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.source_tool == "owned_rust_call" && b.arguments["binding_id"] == spec.id
            })
            .collect::<Vec<_>>();
        ensure!(
            !matches.is_empty(),
            "dependent owned invocation needs a frozen result binding"
        );
        for (index, binding) in matches {
            ensure!(
                binding.source_actor == spec.actor
                    && binding.arguments["arguments"] == recipe.arguments
                    && same_sources(&binding.argument_sources, &recipe.sources),
                "owned recipe differs from frozen binding"
            );
            for input in &recipe.sources {
                let parent = bindings[..index]
                    .iter()
                    .find(|p| p.path == input.path)
                    .context("recipe producer must precede consumer")?;
                ensure!(
                    parent.source_tool == "owned_rust_call"
                        && parent.pointer == "/output"
                        && specs.iter().any(|s| s.id == parent.arguments["binding_id"]
                            && s.actor == parent.source_actor),
                    "recipe producer must be a frozen owned whole-output binding"
                );
            }
        }
    }
    for binding in bindings {
        if binding
            .argument_sources
            .iter()
            .any(|i| i.argument_index.is_some())
        {
            ensure!(
                specs.iter().any(|s| s.id == binding.arguments["binding_id"]
                    && s.actor == binding.source_actor
                    && s.input_recipe.is_some()),
                "positional binding requires its explicit owner-frozen recipe"
            );
        }
    }
    Ok(())
}
pub(crate) struct Resolved {
    pub input: Value,
    pub lineage: Value,
    pub recipe_sha256: String,
}
pub(crate) fn resolve(
    context: &ToolContext<'_>,
    intake: &Value,
    spec: &Spec,
    source: &str,
    admission_cache: &mut crate::owned_components::AdmissionCache,
) -> Result<Resolved> {
    let recipe = spec
        .input_recipe
        .as_ref()
        .context("owned recipe required")?;
    let bindings = crate::artifact_bindings::parse(intake)?;
    let index = bindings
        .iter()
        .position(|b| {
            b.source_tool == "owned_rust_call"
                && b.arguments["binding_id"] == spec.id
                && b.source_actor == spec.actor
        })
        .context("owned recipe binding missing")?;
    let mut cache = crate::bound_sources::Cache::new();
    let args = crate::bound_sources::arguments(
        context.db,
        context.scope,
        context.policy,
        &bindings,
        index,
        &mut cache,
    )?
    .context("owned recipe awaits current-epoch producer receipts")?;
    let input = args["arguments"].clone();
    let specifications = crate::owned_components::specs(intake)?;
    let mut lineage = vec![];
    for reference in &recipe.sources {
        let parent_index = bindings[..index]
            .iter()
            .position(|b| b.path == reference.path)
            .context("owned recipe producer missing")?;
        let binding = &bindings[parent_index];
        let producer = specifications
            .iter()
            .find(|s| s.id == binding.arguments["binding_id"] && s.actor == binding.source_actor)
            .context("owned producer identity missing")?;
        let producer_source = crate::owned_components::source(context, producer)?;
        let pointer = reference.pointer.as_deref().unwrap();
        crate::component_api::connections::describe(
            &producer_source,
            &producer.function,
            pointer,
            source,
            &spec.function,
            reference.argument_index.unwrap(),
        )?;
        let observed = crate::bound_sources::observe(
            context.db,
            context.scope,
            context.policy,
            &bindings,
            parent_index,
            &mut cache,
        )?
        .context("owned producer receipt missing")?;
        let selected = crate::component_api::connections::project_output(
            &producer_source,
            &producer.function,
            pointer,
            &observed.value,
        )?;
        ensure!(
            &input[reference.argument_index.unwrap()] == selected,
            "resolved component value mismatch"
        );
        let parent_args = crate::bound_sources::arguments(
            context.db,
            context.scope,
            context.policy,
            &bindings,
            parent_index,
            &mut cache,
        )?
        .context("producer argument lineage missing")?;
        let producer_scope = crate::Scope {
            actor: producer.actor.clone(),
            ..context.scope.clone()
        };
        crate::owned_components::admit_cached(
            &ToolContext {
                scope: &producer_scope,
                ..*context
            },
            &parent_args,
            admission_cache,
        )?;
        lineage.push(lineage_record(
            context.scope,
            context.policy,
            reference,
            producer,
            &parent_args,
            &observed.result_sha,
            selected,
        ));
    }
    crate::component_api::validate_arguments(source, &spec.function, &input)?;
    Ok(Resolved {
        input,
        lineage: json!(lineage),
        recipe_sha256: recipe.digest()?,
    })
}

/// Projection rows alone do not establish a successful owned invocation. Require
/// its native success record and current recipe lineage as well as the observer's
/// canonical audit match. Recursion follows only earlier validated bindings.
#[allow(clippy::too_many_arguments)]
pub(crate) fn observation_matches(
    db: &rusqlite::Connection,
    scope: &crate::Scope,
    policy: &crate::governance::Policy,
    bindings: &[Binding],
    index: usize,
    arguments: &Value,
    result_sha: &str,
    cache: &mut crate::bound_sources::Cache,
) -> Result<bool> {
    let intake: Value = serde_json::from_str(&db.query_row::<String, _, _>(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get(0),
    )?)?;
    let specs = crate::owned_components::specs(&intake)?;
    let binding = &bindings[index];
    let Some(spec) = specs
        .iter()
        .find(|s| s.id == arguments["binding_id"] && s.actor == binding.source_actor)
    else {
        return Ok(false);
    };
    let mut lineage = vec![];
    let recipe_sha = if let Some(recipe) = &spec.input_recipe {
        for reference in &recipe.sources {
            let parent = bindings[..index]
                .iter()
                .position(|b| b.path == reference.path)
                .context("owned lineage producer missing")?;
            let producer_binding = &bindings[parent];
            let producer = specs
                .iter()
                .find(|s| {
                    s.id == producer_binding.arguments["binding_id"]
                        && s.actor == producer_binding.source_actor
                })
                .context("owned lineage identity missing")?;
            let Some(observed) =
                crate::bound_sources::observe(db, scope, policy, bindings, parent, cache)?
            else {
                return Ok(false);
            };
            let Some(value) = observed
                .value
                .pointer(reference.pointer.as_deref().unwrap())
            else {
                return Ok(false);
            };
            let Some(parent_args) =
                crate::bound_sources::arguments(db, scope, policy, bindings, parent, cache)?
            else {
                return Ok(false);
            };
            lineage.push(lineage_record(
                scope,
                policy,
                reference,
                producer,
                &parent_args,
                &observed.result_sha,
                value,
            ));
        }
        Some(recipe.digest()?)
    } else {
        None
    };
    let input_sha = crate::digest(arguments["arguments"].to_string().as_bytes());
    if spec
        .input_sha256
        .as_ref()
        .is_some_and(|expected| expected != &input_sha)
    {
        return Ok(false);
    }
    let lineage_sha = recipe_sha
        .as_ref()
        .map(|_| crate::digest(json!(lineage).to_string().as_bytes()));
    let columns = db
        .prepare("PRAGMA table_info(owned_rust_call_results)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let has_lineage = columns.iter().any(|c| c == "recipe_sha256")
        && columns.iter().any(|c| c == "lineage_sha256");
    let suffix = if has_lineage {
        " AND recipe_sha256 IS ?10 AND lineage_sha256 IS ?11"
    } else {
        " AND ?10 IS NULL AND ?11 IS NULL"
    };
    let query=format!("SELECT COUNT(*) FROM owned_rust_call_results WHERE run_id=?1 AND policy_revision=?2 AND actor=?3 AND binding_id=?4 AND source_sha256=?5 AND api_sha256=?6 AND input_sha256=?7 AND arguments_sha256=?8 AND result_sha256=?9 AND passed=1{suffix}");
    let count: i64 = db.query_row(
        &query,
        rusqlite::params![
            scope.run.to_string(),
            policy.revision as i64,
            spec.actor,
            spec.id,
            spec.source_sha256,
            spec.api_sha256,
            input_sha,
            crate::digest(arguments.to_string().as_bytes()),
            result_sha,
            recipe_sha,
            lineage_sha
        ],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}
fn lineage_record(
    scope: &crate::Scope,
    policy: &crate::governance::Policy,
    reference: &ArgumentSource,
    producer: &Spec,
    parent_args: &Value,
    result_sha: &str,
    selected: &Value,
) -> Value {
    json!({"run_id":scope.run,"path":reference.path,"argument_index":reference.argument_index,
  "pointer":reference.pointer.as_deref().unwrap(),"producer_binding_id":producer.id,"producer_actor":producer.actor,
  "source_sha256":producer.source_sha256,"api_sha256":producer.api_sha256,"producer_function":producer.function,
  "arguments_sha256":crate::digest(parent_args.to_string().as_bytes()),"result_sha256":result_sha,
  "value_sha256":crate::digest(selected.to_string().as_bytes()),"policy_revision":policy.revision})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn intake() -> Value {
        let reference =
            json!({"field":"arguments","path":"parent.json","argument_index":0,"pointer":""});
        let spec = |id: &str| json!({"id":id,"actor":"A","source_path":format!("{id}.rs"),"source_sha256":"a".repeat(64),"api_sha256":"b".repeat(64),"function":"f","owner_declared_review_package_id":"c".repeat(64)});
        let mut parent = spec("parent");
        parent["input_sha256"] = json!(crate::digest(b"[\"7\"]"));
        let mut child = spec("child");
        child["input_recipe"] = json!({"arguments":[null],"sources":[reference]});
        json!({"required_owned_rust_calls":[parent,child],"artifact_bindings":[
         {"path":"parent.json","source_actor":"A","writer":"A","source_tool":"owned_rust_call","arguments":{"binding_id":"parent","arguments":["7"]},"pointer":"/output","encoding":"json"},
         {"path":"child.json","source_actor":"A","writer":"A","source_tool":"owned_rust_call","arguments":{"binding_id":"child","arguments":[null]},"pointer":"/output","encoding":"json","argument_sources":[reference]}
        ]})
    }
    #[test]
    fn recipe_is_explicit_separate_identity_and_native_graph_admission_is_strict() {
        let good = intake();
        assert!(crate::artifact_bindings::parse(&good).is_ok());
        let recipe: InputRecipe =
            serde_json::from_value(good["required_owned_rust_calls"][1]["input_recipe"].clone())
                .unwrap();
        assert_ne!(
            recipe.digest().unwrap(),
            crate::digest(recipe.arguments.to_string().as_bytes())
        );
        for edit in 0..8 {
            let mut bad = good.clone();
            match edit {
                0 => bad["required_owned_rust_calls"][1]["input_sha256"] = json!("a".repeat(64)),
                1 => bad["required_owned_rust_calls"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("input_recipe")
                    .map(|_| ())
                    .unwrap(),
                2 => {
                    bad["required_owned_rust_calls"][1]["input_recipe"]["arguments"] = json!(["8"]);
                }
                3 => {
                    bad["artifact_bindings"][1]["argument_sources"][0]["pointer"] = json!("/some");
                }
                4 => {
                    bad["artifact_bindings"].as_array_mut().unwrap().swap(0, 1);
                }
                5 => {
                    bad["artifact_bindings"][0]["pointer"] = json!("/passed");
                }
                6 => {
                    bad["required_owned_rust_calls"][1]["input_recipe"]["sources"].as_array_mut().unwrap().push(json!({"field":"arguments","path":"parent.json","argument_index":0,"pointer":""}));
                }
                _ => {
                    bad["artifact_bindings"][1]["arguments"]["arguments"] = json!([[]]);
                }
            }
            assert!(
                crate::artifact_bindings::parse(&bad).is_err(),
                "edit {edit}"
            );
        }
    }
    #[test]
    fn unsupported_cycles_fields_sizes_and_indices_never_admit_a_recipe() {
        let good = intake();
        let base = good["required_owned_rust_calls"][1]["input_recipe"].clone();
        for edit in 0..7 {
            let mut bad = base.clone();
            match edit {
                0 => bad["sources"][0]["argument_index"] = json!(8),
                1 => bad["sources"][0]["field"] = json!("binding_id"),
                2 => bad["sources"][0]["pointer"] = json!("/".repeat(161)),
                3 => bad["sources"][0]["path"] = json!("../escape"),
                4 => bad["sources"] = json!([]),
                5 => bad["arguments"] = json!([null, "x".repeat(8192)]),
                _ => bad["sources"][0]["source_text"] = json!("forbidden"),
            }
            let result = serde_json::from_value::<InputRecipe>(bad)
                .and_then(|r| r.validate().map_err(serde::de::Error::custom));
            assert!(result.is_err(), "edit {edit}");
        }
        let mut cycle = good.clone();
        cycle["required_owned_rust_calls"][0]
            .as_object_mut()
            .unwrap()
            .remove("input_sha256");
        cycle["required_owned_rust_calls"][0]["input_recipe"] = json!({"arguments":[null],"sources":[{"field":"arguments","path":"child.json","argument_index":0,"pointer":""}]});
        cycle["artifact_bindings"][0]["arguments"]["arguments"] = json!([null]);
        cycle["artifact_bindings"][0]["argument_sources"] =
            cycle["required_owned_rust_calls"][0]["input_recipe"]["sources"].clone();
        assert!(crate::artifact_bindings::parse(&cycle).is_err());
    }
}

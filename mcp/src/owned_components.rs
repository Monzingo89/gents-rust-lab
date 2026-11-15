//! Owner-frozen pure-library invocations. Source custody and native evidence,
//! not package promotion, model authority or a new global execution registry.
use crate::{authority::Authority, workspace, Scope, ToolContext};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub id: String,
    pub actor: String,
    pub source_path: String,
    pub source_sha256: String,
    pub api_sha256: String,
    pub function: String,
    #[serde(default)]
    pub input_sha256: Option<String>,
    #[serde(default)]
    pub input_recipe: Option<crate::owned_recipes::InputRecipe>,
    /// Provenance declaration by the approving owner, not native qualification.
    pub owner_declared_review_package_id: String,
}
fn hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn specs(intake: &Value) -> Result<Vec<Spec>> {
    let specs: Vec<Spec> = serde_json::from_value(
        intake
            .get("required_owned_rust_calls")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    ensure!(
        specs.len() <= 16,
        "at most sixteen owner-frozen Rust invocations"
    );
    let mut ids = std::collections::BTreeSet::new();
    for spec in &specs {
        ensure!(
            !spec.id.is_empty()
                && spec.id.len() <= 64
                && spec
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && ids.insert(&spec.id),
            "invalid or duplicate owned invocation id"
        );
        ensure!(
            !spec.actor.trim().is_empty()
                && spec.actor.len() <= 200
                && !spec.actor.chars().any(char::is_control),
            "invalid owned invocation actor"
        );
        ensure!(
            spec.source_path.len() <= 120
                && spec.source_path.ends_with(".rs")
                && crate::workspace_path::is_safe_relative_path(&spec.source_path),
            "invalid owned source path"
        );
        ensure!(
            [
                &spec.source_sha256,
                &spec.api_sha256,
                &spec.owner_declared_review_package_id
            ]
            .iter()
            .all(|v| hash(v)),
            "invalid owned invocation identity hash"
        );
        ensure!(
            spec.input_sha256.is_some() != spec.input_recipe.is_some(),
            "owned invocation requires exactly one literal hash or receipt recipe"
        );
        if let Some(hash_) = &spec.input_sha256 {
            ensure!(hash(hash_), "invalid literal input identity");
        }
        if let Some(recipe) = &spec.input_recipe {
            recipe.validate()?;
        }
        ensure!(
            spec.function.len() <= 128 && syn::parse_str::<syn::Ident>(&spec.function).is_ok(),
            "invalid owned function name"
        );
        ensure!(
            !intake["required_rust_checks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|check| check["source_path"] == spec.source_path
                    || check["tests_path"] == spec.source_path),
            "owned library inputs cannot also be source-authoring or acceptance paths"
        );
    }
    Ok(specs)
}
fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS owned_rust_call_results(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,policy_revision INTEGER NOT NULL,actor TEXT NOT NULL,binding_id TEXT NOT NULL,source_sha256 TEXT NOT NULL,api_sha256 TEXT NOT NULL,input_sha256 TEXT NOT NULL,arguments_sha256 TEXT NOT NULL,result_sha256 TEXT NOT NULL,passed INTEGER NOT NULL);")?;
    let columns = db
        .prepare("PRAGMA table_info(owned_rust_call_results)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for column in ["recipe_sha256", "lineage_sha256"] {
        if !columns.iter().any(|name| name == column) {
            // Fixed trusted identifiers only, never candidate input.
            db.execute_batch(&format!(
                "ALTER TABLE owned_rust_call_results ADD COLUMN {column} TEXT"
            ))?;
        }
    }
    Ok(())
}
struct PreparedInvocation {
    spec: Spec,
    source: String,
    input: Value,
    input_sha256: String,
    recipe_sha256: Option<String>,
    lineage: Value,
}
pub(crate) type AdmissionCache = std::collections::BTreeSet<(String, String)>;
fn prepare(context: &ToolContext<'_>, arguments: &Value) -> Result<PreparedInvocation> {
    prepare_with_cache(context, arguments, &mut AdmissionCache::new(), true)
}
fn prepare_with_cache(
    context: &ToolContext<'_>,
    arguments: &Value,
    cache: &mut AdmissionCache,
    require_active: bool,
) -> Result<PreparedInvocation> {
    let (encoded, state): (String, String) = context.db.query_row(
        "SELECT intake,state FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        !require_active || matches!(state.as_str(), "approved" | "running"),
        "owned library invocation requires an approved active job"
    );
    let intake: Value = serde_json::from_str(&encoded)?;
    let specifications = specs(&intake)?;
    let spec = specifications
        .into_iter()
        .find(|s| s.id == arguments["binding_id"] && s.actor == context.scope.actor)
        .context("owned invocation requires exact owner-frozen binding and actor")?;
    if spec.input_recipe.is_some() {
        let frozen: String = context.db.query_row(
            "SELECT digest FROM jobs WHERE run_id=?1",
            [context.scope.run.to_string()],
            |r| r.get(0),
        )?;
        ensure!(
            crate::digest(encoded.as_bytes()) == frozen,
            "owned recipe intake digest mismatch"
        );
    }
    let input = arguments
        .get("arguments")
        .context("owned invocation arguments required")?;
    ensure!(
        input.is_array() && serde_json::to_vec(input)?.len() <= 8192,
        "owned invocation requires bounded positional data"
    );
    let text = source(context, &spec)?;
    let (recipe_sha256, lineage) = if let Some(expected) = &spec.input_sha256 {
        ensure!(
            crate::digest(&serde_json::to_vec(input)?) == *expected,
            "owned invocation input differs from owner-frozen hash"
        );
        (None, json!([]))
    } else {
        let resolved = crate::owned_recipes::resolve(context, &intake, &spec, &text, cache)?;
        ensure!(
            resolved.input == *input,
            "owned invocation differs from canonical resolved inputs"
        );
        (Some(resolved.recipe_sha256), resolved.lineage)
    };
    crate::component_api::driver(&text, &spec.function, input)?;
    Ok(PreparedInvocation {
        spec,
        source: text,
        input: input.clone(),
        input_sha256: crate::digest(&serde_json::to_vec(input)?),
        recipe_sha256,
        lineage,
    })
}
/// Descriptor-relative current source/API custody, separate from argument resolution.
pub(crate) fn source(context: &ToolContext<'_>, spec: &Spec) -> Result<String> {
    let work = workspace::open(&context.authority.workspace(context.scope)?, false)?;
    let source = work.read_complete(&spec.source_path, 8192)?;
    ensure!(
        source["sha256"] == spec.source_sha256,
        "owned library source changed"
    );
    let text = source["text"]
        .as_str()
        .context("owned source text missing")?;
    crate::safety::require_persistable(text)?;
    let description = crate::component_api::describe(text)?;
    ensure!(
        description["api_sha256"] == spec.api_sha256,
        "owned library API changed"
    );
    Ok(text.to_owned())
}
/// Read-only checks only. The registry still requires its owner opt-in, grants,
/// exact contract arguments and phase; execution repeats every identity check.
pub(crate) fn admit(context: &ToolContext<'_>, arguments: &Value) -> Result<bool> {
    prepare(context, arguments)?;
    Ok(true)
}
pub(crate) fn admit_cached(
    context: &ToolContext<'_>,
    arguments: &Value,
    cache: &mut AdmissionCache,
) -> Result<()> {
    let key = (
        context.scope.actor.clone(),
        crate::digest(arguments.to_string().as_bytes()),
    );
    if !cache.contains(&key) {
        prepare_with_cache(context, arguments, cache, false)?;
        cache.insert(key);
    }
    Ok(())
}
pub fn execute(context: &ToolContext<'_>, arguments: &Value) -> Result<Value> {
    let PreparedInvocation {
        spec,
        source,
        input,
        input_sha256,
        recipe_sha256,
        lineage,
    } = prepare(context, arguments)?;
    let mut result = crate::component_api::invoke(
        &source,
        &spec.source_sha256,
        &spec.api_sha256,
        &spec.function,
        &input,
    )?;
    result["binding_id"] = json!(spec.id);
    result["source_path"] = json!(spec.source_path);
    result["input_sha256"] = json!(input_sha256);
    if let Some(recipe) = &recipe_sha256 {
        result["input_recipe_sha256"] = json!(recipe);
        result["input_lineage"] = lineage.clone();
    }
    result["owner_declared_review_package_id"] = json!(spec.owner_declared_review_package_id);
    crate::safety::require_persistable(&result.to_string())?;
    install(context.db)?;
    context.db.execute(
        "INSERT INTO owned_rust_call_results(id,run_id,policy_revision,actor,binding_id,source_sha256,api_sha256,input_sha256,arguments_sha256,result_sha256,passed,recipe_sha256,lineage_sha256) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            uuid::Uuid::new_v4().to_string(),
            context.scope.run.to_string(),
            context.policy.revision as i64,
            context.scope.actor,
            spec.id,
            spec.source_sha256,
            spec.api_sha256,
            input_sha256,
            crate::digest(&serde_json::to_vec(arguments)?),
            crate::digest(&serde_json::to_vec(&result)?),
            result["passed"] == true,
            recipe_sha256,
            if spec.input_recipe.is_some(){Some(crate::digest(lineage.to_string().as_bytes()))}else{None}
        ],
    )?;
    Ok(result)
}
pub fn completion(
    db: &Connection,
    authority: &dyn Authority,
    scope: &Scope,
    intake: &Value,
    revision: u64,
) -> Result<Vec<Value>> {
    let specifications = specs(intake)?;
    if specifications.is_empty() {
        return Ok(vec![]);
    }
    install(db)?;
    let work = workspace::open(&authority.workspace(scope)?, false);
    let mut checks = Vec::new();
    for spec in specifications {
        let current = work
            .as_ref()
            .ok()
            .and_then(|w| w.read_complete(&spec.source_path, 8192).ok());
        let current_matches = current
            .as_ref()
            .is_some_and(|s| s["sha256"] == spec.source_sha256);
        let api_matches = current
            .as_ref()
            .and_then(|source| source["text"].as_str())
            .and_then(|text| crate::component_api::describe(text).ok())
            .is_some_and(|description| description["api_sha256"] == spec.api_sha256);
        let resolved = if current_matches && api_matches && spec.input_recipe.is_some() {
            governance_resolution(db, authority, scope, intake, &spec, revision).ok()
        } else {
            None
        };
        let input_sha = spec.input_sha256.clone().or_else(|| {
            resolved
                .as_ref()
                .map(|r| crate::digest(r.input.to_string().as_bytes()))
        });
        let recipe_sha = resolved.as_ref().map(|r| r.recipe_sha256.clone());
        let lineage_sha = resolved
            .as_ref()
            .map(|r| crate::digest(r.lineage.to_string().as_bytes()));
        let count: i64 = if current_matches && api_matches && input_sha.is_some() {
            db.query_row("SELECT COUNT(*) FROM owned_rust_call_results r WHERE r.run_id=?1 AND r.policy_revision=?2 AND r.actor=?3 AND r.binding_id=?4 AND r.source_sha256=?5 AND r.api_sha256=?6 AND r.input_sha256=?7 AND r.recipe_sha256 IS ?8 AND r.lineage_sha256 IS ?9 AND r.passed=1 AND EXISTS(SELECT 1 FROM audit a WHERE a.run_id=r.run_id AND a.kind='mcp_tool_call' AND json_extract(a.body,'$.status')='completed' AND json_extract(a.body,'$.actor')=r.actor AND json_extract(a.body,'$.tool')='owned_rust_call' AND json_extract(a.body,'$.policy_revision')=r.policy_revision AND json_extract(a.body,'$.arguments_sha256')=r.arguments_sha256 AND json_extract(a.body,'$.result_sha256')=r.result_sha256)",params![scope.run.to_string(),revision as i64,spec.actor,spec.id,spec.source_sha256,spec.api_sha256,input_sha,recipe_sha,lineage_sha],|r|r.get(0))?
        } else {
            0
        };
        checks.push(json!({"binding_id":spec.id,"actor":spec.actor,"source_path":spec.source_path,"source_sha256":spec.source_sha256,"api_sha256":spec.api_sha256,"input_sha256":input_sha,"input_recipe_sha256":recipe_sha,"current_source_matches":current_matches,"current_api_matches":api_matches,"successful_current_epoch_invocations":count,"passed":count>0}));
    }
    Ok(checks)
}

fn governance_resolution(
    db: &Connection,
    authority: &dyn Authority,
    scope: &Scope,
    intake: &Value,
    spec: &Spec,
    revision: u64,
) -> Result<crate::owned_recipes::Resolved> {
    let (encoded, digest): (String, String) = db.query_row(
        "SELECT intake,digest FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        crate::digest(encoded.as_bytes()) == digest
            && serde_json::from_str::<Value>(&encoded)? == *intake,
        "owned recipe completion intake differs from frozen approval"
    );
    let policy = crate::governance::load(db, &scope.run.to_string())?
        .context("owned recipe policy missing")?;
    ensure!(
        policy.revision == revision,
        "owned recipe policy epoch changed"
    );
    let source_scope = Scope {
        actor: spec.actor.clone(),
        phase: crate::Phase::Worker,
        expected_revision: Some(revision),
        ..scope.clone()
    };
    let context = ToolContext {
        db,
        authority,
        scope: &source_scope,
        policy: &policy,
    };
    let source = source(&context, spec)?;
    let resolved =
        crate::owned_recipes::resolve(&context, intake, spec, &source, &mut AdmissionCache::new())?;
    policy.authorize(
        &spec.actor,
        "owned_rust_call",
        &json!({"binding_id":spec.id,"arguments":resolved.input}),
    )?;
    Ok(resolved)
}

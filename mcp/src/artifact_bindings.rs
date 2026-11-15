//! Owner-frozen native source -> exact artifact contracts. Only selected result
//! projections are retained, atomically with their canonical native receipts.
use crate::{
    authority::Authority,
    contracts::{EmbeddedContracts, ToolContracts},
    digest, governance, now_ms, Registry, Scope,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Clone, Deserialize, serde::Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArgumentSource {
    pub field: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub path: String,
    pub source_actor: String,
    pub writer: String,
    pub source_tool: String,
    pub arguments: Value,
    pub pointer: String,
    pub encoding: String,
    #[serde(default)]
    pub receipt_only: bool,
    #[serde(default)]
    pub argument_sources: Vec<ArgumentSource>,
}
pub fn is_placeholder(value: &Value) -> bool {
    value.is_null()
        || value.as_array().is_some_and(Vec::is_empty)
        || value.as_object().is_some_and(serde_json::Map::is_empty)
        || value.as_str().is_some_and(str::is_empty)
}
pub fn parse(intake: &Value) -> Result<Vec<Binding>> {
    let bindings: Vec<Binding> = serde_json::from_value(
        intake
            .get("artifact_bindings")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    ensure!(
        bindings.len() <= 32
            && bindings.iter().filter(|b| !b.receipt_only).count() <= 16
            && bindings.iter().filter(|b| b.receipt_only).count() <= 16,
        "at most sixteen published outputs and sixteen receipt-only bindings"
    );
    let mut paths = std::collections::BTreeSet::new();
    for (index, binding) in bindings.iter().enumerate() {
        ensure!(
            binding.receipt_only || !crate::owner_inputs::protected(intake, &binding.path),
            "artifact destination overlaps a frozen owner input"
        );
        ensure!(
            crate::workspace_path::is_safe_relative_path(&binding.path)
                && paths.insert(&binding.path),
            "invalid or duplicate bound artifact path"
        );
        for actor in [&binding.source_actor, &binding.writer] {
            ensure!(
                !actor.trim().is_empty()
                    && actor.len() <= 200
                    && !actor.chars().any(char::is_control),
                "invalid binding actor"
            );
        }
        ensure!(
            Registry::new().contains(&binding.source_tool)
                && binding.arguments.is_object()
                && binding.arguments.to_string().len() <= 16384,
            "invalid native binding source or arguments"
        );
        EmbeddedContracts.validate(&binding.source_tool, &binding.arguments)?;
        ensure!(
            !binding.receipt_only
                || binding.writer == binding.source_actor
                    && Registry::new().read_only_call(&binding.source_tool, &binding.arguments),
            "receipt-only bindings require a read-only source and no separate file writer"
        );
        ensure!(
            binding.argument_sources.len() <= 8,
            "at most eight native input references per source"
        );
        let mut fields = std::collections::BTreeSet::new();
        for input in &binding.argument_sources {
            ensure!(
                !input.field.is_empty()
                    && input.field.len() <= 40
                    && input
                        .field
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    && fields.insert((&input.field, input.argument_index))
                    && binding.arguments.get(&input.field).is_some(),
                "invalid or duplicate native input field"
            );
            if let Some(slot) = input.argument_index {
                ensure!(
                    binding.source_tool == "owned_rust_call" && input.field == "arguments",
                    "positional references require an owned component argument tuple"
                );
                ensure!(
                    input
                        .pointer
                        .as_ref()
                        .is_some_and(|p| p.len() <= 160 && !p.chars().any(char::is_control)),
                    "positional reference requires a bounded explicit projection"
                );
                ensure!(
                    binding.arguments["arguments"]
                        .as_array()
                        .and_then(|a| a.get(slot))
                        .is_some_and(is_placeholder),
                    "referenced positional argument must retain its empty placeholder"
                );
            } else {
                ensure!(
                    input.pointer.is_none(),
                    "nested projection requires a positional reference"
                );
                ensure!(
                    is_placeholder(&binding.arguments[&input.field]),
                    "derived input field must use an empty placeholder, never replace a literal"
                );
            }
            ensure!(
                bindings[..index]
                    .iter()
                    .any(|prior| prior.path == input.path),
                "native input dependency is missing, cyclic or forward"
            );
        }
        if !binding.argument_sources.is_empty() {
            ensure!(
                Registry::new().read_only_call(&binding.source_tool, &binding.arguments),
                "native input references require a read-only consumer"
            );
        }

        Registry::new()
            .validate_projection(
                &binding.source_tool,
                &binding.arguments,
                &binding.pointer,
                &binding.encoding,
            )
            .map_err(|e| anyhow::anyhow!("artifact binding {index}: {e}"))?;
        ensure!(
            binding.pointer.starts_with('/')
                && binding.pointer.len() <= 160
                && !binding.pointer.chars().any(char::is_control),
            "invalid binding projection pointer"
        );
        ensure!(
            matches!(
                binding.encoding.as_str(),
                "integer_text" | "utf8_text" | "json"
            ),
            "unsupported bound artifact encoding"
        );
    }
    crate::owned_recipes::validate_bindings(intake, &bindings)?;
    Ok(bindings)
}
fn intake(db: &Connection, scope: &Scope) -> Result<Value> {
    Ok(serde_json::from_str(&db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get::<_, String>(0),
    )?)?)
}
pub(crate) fn encoded(value: &Value, encoding: &str) -> Result<String> {
    let text = match encoding {
        "integer_text" => {
            ensure!(
                value.is_i64() || value.is_u64(),
                "binding source must be an integer, not a coercible string or boolean"
            );
            value.to_string()
        }
        "utf8_text" => value
            .as_str()
            .context("binding source must be UTF-8 text")?
            .to_string(),
        "json" => serde_json::to_string(value)?,
        _ => anyhow::bail!("unknown binding encoding"),
    };
    ensure!(
        text.len() <= 8192,
        "bound artifacts are limited to 8192 UTF-8 bytes"
    );
    crate::safety::require_persistable(&text)?;
    Ok(text)
}
pub fn capture(
    db: &Connection,
    scope: &Scope,
    tool: &str,
    arguments: &Value,
    result: &Value,
    revision: u64,
) -> Result<()> {
    if scope.phase != crate::Phase::Worker {
        return Ok(());
    }
    let argument_sha = digest(&serde_json::to_vec(arguments)?);
    let result_sha = digest(&serde_json::to_vec(result)?);
    let bindings = parse(&intake(db, scope)?)?;
    let policy =
        governance::load(db, &scope.run.to_string())?.context("missing native input policy")?;
    ensure!(
        policy.revision == revision,
        "native input policy epoch changed"
    );
    let mut cache = crate::bound_sources::Cache::new();
    for (index, binding) in bindings.iter().enumerate() {
        let resolved = if binding.source_actor == scope.actor && binding.source_tool == tool {
            crate::bound_sources::arguments(db, scope, &policy, &bindings, index, &mut cache)?
        } else {
            None
        };
        if resolved
            .as_ref()
            .is_some_and(|args| digest(args.to_string().as_bytes()) == argument_sha)
        {
            let value = result
                .pointer(&binding.pointer)
                .context("native source lacks the frozen projection pointer")?;
            encoded(value, &binding.encoding)?;
            let value_json = value.to_string();
            db.execute("INSERT INTO mcp_binding_sources_v1(run_id,policy_revision,path,actor,tool,arguments_sha256,result_sha256,pointer,value_json,value_sha256,created) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(run_id,policy_revision,path) DO UPDATE SET arguments_sha256=excluded.arguments_sha256,result_sha256=excluded.result_sha256,value_json=excluded.value_json,value_sha256=excluded.value_sha256,created=excluded.created",params![scope.run.to_string(),revision as i64,binding.path,scope.actor,tool,argument_sha,result_sha,binding.pointer,value_json,digest(value_json.as_bytes()),now_ms() as i64])?;
        }
        if !binding.receipt_only
            && tool == "files"
            && scope.actor == binding.writer
            && arguments["path"] == binding.path
        {
            let op = arguments["op"].as_str().unwrap_or("");
            let complete = op == "write"
                || (op == "read" && result["offset_bytes"] == 0 && result["truncated"] == false);
            if complete && matches!(op, "read" | "write") {
                let artifact_sha = result["sha256"]
                    .as_str()
                    .context("native file receipt lacks artifact hash")?;
                db.execute("INSERT INTO mcp_binding_artifacts_v1(run_id,policy_revision,path,actor,op,arguments_sha256,result_sha256,artifact_sha256,created) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(run_id,policy_revision,path,op) DO UPDATE SET arguments_sha256=excluded.arguments_sha256,result_sha256=excluded.result_sha256,artifact_sha256=excluded.artifact_sha256,created=excluded.created",params![scope.run.to_string(),revision as i64,binding.path,scope.actor,op,argument_sha,result_sha,artifact_sha,now_ms() as i64])?;
            }
        }
    }
    Ok(())
}
pub fn check(authority: &dyn Authority, scope: &Scope, final_stage: bool) -> Result<Value> {
    let db = authority.open(scope)?;
    let tx = db.unchecked_transaction()?;
    let policy = governance::load(&tx, &scope.run.to_string())?
        .context("artifact bindings require installed policy")?;
    ensure!(
        scope.expected_revision.is_none_or(|r| r == policy.revision),
        "binding source policy epoch changed"
    );
    let mut checks = Vec::new();
    let bindings = parse(&intake(&tx, scope)?)?;
    let mut cache = crate::bound_sources::Cache::new();
    for (index, binding) in bindings.iter().enumerate() {
        if !final_stage && scope.actor != binding.source_actor && scope.actor != binding.writer {
            continue;
        }
        let grant = policy
            .grants
            .get(&scope.actor)
            .context("binding recipient has no current grant")?;
        ensure!(
            scope.actor == binding.source_actor
                || grant.memory_from.contains(&binding.source_actor),
            "binding recipient cannot consume this source"
        );
        let Some(arguments) =
            crate::bound_sources::arguments(&tx, scope, &policy, &bindings, index, &mut cache)?
        else {
            checks.push(json!({"path":binding.path,"source_actor":binding.source_actor,"writer":binding.writer,"passed":false,"reason":"waiting for verified current-epoch native input dependencies"}));
            continue;
        };
        let Some(observed) =
            crate::bound_sources::observe(&tx, scope, &policy, &bindings, index, &mut cache)?
        else {
            checks.push(json!({"path":binding.path,"source_actor":binding.source_actor,"writer":binding.writer,"passed":false,"reason":"missing successful current-epoch native source with exact approved arguments","required_source_tool":binding.source_tool,"required_source_arguments":arguments}));
            continue;
        };
        let value = observed.value.to_string();
        let source_sha = observed.result_sha;
        if binding.receipt_only {
            checks.push(json!({"path":binding.path,"receipt_only":true,"source_actor":binding.source_actor,"passed":true,
                "phase":"verified native receipt; no file publication requested","source_result_sha256":source_sha,
                "source_projection":binding.pointer,"source_arguments_sha256":digest(arguments.to_string().as_bytes()),
                "native_input_paths":binding.argument_sources.iter().map(|r|&r.path).collect::<Vec<_>>()}));
            continue;
        }
        if !final_stage && scope.actor == binding.source_actor && scope.actor != binding.writer {
            checks.push(json!({"path":binding.path,"source_actor":binding.source_actor,"passed":true,"phase":"native_source_ready; downstream artifact is not yet required","source_result_sha256":source_sha,"source_arguments_sha256":digest(arguments.to_string().as_bytes()),"native_input_paths":binding.argument_sources.iter().map(|r|&r.path).collect::<Vec<_>>()}));
            continue;
        }
        policy.authorize(&binding.writer, "files", &json!({"op":"read"}))?;
        let expected = encoded(&serde_json::from_str::<Value>(&value)?, &binding.encoding)?;
        let workspace = crate::workspace::open(&authority.workspace(scope)?, false);
        let page = workspace.and_then(|w| w.read(&binding.path, 0, 8192)).ok();
        let expected_sha = digest(expected.as_bytes());
        let mut operations = Vec::new();
        for op in ["write", "read"] {
            let count:i64=tx.query_row("SELECT COUNT(*) FROM mcp_binding_artifacts_v1 r WHERE r.run_id=?1 AND r.policy_revision=?2 AND r.path=?3 AND r.actor=?4 AND r.op=?5 AND r.artifact_sha256=?6 AND EXISTS(SELECT 1 FROM audit a WHERE a.run_id=r.run_id AND a.kind='mcp_tool_call' AND json_extract(a.body,'$.status')='completed' AND json_extract(a.body,'$.phase')='worker' AND json_extract(a.body,'$.actor')=r.actor AND json_extract(a.body,'$.tool')='files' AND json_extract(a.body,'$.policy_revision')=r.policy_revision AND json_extract(a.body,'$.arguments_sha256')=r.arguments_sha256 AND json_extract(a.body,'$.result_sha256')=r.result_sha256)",params![scope.run.to_string(),policy.revision as i64,binding.path,binding.writer,op,expected_sha],|r|r.get(0))?;
            operations.push(json!({"operation":op,"passed":count>0}));
        }
        let passed = page
            .as_ref()
            .is_some_and(|v| v["text"] == expected && v["truncated"] == false)
            && operations.iter().all(|v| v["passed"] == true);
        let mut row = json!({"path":binding.path,"source_actor":binding.source_actor,"writer":binding.writer,"passed":passed,"source_result_sha256":source_sha,"writer_native_operations":operations,"expected_sha256":expected_sha,"observed_sha256":page.as_ref().map(|v|&v["sha256"])});
        row["source_projection"] = json!(binding.pointer);
        row["source_arguments_sha256"] = json!(digest(arguments.to_string().as_bytes()));
        row["native_input_paths"] = json!(binding
            .argument_sources
            .iter()
            .map(|r| &r.path)
            .collect::<Vec<_>>());
        if passed && expected.len() <= 128 {
            row["verified_native_value"] = serde_json::from_str::<Value>(&value)?;
        }
        if !passed {
            row["reason"] = json!("artifact bytes do not match the verified native projection");
            if expected.len() <= 128 {
                row["expected_from_native_source"] = json!(expected);
            }
        }
        checks.push(row);
    }
    tx.commit()?;
    Ok(
        json!({"passed":checks.iter().all(|c|c["passed"]==true),"checks":checks,"policy_revision":policy.revision,"source":"owner-frozen native projection plus canonical receipt; not model claims"}),
    )
}

/// Describe an already observed owner-bound pure computation. This neither
/// executes a call nor manufactures a new receipt. Mutable observations and
/// different actors/arguments/epochs never use this suppression hint.
pub fn observed_pure_call(
    authority: &dyn Authority,
    scope: &Scope,
    tool: &str,
    arguments: &Value,
) -> Result<Option<Value>> {
    if !Registry::new().deterministic(tool) {
        return Ok(None);
    }
    ensure!(
        scope.phase == crate::Phase::Worker,
        "source observation requires worker scope"
    );
    let db = authority.open(scope)?;
    let tx = db.unchecked_transaction()?;
    let policy =
        governance::load(&tx, &scope.run.to_string())?.context("binding policy missing")?;
    ensure!(
        scope.expected_revision.is_none_or(|r| r == policy.revision),
        "binding source policy epoch changed"
    );
    policy.authorize(&scope.actor, tool, arguments)?;
    let bindings = parse(&intake(&tx, scope)?)?;
    let mut cache = crate::bound_sources::Cache::new();
    let mut observed = None;
    let mut missing_projection = false;
    for (index, binding) in bindings.iter().enumerate() {
        if binding.source_actor != scope.actor || binding.source_tool != tool {
            continue;
        }
        if crate::bound_sources::arguments(&tx, scope, &policy, &bindings, index, &mut cache)?
            .as_ref()
            != Some(arguments)
        {
            continue;
        }
        if let Some(value) =
            crate::bound_sources::observe(&tx, scope, &policy, &bindings, index, &mut cache)?
        {
            observed = Some(
                json!({"source":"canonical current-epoch bound pure computation; not a new tool receipt","actor":scope.actor,"tool":tool,"arguments_sha256":digest(arguments.to_string().as_bytes()),"source_result_sha256":value.result_sha,"policy_revision":policy.revision}),
            );
        } else {
            // Another bound projection may have become eligible only after a
            // dependency arrived. It still requires a real fresh execution.
            missing_projection = true;
        }
    }
    tx.commit()?;
    Ok(if missing_projection { None } else { observed })
}

/// Compile an already approved source/data-copy request; never execute it here.
/// The outer controller still performs current-policy and independent admin
/// review, records intent, and refuses automatic repeated mutating requests.
pub fn next_call(
    authority: &dyn Authority,
    scope: &Scope,
    final_stage: bool,
) -> Result<Option<Value>> {
    let gate = check(authority, scope, final_stage)?;
    let db = authority.open(scope)?;
    let policy =
        governance::load(&db, &scope.run.to_string())?.context("binding policy missing")?;
    ensure!(
        gate["policy_revision"] == policy.revision,
        "binding policy changed before proposal compilation"
    );
    let bindings = parse(&intake(&db, scope)?)?;
    for row in gate["checks"]
        .as_array()
        .context("missing binding checks")?
    {
        if row["passed"] == true {
            continue;
        }
        let binding = bindings
            .iter()
            .find(|b| row["path"] == b.path)
            .context("binding check path missing")?;
        if row["required_source_arguments"].is_object() && scope.actor == binding.source_actor {
            policy.authorize(
                &scope.actor,
                &binding.source_tool,
                &row["required_source_arguments"],
            )?;
            return Ok(Some(
                json!({"action":binding.source_tool,"arguments":row["required_source_arguments"],"native_input_paths":binding.argument_sources.iter().map(|r|&r.path).collect::<Vec<_>>(),"summary":"Host-compiled owner-approved native source request; independent admin review still required"}),
            ));
        }
        if binding.receipt_only
            || scope.actor != binding.writer
            || !row["source_result_sha256"].is_string()
        {
            continue;
        }
        let value_json:String=db.query_row("SELECT value_json FROM mcp_binding_sources_v1 WHERE run_id=?1 AND policy_revision=?2 AND path=?3 AND result_sha256=?4",params![scope.run.to_string(),policy.revision as i64,binding.path,row["source_result_sha256"].as_str()],|r|r.get(0))?;
        let expected = encoded(
            &serde_json::from_str::<Value>(&value_json)?,
            &binding.encoding,
        )?;
        ensure!(
            row["expected_sha256"] == digest(expected.as_bytes()),
            "binding projection changed before proposal compilation"
        );
        for operation in row["writer_native_operations"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if operation["passed"] == true {
                continue;
            }
            let op = operation["operation"]
                .as_str()
                .context("missing writer operation")?;
            let arguments = if op == "write" {
                let mut args = json!({"op":"write","path":binding.path,"content":expected});
                if crate::contract_execution::enabled(&policy, scope)
                    && row["observed_sha256"].is_string()
                {
                    args["expected_sha256"] = row["observed_sha256"].clone();
                }
                args
            } else {
                json!({"op":"read","path":binding.path})
            };
            policy.authorize(&scope.actor, "files", &arguments)?;
            return Ok(Some(
                json!({"action":"files","arguments":arguments,"summary":"Host-compiled approved native source-to-artifact copy/readback; independent admin review still required"}),
            ));
        }
    }
    Ok(None)
}

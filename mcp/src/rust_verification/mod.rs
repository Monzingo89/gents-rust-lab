//! Source-bound, owner-frozen Rust acceptance. No source promotion or general shell.
mod diagnostics;
mod failure_context;
mod identity;
mod preflight;
mod progress;
pub(crate) mod sandbox;
mod source_policy;
use crate::{authority::Authority, workspace, Scope};
use anyhow::{ensure, Context, Result};
pub use identity::request_identity;
pub use preflight::{acceptance_policy_observation, source_policy_observation};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub actor: String,
    pub source_path: String,
    pub tests_path: String,
    pub tests_sha256: String,
}
fn specs(intake: &Value) -> Result<Vec<Spec>> {
    let specs: Vec<Spec> = serde_json::from_value(
        intake
            .get("required_rust_checks")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    ensure!(specs.len() <= 4, "at most four frozen Rust checks");
    let mut paths = std::collections::BTreeSet::new();
    for spec in &specs {
        ensure!(
            !spec.actor.trim().is_empty()
                && spec.actor.len() <= 200
                && !spec.actor.chars().any(char::is_control)
                && [&spec.source_path, &spec.tests_path]
                    .iter()
                    .all(|p| p.len() <= 120
                        && p.ends_with(".rs")
                        && crate::workspace_path::is_safe_relative_path(p))
                && spec.source_path != spec.tests_path
                && paths.insert(&spec.source_path)
                && spec.tests_sha256.len() == 64
                && spec
                    .tests_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid frozen Rust check"
        );
    }
    ensure!(
        specs.iter().all(|spec| !paths.contains(&spec.tests_path)),
        "Rust test paths cannot also be candidate source paths"
    );
    Ok(specs)
}
pub fn validate(intake: &Value) -> Result<()> {
    specs(intake).map(|_| ())
}
fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS rust_check_results(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,policy_revision INTEGER NOT NULL,actor TEXT NOT NULL,source_path TEXT NOT NULL,source_sha256 TEXT NOT NULL,tests_sha256 TEXT NOT NULL,passed INTEGER NOT NULL,result TEXT NOT NULL);")?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS rust_source_readbacks(run_id TEXT NOT NULL,policy_revision INTEGER NOT NULL,actor TEXT NOT NULL,path TEXT NOT NULL,source_sha256 TEXT NOT NULL,arguments_sha256 TEXT NOT NULL,result_sha256 TEXT NOT NULL,PRIMARY KEY(run_id,policy_revision,actor,path));")?;
    Ok(())
}
/// Capture a full actual source read atomically with its canonical tool receipt.
pub fn capture_readback(
    db: &Connection,
    scope: &Scope,
    name: &str,
    args: &Value,
    result: &Value,
    revision: u64,
) -> Result<()> {
    if name != "files"
        || args["op"] != "read"
        || scope.phase != crate::Phase::Worker
        || result["offset_bytes"] != 0
        || result["truncated"] != false
    {
        return Ok(());
    }
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    if specs(&intake)?
        .iter()
        .any(|spec| spec.actor == scope.actor && args["path"] == spec.source_path)
    {
        install(db)?;
        db.execute("INSERT INTO rust_source_readbacks VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(run_id,policy_revision,actor,path) DO UPDATE SET source_sha256=excluded.source_sha256,arguments_sha256=excluded.arguments_sha256,result_sha256=excluded.result_sha256",params![scope.run.to_string(),revision as i64,scope.actor,args["path"].as_str(),result["sha256"].as_str(),crate::digest(&serde_json::to_vec(args)?),crate::digest(&serde_json::to_vec(result)?)])?;
    }
    Ok(())
}
pub fn execute(context: &crate::ToolContext<'_>, path: &str) -> Result<Value> {
    let encoded: String = context.db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let specs = specs(&intake)?;
    let spec = specs
        .iter()
        .find(|spec| spec.source_path == path && spec.actor == context.scope.actor)
        .context(
            "Rust verification requires the exact owner-frozen source path and assigned actor",
        )?;
    let root = context.authority.workspace(context.scope)?;
    let work = workspace::open(&root, false)?;
    let source = work.read_complete(path, 8192)?;
    let tests = work.read_complete(&spec.tests_path, 8192)?;
    ensure!(
        tests["sha256"] == spec.tests_sha256,
        "acceptance bytes differ from frozen tests_sha256"
    );
    let source_text = source["text"]
        .as_str()
        .context("Rust source text missing")?;
    let tests_text = tests["text"].as_str().context("acceptance text missing")?;
    let mut result = verify_snapshot(
        source_text,
        source["sha256"].as_str().context("source hash missing")?,
        tests_text,
        &spec.tests_sha256,
    )?;
    result["source_path"] = json!(path);
    if context
        .policy
        .authorize(
            &context.scope.actor,
            "files",
            &json!({"op":"read","path":spec.tests_path}),
        )
        .is_ok()
    {
        if let Some(frame) = failure_context::extract(&result, tests_text, &spec.tests_path) {
            result["failure_context"] = frame;
        }
    }
    crate::safety::require_persistable(&result.to_string())?;
    install(context.db)?;
    context.db.execute(
        "INSERT INTO rust_check_results VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            uuid::Uuid::new_v4().to_string(),
            context.scope.run.to_string(),
            context.policy.revision as i64,
            context.scope.actor,
            path,
            source["sha256"].as_str(),
            tests["sha256"].as_str(),
            result["passed"] == true,
            result.to_string()
        ],
    )?;
    Ok(result)
}

/// Private trusted-host snapshot evaluation, also used by the governed tool.
/// Receives bytes and identities, never paths or authority. A returned PASS is
/// scoped to this frozen suite; it is not a native run receipt or promotion.
pub fn verify_snapshot(
    source: &str,
    source_sha256: &str,
    tests: &str,
    tests_sha256: &str,
) -> Result<Value> {
    ensure!(
        crate::digest(source.as_bytes()) == source_sha256
            && crate::digest(tests.as_bytes()) == tests_sha256,
        "snapshot source or acceptance identity mismatch"
    );
    crate::safety::require_persistable(source)?;
    crate::safety::require_persistable(tests)?;
    let outcome = (|| -> Result<Value> {
        source_policy::validate(source, false)?;
        let count = source_policy::validate(tests, true)?;
        sandbox::verify(source, tests, count)
    })();
    let mut result = match outcome {
        Ok(value) => value,
        Err(error) => json!({"passed":false,"stage":"admission","diagnostics":error.to_string()}),
    };
    result["source_sha256"] = json!(source_sha256);
    result["tests_sha256"] = json!(tests_sha256);
    result["provider_cost_usd"] = json!(0);
    result["promotion_authorized"] = json!(false);
    crate::safety::require_persistable(&result.to_string())?;
    Ok(result)
}
pub fn completion(
    db: &Connection,
    authority: &dyn Authority,
    scope: &Scope,
    intake: &Value,
    revision: u64,
) -> Result<Vec<Value>> {
    let specs = specs(intake)?;
    if specs.is_empty() {
        return Ok(vec![]);
    }
    install(db)?;
    let root = authority.workspace(scope)?;
    let work = workspace::open(&root, false);
    let mut checks = vec![];
    for spec in specs {
        let current = work
            .as_ref()
            .ok()
            .and_then(|w| w.read_complete(&spec.source_path, 8192).ok());
        let tests = work
            .as_ref()
            .ok()
            .and_then(|w| w.read_complete(&spec.tests_path, 8192).ok());
        let mut count = 0_i64;
        let mut readbacks = 0_i64;
        let mut latest_check = Value::Null;
        if let (Some(source), Some(tests)) = (&current, &tests) {
            if tests["sha256"] == spec.tests_sha256 {
                let latest:Option<(bool,String)>=db.query_row("SELECT passed,result FROM rust_check_results WHERE run_id=?1 AND policy_revision=?2 AND actor=?3 AND source_path=?4 AND source_sha256=?5 AND tests_sha256=?6 ORDER BY rowid DESC LIMIT 1",params![scope.run.to_string(),revision as i64,spec.actor,spec.source_path,source["sha256"].as_str(),spec.tests_sha256],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
                if let Some((passed, result)) = latest {
                    let receipts:i64=db.query_row("SELECT COUNT(*) FROM audit WHERE run_id=?1 AND kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.tool')='rust_verify' AND json_extract(body,'$.policy_revision')=?3 AND json_extract(body,'$.result_sha256')=?4",params![scope.run.to_string(),spec.actor,revision as i64,crate::digest(result.as_bytes())],|r|r.get(0))?;
                    if receipts > 0 {
                        let value: Value = serde_json::from_str(&result)?;
                        let frame = value.get("failure_context").cloned();
                        let frame_chars = frame
                            .as_ref()
                            .map_or(0, |frame| frame.to_string().chars().count());
                        latest_check = json!({"passed":passed,"stage":value["stage"],"diagnostics_excerpt":diagnostics::excerpt(&value["diagnostics"]).chars().take(900_usize.saturating_sub(frame_chars)).collect::<String>(),"canonical_receipt_verified":true});
                        if let Some(frame) = frame {
                            latest_check["failure_context"] = frame;
                        }
                        if passed {
                            count = receipts;
                        }
                    }
                }
                readbacks = db.query_row("SELECT COUNT(*) FROM rust_source_readbacks r WHERE r.run_id=?1 AND r.policy_revision=?2 AND r.actor=?3 AND r.path=?4 AND r.source_sha256=?5 AND EXISTS(SELECT 1 FROM audit a WHERE a.run_id=r.run_id AND a.kind='mcp_tool_call' AND json_extract(a.body,'$.status')='completed' AND json_extract(a.body,'$.phase')='worker' AND json_extract(a.body,'$.actor')=r.actor AND json_extract(a.body,'$.tool')='files' AND json_extract(a.body,'$.policy_revision')=r.policy_revision AND json_extract(a.body,'$.arguments_sha256')=r.arguments_sha256 AND json_extract(a.body,'$.result_sha256')=r.result_sha256)",params![scope.run.to_string(),revision as i64,spec.actor,spec.source_path,source["sha256"].as_str()],|r|r.get(0))?;
            }
        }
        let progress = progress::view(
            db,
            scope,
            &spec,
            revision,
            current
                .as_ref()
                .and_then(|source| source["sha256"].as_str()),
        )?;
        checks.push(json!({"actor":spec.actor,"source_path":spec.source_path,"tests_sha256":spec.tests_sha256,"current_source_sha256":current.as_ref().map(|v|&v["sha256"]),"successful_current_source_checks":count,"successful_current_source_readbacks":readbacks,"latest_check":latest_check,"execution_progress":progress,"passed":count>0 && readbacks>0,"source":"native source-bound isolated Rust acceptance and full source readback; not model prose"}));
    }
    Ok(checks)
}

/// Trusted host configuration, never an MCP argument. Only this executable path
/// crosses the cleared child environment; no HOME, credentials or provider vars.
pub fn compiler_path() -> Result<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("GENTS_RUSTC") {
        let path = std::path::PathBuf::from(path);
        ensure!(path.is_absolute(), "GENTS_RUSTC must be absolute");
        return Ok(path);
    }
    #[cfg(target_os = "macos")]
    {
        Ok(std::path::PathBuf::from(
            std::env::var_os("HOME").context("installed Rust compiler path is not configured")?,
        )
        .join(format!(
            ".rustup/toolchains/stable-{}-apple-darwin/bin/rustc",
            std::env::consts::ARCH
        )))
    }
    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("isolated Rust verifier requires a supported OS adapter")
    }
}

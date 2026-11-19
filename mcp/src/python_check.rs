//! Owner-scoped Python test diagnostics. A Python zero exit is never acceptance.
use crate::{authority::Authority, governance, workspace, Scope, ToolContext};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};

const HELPER: &str = include_str!("../../scripts/python_snapshot_check.py");
const MAX_BYTES: usize = 256_000;

#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    id: String,
    actor: String,
    source_paths: Vec<String>,
    tests_path: String,
    tests_sha256: String,
    interpreter_sha256: String,
    #[serde(default)]
    frozen_inputs: BTreeMap<String, String>,
    #[serde(default)]
    scratch_bytes: u64,
}
fn hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn specs(intake: &Value) -> Result<Vec<Spec>> {
    let specs: Vec<Spec> = serde_json::from_value(
        intake
            .get("python_check_contracts")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    ensure!(specs.len() <= 4, "at most four Python diagnostic contracts");
    let mut ids = std::collections::BTreeSet::new();
    let mut sources = std::collections::BTreeSet::new();
    let mut tests = std::collections::BTreeSet::new();
    for spec in &specs {
        ensure!(
            !spec.id.is_empty()
                && spec.id.len() <= 80
                && spec
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && ids.insert(&spec.id)
                && !spec.actor.trim().is_empty()
                && spec.actor.len() <= 200
                && !spec.actor.chars().any(char::is_control)
                && hash(&spec.tests_sha256)
                && hash(&spec.interpreter_sha256)
                && !spec.source_paths.is_empty()
                && spec.source_paths.len() <= 31,
            "invalid owner Python diagnostic contract"
        );
        ensure!(
            spec.scratch_bytes <= 8_388_608
                && spec
                    .frozen_inputs
                    .iter()
                    .all(|(path, digest)| spec.source_paths.contains(path) && hash(digest)),
            "invalid frozen Python dependencies or scratch ceiling"
        );
        let mut paths = std::collections::BTreeSet::new();
        for path in spec
            .source_paths
            .iter()
            .chain(std::iter::once(&spec.tests_path))
        {
            ensure!(
                path.len() <= 120
                    && path.ends_with(".py")
                    && crate::workspace_path::is_safe_relative_path(path)
                    && paths.insert(path),
                "invalid or duplicate Python diagnostic input"
            );
        }
        ensure!(
            !spec.tests_path.contains('/') && !spec.tests_path.contains(['*', '?', '[', ']']),
            "Python diagnostic suite must be one literal top-level filename"
        );
        sources.extend(spec.source_paths.iter());
        tests.insert(&spec.tests_path);
    }
    ensure!(
        sources.is_disjoint(&tests),
        "frozen Python suites cannot be candidate inputs"
    );
    Ok(specs)
}
pub fn validate(intake: &Value) -> Result<()> {
    specs(intake).map(|_| ())
}
pub(crate) fn source_for_actor(intake: &Value, actor: &str, path: &str) -> Result<bool> {
    Ok(specs(intake)?
        .iter()
        .any(|spec| spec.actor == actor && spec.source_paths.iter().any(|p| p == path)))
}
fn install(db: &rusqlite::Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS python_diagnostic_observations(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,actor TEXT NOT NULL,policy_revision INTEGER NOT NULL,contract_id TEXT NOT NULL,verification_identity TEXT NOT NULL,result TEXT NOT NULL);")?;
    Ok(())
}
/// Trusted launching host only, forwarded explicitly across the cleared MCP env.
pub fn interpreter_path() -> Result<PathBuf> {
    let path = PathBuf::from(std::env::var_os("GENTS_PYTHON").context(
        "Python diagnostics require owner-configured absolute GENTS_PYTHON; no downloads or fallback",
    )?);
    ensure!(path.is_absolute(), "GENTS_PYTHON must be absolute");
    let path = path.canonicalize()?;
    ensure!(path.is_file(), "installed Python interpreter unavailable");
    Ok(path)
}
fn interpreter_hash(path: &std::path::Path) -> Result<String> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut bytes = vec![];
    file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= 64 * 1024 * 1024,
        "Python interpreter exceeds identity budget"
    );
    Ok(crate::digest(&bytes))
}
struct Prepared {
    files: BTreeMap<String, String>,
    hashes: BTreeMap<String, String>,
    spec: Spec,
    interpreter: PathBuf,
    identity: String,
}
fn prepare(context: &ToolContext<'_>, id: &str) -> Result<Prepared> {
    ensure!(
        context.scope.phase == crate::Phase::Worker,
        "Python diagnostics are worker-scoped"
    );
    context.policy.authorize(
        &context.scope.actor,
        "python_check",
        &json!({"contract_id":id}),
    )?;
    let encoded: String = context.db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| r.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let spec = specs(&intake)?
        .into_iter()
        .find(|s| s.id == id && s.actor == context.scope.actor)
        .context("Python diagnostics require an exact owner-frozen contract and actor")?;
    let interpreter = interpreter_path()?;
    ensure!(
        interpreter_hash(&interpreter)? == spec.interpreter_sha256,
        "Python interpreter differs from owner-frozen hash"
    );
    let root = context.authority.workspace(context.scope)?;
    let work = workspace::open(&root, false)?;
    let mut files = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut total = 0;
    for path in spec
        .source_paths
        .iter()
        .chain(std::iter::once(&spec.tests_path))
    {
        context.policy.authorize(
            &context.scope.actor,
            "files",
            &json!({"op":"read","path":path}),
        )?;
        let bytes = work.read_bytes_complete(path, MAX_BYTES)?;
        let content =
            std::str::from_utf8(&bytes).context("Python diagnostic input must be UTF-8")?;
        crate::safety::require_persistable(content)?;
        total += content.len();
        ensure!(total <= MAX_BYTES, "Python snapshot byte limit exceeded");
        files.insert(path.clone(), content.to_owned());
        hashes.insert(path.clone(), crate::digest(content.as_bytes()));
    }
    ensure!(
        hashes.get(&spec.tests_path) == Some(&spec.tests_sha256),
        "frozen Python suite hash mismatch"
    );
    ensure!(
        spec.frozen_inputs
            .iter()
            .all(|(path, expected)| hashes.get(path) == Some(expected)),
        "frozen Python dependency hash mismatch"
    );
    let identity = crate::digest(json!({"run":context.scope.run,"actor":context.scope.actor,
        "policy_revision":context.policy.revision,"contract":spec,"inputs":hashes,
        "implementation":crate::digest(concat!(include_str!("python_check.rs"),
            include_str!("functions/python_check.rs"), include_str!("python_custody.rs"), include_str!("../../scripts/python_snapshot_check.py")).as_bytes())
    }).to_string().as_bytes());
    Ok(Prepared {
        files,
        hashes,
        spec,
        interpreter,
        identity,
    })
}
pub fn request_identity(authority: &dyn Authority, scope: &Scope, id: &str) -> Result<String> {
    let db = authority.open(scope)?;
    let policy =
        governance::load(&db, &scope.run.to_string())?.context("Python diagnostics need policy")?;
    ensure!(
        scope.expected_revision.is_none_or(|r| r == policy.revision),
        "Python diagnostic policy changed"
    );
    Ok(prepare(
        &ToolContext {
            authority,
            scope,
            policy: &policy,
            db: &db,
        },
        id,
    )?
    .identity)
}
/// Same current-source/program identity for paged source reads. Returns an
/// error outside an explicitly frozen Python source/actor contract.
pub fn source_read_identity(
    authority: &dyn Authority,
    scope: &Scope,
    path: &str,
) -> Result<String> {
    let db = authority.open(scope)?;
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |row| row.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let spec = specs(&intake)?
        .into_iter()
        .find(|s| s.actor == scope.actor && s.source_paths.iter().any(|p| p == path))
        .context("Source read identity requires a frozen Python input and actor")?;
    request_identity(authority, scope, &spec.id)
}
pub fn execute(context: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let prepared = prepare(
        context,
        args["contract_id"]
            .as_str()
            .context("contract_id required")?,
    )?;
    ensure!(
        args["expected_source_identity"] == prepared.identity,
        "Python diagnostic source identity changed or was not bound by host"
    );
    let request = json!({"files":prepared.files,"suite":prepared.spec.tests_path,
        "suite_sha256":prepared.spec.tests_sha256,"scratch_bytes":prepared.spec.scratch_bytes});
    let mut result = run(&prepared.interpreter, &request)?;
    ensure!(
        result["inputs_sha256"] == json!(prepared.hashes)
            && result["suite_sha256"] == prepared.spec.tests_sha256
            && result["interpreter_sha256"] == prepared.spec.interpreter_sha256
            && matches!(
                result["status"].as_str(),
                Some("reported_pass" | "reported_fail" | "indeterminate")
            )
            && result["qualified"] == false
            && result["promoted"] == false
            && result["authority_granted"] == false
            && result["model_calls"] == 0
            && result["provider_cost_usd"] == 0
            && result["scratch_limit_bytes"] == prepared.spec.scratch_bytes,
        "invalid Python diagnostic observation"
    );
    result["verification_identity"] = json!(prepared.identity);
    result["contract_id"] = json!(prepared.spec.id);
    result["scope"] = json!("Native owner-scoped Python test DIAGNOSTICS only. Copied sources stay read-only; only owner-frozen scratch I/O is admitted. No run-workspace writes, shell, network, subprocesses, downloads, result reuse or promotion. A reported zero exit can be spoofed by Python code and is NOT independent acceptance or company completion. Runtime library closure is not content-addressed.");
    crate::safety::require_persistable(&result.to_string())?;
    install(context.db)?;
    context.db.execute(
        "INSERT INTO python_diagnostic_observations VALUES(?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            context.scope.run.to_string(),
            context.scope.actor,
            context.policy.revision as i64,
            prepared.spec.id,
            prepared.identity,
            result.to_string()
        ],
    )?;
    Ok(result)
}

/// Bounded current-source facts for worker planning. A historical PASS cannot
/// survive changed source, policy or implementation. Never re-executes tests,
/// grants access, clears retry history or establishes independent acceptance.
pub fn observations(authority: &dyn Authority, scope: &Scope) -> Result<Vec<Value>> {
    use rusqlite::OptionalExtension;
    let db = authority.open(scope)?;
    let tx = db.unchecked_transaction()?;
    let encoded: String = tx.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |row| row.get(0),
    )?;
    let intake: Value = serde_json::from_str(&encoded)?;
    let contracts: Vec<Spec> = specs(&intake)?
        .into_iter()
        .filter(|s| s.actor == scope.actor)
        .collect();
    if contracts.is_empty() {
        return Ok(vec![]);
    }
    let policy = governance::load(&tx, &scope.run.to_string())?
        .context("Python observations require policy")?;
    ensure!(
        scope.expected_revision.is_none_or(|r| r == policy.revision),
        "Python observation epoch changed"
    );
    let context = ToolContext {
        authority,
        scope,
        policy: &policy,
        db: &tx,
    };
    install(&tx)?;
    let mut views = vec![];
    for spec in contracts {
        let Ok(prepared) = prepare(&context, &spec.id) else {
            views.push(
                json!({"contract_id":spec.id,"current_source_available":false,"policy_revision":policy.revision,
                "latest_current_diagnostic":null,"independent_acceptance":false,
                "authority_granted":false}),
            );
            continue;
        };
        let encoded:Option<String>=tx.query_row("SELECT result FROM python_diagnostic_observations WHERE run_id=?1 AND actor=?2 AND policy_revision=?3 AND contract_id=?4 AND verification_identity=?5 ORDER BY rowid DESC LIMIT 1",
            rusqlite::params![scope.run.to_string(),scope.actor,policy.revision as i64,spec.id,prepared.identity],|row|row.get(0)).optional()?;
        let mut diagnostic = Value::Null;
        let arguments_sha = crate::digest(
            json!({"contract_id":spec.id,
            "expected_source_identity":prepared.identity})
            .to_string()
            .as_bytes(),
        );
        let attempt:Option<String>=tx.query_row("SELECT body FROM audit WHERE run_id=?1 AND kind IN ('mcp_tool_call','mcp_tool_effect_uncertain','mcp_tool_rejected') AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.phase')='worker' AND json_extract(body,'$.tool')='python_check' AND json_extract(body,'$.arguments_sha256')=?3 ORDER BY seq DESC LIMIT 1",
            rusqlite::params![scope.run.to_string(),scope.actor,arguments_sha],|row|row.get(0)).optional()?;
        let attempt: Value = attempt
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or(Value::Null);
        if let Some(encoded) = encoded {
            let value: Value = serde_json::from_str(&encoded)?;
            if attempt["status"] == "completed"
                && attempt["policy_revision"] == policy.revision
                && attempt["result_sha256"] == crate::digest(encoded.as_bytes())
                && value["verification_identity"] == prepared.identity
                && value["inputs_sha256"] == json!(prepared.hashes)
            {
                diagnostic = json!({"status":value["status"],"canonical_receipt_verified":true,
                    "stderr_excerpt":value["stderr"].as_str().unwrap_or("").chars().take(600).collect::<String>()});
            }
        }
        let mut custody = std::collections::BTreeMap::new();
        for path in &prepared.spec.source_paths {
            custody.insert(
                path.clone(),
                crate::python_custody::check(
                    &tx,
                    scope,
                    policy.revision,
                    path,
                    &prepared.files[path],
                )?,
            );
        }
        views.push(json!({"contract_id":spec.id,"current_source_available":true,"policy_revision":policy.revision,
            "current_source_identity":prepared.identity,"latest_current_diagnostic":diagnostic,
            "latest_attempt_uncertain":attempt["status"]=="effect_uncertain",
            "current_source_custody":custody,
            "independent_acceptance":false,"authority_granted":false,
            "scope":"Current-source diagnostic observation only. A source change invalidates older test results. Do not rewrite code merely because independent acceptance belongs to another reviewer. Continue only unmet frozen criteria; a review-stage submission is not self-approval."}));
    }
    tx.commit()?;
    Ok(views)
}
#[cfg(not(target_os = "macos"))]
fn run(_: &std::path::Path, _: &Value) -> Result<Value> {
    let _ = HELPER;
    anyhow::bail!("Python diagnostics require macOS sandbox-exec; no unsandboxed fallback")
}
#[cfg(target_os = "macos")]
fn run(interpreter: &std::path::Path, request: &Value) -> Result<Value> {
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    // The trusted helper is embedded in the compiled MCP binary. Candidate
    // Python enters stdin as JSON data; never interpolate it into -c code.
    let child = Command::new(interpreter)
        .args(["-I", "-S", "-B", "-c", HELPER])
        .env_clear()
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    struct Owned(std::process::Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = Owned(child);
    child
        .0
        .stdin
        .take()
        .context("Python helper stdin unavailable")?
        .write_all(&serde_json::to_vec(request)?)?;
    let stdout = child
        .0
        .stdout
        .take()
        .context("Python helper stdout unavailable")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout.take(196_609).read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        ensure!(
            started.elapsed() < Duration::from_secs(9),
            "Python helper exceeded wall budget; inspect intent before retry"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Python helper reader failed"))??;
    ensure!(
        status.success() && bytes.len() <= 196_608,
        "Python helper failed or exceeded output budget"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

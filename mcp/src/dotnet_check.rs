//! Source-bound offline compilation and separately granted finite HTTP checks.
//! Neither mode establishes general correctness or independent acceptance.
pub mod api;
pub(crate) mod sandbox;
use crate::{authority::Authority, governance, workspace, Scope, ToolContext};
use anyhow::{ensure, Context, Result};
use quick_xml::{events::Event, Reader};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Build,
    Api,
}
impl Mode {
    pub(crate) fn tool(self) -> &'static str {
        if self == Self::Api {
            "dotnet_api_check"
        } else {
            "dotnet_check"
        }
    }
    pub(crate) fn validator(self) -> &'static str {
        if self == Self::Api {
            "dotnet_api"
        } else {
            "dotnet_build"
        }
    }
    pub(crate) fn from_tool(tool: &str) -> Result<Self> {
        match tool {
            "dotnet_check" => Ok(Self::Build),
            "dotnet_api_check" => Ok(Self::Api),
            _ => anyhow::bail!("unknown SDK execution mode"),
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    id: String,
    actor: String,
    project_path: String,
    source_paths: Vec<String>,
    sdk_version: String,
    dotnet_sha256: String,
    artifact_mode: String,
    frozen_inputs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api: Option<api::Contract>,
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
        && !value.starts_with('.')
}
fn specs(intake: &Value) -> Result<Vec<Spec>> {
    let contracts: Vec<Spec> = serde_json::from_value(
        intake
            .get("dotnet_check_contracts")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    ensure!(contracts.len() <= 4, "at most four .NET build contracts");
    let mut ids = BTreeSet::new();
    for spec in &contracts {
        if let Some(api) = &spec.api {
            api.validate()?;
        }
        ensure!(
            component(&spec.id)
                && ids.insert(&spec.id)
                && !spec.actor.trim().is_empty()
                && spec.actor.len() <= 200
                && !spec.actor.chars().any(char::is_control),
            "invalid .NET contract identity"
        );
        ensure!(
            hash(&spec.dotnet_sha256) && spec.artifact_mode == "managed_dll",
            ".NET contract requires a frozen executable hash and explicit managed_dll mode"
        );
        ensure!(
            spec.sdk_version.len() <= 20
                && spec.sdk_version.split('.').count() == 3
                && spec
                    .sdk_version
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())),
            "exact stable installed SDK version required"
        );
        ensure!(
            !spec.source_paths.is_empty()
                && spec.source_paths.len() <= 63
                && spec.project_path.ends_with(".csproj"),
            "explicit project and 1..63 C# source files required"
        );
        let mut paths = BTreeSet::new();
        let base = Path::new(&spec.project_path)
            .parent()
            .context("project parent unavailable")?;
        for path in std::iter::once(&spec.project_path).chain(&spec.source_paths) {
            ensure!(
                path.len() <= 120
                    && crate::workspace_path::is_safe_relative_path(path)
                    && paths.insert(path)
                    && Path::new(path)
                        .components()
                        .all(|c| c.as_os_str().to_str().is_some_and(component)),
                "unsafe, hidden or duplicate .NET snapshot path"
            );
            ensure!(
                Path::new(path).strip_prefix(base).is_ok()
                    && !Path::new(path)
                        .components()
                        .any(|c| matches!(c.as_os_str().to_str(), Some("bin" | "obj"))),
                ".NET sources must be inside the project subtree and outside bin/obj"
            );
        }
        ensure!(
            spec.source_paths.iter().all(|p| p.ends_with(".cs")),
            "only explicit .cs source inputs supported"
        );
        ensure!(spec.frozen_inputs.contains_key(&spec.project_path) && spec.frozen_inputs.iter().all(|(path,digest)| paths.contains(path) && hash(digest)), "project definition must be owner-frozen; dependencies must be explicit hash-frozen inputs");
    }
    Ok(contracts)
}
pub fn validate(intake: &Value) -> Result<()> {
    specs(intake).map(|_| ())
}
pub fn has_contract(intake: &Value, actor: &str) -> Result<bool> {
    Ok(specs(intake)?.iter().any(|s| s.actor == actor))
}
/// Trusted async admission only; the SDK path comes from the launching host.
pub fn bindings(context: &ToolContext<'_>) -> Result<Vec<crate::task_engine::validators::Binding>> {
    runtime_bindings(context, &BTreeSet::from(["dotnet_build".into()]))
}
pub fn runtime_bindings(
    context: &ToolContext<'_>,
    wanted: &BTreeSet<String>,
) -> Result<Vec<crate::task_engine::validators::Binding>> {
    let encoded: String = context.db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| r.get(0),
    )?;
    let mut bindings = vec![];
    for spec in specs(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .filter(|s| s.actor == context.scope.actor)
    {
        for mode in [Mode::Build, Mode::Api] {
            if !wanted.contains(mode.validator())
                || mode == Mode::Api
                    && (spec.api.is_none()
                        || context
                            .policy
                            .authorize(&context.scope.actor, mode.tool(), &json!({}))
                            .is_err())
            {
                continue;
            }
            let prepared = prepare_mode(context, &spec.id, mode)?;
            bindings.push(crate::task_engine::validators::Binding {
                validator: mode.validator().into(),
                contract_id: spec.id.clone(),
                source_identity: prepared.identity,
            });
        }
    }
    Ok(bindings)
}

/// Recheck actual source/SDK/owner custody and the latest canonical tool audit.
/// This is callable under an existing native transaction; it never reopens the
/// database, dispatches SDK work or trusts a model-provided receipt.
pub fn reference_current(
    context: &ToolContext<'_>,
    reference: &crate::task_engine::validators::NativeReceipt,
) -> Result<bool> {
    use crate::task_engine::contracts::{CheckStatus, VERSION};
    use rusqlite::OptionalExtension;
    let mode = Mode::from_tool(&reference.tool)?;
    ensure!(
        reference.schema_version == VERSION
            && (reference.validator == mode.validator()
                || mode == Mode::Api && reference.validator == "dotnet_build"),
        "unregistered native validation receipt"
    );
    let current = prepare_mode(context, &reference.contract_id, mode)?;
    if reference.api_contract_sha256
        != current
            .spec
            .api
            .as_ref()
            .filter(|_| mode == Mode::Api)
            .map(|api| crate::digest(&serde_json::to_vec(api).unwrap()))
    {
        return Ok(false);
    }

    if current.identity != reference.source_identity
        || crate::digest(&serde_json::to_vec(&current.hashes)?) != reference.input_snapshot_sha256
        || reference.policy_revision != context.policy.revision
    {
        return Ok(false);
    }
    let args = json!({"contract_id":reference.contract_id,"expected_source_identity":reference.source_identity});
    if crate::digest(args.to_string().as_bytes()) != reference.arguments_sha256 {
        return Ok(false);
    }
    let result:Option<String>=context.db.query_row("SELECT result FROM dotnet_build_observations WHERE run_id=?1 AND actor=?2 AND policy_revision=?3 AND contract_id=?4 AND verification_identity=?5 ORDER BY rowid DESC LIMIT 1",rusqlite::params![context.scope.run.to_string(),context.scope.actor,context.policy.revision as i64,reference.contract_id,reference.source_identity],|r|r.get(0)).optional()?;
    let audit:Option<String>=context.db.query_row("SELECT body FROM audit WHERE run_id=?1 AND kind IN ('mcp_tool_call','mcp_tool_effect_uncertain','mcp_tool_rejected') AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.phase')='worker' AND json_extract(body,'$.tool')=?4 AND json_extract(body,'$.arguments_sha256')=?3 ORDER BY seq DESC LIMIT 1",rusqlite::params![context.scope.run.to_string(),context.scope.actor,reference.arguments_sha256,reference.tool],|r|r.get(0)).optional()?;
    let (Some(result), Some(audit)) = (result, audit) else {
        return Ok(false);
    };
    let value: Value = serde_json::from_str(&result)?;
    let audit: Value = serde_json::from_str(&audit)?;
    Ok(crate::digest(result.as_bytes()) == reference.result_sha256
        && audit["status"] == "completed"
        && audit["policy_revision"] == reference.policy_revision
        && audit["result_sha256"] == reference.result_sha256
        && value["verification_identity"] == reference.source_identity
        && value["snapshot_unchanged"] == true
        && value["artifact_sha256"] == json!(reference.artifact_sha256)
        && match reference.outcome {
            CheckStatus::Passed => {
                if reference.validator == "dotnet_api" {
                    value["status"] == "api_passed"
                } else {
                    value["compile_passed"] == true
                }
            }
            CheckStatus::Failed => {
                if reference.validator == "dotnet_api" {
                    matches!(
                        value["status"].as_str(),
                        Some("api_failed" | "indeterminate")
                    )
                } else {
                    value["compile_passed"] == false
                }
            }
            _ => false,
        })
}
pub fn reference(
    context: &ToolContext<'_>,
    binding: &crate::task_engine::validators::Binding,
    value: &Value,
    audit: &Value,
) -> Result<crate::task_engine::validators::NativeReceipt> {
    use crate::task_engine::{
        contracts::{CheckStatus, VERSION},
        validators::NativeReceipt,
    };
    let mode = Mode::from_tool(
        audit["tool"]
            .as_str()
            .context("native tool receipt name missing")?,
    )?;
    ensure!(
        binding.validator == mode.validator(),
        "native result differs from bound validator"
    );
    ensure!(
        value["verification_identity"] == binding.source_identity
            && value["contract_id"] == binding.contract_id
            && value["qualified"] == false
            && value["promoted"] == false
            && value["model_calls"] == 0
            && audit["actor"] == context.scope.actor
            && audit["tool"] == mode.tool()
            && audit["phase"] == "worker",
        "native SDK result/actor differs from frozen check"
    );
    let required = |field: &str| {
        audit[field]
            .as_str()
            .map(str::to_owned)
            .context("canonical native receipt field missing")
    };
    let reference = NativeReceipt {
        schema_version: VERSION,
        validator: mode.validator().into(),
        tool: mode.tool().into(),
        contract_id: binding.contract_id.clone(),
        source_identity: binding.source_identity.clone(),
        policy_revision: context.policy.revision,
        arguments_sha256: required("arguments_sha256")?,
        result_sha256: required("result_sha256")?,
        input_snapshot_sha256: crate::digest(&serde_json::to_vec(&value["inputs_sha256"])?),
        artifact_sha256: serde_json::from_value(value["artifact_sha256"].clone())?,
        api_contract_sha256: if mode == Mode::Api {
            let prepared = prepare_mode(context, &binding.contract_id, mode)?;
            Some(crate::digest(&serde_json::to_vec(
                prepared.spec.api.as_ref().unwrap(),
            )?))
        } else {
            None
        },
        outcome: if value["status"]
            == if mode == Mode::Api {
                "api_passed"
            } else {
                "build_passed"
            } {
            CheckStatus::Passed
        } else {
            CheckStatus::Failed
        },
        elapsed_micros: value["elapsed_ms"]
            .as_u64()
            .unwrap_or(0)
            .saturating_mul(1000),
        sampled_process_group_rss_bytes: value["phases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["sampled_process_group_rss_bytes"].as_u64())
            .chain(value["api"]["runtime"]["sampled_process_group_rss_bytes"].as_u64())
            .filter(|n| *n > 0)
            .max(),
    };
    ensure!(
        reference_current(context, &reference)?,
        "native SDK receipt no longer current or canonical"
    );
    Ok(reference)
}
/// A single API invocation also supplies its observed compilation facet. This
/// is reused only within that same task, with current canonical receipt checks.
pub fn compile_reference(
    context: &ToolContext<'_>,
    api_reference: &crate::task_engine::validators::NativeReceipt,
    value: &Value,
) -> Result<crate::task_engine::validators::NativeReceipt> {
    ensure!(
        api_reference.validator == "dotnet_api" && reference_current(context, api_reference)?,
        "API receipt cannot supply a compile observation"
    );
    let mut reference = api_reference.clone();
    reference.validator = "dotnet_build".into();
    reference.outcome = if value["compile_passed"] == true {
        crate::task_engine::contracts::CheckStatus::Passed
    } else {
        crate::task_engine::contracts::CheckStatus::Failed
    };
    ensure!(
        reference_current(context, &reference)?,
        "API compile observation no longer current"
    );
    Ok(reference)
}

/// Strict subset prevents MSBuild from executing project-authored code. No
/// expression/condition/import/target/reference/analyzer/task semantics are inferred.
fn project(text: &str, name: &str, sdk: &str) -> Result<String> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().expand_empty_elements = true;
    let mut stack: Vec<String> = vec![];
    let mut properties = BTreeMap::<String, String>::new();
    let mut root_seen = false;
    let mut ended = false;
    for _ in 0..1024 {
        match reader.read_event()? {
            Event::Start(start) => {
                let tag = start.name().as_ref().to_string();
                let attributes = start
                    .attributes()
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                match stack.len() {
                    0 => {
                        ensure!(!root_seen && tag == "Project" && attributes.len() == 1 && attributes[0].key.as_ref() == "Sdk", "one standard Project SDK required");
                        let value = attributes[0].normalized_value(quick_xml::XmlVersion::Implicit1_0)?;
                        ensure!(matches!(value.as_ref(), "Microsoft.NET.Sdk"|"Microsoft.NET.Sdk.Web"), "custom .NET SDK unsupported");
                        root_seen = true;
                    }
                    1 => ensure!(tag == "PropertyGroup" && attributes.is_empty(), "only unconditional PropertyGroup supported; no custom build actions/references"),
                    2 => {
                        ensure!(attributes.is_empty() && matches!(tag.as_str(),"TargetFramework"|"OutputType"|"UseAppHost"|"ImplicitUsings"|"Nullable"|"LangVersion"|"AssemblyName"|"RootNamespace"|"TreatWarningsAsErrors") && !properties.contains_key(&tag), "unsupported or duplicate project property");
                        properties.insert(tag.clone(), String::new());
                    }
                    _ => anyhow::bail!("nested MSBuild expressions unsupported"),
                }
                stack.push(tag);
            }
            Event::Text(text) => {
                let content = text.xml10_content();
                if stack.len() == 3 {
                    properties.get_mut(&stack[2]).unwrap().push_str(&content);
                } else {
                    ensure!(content.trim().is_empty(), "unexpected MSBuild text");
                }
            }
            Event::End(_) => {
                ensure!(stack.pop().is_some(), "unbalanced project XML");
            }
            Event::Decl(_) | Event::Comment(_) => (),
            Event::Eof => {
                ended = true;
                break;
            }
            _ => anyhow::bail!("MSBuild entity/CDATA/DTD/processing instructions unsupported"),
        }
    }
    ensure!(
        ended && root_seen && stack.is_empty(),
        "incomplete or excessive project XML"
    );
    for value in properties.values_mut() {
        *value = value.trim().to_string();
        ensure!(
            !value.is_empty()
                && value.len() <= 80
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
            "literal project properties only"
        );
    }
    ensure!(
        properties.get("UseAppHost").map(String::as_str) == Some("false"),
        "managed_dll contract requires explicit UseAppHost=false; no native executable claim"
    );
    let target = properties
        .get("TargetFramework")
        .context("TargetFramework required")?;
    ensure!(
        matches!(target.as_str(), "net9.0" | "net10.0")
            && target[3..].split('.').next() == sdk.split('.').next(),
        "supported TargetFramework must match frozen SDK major"
    );
    if let Some(output) = properties.get("OutputType") {
        ensure!(
            matches!(output.as_str(), "Exe" | "Library"),
            "unsupported OutputType"
        );
    }
    let assembly = properties
        .get("AssemblyName")
        .map(String::as_str)
        .unwrap_or(name);
    ensure!(component(assembly), "invalid managed assembly name");
    Ok(format!("bin/Debug/{target}/{assembly}.dll"))
}

pub fn executable_path() -> Result<PathBuf> {
    let path = PathBuf::from(std::env::var_os("GENTS_DOTNET").context(
        ".NET check needs owner-configured absolute GENTS_DOTNET; no discovery/download fallback",
    )?);
    ensure!(path.is_absolute(), "GENTS_DOTNET must be absolute");
    let path = path.canonicalize()?;
    ensure!(
        path.is_file() && path.file_name().is_some_and(|name| name == "dotnet"),
        "installed dotnet executable required"
    );
    Ok(path)
}
fn executable_hash(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= 64 * 1024 * 1024,
        "dotnet executable identity bound exceeded"
    );
    Ok(crate::digest(&bytes))
}
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) struct Prepared {
    mode: Mode,
    spec: Spec,
    executable: PathBuf,
    files: BTreeMap<String, String>,
    hashes: BTreeMap<String, String>,
    identity: String,
    artifact: String,
}
fn prepare_mode(context: &ToolContext<'_>, id: &str, mode: Mode) -> Result<Prepared> {
    ensure!(
        context.scope.phase == crate::Phase::Worker,
        ".NET check is worker-scoped"
    );
    context.policy.authorize(
        &context.scope.actor,
        "dotnet_check",
        &json!({"contract_id":id}),
    )?;
    if mode == Mode::Api {
        context.policy.authorize(
            &context.scope.actor,
            mode.tool(),
            &json!({"contract_id":id}),
        )?;
    }
    let encoded: String = context.db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [context.scope.run.to_string()],
        |r| r.get(0),
    )?;
    let spec = specs(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .find(|spec| spec.id == id && spec.actor == context.scope.actor)
        .context("exact owner-frozen .NET contract and actor required")?;
    ensure!(
        mode != Mode::Api || spec.api.is_some(),
        "API execution requires an explicit frozen API contract"
    );
    let executable = executable_path()?;
    ensure!(
        executable_hash(&executable)? == spec.dotnet_sha256,
        "dotnet executable differs from frozen hash"
    );
    let work = workspace::open(&context.authority.workspace(context.scope)?, false)?;
    let mut files = BTreeMap::new();
    let mut hashes = BTreeMap::new();
    let mut total = 0;
    for path in std::iter::once(&spec.project_path).chain(&spec.source_paths) {
        context.policy.authorize(
            &context.scope.actor,
            "files",
            &json!({"op":"read","path":path}),
        )?;
        let bytes = work.read_bytes_complete(path, MAX_BYTES)?;
        total += bytes.len();
        ensure!(total <= MAX_BYTES, ".NET snapshot exceeds 2 MiB");
        let text = std::str::from_utf8(&bytes).context(".NET snapshot must be UTF-8")?;
        crate::safety::require_persistable(text)?;
        hashes.insert(path.clone(), crate::digest(&bytes));
        files.insert(path.clone(), text.to_string());
    }
    ensure!(
        spec.frozen_inputs
            .iter()
            .all(|(path, hash)| hashes.get(path) == Some(hash)),
        "frozen .NET project/dependency hash mismatch"
    );
    let name = Path::new(&spec.project_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .context("invalid project name")?;
    let artifact = project(&files[&spec.project_path], name, &spec.sdk_version)?;
    if mode == Mode::Api {
        let mut xml = Reader::from_str(&files[&spec.project_path]);
        let mut web = false;
        for _ in 0..1024 {
            match xml.read_event()? {
                Event::Start(start) => {
                    let attribute = start
                        .attributes()
                        .next()
                        .transpose()?
                        .context("SDK attribute missing")?;
                    web = attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                        == "Microsoft.NET.Sdk.Web";
                    break;
                }
                Event::Eof => break,
                _ => (),
            }
        }
        ensure!(web, "API execution requires the standard Web SDK");
    }
    let identity = crate::digest(json!({"run":context.scope.run,"actor":context.scope.actor,
        "policy_revision":context.policy.revision,"contract":spec,"inputs":hashes,"execution_mode":mode.tool(),
        "implementation":crate::digest(concat!(include_str!("dotnet_check.rs"),include_str!("dotnet_check/sandbox.rs"),include_str!("dotnet_check/api.rs"),include_str!("strict_json.rs"),include_str!("functions/dotnet_check.rs"), include_str!("registry_dotnet.rs")).as_bytes())}).to_string().as_bytes());
    Ok(Prepared {
        mode,
        spec,
        executable,
        files,
        hashes,
        identity,
        artifact,
    })
}
pub fn request_identity(authority: &dyn Authority, scope: &Scope, id: &str) -> Result<String> {
    request_identity_mode(authority, scope, id, Mode::Build)
}
pub fn api_request_identity(authority: &dyn Authority, scope: &Scope, id: &str) -> Result<String> {
    request_identity_mode(authority, scope, id, Mode::Api)
}
fn request_identity_mode(
    authority: &dyn Authority,
    scope: &Scope,
    id: &str,
    mode: Mode,
) -> Result<String> {
    let db = authority.open(scope)?;
    let policy = governance::load(&db, &scope.run.to_string())?
        .context(".NET check needs current policy")?;
    ensure!(
        scope.expected_revision.is_none_or(|r| r == policy.revision),
        ".NET policy changed"
    );
    Ok(prepare_mode(
        &ToolContext {
            authority,
            scope,
            policy: &policy,
            db: &db,
        },
        id,
        mode,
    )?
    .identity)
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
    let spec = specs(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .find(|s| {
            s.actor == scope.actor
                && (s.project_path == path || s.source_paths.iter().any(|p| p == path))
        })
        .context("source outside frozen .NET contract")?;
    request_identity(authority, scope, &spec.id)
}
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildResult {
    status: String,
    contract_id: String,
    verification_identity: String,
    inputs_sha256: BTreeMap<String, String>,
    dotnet_sha256: String,
    sdk_version: String,
    snapshot_unchanged: bool,
    artifact_mode: String,
    artifact_sha256: Option<String>,
    compile_passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    api: Option<api::Report>,
    phases: Vec<sandbox::PhaseResult>,
    elapsed_ms: u64,
    model_calls: u64,
    provider_cost_usd: u64,
    qualified: bool,
    promoted: bool,
    authority_granted: bool,
    scope: String,
}
pub(crate) fn prepare_call_mode(
    context: &ToolContext<'_>,
    args: &Value,
    mode: Mode,
) -> Result<Prepared> {
    let prepared = prepare_mode(
        context,
        args["contract_id"]
            .as_str()
            .context("contract_id required")?,
        mode,
    )?;
    ensure!(
        args["expected_source_identity"] == prepared.identity,
        ".NET source identity changed or was not host-bound"
    );
    Ok(prepared)
}
pub(crate) fn compute(prepared: &Prepared) -> Result<Value> {
    let start = std::time::Instant::now();
    let (phases, artifact, api) = sandbox::build(prepared)?;
    let unchanged = executable_hash(&prepared.executable)? == prepared.spec.dotnet_sha256;
    let compile_passed = phases.len() == 3 && phases.iter().all(|p| p.passed) && artifact.is_some();
    let status = if !unchanged
        || phases
            .iter()
            .any(|p| p.limit.is_some() || p.output_truncated)
    {
        "indeterminate"
    } else if prepared.mode == Mode::Api {
        if compile_passed && api.as_ref().is_some_and(|a| a.passed) {
            "api_passed"
        } else {
            "api_failed"
        }
    } else if compile_passed {
        "build_passed"
    } else {
        "build_failed"
    };
    let result = serde_json::to_value(BuildResult {
        status:status.into(),contract_id:prepared.spec.id.clone(), verification_identity:prepared.identity.clone(),
        inputs_sha256:prepared.hashes.clone(),dotnet_sha256:prepared.spec.dotnet_sha256.clone(),sdk_version:prepared.spec.sdk_version.clone(),
        snapshot_unchanged:unchanged,artifact_mode:prepared.spec.artifact_mode.clone(),artifact_sha256:artifact,phases,
        compile_passed,api,
        elapsed_ms:start.elapsed().as_millis() as u64, model_calls:0,provider_cost_usd:0,qualified:false,promoted:false,authority_granted:false,
        scope:if prepared.mode==Mode::Api {"Owner-frozen bounded HTTP status/JSON checks against the actual copied Web SDK application, through an inherited loopback socket. No new binds, outbound connections, shell, subprocesses, filesystem writes, downloads or run-workspace writes are allowed at runtime. Only frozen cases are checked; no general API correctness, expert qualification, company completion or promotion. SDK/runtime library closure is not fully content-addressed; memory is sampled process-group RSS."}else{"Offline package-free managed-DLL compile facts for explicit copied inputs only; no candidate app executed, functional/API correctness, independent acceptance, expert qualification or promotion. Deny-default macOS isolation; no network, shell, downloads or run-workspace writes. Executable hash and exact SDK selection recorded; SDK/runtime library closure is not content-addressed. RSS and scratch size are sampled enforcement observations, not total unified-memory measurements."}.into(),
    })?;
    crate::safety::require_persistable(&result.to_string())?;
    Ok(result)
}
pub(crate) fn finish(
    context: &ToolContext<'_>,
    args: &Value,
    prepared: &Prepared,
    result: Value,
) -> Result<Value> {
    let current = prepare_call_mode(context, args, prepared.mode)?;
    ensure!(
        current.identity == prepared.identity,
        ".NET source/configuration changed during detached build; no result accepted"
    );
    context.db.execute_batch("CREATE TABLE IF NOT EXISTS dotnet_build_observations(id TEXT PRIMARY KEY,run_id TEXT NOT NULL,actor TEXT NOT NULL,policy_revision INTEGER NOT NULL,contract_id TEXT NOT NULL,verification_identity TEXT NOT NULL,result TEXT NOT NULL);")?;
    context.db.execute(
        "INSERT INTO dotnet_build_observations VALUES(?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            context.scope.run.to_string(),
            context.scope.actor,
            context.policy.revision as i64,
            result["contract_id"].as_str(),
            result["verification_identity"].as_str(),
            result.to_string()
        ],
    )?;
    Ok(result)
}

pub fn execute(context: &ToolContext<'_>, args: &Value) -> Result<Value> {
    execute_mode(context, args, Mode::Build)
}
pub(crate) fn execute_mode(context: &ToolContext<'_>, args: &Value, mode: Mode) -> Result<Value> {
    let prepared = prepare_call_mode(context, args, mode)?;
    let result = compute(&prepared)?;
    finish(context, args, &prepared, result)
}

/// Revalidates authorization, source and canonical MCP receipt; never executes a check.
pub fn observations(authority: &dyn Authority, scope: &Scope) -> Result<Vec<Value>> {
    let db = authority.open(scope)?;
    let encoded: String = db.query_row(
        "SELECT intake FROM jobs WHERE run_id=?1",
        [scope.run.to_string()],
        |r| r.get(0),
    )?;
    let mut views = vec![];
    for spec in specs(&serde_json::from_str(&encoded)?)?
        .into_iter()
        .filter(|s| s.actor == scope.actor)
    {
        let (identity, build) = current_observation(authority, scope, &db, &spec.id, Mode::Build)?;
        let mut view = json!({"contract_id":spec.id,"current_source_available":identity.is_some(),"current_source_identity":identity,"latest_current_build":build,"independent_acceptance":false,"authority_granted":false});
        if spec.api.is_some() {
            let (identity, api) = current_observation(authority, scope, &db, &spec.id, Mode::Api)?;
            view["api_current_source_available"] = json!(identity.is_some());
            view["api_current_source_identity"] = json!(identity);
            view["latest_current_api"] = api;
        }
        views.push(view);
    }
    Ok(views)
}
fn current_observation(
    authority: &dyn Authority,
    scope: &Scope,
    db: &rusqlite::Connection,
    id: &str,
    mode: Mode,
) -> Result<(Option<String>, Value)> {
    use rusqlite::OptionalExtension;
    let identity_fn = if mode == Mode::Api {
        api_request_identity
    } else {
        request_identity
    };
    let Ok(identity) = identity_fn(authority, scope, id) else {
        return Ok((None, Value::Null));
    };
    let table: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='dotnet_build_observations')",[],|r|r.get(0))?;
    let encoded: Option<String> = if table {
        db.query_row("SELECT result FROM dotnet_build_observations WHERE run_id=?1 AND actor=?2 AND contract_id=?3 AND verification_identity=?4 ORDER BY rowid DESC LIMIT 1",rusqlite::params![scope.run.to_string(),scope.actor,id,identity],|r|r.get(0)).optional()?
    } else {
        None
    };
    let mut observation = Value::Null;
    if let Some(encoded) = encoded {
        let sha = crate::digest(
            json!({"contract_id":id,"expected_source_identity":identity})
                .to_string()
                .as_bytes(),
        );
        let receipt: Option<String> = db.query_row("SELECT body FROM audit WHERE run_id=?1 AND kind IN ('mcp_tool_call','mcp_tool_effect_uncertain','mcp_tool_rejected') AND json_extract(body,'$.actor')=?2 AND json_extract(body,'$.phase')='worker' AND json_extract(body,'$.tool')=?4 AND json_extract(body,'$.arguments_sha256')=?3 ORDER BY seq DESC LIMIT 1",rusqlite::params![scope.run.to_string(),scope.actor,sha,mode.tool()],|r|r.get(0)).optional()?;
        if let Some(receipt) = receipt {
            let receipt: Value = serde_json::from_str(&receipt)?;
            if receipt["status"] == "completed"
                && receipt["result_sha256"] == crate::digest(encoded.as_bytes())
            {
                let value: Value = serde_json::from_str(&encoded)?;
                observation = json!({"status":value["status"],"compile_passed":value["compile_passed"],"artifact_sha256":value["artifact_sha256"],"canonical_receipt_verified":true});
                if mode == Mode::Api {
                    observation["api"] = json!({"passed":value["api"]["passed"],"contract_sha256":value["api"]["contract_sha256"],"cases":value["api"]["cases"]});
                }
            }
        }
    }
    // Rechecking cannot resurrect a historical claim across a source/policy change.
    if identity_fn(authority, scope, id).ok().as_deref() != Some(&identity) {
        return Ok((None, Value::Null));
    }
    Ok((Some(identity), observation))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_msbuild_and_misleading_apphost_claims_are_rejected() {
        let valid = "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net10.0</TargetFramework><UseAppHost>false</UseAppHost></PropertyGroup></Project>";
        assert_eq!(
            project(valid, "Probe", "10.0.401").unwrap(),
            "bin/Debug/net10.0/Probe.dll"
        );
        for invalid in [valid.replace("false","true"),valid.replace("net10.0","net9.0"),valid.replace("Microsoft.NET.Sdk","Evil.Sdk"),valid.replace("<PropertyGroup>","<PropertyGroup Condition=\"'x'=='x'\">"),valid.replace("</Project>","<Target Name=\"Build\"><Exec Command=\"touch forbidden\"/></Target></Project>"),valid.replace("</Project>","<ItemGroup><PackageReference Include=\"Fake\" Version=\"1\"/></ItemGroup></Project>"),valid.replace("</PropertyGroup>","<AssemblyName>$(Evil)</AssemblyName></PropertyGroup>"),valid.replace("</Project>","<Import Project=\"/tmp/evil.targets\"/></Project>"),valid.replace("</Project>","<?evil /tmp?></Project>")] {
            assert!(project(&invalid,"Probe","10.0.401").is_err(),"{invalid}");
        }
    }
}

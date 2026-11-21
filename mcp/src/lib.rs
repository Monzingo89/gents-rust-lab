//! Local-only, process-bound MCP tools. Transport does not create authority.
pub mod angular_check;
pub mod artifact_bindings;
pub mod audio;
pub mod authority;
mod bound_sources;
pub mod client;
pub mod completion;
pub mod component_api;
pub mod contract_execution;
pub mod contracts;
pub mod cosmos_check;
pub mod functions;
pub mod governance;
pub mod host_memory;
pub mod media;
pub mod owned_components;
pub mod owned_recipes;
mod owner_inputs;

/// Exact declaration match for host context views. This pure metadata query
/// does not inspect aliases, grant access or replace the native write guard.
pub fn frozen_input_path(intake: &serde_json::Value, path: &str) -> bool {
    owner_inputs::protected(intake, path)
}
pub mod dotnet_check;
pub mod packet_budget;
pub mod proposal_feedback;
pub mod proposals;
pub mod protocol;
pub mod python_check;
mod python_custody;
mod registry_angular;
mod registry_dotnet;
mod runtime_bundle;
pub mod rust_verification;
pub mod safety;
pub mod skills;
mod strict_json;
pub mod task_engine;
pub mod transports;
pub mod workspace;
pub mod workspace_path;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Planner,
    Worker,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub company: String,
    pub owner: String,
    pub run: Uuid,
    pub actor: String,
    pub phase: Phase,
    /// Optional trusted-host source epoch. It only denies stale execution.
    pub expected_revision: Option<u64>,
}
impl Scope {
    pub fn validate(&self) -> Result<()> {
        if self.company.is_empty()
            || self.company.len() > 80
            || !self
                .company
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            bail!("invalid company scope");
        }
        for value in [&self.owner, &self.actor] {
            if value.is_empty() || value.len() > 200 || value.chars().any(char::is_control) {
                bail!("invalid process identity");
            }
        }
        Ok(())
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Trusted implementation requirements, never accepted from model arguments.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RuntimeRequirements {
    pub installed_rustc: bool,
    pub installed_python: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub installed_dotnet: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub installed_angular: bool,
    pub call_timeout_seconds: u64,
}
impl Default for RuntimeRequirements {
    fn default() -> Self {
        Self {
            installed_rustc: false,
            installed_python: false,
            installed_dotnet: false,
            installed_angular: false,
            call_timeout_seconds: 10,
        }
    }
}
impl RuntimeRequirements {
    pub const fn isolated_rust() -> Self {
        Self {
            installed_rustc: true,
            installed_python: false,
            installed_dotnet: false,
            installed_angular: false,
            call_timeout_seconds: 110,
        }
    }
}
fn is_false(value: &bool) -> bool {
    !value
}
/// Implementations receive neither network credentials nor an arbitrary shell.
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str;
    fn schema(&self) -> Value;
    /// Schema of the native result value, not its MCP receipt envelope.
    fn result_schema(&self, _arguments: Option<&Value>) -> Option<Value> {
        None
    }
    fn scoped_schema(&self, scope: &Scope, _policy: &governance::Policy) -> Result<Option<Value>> {
        Ok(self.phases().contains(&scope.phase).then(|| self.schema()))
    }
    fn description(&self) -> &'static str;
    fn runtime_requirements(&self) -> RuntimeRequirements {
        RuntimeRequirements::default()
    }
    fn read_only(&self) -> bool;
    /// Additional typed, read-only source admission for exact owner contracts.
    /// Must inspect bindings only; never execute a candidate or create authority.
    fn frozen_source_admission(
        &self,
        _context: &ToolContext<'_>,
        _arguments: &Value,
    ) -> Result<bool> {
        Ok(false)
    }
    /// Same validated arguments always produce the same result without I/O.
    fn deterministic(&self) -> bool {
        false
    }
    /// Array data fields that a CEO-derived source must fill only from native receipts.
    fn data_input_fields(&self) -> &'static [&'static str] {
        &[]
    }
    /// Workspace path fields that become immutable when supplied in the
    /// owner's frozen source inputs. Does not grant access or perform I/O.
    fn file_input_fields(&self) -> &'static [&'static str] {
        &[]
    }
    /// Destination fields checked against frozen owner inputs before effects.
    fn file_output_fields(&self) -> &'static [&'static str] {
        &[]
    }
    fn read_only_call(&self, _arguments: &Value) -> bool {
        self.read_only()
    }
    /// Non-DB effects cannot be rolled back with the SQLite transaction.
    fn external_effects(&self) -> bool {
        false
    }
    /// Optional bounded discovery, never a write or a task-answer generator.
    fn recovery_arguments(&self) -> Option<Value> {
        None
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Planner, Phase::Worker]
    }
    fn execute(&self, context: &ToolContext<'_>, arguments: Value) -> Result<Value>;
}
pub struct ToolContext<'a> {
    pub authority: &'a dyn authority::Authority,
    pub scope: &'a Scope,
    pub policy: &'a governance::Policy,
    pub db: &'a Connection,
}
pub struct Registry {
    tools: BTreeMap<&'static str, Box<dyn Tool>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
impl Registry {
    pub(crate) fn file_inputs<'a>(&self, name: &str, args: &'a Value) -> Vec<&'a str> {
        self.tools
            .get(name)
            .into_iter()
            .flat_map(|tool| tool.file_input_fields().iter())
            .filter_map(|field| args[*field].as_str())
            .collect()
    }
    pub fn recovery(
        &self,
        authority: &dyn authority::Authority,
        scope: &Scope,
    ) -> Result<Option<Value>> {
        let gate = completion::check(authority, scope)?;
        let db = authority.open(scope)?;
        let policy = governance::load(&db, &scope.run.to_string())?.context("missing policy")?;
        if gate["policy_revision"] != policy.revision {
            return Ok(None);
        }
        for check in gate["checks"]
            .as_array()
            .context("missing completion checks")?
        {
            if check["passed"] == true || check["actor"] != scope.actor {
                continue;
            }
            let name = check["tool"].as_str().context("invalid requirement")?;
            let tool = self
                .tools
                .get(name)
                .context("missing registered recovery tool")?;
            if !tool.read_only() || !tool.phases().contains(&scope.phase) {
                continue;
            }
            if let Some(arguments) = tool.recovery_arguments() {
                policy.authorize(&scope.actor, name, &arguments)?;
                return Ok(Some(
                    json!({"action":name,"arguments":arguments,"summary":"Recover a frozen missing read-only tool requirement with bounded governed discovery; independent admin review is still required."}),
                ));
            }
        }
        Ok(None)
    }
    /// Implementation catalog for inspection only; this does not grant authority.
    pub fn catalog(&self) -> Value {
        json!({"server":"gents-local-mcp","protocol_version":protocol::VERSION,"scope":"implementation_catalog_not_agent_grants","transports":["stdio","sse"],"tools":self.tools.values().map(|tool|json!({"name":tool.name(),"description":tool.description(),"source":tool.source(),"inputSchema":tool.schema(),"resultSchema":tool.result_schema(None),"phases":tool.phases(),"read_only":tool.read_only(),"deterministic":tool.deterministic(),"data_input_fields":tool.data_input_fields(),"file_input_fields":tool.file_input_fields(),"external_effects":tool.external_effects(),"runtime_requirements":tool.runtime_requirements(),"provider_cost_usd":0})).collect::<Vec<_>>()})
    }
    pub fn deterministic(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .is_some_and(|tool| tool.deterministic())
    }
    pub fn read_only_call(&self, name: &str, arguments: &Value) -> bool {
        self.tools
            .get(name)
            .is_some_and(|tool| tool.read_only_call(arguments))
    }
    pub(crate) fn approved_source(
        &self,
        context: &ToolContext<'_>,
        name: &str,
        args: &Value,
    ) -> Result<bool> {
        let tool = self.tools.get(name).context("unknown native source")?;
        if !tool.read_only_call(args)
            || tool.external_effects()
            || !tool.phases().contains(&context.scope.phase)
        {
            return Ok(false);
        }
        context.policy.authorize(&context.scope.actor, name, args)?;
        let pinned_file = !self.file_inputs(name, args).is_empty()
            && args["expected_sha256"].as_str().is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            });
        if tool.deterministic() || pinned_file {
            return Ok(true);
        }
        tool.frozen_source_admission(context, args)
    }
    /// Validate declared result selections without executing any function.
    pub fn validate_projection(
        &self,
        name: &str,
        arguments: &Value,
        pointer: &str,
        encoding: &str,
    ) -> Result<()> {
        let tool = self
            .tools
            .get(name)
            .context("unknown native projection tool")?;
        let Some(mut schema) = tool.result_schema(Some(arguments)) else {
            return Ok(());
        };
        anyhow::ensure!(
            pointer.starts_with('/'),
            "result projection must be a JSON pointer"
        );
        for segment in pointer[1..].split('/') {
            let mut key = String::new();
            let mut chars = segment.chars();
            while let Some(ch) = chars.next() {
                if ch == '~' {
                    key.push(match chars.next() {
                        Some('0') => '~',
                        Some('1') => '/',
                        _ => bail!("invalid JSON pointer escape"),
                    });
                } else {
                    key.push(ch);
                }
            }
            schema = match schema["type"].as_str() {
                Some("object") => schema["properties"]
                    .get(&key)
                    .cloned()
                    .with_context(||format!("result projection field is not declared by native {name}; this object exposes {}",schema["properties"].as_object().map(|p|p.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default()))?,
                Some("array") => {
                    let index = key
                        .parse::<u64>()
                        .context("result projection requires an array index")?;
                    anyhow::ensure!(
                        index.to_string() == key
                            && schema["maxItems"]
                                .as_u64()
                                .is_none_or(|bound| index < bound),
                        "result projection row is outside declared native bounds"
                    );
                    schema["items"].clone()
                }
                _ => bail!("result projection descends through a scalar or undeclared type"),
            };
        }
        anyhow::ensure!(
            encoding == "json"
                || encoding == "integer_text" && schema["type"] == "integer"
                || encoding == "utf8_text" && schema["type"] == "string",
            "projection encoding does not match declared native result type"
        );
        Ok(())
    }
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
    pub fn runtime_requirements(&self, name: &str) -> Result<RuntimeRequirements> {
        Ok(self
            .tools
            .get(name)
            .context("unknown local tool")?
            .runtime_requirements())
    }
    pub fn new() -> Self {
        let mut tools: BTreeMap<&'static str, Box<dyn Tool>> = BTreeMap::new();
        for tool in functions::builtins() {
            assert!(tools.insert(tool.name(), tool).is_none());
        }
        Self { tools }
    }
    pub fn list(&self, authority: &dyn authority::Authority, scope: &Scope) -> Result<Value> {
        let mut db = authority.open(scope)?;
        let tx = db.transaction()?;
        let policy = governance::load(&tx, &scope.run.to_string())?
            .context("run has no installed policy")?;
        anyhow::ensure!(
            scope
                .expected_revision
                .is_none_or(|revision| revision == policy.revision),
            "MCP source policy epoch changed before catalog admission"
        );
        let mut list = Vec::new();
        if let Some(grant) = policy.grants.get(&scope.actor) {
            for tool in self.tools.values() {
                let Some(mut schema) = tool.scoped_schema(scope, &policy)? else {
                    continue;
                };
                if !grant.tools.contains(tool.name()) {
                    let Some(branches) = schema.get_mut("oneOf").and_then(Value::as_array_mut)
                    else {
                        continue;
                    };
                    branches.retain(|branch| {
                        grant.permits(
                            tool.name(),
                            &json!({"op":branch["properties"]["op"]["const"]}),
                        )
                    });
                    if branches.is_empty() {
                        continue;
                    }
                }
                let mut definition = json!({"name":tool.name(),"description":tool.description(),"inputSchema":schema,"annotations":{"readOnlyHint":tool.read_only(),"openWorldHint":false,"destructiveHint":!tool.read_only()}});
                if let Some(output) = tool
                    .result_schema(None)
                    .filter(|_| functions::is_task_capability(tool.name()))
                {
                    definition["outputSchema"] = json!({"type":"object","additionalProperties":false,"required":["value","receipt"],"properties":{"value":output,"receipt":{"type":"object","required":["actor","phase","tool","arguments_sha256","result_sha256","policy_revision","provider_cost_usd","status"],"properties":{"actor":{"type":"string"},"phase":{"type":"string"},"tool":{"type":"string"},"arguments_sha256":{"type":"string"},"result_sha256":{"type":"string"},"policy_revision":{"type":"integer"},"provider_cost_usd":{"const":0},"status":{"const":"completed"}}}}});
                }
                list.push(definition);
            }
        }
        tx.commit()?;
        Ok(json!({"tools":list}))
    }
    pub fn call(
        &self,
        authority: &dyn authority::Authority,
        scope: &Scope,
        name: &str,
        arguments: Value,
    ) -> Result<Value> {
        if name == "angular_check" {
            return self.call_angular(authority, scope, name, arguments);
        }
        if matches!(name, "dotnet_check" | "dotnet_api_check") {
            return self.call_dotnet(authority, scope, name, arguments);
        }
        let sdk_authority = matches!(name, "get_task_status" | "cancel_task")
            .then(|| crate::angular_check::ReceiptAuthority::for_scope(authority, scope));
        let authority: &dyn authority::Authority = if let Some(adapter) = &sdk_authority {
            adapter
        } else {
            authority
        };
        let mut db = authority.open(scope)?;
        // Immediate transaction fences policy changes and native memory writes
        // across processes. A revoked tool cannot commit against an old epoch.
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let arguments_sha = digest(&serde_json::to_vec(&arguments)?);
        let mut external_effect_started = false;
        let mut contract_enforced = false;
        let outcome = (|| -> Result<(Value, u64)> {
            let policy = governance::load(&tx, &scope.run.to_string())?
                .context("run has no installed policy")?;
            anyhow::ensure!(
                scope
                    .expected_revision
                    .is_none_or(|revision| revision == policy.revision),
                "MCP source policy epoch changed before execution; inspect before retrying"
            );
            let tool = self.tools.get(name).context("unknown local tool")?;
            if !tool.phases().contains(&scope.phase) {
                bail!("tool is not assigned to this phase");
            }
            policy.authorize(&scope.actor, name, &arguments)?;
            contract_execution::check(self, authority, &tx, scope, &policy, name, &arguments)?;
            contract_enforced = contract_execution::enabled(&policy, scope);
            if !tool.read_only_call(&arguments) {
                let state: String = tx.query_row(
                    "SELECT state FROM jobs WHERE run_id=?1",
                    [scope.run.to_string()],
                    |r| r.get(0),
                )?;
                if !matches!(state.as_str(), "approved" | "running") {
                    bail!("mutating tools require an approved active job");
                }
            }
            if name == "files" && matches!(arguments["op"].as_str(), Some("write" | "rename")) {
                let encoded: String = tx.query_row(
                    "SELECT intake FROM jobs WHERE run_id=?1",
                    [scope.run.to_string()],
                    |r| r.get(0),
                )?;
                let intake: Value = serde_json::from_str(&encoded)?;
                let path = arguments["path"].as_str().context("write path required")?;
                anyhow::ensure!(
                    !owner_inputs::write_protected(&authority.workspace(scope)?, &intake, path)?,
                    "frozen owner input files cannot be overwritten by agents"
                );
                if arguments["op"] == "rename" {
                    let destination = arguments["destination"]
                        .as_str()
                        .context("rename destination required")?;
                    anyhow::ensure!(
                        !owner_inputs::write_protected(
                            &authority.workspace(scope)?,
                            &intake,
                            destination
                        )?,
                        "frozen owner input files cannot be rename destinations"
                    );
                }
                if arguments.get("expected_sha256").is_some() {
                    policy.authorize(&scope.actor, "files", &json!({"op":"read","path":path}))?;
                }
            }
            for field in tool.file_output_fields() {
                let path = arguments[*field].as_str().context("output path required")?;
                let encoded: String = tx.query_row(
                    "SELECT intake FROM jobs WHERE run_id=?1",
                    [scope.run.to_string()],
                    |r| r.get(0),
                )?;
                let intake: Value = serde_json::from_str(&encoded)?;
                anyhow::ensure!(
                    !owner_inputs::write_protected(&authority.workspace(scope)?, &intake, path)?,
                    "frozen owner input files cannot be media destinations"
                );
            }
            external_effect_started = tool.external_effects() && !tool.read_only_call(&arguments);
            let result = tool.execute(
                &ToolContext {
                    authority,
                    scope,
                    policy: &policy,
                    db: &tx,
                },
                arguments.clone(),
            )?;
            if let Some(schema) = tool.result_schema(Some(&arguments)) {
                contracts::validate_value(&schema, &result, "native_result")?;
            }
            artifact_bindings::capture(&tx, scope, name, &arguments, &result, policy.revision)?;
            rust_verification::capture_readback(
                &tx,
                scope,
                name,
                &arguments,
                &result,
                policy.revision,
            )?;
            python_custody::capture(&tx, scope, name, &arguments, &result, policy.revision)?;
            Ok((result, policy.revision))
        })();
        match outcome {
            Ok((result, revision)) => {
                let mut receipt = json!({"actor":scope.actor,"phase":scope.phase,"tool":name,"arguments_sha256":arguments_sha,"result_sha256":digest(&serde_json::to_vec(&result)?),"policy_revision":revision,"provider_cost_usd":0,"status":"completed"});
                if contract_enforced {
                    receipt["contract_enforced"] = json!(true);
                }
                let recorded = (|| -> Result<()> {
                    tx.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'mcp_tool_call',?2,?3)",params![scope.run.to_string(),receipt.to_string(),now_ms() as i64])?;
                    tx.commit()?;
                    Ok(())
                })();
                if let Err(error) = recorded {
                    if external_effect_started {
                        // The filesystem cannot be rolled back with SQLite.
                        // This separate uncertainty record is best-effort if
                        // the database itself is unavailable; never claim success.
                        let _=db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,'mcp_tool_effect_uncertain',?2,?3)",params![scope.run.to_string(),json!({"actor":scope.actor,"phase":scope.phase,"tool":name,"arguments_sha256":arguments_sha,"status":"effect_uncertain","reason":"audit_commit_failed","payload_logged":false}).to_string(),now_ms() as i64]);
                        let message = if name == "files" && arguments["op"] == "rename" {
                            "External rename completed but its audit receipt failed; inspect BOTH source and destination workspace paths and hashes before retrying"
                        } else {
                            "External write completed but its audit receipt failed; inspect the workspace path and hash before retrying"
                        };
                        return Err(error.context(message));
                    }
                    return Err(error);
                }
                Ok(json!({"value":result,"receipt":receipt}))
            }
            Err(error) => {
                tx.rollback()?;
                let status = if external_effect_started {
                    "effect_uncertain"
                } else {
                    "rejected"
                };
                db.execute("INSERT INTO audit(run_id,kind,body,created) VALUES(?1,?2,?3,?4)",params![scope.run.to_string(),if external_effect_started{"mcp_tool_effect_uncertain"}else{"mcp_tool_rejected"},json!({"actor":scope.actor,"phase":scope.phase,"tool":if self.contains(name){name}else{"unregistered"},"tool_name_sha256":digest(name.as_bytes()),"arguments_sha256":arguments_sha,"status":status,"payload_logged":false}).to_string(),now_ms() as i64])?;
                if external_effect_started {
                    let message = if name == "files" && arguments["op"] == "rename" {
                        "External rename was not proven absent; inspect BOTH source and destination workspace paths and hashes before retrying"
                    } else {
                        "External write was not proven absent; inspect the workspace path and hash before retrying"
                    };
                    Err(error.context(message))
                } else {
                    Err(error)
                }
            }
        }
    }
}
pub fn default_authority(root: PathBuf) -> Result<authority::SqliteAuthority> {
    authority::SqliteAuthority::new(root)
}

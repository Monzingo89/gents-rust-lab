//! One governed workspace capability, with operation-specific ceilings.
use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    workspace, Phase, Tool, ToolContext,
};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;
pub struct Files;
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Read {
        path: String,
        #[serde(default)]
        offset_bytes: usize,
        #[serde(default = "page_size")]
        max_bytes: usize,
    },
    Write {
        path: String,
        content: String,
        #[serde(default)]
        expected_sha256: Option<String>,
    },
    List {
        path: String,
    },
}
fn page_size() -> usize {
    8192
}
impl Tool for Files {
    fn source(&self) -> &'static str {
        file!()
    }
    fn external_effects(&self) -> bool {
        true
    }
    fn name(&self) -> &'static str {
        "files"
    }
    fn schema(&self) -> Value {
        let mut schema = EmbeddedContracts.input_schema(self.name()).unwrap();
        schema["type"] = "object".into();
        for branch in schema["oneOf"].as_array_mut().unwrap() {
            if branch["properties"]["op"]["const"] == "write" {
                branch["properties"]["content"]["maxLength"] = 32768.into();
            }
        }
        schema
    }
    fn scoped_schema(
        &self,
        scope: &crate::Scope,
        policy: &crate::governance::Policy,
    ) -> Result<Option<Value>> {
        let mut schema = self.schema();
        if scope.phase == Phase::Planner {
            schema["oneOf"]
                .as_array_mut()
                .unwrap()
                .retain(|branch| branch["properties"]["op"]["const"] != "write");
        }
        if policy
            .authorize(&scope.actor, "files", &serde_json::json!({"op":"read"}))
            .is_err()
        {
            for branch in schema["oneOf"].as_array_mut().unwrap() {
                if branch["properties"]["op"]["const"] == "write" {
                    branch["properties"]
                        .as_object_mut()
                        .unwrap()
                        .remove("expected_sha256");
                }
            }
        }
        Ok(Some(schema))
    }
    fn description(&self) -> &'static str {
        "Governed run-workspace files: read bounded UTF-8 pages, atomically replace bounded text, or list bounded safe entries. Optional write expected_sha256 requires read permission and an existing matching file hash before replacement; MCP writers are serialized. Frozen owner inputs cannot be overwritten. No symlinks, secret files, subprocesses or arbitrary paths. Planner phase cannot write."
    }
    fn read_only(&self) -> bool {
        false
    }
    fn read_only_call(&self, arguments: &Value) -> bool {
        matches!(arguments["op"].as_str(), Some("read" | "list"))
    }
    fn execute(&self, context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &arguments)?;
        let request: Request = serde_json::from_value(arguments)?;
        let writing = matches!(request, Request::Write { .. });
        if writing && context.scope.phase != Phase::Worker {
            bail!("planner phase cannot write workspace files");
        }
        let root = context.authority.workspace(context.scope)?;
        let directory = workspace::open(&root, writing)?;
        match request {
            Request::Read {
                path,
                offset_bytes,
                max_bytes,
            } => directory.read(&path, offset_bytes, max_bytes),
            Request::Write {
                path,
                content,
                expected_sha256,
            } => directory.write_if(&path, &content, expected_sha256.as_deref()),
            Request::List { path } => directory.list(&path),
        }
    }
}

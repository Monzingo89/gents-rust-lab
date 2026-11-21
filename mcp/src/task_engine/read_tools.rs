//! Finite read-only tool registry. Model arguments contain no root, endpoint,
//! executable command, credential or authority field.
use super::workflows::ToolRequest;
use anyhow::{ensure, Result};
use serde::Deserialize;
use serde_json::{json, Value};

pub const NAMES: &[&str] = &["search_knowledge", "workspace_read", "workspace_list"];
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub query: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Read {
    pub path: String,
    pub offset_bytes: usize,
    pub limit_bytes: usize,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    pub path: String,
}
pub enum Arguments {
    Search(Query),
    Read(Read),
    List(List),
}
pub fn validate(request: &ToolRequest) -> Result<Arguments> {
    crate::safety::require_persistable(&serde_json::to_string(request)?)?;
    Ok(match request.name.as_str() {
        "search_knowledge" => {
            let args: Query = serde_json::from_value(request.arguments.clone())?;
            ensure!(
                !args.query.trim().is_empty() && args.query.len() <= 1000,
                "bounded search query required"
            );
            Arguments::Search(args)
        }
        "workspace_read" => {
            let args: Read = serde_json::from_value(request.arguments.clone())?;
            ensure!(
                crate::workspace_path::is_safe_relative_path(&args.path)
                    && (1..=8192).contains(&args.limit_bytes),
                "safe path and 1..8192 byte page required"
            );
            Arguments::Read(args)
        }
        "workspace_list" => {
            let args: List = serde_json::from_value(request.arguments.clone())?;
            ensure!(
                args.path.is_empty()
                    || args.path == "."
                    || crate::workspace_path::is_safe_relative_path(&args.path),
                "safe directory path required"
            );
            Arguments::List(args)
        }
        _ => anyhow::bail!("unknown registered read tool"),
    })
}
pub fn schema(name: &str) -> Option<Value> {
    let path = json!({"type":"string","maxLength":1000});
    let arguments = match name {
        "search_knowledge" => {
            json!({"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string","minLength":1,"maxLength":1000}}})
        }
        "workspace_read" => {
            json!({"type":"object","additionalProperties":false,"required":["path","offset_bytes","limit_bytes"],"properties":{"path":path,"offset_bytes":{"type":"integer","minimum":0,"maximum":16777216},"limit_bytes":{"type":"integer","minimum":1,"maximum":8192}}})
        }
        "workspace_list" => {
            json!({"type":"object","additionalProperties":false,"required":["path"],"properties":{"path":path}})
        }
        _ => return None,
    };
    Some(
        json!({"type":"object","additionalProperties":false,"required":["name","arguments"],"properties":{"name":{"const":name},"arguments":arguments}}),
    )
}
pub fn schemas(names: &[String]) -> Value {
    json!({"oneOf":names.iter().filter_map(|n|schema(n)).collect::<Vec<_>>()})
}

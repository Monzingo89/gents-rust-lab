//! Finite, owner-frozen NoSQL query oracles. These types grant no access and
//! accept no endpoint, credential, executable, provisioning or mutation.
use anyhow::{ensure, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
pub mod runtime;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema_version: u32,
    pub id: String,
    pub actor: String,
    pub query_path: String,
    pub api: String,
    pub database: String,
    pub container: String,
    pub partition_key: PartitionKey,
    pub request_timeout_ms: u64,
    pub total_timeout_ms: u64,
    pub require_query_plan: bool,
    pub require_query_metrics: bool,
    pub require_real_ru: bool,
    pub cases: Vec<Case>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PartitionKey {
    pub paths: Vec<String>,
    pub kind: String,
    pub version: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    /// None explicitly authorizes a cross-partition query. A query cannot
    /// silently turn a frozen single-partition case into a cross-partition one.
    #[serde(deserialize_with = "explicit_partition")]
    #[schemars(required)]
    pub partition: Option<Vec<Value>>,
    pub expected_rows: Vec<Value>,
    pub ordered: bool,
    pub page_size_hint: u32,
    pub max_pages: u32,
    pub max_rows: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueryFile {
    pub schema_version: u32,
    pub queries: Vec<Query>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub id: String,
    pub sql: String,
    pub parameters: Vec<Parameter>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub value: Value,
}
fn explicit_partition<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<Option<Vec<Value>>, D::Error> {
    // Omission must not silently authorize a cross-partition query. Owners
    // supply either an explicit value array or an explicit JSON null.
    Option::<Vec<Value>>::deserialize(decoder)
}
fn component(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && !s.starts_with('.')
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}
impl Contract {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && self.api == "nosql_gateway",
            "Cosmos checks require version 1 and NoSQL gateway API"
        );
        ensure!(
            component(&self.id)
                && component(&self.database)
                && component(&self.container)
                && !self.actor.is_empty()
                && self.actor.len() <= 200
                && !self.actor.chars().any(char::is_control),
            "Invalid Cosmos contract/resource identity"
        );
        ensure!(
            crate::workspace_path::is_safe_relative_path(&self.query_path)
                && self.query_path.ends_with(".json")
                && self.query_path.len() <= 256,
            "A bounded workspace JSON query file is required"
        );
        ensure!(
            self.partition_key.kind == "Hash"
                && self.partition_key.version == 2
                && self.partition_key.paths.len() == 1
                && self.partition_key.paths[0].starts_with('/')
                && self.partition_key.paths[0][1..].split('/').all(component),
            "Only explicit single-path Hash v2 partition keys are implemented"
        );
        ensure!(
            (100..=2000).contains(&self.request_timeout_ms)
                && (self.request_timeout_ms..=30000).contains(&self.total_timeout_ms)
                && !self.cases.is_empty()
                && self.cases.len() <= 8,
            "Invalid finite Cosmos request/deadline/case budget"
        );
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            ensure!(
                component(&case.id)
                    && ids.insert(&case.id)
                    && (1..=32).contains(&case.page_size_hint)
                    && (1..=4).contains(&case.max_pages)
                    && (1..=64).contains(&case.max_rows)
                    && case.expected_rows.len() <= case.max_rows as usize,
                "Invalid bounded Cosmos case"
            );
            if let Some(values) = &case.partition {
                ensure!(
                    values.len() == 1
                        && values.iter().all(|v| v.is_null()
                            || v.is_string()
                            || v.is_number()
                            || v.is_boolean()),
                    "Partition values must be explicit scalar JSON values"
                );
            }
        }
        let encoded = serde_json::to_string(self)?;
        ensure!(encoded.len() <= 32768, "Cosmos contract exceeds bound");
        crate::safety::require_persistable(&encoded)?;
        Ok(())
    }
}
impl QueryFile {
    pub fn parse(text: &str, contract: &Contract) -> Result<Self> {
        contract.validate()?;
        ensure!(text.len() <= 16384, "Cosmos query file exceeds bound");
        crate::safety::require_persistable(text)?;
        let parsed: Self = serde_json::from_value(crate::strict_json::parse(text)?)?;
        crate::safety::require_persistable(&serde_json::to_string(&parsed)?)?;
        ensure!(
            parsed.schema_version == 1 && parsed.queries.len() == contract.cases.len(),
            "Every frozen case requires exactly one query"
        );
        let mut ids = BTreeSet::new();
        for query in &parsed.queries {
            ensure!(
                component(&query.id)
                    && ids.insert(&query.id)
                    && contract.cases.iter().any(|c| c.id == query.id),
                "Unknown/duplicate Cosmos query case"
            );
            // The service's query endpoint accepts SELECT only. This conservative
            // lexical subset additionally excludes comments, batches and UDFs.
            let upper = query.sql.to_ascii_uppercase();
            ensure!(
                query.sql.len() <= 4096
                    && query.sql.is_ascii()
                    && upper.trim_start().starts_with("SELECT ")
                    && !query.sql.chars().any(|c| c.is_control() || c == ';')
                    && !upper.contains("--")
                    && !upper.contains("/*")
                    && !upper
                        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .any(|token| token == "UDF")
                    && query.parameters.len() <= 16,
                "Only bounded single SELECT queries without comments or UDFs are implemented"
            );
            let mut names = BTreeSet::new();
            for parameter in &query.parameters {
                ensure!(
                    parameter.name.starts_with('@')
                        && component(&parameter.name[1..])
                        && names.insert(&parameter.name)
                        && serde_json::to_vec(&parameter.value)?.len() <= 2048,
                    "Invalid bounded Cosmos parameter"
                );
            }
        }
        Ok(parsed)
    }
}

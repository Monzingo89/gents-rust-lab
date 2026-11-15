//! Pure, bounded flat-table aggregation. No expressions, coercion or I/O.
use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Tool, ToolContext,
};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
pub struct TableAggregate;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Filter {
    field: String,
    equals: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    rows: Vec<Map<String, Value>>,
    filter: Option<Filter>,
    group_by: String,
    value_field: String,
}
pub(super) fn field(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
impl Tool for TableAggregate {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "table_aggregate"
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn description(&self) -> &'static str {
        "Pure bounded flat-table aggregation: exact typed equality filter (null means no filter), group_by and integer value_field. Returns input_rows,matched_rows,excluded_rows,groups:[{key,rows,total}],total,exact:true. Group keys use canonical JSON lexical order; duplicates count as separate rows and zero-total groups remain. Every selected field must exist on every input row, even excluded rows. No floats, coercion, expressions, filesystem or network. Use /groups with json encoding or /total with integer_text for artifacts; never invent aggregate answers during planning."
    }
    fn data_input_fields(&self) -> &'static [&'static str] {
        &["rows"]
    }
    fn deterministic(&self) -> bool {
        true
    }
    fn read_only(&self) -> bool {
        true
    }
    fn result_schema(&self, _arguments: Option<&Value>) -> Option<Value> {
        Some(
            json!({"type":"object","additionalProperties":false,"required":["input_rows","matched_rows","excluded_rows","groups","total","exact"],"properties":{
                "input_rows":{"type":"integer"},"matched_rows":{"type":"integer"},"excluded_rows":{"type":"integer"},"total":{"type":"integer"},"exact":{"type":"boolean","const":true},
                "groups":{"type":"array","maxItems":64,"items":{"type":"object","additionalProperties":false,"required":["key","rows","total"],"properties":{"key":{"oneOf":[{"type":"string"},{"type":"integer"},{"type":"boolean"},{"type":"null"}]},"rows":{"type":"integer"},"total":{"type":"integer"}}}}
            }}),
        )
    }
    fn execute(&self, _context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        ensure!(
            arguments.to_string().len() <= 16384,
            "table input exceeds 16384 bytes"
        );
        crate::safety::require_persistable(&arguments.to_string())?;
        EmbeddedContracts.validate(self.name(), &arguments)?;
        let args: Args = serde_json::from_value(arguments)?;
        ensure!(
            field(&args.group_by)
                && field(&args.value_field)
                && args.filter.as_ref().is_none_or(|f| field(&f.field)),
            "invalid selected table field"
        );
        let mut groups = BTreeMap::<String, (Value, u64, i64)>::new();
        let mut total = 0i64;
        let mut matched = 0u64;
        for row in &args.rows {
            ensure!(
                row.keys().all(|key| field(key)),
                "table field names must contain 1..40 ASCII letters, digits or underscores"
            );
            let key = row
                .get(&args.group_by)
                .context("missing group_by field on an input row")?;
            let value = row
                .get(&args.value_field)
                .context("missing value_field on an input row")?
                .as_i64()
                .context("value_field must be an actual integer on every input row")?;
            let include = if let Some(filter) = &args.filter {
                row.get(&filter.field)
                    .context("missing filter field on an input row")?
                    == &filter.equals
            } else {
                true
            };
            if !include {
                continue;
            }
            let entry = groups
                .entry(serde_json::to_string(key)?)
                .or_insert((key.clone(), 0, 0));
            entry.1 += 1;
            entry.2 = entry.2.checked_add(value).context("group total overflow")?;
            total = total.checked_add(value).context("table total overflow")?;
            matched += 1;
        }
        let result = json!({"input_rows":args.rows.len(),"matched_rows":matched,"excluded_rows":args.rows.len() as u64-matched,"groups":groups.into_values().map(|(key,rows,total)|json!({"key":key,"rows":rows,"total":total})).collect::<Vec<_>>(),"total":total,"exact":true});
        ensure!(
            result.to_string().len() <= 16384,
            "table output exceeds 16384 bytes"
        );
        Ok(result)
    }
}

//! Native migration of the existing bounded exact-arithmetic function.
use crate::{Tool, ToolContext};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
pub struct IntegerArithmetic;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    calculations: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    values: Vec<i64>,
    subtract: i64,
}
impl Tool for IntegerArithmetic {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "integer_arithmetic"
    }
    fn description(&self) -> &'static str {
        "Exact bounded integer sums and differences. Returns {calculations:[{id,sum,difference}],exact:true}; difference = sum - subtract. Preserves each id and row order. Output does not echo input values or subtract fields. No code evaluation or policy decision."
    }
    fn result_schema(&self, arguments: Option<&Value>) -> Option<Value> {
        let length = arguments
            .and_then(|v| v["calculations"].as_array())
            .map_or(12, Vec::len);
        Some(
            json!({"type":"object","additionalProperties":false,"required":["calculations","exact"],"properties":{"exact":{"type":"boolean","const":true},"calculations":{"type":"array","maxItems":length,"items":{"type":"object","additionalProperties":false,"required":["id","sum","difference"],"properties":{"id":{"type":"string"},"sum":{"type":"integer"},"difference":{"type":"integer"}}}}}}),
        )
    }
    fn deterministic(&self) -> bool {
        true
    }
    fn read_only(&self) -> bool {
        true
    }
    fn schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"required":["calculations"],"properties":{"calculations":{"type":"array","minItems":1,"maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["id","values","subtract"],"properties":{"id":{"type":"string","minLength":1,"maxLength":40,"pattern":"^[A-Za-z0-9_-]{1,40}$"},"values":{"type":"array","maxItems":12,"items":{"type":"integer","minimum":-10000000,"maximum":10000000}},"subtract":{"type":"integer","minimum":-10000000,"maximum":10000000}}}}}})
    }
    fn execute(&self, _: &ToolContext<'_>, args: Value) -> Result<Value> {
        let args: Args = serde_json::from_value(args)?;
        if args.calculations.is_empty() || args.calculations.len() > 12 {
            bail!("provide 1..12 calculations");
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut result = vec![];
        for row in args.calculations {
            if row.id.is_empty()
                || row.id.len() > 40
                || !row
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !ids.insert(row.id.clone())
            {
                bail!("calculation id must be unique and contain 1..40 ASCII letters, digits, underscores or dashes");
            }
            if row.values.len() > 12
                || row
                    .values
                    .iter()
                    .chain(std::iter::once(&row.subtract))
                    .any(|v| !(-10000000..=10000000).contains(v))
            {
                bail!("integers exceed bounded input limits");
            }
            let sum: i64 = row.values.iter().sum();
            result.push(json!({"id":row.id,"sum":sum,"difference":sum-row.subtract}));
        }
        Ok(json!({"calculations":result,"exact":true}))
    }
}

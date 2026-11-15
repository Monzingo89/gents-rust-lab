//! Pure bounded typed joins with explicit projections and stable source order.
use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Tool, ToolContext,
};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
pub struct TableJoin;
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Side {
    Left,
    Right,
}
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Inner,
    Left,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Column {
    side: Side,
    field: String,
    r#as: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    left_rows: Vec<Map<String, Value>>,
    right_rows: Vec<Map<String, Value>>,
    left_key: String,
    right_key: String,
    kind: Kind,
    columns: Vec<Column>,
}
impl Tool for TableJoin {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "table_join"
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn description(&self) -> &'static str {
        "Pure bounded inner/left join of left_rows and right_rows (each <=64 flat scalar records). left_key/right_key use exact JSON scalar equality: booleans, strings and integers stay distinct; null matches null. Duplicate matches multiply; never deduplicate. columns:[{side:left|right,field,as}] explicitly selects and renames 1..16 unique output fields. Every key/selected field must exist on every corresponding input row, including unmatched rows. Inner joins discard unmatched left records, but unmatched_left_rows still counts those original left records. Left joins preserve unmatched left records and project absent selected right fields as null. Choose inner when unmatched input records must be discarded, and left when they must be retained. Stable order: original left rows, then original matching right rows. Returns rows,left_rows,right_rows,matched_left_rows,unmatched_left_rows,exact:true. At most 64 output rows and 16384 input/output bytes; overflow fails without a successful partial result. No coercion, expressions, I/O, network or policy decisions. Feed actual /rows into downstream tools via approved argument_sources."
    }
    fn data_input_fields(&self) -> &'static [&'static str] {
        &["left_rows", "right_rows"]
    }
    fn deterministic(&self) -> bool {
        true
    }
    fn read_only(&self) -> bool {
        true
    }
    fn result_schema(&self, _arguments: Option<&Value>) -> Option<Value> {
        Some(
            json!({"type":"object","additionalProperties":false,"required":["rows","left_rows","right_rows","matched_left_rows","unmatched_left_rows","exact"],"properties":{
                "rows":EmbeddedContracts.input_schema("table_aggregate").unwrap()["properties"]["rows"],
                "left_rows":{"type":"integer"},"right_rows":{"type":"integer"},"matched_left_rows":{"type":"integer"},"unmatched_left_rows":{"type":"integer"},"exact":{"type":"boolean","const":true}
            }}),
        )
    }
    fn execute(&self, _context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        ensure!(
            arguments.to_string().len() <= 16384,
            "join input exceeds 16384 bytes"
        );
        crate::safety::require_persistable(&arguments.to_string())?;
        EmbeddedContracts.validate(self.name(), &arguments)?;
        let args: Args = serde_json::from_value(arguments)?;
        let valid = super::table::field;
        ensure!(
            valid(&args.left_key) && valid(&args.right_key),
            "invalid join key field"
        );
        let mut aliases = BTreeSet::new();
        for column in &args.columns {
            ensure!(
                valid(&column.field) && valid(&column.r#as) && aliases.insert(&column.r#as),
                "invalid or duplicate join output field"
            );
        }
        for (side, rows, key) in [
            (Side::Left, &args.left_rows, &args.left_key),
            (Side::Right, &args.right_rows, &args.right_key),
        ] {
            for row in rows {
                ensure!(
                    row.keys().all(|field| valid(field)),
                    "invalid join input field name"
                );
                ensure!(row.contains_key(key), "missing join key on an input row");
                for column in args.columns.iter().filter(|c| c.side == side) {
                    ensure!(
                        row.contains_key(&column.field),
                        "missing selected join field on an input row"
                    );
                }
            }
        }
        let mut output = Vec::new();
        let mut matched = 0;
        let project =
            |left: &Map<String, Value>, right: Option<&Map<String, Value>>| -> Result<Value> {
                let mut row = Map::new();
                for column in &args.columns {
                    let value = match column.side {
                        Side::Left => left
                            .get(&column.field)
                            .context("missing selected left field")?,
                        Side::Right => match right {
                            Some(right) => right
                                .get(&column.field)
                                .context("missing selected right field")?,
                            None => &Value::Null,
                        },
                    };
                    row.insert(column.r#as.clone(), value.clone());
                }
                Ok(Value::Object(row))
            };
        for left in &args.left_rows {
            let mut found = false;
            // At most 4096 comparisons; explicit bounded iteration preserves
            // typed equality and source order without a coercing key encoder.
            for right in &args.right_rows {
                if left[&args.left_key] != right[&args.right_key] {
                    continue;
                }
                ensure!(
                    output.len() < 64,
                    "join output exceeds 64 rows; no partial result accepted"
                );
                output.push(project(left, Some(right))?);
                found = true;
            }
            if found {
                matched += 1;
            } else if args.kind == Kind::Left {
                ensure!(
                    output.len() < 64,
                    "join output exceeds 64 rows; no partial result accepted"
                );
                output.push(project(left, None)?);
            }
        }
        let result = json!({"rows":output,"left_rows":args.left_rows.len(),"right_rows":args.right_rows.len(),"matched_left_rows":matched,"unmatched_left_rows":args.left_rows.len()-matched,"exact":true});
        ensure!(
            result.to_string().len() <= 16384,
            "join output exceeds 16384 bytes"
        );
        Ok(result)
    }
}

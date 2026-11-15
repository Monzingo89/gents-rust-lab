//! Complete bounded workspace JSON tables; never aggregate a truncated page.
use crate::{
    contracts::{validate_value, EmbeddedContracts, ToolContracts},
    workspace, Tool, ToolContext,
};
use anyhow::{ensure, Context, Result};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{json, Map, Value};
use std::fmt;

pub struct TableReadJson;

// Value's ordinary object decoder accepts duplicate keys. Source data must not
// silently lose a column; check decoded names too (e.g. "x" and "\u0078").
struct Row(Map<String, Value>);
impl<'de> Deserialize<'de> for Row {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct RowVisitor;
        impl<'de> Visitor<'de> for RowVisitor {
            type Value = Row;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a flat table record with unique field names")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut fields: M,
            ) -> std::result::Result<Row, M::Error> {
                let mut row = Map::new();
                while let Some((key, value)) = fields.next_entry::<String, Value>()? {
                    if row.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate table field"));
                    }
                }
                Ok(Row(row))
            }
        }
        decoder.deserialize_map(RowVisitor)
    }
}
fn rows_schema() -> Value {
    EmbeddedContracts.input_schema("table_aggregate").unwrap()["properties"]["rows"].clone()
}
impl Tool for TableReadJson {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "table_read_json"
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn description(&self) -> &'static str {
        "Read one complete UTF-8 JSON array of flat scalar records from a confined run-workspace path, at most 8192 source bytes, 64 rows, 16 fields/row. Rejects truncated pages, trailing data, duplicate decoded keys, floats, nested values and invalid field names. Returns rows (for a table_aggregate rows argument_source), source_path, source_sha256 and source_bytes. Preserves row order, duplicates and scalar types; never computes totals or executes content. Optional expected_sha256 pins complete source bytes to an approved lowercase SHA-256; a mismatch imports nothing. Read-only specific authority; no arbitrary filesystem paths, symlinks, subprocesses or network."
    }
    fn read_only(&self) -> bool {
        true
    }
    fn file_input_fields(&self) -> &'static [&'static str] {
        &["path"]
    }
    fn result_schema(&self, _arguments: Option<&Value>) -> Option<Value> {
        Some(
            json!({"type":"object","additionalProperties":false,"required":["rows","source_path","source_sha256","source_bytes"],"properties":{
                "rows":rows_schema(),"source_path":{"type":"string"},"source_sha256":{"type":"string","minLength":64,"maxLength":64},"source_bytes":{"type":"integer","minimum":0,"maximum":8192}
            }}),
        )
    }
    fn execute(&self, context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &arguments)?;
        let path = arguments["path"].as_str().context("missing table path")?;
        let root = context.authority.workspace(context.scope)?;
        let expected = arguments.get("expected_sha256").and_then(Value::as_str);
        if let Some(hash) = expected {
            ensure!(
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "expected_sha256 must be 64 lowercase hexadecimal characters"
            );
        }
        let page = workspace::open(&root, false)?.read_complete(path, 8192)?;
        if let Some(hash) = expected {
            ensure!(
                page["sha256"] == hash,
                "source hash differs from the approved expected_sha256; no rows were imported"
            );
        }
        ensure!(
            page["truncated"] == false
                && page["offset_bytes"] == 0
                && page["bytes"] == page["total_bytes"],
            "JSON table requires a complete file of at most 8192 bytes"
        );
        let text = page["text"].as_str().context("missing UTF-8 source")?;
        let rows = validate_text(text)?;
        let result = json!({"rows":rows,"source_path":path,"source_sha256":page["sha256"],"source_bytes":page["total_bytes"]});
        ensure!(
            result.to_string().len() <= 16384,
            "JSON table result exceeds 16384 bytes"
        );
        Ok(result)
    }
}

/// Validate owner-attached tables with exactly the native import rules. No I/O or execution.
pub fn validate_text(text: &str) -> Result<Value> {
    ensure!(text.len() <= 8192, "JSON table exceeds 8192 source bytes");
    let mut decoder = serde_json::Deserializer::from_str(text);
    // Do not propagate serde's source-dependent messages into retained
    // diagnostics; source text stays in the governed result channel.
    let decoded = Vec::<Row>::deserialize(&mut decoder)
        .map_err(|_| anyhow::anyhow!("invalid JSON table: expected unique-key flat records"))?;
    decoder
        .end()
        .map_err(|_| anyhow::anyhow!("JSON table has trailing data"))?;
    let rows = Value::Array(
        decoded
            .into_iter()
            .map(|row| Value::Object(row.0))
            .collect(),
    );
    // Recheck before schema diagnostics: decoded identifiers must not leak
    // through validation paths either.
    crate::safety::require_persistable(&rows.to_string())?;
    ensure!(
        rows.as_array().unwrap().iter().all(|row| row
            .as_object()
            .unwrap()
            .keys()
            .all(|key| super::table::field(key))),
        "invalid table field name"
    );
    validate_value(&rows_schema(), &rows, "table rows")?;
    Ok(rows)
}

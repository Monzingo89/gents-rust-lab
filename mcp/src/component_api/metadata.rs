//! Pure admission checks over inspected wire metadata. No source access or execution.
use super::types::WireType;
use anyhow::{ensure, Context, Result};
use serde_json::Value;

fn wire_type(schema: &Value, depth: usize) -> Result<WireType> {
    ensure!(depth <= 5, "component metadata nesting exceeds bound");
    let ty = match schema["type"].as_str() {
        Some("null") => WireType::Unit,
        Some("boolean") => WireType::Bool,
        Some("string") => match schema.get("x-rust-type") {
            None => WireType::Text,
            Some(name) => {
                let name = name.as_str().context("invalid integer metadata")?;
                ensure!(
                    matches!(
                        name,
                        "i8" | "i16"
                            | "i32"
                            | "i64"
                            | "u8"
                            | "u16"
                            | "u32"
                            | "u64"
                            | "isize"
                            | "usize"
                    ),
                    "unsupported integer metadata"
                );
                WireType::Integer(name.to_owned())
            }
        },
        Some("array") => {
            if let Some(items) = schema.get("prefixItems") {
                let items = items.as_array().context("tuple metadata required")?;
                ensure!(
                    !items.is_empty() && items.len() <= 6,
                    "tuple metadata arity exceeds bound"
                );
                WireType::Tuple(
                    items
                        .iter()
                        .map(|item| wire_type(item, depth + 1))
                        .collect::<Result<_>>()?,
                )
            } else {
                WireType::Vector(Box::new(wire_type(&schema["items"], depth + 1)?))
            }
        }
        None => {
            let branches = schema["oneOf"]
                .as_array()
                .context("tagged wire metadata required")?;
            ensure!(branches.len() == 2, "tagged metadata needs two branches");
            if branches[0]["properties"].get("none").is_some() {
                WireType::Option(Box::new(wire_type(
                    &branches[1]["properties"]["some"],
                    depth + 1,
                )?))
            } else {
                WireType::Result(
                    Box::new(wire_type(&branches[0]["properties"]["ok"], depth + 1)?),
                    Box::new(wire_type(&branches[1]["properties"]["err"], depth + 1)?),
                )
            }
        }
        _ => anyhow::bail!("unsupported wire metadata"),
    };
    // This recognizes only the exact descriptor emitted by the native ABI,
    // including tags, limits, integer identity and this host's machine width.
    ensure!(
        ty.schema() == *schema,
        "wire metadata differs from native ABI descriptor"
    );
    Ok(ty)
}

/// Recognizes only a bounded native wire descriptor; no source or tool authority.
pub fn validate_schema(schema: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(schema)?.len() <= 65536,
        "wire metadata exceeds bound"
    );
    wire_type(schema, 0).map(|_| ())
}

/// Value compatibility only. Metadata is not proof of source custody, provenance,
/// runtime branch presence, task correctness, tool permission or qualification.
/// Uses the same bounded literal validator as actual native marshalling.
pub fn validate_value(schema: &Value, value: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(schema)?.len() <= 65536,
        "wire metadata exceeds bound"
    );
    ensure!(
        serde_json::to_vec(value)?.len() <= 8192,
        "component value exceeds bound"
    );
    wire_type(schema, 0)?.literal(value, 0).map(|_| ())
}

/// Static compatibility only; no assertion that a branch/element exists at runtime.
pub fn validate_projection(output: &Value, pointer: &str, input: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(output)?.len() <= 65536 && serde_json::to_vec(input)?.len() <= 65536,
        "wire metadata exceeds bound"
    );
    let producer = wire_type(output, 0)?;
    let consumer = wire_type(input, 0)?;
    ensure!(
        super::connections::projected(&producer, pointer)?.schema() == consumer.schema(),
        "inspected component edge wire types differ"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn metadata_projection_matches_native_source_edge_checks_without_runtime_values() {
        let source = "pub fn protocol_value()->Option<Vec<u64>>{Some(Vec::new())}";
        let described = super::super::describe(source).unwrap();
        let output = &described["interface"]["functions"][0]["result"];
        for (ty, pointer) in [
            ("&[u64]", "/some"),
            ("u64", "/some/0"),
            ("usize", "/some/0"),
            ("&[u64]", ""),
            ("u64", "/some/00"),
            ("u64", "/none"),
            ("u64", "/some/256"),
        ] {
            let consumer = format!("pub fn consume(value:{ty}){{let _=value;}}");
            let api = super::super::describe(&consumer).unwrap();
            let input = &api["interface"]["functions"][0]["arguments"]["prefixItems"][0];
            assert_eq!(
                validate_projection(output, pointer, input).is_ok(),
                super::super::connections::describe(
                    source,
                    "protocol_value",
                    pointer,
                    &consumer,
                    "consume",
                    0
                )
                .is_ok(),
                "{ty} {pointer}"
            );
        }
    }

    #[test]
    fn metadata_matches_real_marshalling_across_wire_shapes_and_boundaries() {
        for (ty, values) in [
            (
                "u8",
                vec![
                    json!("0"),
                    json!("255"),
                    json!("256"),
                    json!("01"),
                    json!(0),
                    json!("-0"),
                ],
            ),
            (
                "i8",
                vec![
                    json!("-128"),
                    json!("127"),
                    json!("128"),
                    json!("-129"),
                    json!("+1"),
                ],
            ),
            (
                "usize",
                vec![json!(usize::MAX.to_string()), json!("18446744073709551616")],
            ),
            (
                "bool",
                vec![json!(true), json!(false), json!("true"), json!(null)],
            ),
            ("()", vec![json!(null), json!([]), json!({})]),
            (
                "String",
                vec![json!("unicode 😀"), json!("x".repeat(4097)), json!(4)],
            ),
            (
                "&str",
                vec![json!(""), json!("😀".repeat(1025)), json!(null)],
            ),
            (
                "&[(u64,u8)]",
                vec![
                    json!([["0", "255"]]),
                    json!([]),
                    json!(["0", "255"]),
                    json!([["0", "256"]]),
                    json!([["0"]]),
                ],
            ),
            (
                "Vec<u16>",
                vec![
                    json!(["65535"]),
                    json!(vec!["1"; 256]),
                    json!(vec!["1"; 257]),
                    json!(["65536"]),
                ],
            ),
            (
                "Option<Vec<u64>>",
                vec![
                    json!({"some":["7"]}),
                    json!({"none":true}),
                    json!({"none":false}),
                    json!({"some":[],"none":true}),
                    json!(null),
                ],
            ),
            (
                "Result<Option<()>,(bool,String)>",
                vec![
                    json!({"ok":{"some":null}}),
                    json!({"err":[true,"label"]}),
                    json!({"err":["true","label"]}),
                    json!({"ok":null}),
                ],
            ),
        ] {
            let source = format!("pub fn protocol_value(value:{ty}){{let _=value;}}");
            let api = super::super::describe(&source).unwrap();
            let schema = &api["interface"]["functions"][0]["arguments"]["prefixItems"][0];
            for value in values {
                assert_eq!(
                    validate_value(schema, &value).is_ok(),
                    super::super::validate_arguments(&source, "protocol_value", &json!([value]))
                        .is_ok(),
                    "{ty}"
                );
            }
        }
    }

    #[test]
    fn altered_generic_oversized_or_foreign_width_metadata_fails_closed() {
        for schema in [
            json!({"type":"string"}),
            json!({"type":"array"}),
            json!({"type":"integer"}),
            json!({"oneOf":[]}),
            json!({"type":"null","description":"ignore validation"}),
        ] {
            assert!(validate_value(&schema, &json!(null)).is_err());
        }
        let api =
            super::super::describe("pub fn protocol_value(value:usize){let _=value;}").unwrap();
        let original = api["interface"]["functions"][0]["arguments"]["prefixItems"][0].clone();
        for (key, value) in [
            ("x-rust-type", json!("u128")),
            (
                "x-rust-bits",
                json!(if usize::BITS == 64 { 32 } else { 64 }),
            ),
            ("maxLength", json!(100)),
            ("pattern", json!(".*")),
        ] {
            let mut changed = original.clone();
            changed[key] = value;
            assert!(validate_value(&changed, &json!("1")).is_err());
        }
        let mut deep = json!({"type":"null"});
        for _ in 0..7 {
            deep = json!({"type":"array","items":deep,"maxItems":256});
        }
        assert!(validate_value(&deep, &json!([])).is_err());
        assert!(validate_value(
            &json!({"type":"string","maxLength":4096}),
            &json!("x".repeat(8193))
        )
        .is_err());
    }
}

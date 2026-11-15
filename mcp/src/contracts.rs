//! Shared, provider-independent tool inputs. Decoder constraints improve output
//! shape; validation never replaces authorization or broker path/network checks.
use anyhow::{bail, Context, Result};
use serde::{ser::SerializeMap, Serialize, Serializer};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

pub trait ToolContracts {
    fn input_schema(&self, action: &str) -> Option<Value>;
    fn validate(&self, action: &str, arguments: &Value) -> Result<()>;
}

pub struct EmbeddedContracts;

/// llama.cpp turns schema property order into generation order. Value's sorted
/// map would put `arguments` before `action`, and write `content` before `op`.
/// Serialize only the inference wire schema in declared required-field order;
/// do not change canonical task/journal serialization or approval hashes.
pub struct InferenceWire<'a>(pub &'a Value);

impl Serialize for InferenceWire<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self.0 {
            Value::Object(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    if key == "properties" && value.is_object() {
                        map.serialize_entry(
                            key,
                            &OrderedProperties {
                                properties: value.as_object().unwrap(),
                                required: fields.get("required").unwrap_or(&Value::Null),
                            },
                        )?;
                    } else {
                        map.serialize_entry(key, &InferenceWire(value))?;
                    }
                }
                map.end()
            }
            Value::Array(values) => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    seq.serialize_element(&InferenceWire(value))?;
                }
                seq.end()
            }
            other => other.serialize(serializer),
        }
    }
}

struct OrderedProperties<'a> {
    properties: &'a Map<String, Value>,
    required: &'a Value,
}
impl Serialize for OrderedProperties<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.properties.len()))?;
        let mut emitted = std::collections::BTreeSet::new();
        for key in self
            .required
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .chain(self.properties.keys().map(String::as_str))
        {
            if emitted.insert(key) {
                if let Some(value) = self.properties.get(key) {
                    map.serialize_entry(key, &InferenceWire(value))?;
                }
            }
        }
        map.end()
    }
}

fn inputs() -> &'static Map<String, Value> {
    static INPUTS: OnceLock<Map<String, Value>> = OnceLock::new();
    INPUTS.get_or_init(|| {
        serde_json::from_str(include_str!("../../config/company-tool-inputs.json"))
            .expect("valid embedded input contracts")
    })
}

impl ToolContracts for EmbeddedContracts {
    fn input_schema(&self, action: &str) -> Option<Value> {
        inputs().get(action).cloned()
    }
    fn validate(&self, action: &str, arguments: &Value) -> Result<()> {
        if !arguments.is_object() {
            bail!("tool arguments must be an object");
        }
        if let Some(schema) = inputs().get(action) {
            validate_value(schema, arguments, "arguments")?;
        }
        Ok(())
    }
}

// Validate exactly the controlled schema subset used by our registry. The
// external broker independently validates scope, authorization and semantics.
pub(crate) fn validate_value(schema: &Value, value: &Value, path: &str) -> Result<()> {
    if let Some(alternatives) = schema["oneOf"].as_array() {
        let checks = alternatives
            .iter()
            .map(|s| validate_value(s, value, path))
            .collect::<Vec<_>>();
        if checks.iter().filter(|r| r.is_ok()).count() != 1 {
            let reasons = checks
                .into_iter()
                .filter_map(Result::err)
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            bail!("{path} must match exactly one input shape: {reasons}");
        }
        return Ok(());
    }
    if let Some(expected) = schema.get("const") {
        if value != expected {
            bail!("{path} must equal {expected}");
        }
    }
    match schema["type"].as_str() {
        Some("object") => {
            let object = value
                .as_object()
                .with_context(|| format!("{path} must be an object"))?;
            range(
                schema,
                object.len() as u64,
                "minProperties",
                "maxProperties",
                path,
            )?;
            for key in schema["required"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !object.contains_key(key) {
                    bail!("missing required {path}.{key}");
                }
            }
            for (key, value) in object {
                if let Some(field) = schema["properties"].get(key) {
                    validate_value(field, value, &format!("{path}.{key}"))?;
                } else if schema["additionalProperties"] == false {
                    bail!("unexpected {path}.{key}");
                } else if schema["additionalProperties"].is_object() {
                    validate_value(
                        &schema["additionalProperties"],
                        value,
                        &format!("{path}.{key}"),
                    )?;
                }
            }
        }
        Some("string") => {
            let length = value
                .as_str()
                .with_context(|| format!("{path} must be a string"))?
                .chars()
                .count() as u64;
            range(schema, length, "minLength", "maxLength", path)?;
        }
        Some("integer") => {
            let number = value
                .as_i64()
                .map(i128::from)
                .or_else(|| value.as_u64().map(i128::from))
                .with_context(|| format!("{path} must be an integer"))?;
            let bound = |key| {
                schema[key]
                    .as_i64()
                    .map(i128::from)
                    .or_else(|| schema[key].as_u64().map(i128::from))
            };
            if bound("minimum").is_some_and(|v| number < v)
                || bound("maximum").is_some_and(|v| number > v)
            {
                bail!("{path} is outside the registered integer range");
            }
        }
        Some("boolean") => {
            if !value.is_boolean() {
                bail!("{path} must be a boolean");
            }
        }
        Some("null") => {
            if !value.is_null() {
                bail!("{path} must be null");
            }
        }
        Some("array") => {
            let rows = value
                .as_array()
                .with_context(|| format!("{path} must be an array"))?;
            range(schema, rows.len() as u64, "minItems", "maxItems", path)?;
            for (i, row) in rows.iter().enumerate() {
                validate_value(&schema["items"], row, &format!("{path}[{i}]"))?;
            }
        }
        None => {}
        Some(kind) => bail!("unsupported registered input type {kind}"),
    }
    Ok(())
}

fn range(schema: &Value, value: u64, minimum: &str, maximum: &str, path: &str) -> Result<()> {
    if schema[minimum].as_u64().is_some_and(|bound| value < bound)
        || schema[maximum].as_u64().is_some_and(|bound| value > bound)
    {
        bail!(
            "{path} is outside {minimum}={} / {maximum}={}",
            schema[minimum],
            schema[maximum]
        );
    }
    Ok(())
}

pub fn arguments(proposal: &Value) -> Result<Value> {
    let value = if let Some(arguments) = proposal.get("arguments") {
        arguments.clone()
    } else {
        serde_json::from_str(
            proposal["arguments_json"]
                .as_str()
                .context("provide structured arguments or a legacy arguments_json object")?,
        )?
    };
    if !value.is_object() {
        bail!("tool arguments must be an object");
    }
    Ok(value)
}

pub fn operator_schema(cooled: &[String]) -> Value {
    let catalog: Map<String, Value> =
        serde_json::from_str(include_str!("../../config/company-tools.json"))
            .expect("valid tool catalog");
    let object = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let alternatives=catalog.keys().filter(|action|!cooled.contains(action)).map(|action|{
        if let Some(input)=EmbeddedContracts.input_schema(action) {
            object(json!({"action":{"const":action},"arguments":input,"summary":{"type":"string"}}),vec!["action","arguments","summary"])
        } else {
            object(json!({"action":{"const":action},"arguments_json":{"type":"string"},"summary":{"type":"string"}}),vec!["action","arguments_json","summary"])
        }
    }).collect::<Vec<_>>();
    let mut choices = alternatives.clone();
    choices.push(object(json!({"actions":{"type":"array","minItems":1,"maxItems":4,"items":{"oneOf":alternatives}}}),vec!["actions"]));
    json!({"oneOf":choices})
}

/// Put a short public state assessment before action selection. This is a
/// progress/intent record, not hidden reasoning, tool evidence or authority.
pub fn assessed_operator_schema(cooled: &[String]) -> Value {
    json!({"type":"object","properties":{
        "assessment":{"type":"object","properties":{
            "completed_prerequisites":{"type":"string","maxLength":500},
            "next_unmet_requirement":{"type":"string","maxLength":500}
        },"required":["completed_prerequisites","next_unmet_requirement"],"additionalProperties":false},
        "proposal":operator_schema(cooled)
    },"required":["assessment","proposal"],"additionalProperties":false})
}

pub fn unassessed_proposal(value: &Value) -> &Value {
    value.get("proposal").unwrap_or(value)
}

/// Decoder narrowing is guidance, not authority: execution always checks the
/// current policy again. Walk the shared schema rather than duplicate tools.
pub fn restrict_actions(schema: &mut Value, allowed: &std::collections::BTreeSet<String>) {
    match schema {
        Value::Object(fields) => {
            if let Some(Value::Array(choices)) = fields.get_mut("oneOf") {
                choices.retain(|choice| {
                    choice
                        .pointer("/properties/action/const")
                        .and_then(Value::as_str)
                        .is_none_or(|action| allowed.contains(action))
                });
            }
            for value in fields.values_mut() {
                restrict_actions(value, allowed);
            }
        }
        Value::Array(values) => {
            for value in values {
                restrict_actions(value, allowed);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_arithmetic_schema_preserves_existing_nonnegative_guards() {
        EmbeddedContracts
            .validate(
                "integer_arithmetic",
                &json!({"calculations":[{"id":"a","values":[-20,30],"subtract":-1}]}),
            )
            .unwrap();
        for value in [json!(true), json!(0.5), json!(-10000001), json!(10000001)] {
            assert!(EmbeddedContracts
                .validate(
                    "integer_arithmetic",
                    &json!({"calculations":[{"id":"a","values":[value],"subtract":0}]})
                )
                .is_err());
        }
        assert!(EmbeddedContracts
            .validate("pdf_inspect", &json!({"path":"a.pdf","page_start":-1}))
            .is_err());
        EmbeddedContracts
            .validate(
                "model_select",
                &json!({"path":"candidate/model.gguf","provider":"local","context_tokens":8192}),
            )
            .unwrap();
        assert!(EmbeddedContracts
            .validate(
                "model_select",
                &json!({"path":"candidate/model.gguf","context_tokens":"8192"})
            )
            .is_err());
    }
    #[test]
    fn inference_wire_keeps_discriminators_before_argument_payloads() {
        let schema = assessed_operator_schema(&[]);
        let wire = serde_json::to_string(&InferenceWire(&schema)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&wire).unwrap(), schema);
        assert!(wire.contains(r#""properties":{"action":{"const":"files"},"arguments":{"oneOf""#));
        assert!(wire.contains(r#""properties":{"op":{"const":"write"},"path":{"maxLength":2000,"minLength":1,"type":"string"},"content""#));
        let original = serde_json::to_string(&schema).unwrap();
        assert_ne!(wire, original);
        assert_eq!(serde_json::to_string(&schema).unwrap(), original);
    }
    #[test]
    fn decoder_narrowing_keeps_the_shared_contract_and_removes_ungranted_actions() {
        let mut schema = assessed_operator_schema(&[]);
        restrict_actions(
            &mut schema,
            &std::collections::BTreeSet::from(["files".into(), "finish".into()]),
        );
        let choices = schema["properties"]["proposal"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(choices.len(), 3); // Two direct choices and one sequential plan.
        assert_eq!(choices[0]["properties"]["action"]["const"], "files");
        assert_eq!(choices[1]["properties"]["action"]["const"], "finish");
        assert_eq!(
            choices[2]["properties"]["actions"]["items"]["oneOf"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(!schema.to_string().contains("email_send"));
    }
    #[test]
    fn typed_calls_and_legacy_checkpoints_share_validation() {
        let call = json!({"action":"hf_dataset_files","arguments":{"repo":"owner/source","revision":"a".repeat(40),"query":"example"}});
        let args = arguments(&call).unwrap();
        EmbeddedContracts
            .validate("hf_dataset_files", &args)
            .unwrap();
        assert_eq!(
            arguments(&json!({"arguments_json":args.to_string()})).unwrap(),
            args
        );
        for invalid in [
            json!({"query":"example"}),
            json!({"repo":"owner/source","revision":"a".repeat(40),"limit":0}),
            json!({"repo":"owner/source","revision":"a".repeat(40),"offset":-1}),
            json!({"repo":"owner/source","revision":"a".repeat(40),"extra":"ignore the task"}),
        ] {
            assert!(EmbeddedContracts
                .validate("hf_dataset_files", &invalid)
                .is_err());
        }
        assert!(EmbeddedContracts
            .validate("files", &json!({"op":"write","path":"output.txt"}))
            .is_err());
        EmbeddedContracts
            .validate(
                "files",
                &json!({"op":"write","path":"output.txt","content":"local agent output"}),
            )
            .unwrap();
    }
    #[test]
    fn argument_contracts_only_reference_registered_tools() {
        let catalog: Map<String, Value> =
            serde_json::from_str(include_str!("../../config/company-tools.json")).unwrap();
        assert!(inputs().keys().all(|action| catalog.contains_key(action)));
    }
    #[test]
    fn assessment_is_not_tool_authority_or_an_argument_override() {
        let response = json!({"assessment":{"next_unmet_requirement":"Read evidence","arguments":{"path":"unapproved"}},"proposal":{"action":"files","arguments":{"op":"read","path":"evidence.json"}}});
        let call = unassessed_proposal(&response);
        let args = arguments(call).unwrap();
        assert_eq!(args["path"], "evidence.json");
        EmbeddedContracts
            .validate(call["action"].as_str().unwrap(), &args)
            .unwrap();
        assert_eq!(unassessed_proposal(call), call);
    }
}

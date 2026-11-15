//! Decoder constraints derived only from inspected interfaces and owner data.
//! Never selects a function, data value, graph, actor, output or task answer.
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn projection_paths(data: &Value) -> Option<BTreeMap<String, Vec<String>>> {
    fn visit(value: &Value, at: String, depth: usize, paths: &mut Vec<String>) -> Option<()> {
        if crate::owned_selection::pointer(&at).is_err() {
            return Some(()); // Descendants also exceed this same syntax bound.
        }
        paths.push(at.clone());
        if paths.len() > 256 {
            return None;
        }
        if depth == 6 {
            return Some(());
        }
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    visit(child, format!("{at}/{escaped}"), depth + 1, paths)?;
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    visit(child, format!("{at}/{index}"), depth + 1, paths)?;
                }
            }
            _ => (),
        }
        Some(())
    }
    let mut result = BTreeMap::new();
    let mut count = 0;
    for (name, value) in data.as_object()? {
        let mut paths = Vec::new();
        visit(value, String::new(), 0, &mut paths)?;
        count += paths.len();
        if count > 256 {
            return None;
        }
        result.insert(name.clone(), paths);
    }
    if serde_json::to_vec(&result).ok()?.len() > 8192 {
        return None;
    }
    Some(result)
}

/// Returns None unless the existing enumeration is complete and every selected
/// interface can be represented within the grammar budget. Callers then retain
/// their original generic language, rather than silently pruning valid choices.
pub fn calls(components: &Value, data: &Value, path: &Value) -> Option<Value> {
    let fields = data.as_object()?;
    let enumerated = projection_paths(data)?;
    let result = json!({"type":"object","additionalProperties":false,"required":["kind","source","pointer"],"properties":{"kind":{"const":"result"},"source":{"type":"string","maxLength":40},"pointer":path}});
    let mut choices = Vec::new();
    let mut identities = BTreeSet::new();
    for component in components.as_array()? {
        let component_id = component["component_id"].as_str()?;
        for function in component["functions"].as_array()? {
            let name = function["name"].as_str()?;
            if !identities.insert((component_id, name)) {
                return None;
            }
            let parameters = function["arguments"]["prefixItems"].as_array()?;
            if parameters.len() > 8 || choices.len() >= 64 {
                return None;
            }
            let mut arguments = Vec::new();
            for parameter in parameters {
                gents_local_mcp::component_api::metadata::validate_schema(parameter).ok()?;
                let mut alternatives = Vec::new();
                for (field, paths) in &enumerated {
                    let mut compatible = Vec::new();
                    for pointer in paths {
                        let value = fields[field].pointer(pointer)?;
                        if gents_local_mcp::component_api::metadata::validate_value(
                            parameter, value,
                        )
                        .is_ok()
                        {
                            compatible.push(pointer.clone());
                        }
                    }
                    if !compatible.is_empty() {
                        alternatives.push(json!({"type":"object","additionalProperties":false,"required":["kind","data","pointer"],"properties":{"kind":{"const":"data"},"data":{"const":field},"pointer":{"type":"string","enum":compatible}}}));
                    }
                }
                // Result references remain model choices. Native review still
                // checks prior identity, projection type and graph safety.
                alternatives.push(result.clone());
                arguments.push(json!({"oneOf":alternatives}));
            }
            choices.push(json!({"type":"object","additionalProperties":false,"required":["id","component_id","function","arguments"],"properties":{
                "id":{"type":"string","pattern":"^[a-z0-9-]+$","maxLength":40},
                "component_id":{"type":"string","const":component_id},"function":{"type":"string","const":name},
                "arguments":{"type":"array","prefixItems":arguments,"minItems":parameters.len(),"maxItems":parameters.len()}
            }}));
            if serde_json::to_vec(&choices).ok()?.len() > 65536 {
                return None;
            }
        }
    }
    if choices.is_empty() {
        return None;
    }
    Some(json!({"oneOf":choices}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn components() -> Value {
        let api = gents_local_mcp::component_api::describe("pub fn matrix(values:&[(u64,u8)]){let _=values;} pub fn scalar(value:u8){let _=value;} pub fn no_arguments(){}").unwrap();
        json!([{"component_id":"a".repeat(64),"functions":api["interface"]["functions"]}])
    }
    fn path() -> Value {
        json!({"type":"string","maxLength":160,"pattern":"^(/.*)?$"})
    }
    #[test]
    fn typed_positional_grammar_keeps_all_compatible_data_choices_and_functions() {
        let data = json!({"north":[["0","7"],["3","9"]],"south":[["2","8"]],"amount":"255","too-large":"256","opaque":{"a/b~c":null},"text":"private payload sentinel"});
        let grammar = calls(&components(), &data, &path()).unwrap();
        let choices = grammar["oneOf"].as_array().unwrap();
        assert_eq!(choices.len(), 3);
        let matrix = &choices[0]["properties"]["arguments"];
        assert_eq!(matrix["minItems"], 1);
        assert_eq!(matrix["maxItems"], 1);
        // The owned backend rejects boolean `items`. Exact min/max bounds make
        // omission equivalent: all allowed elements are covered by prefixItems.
        assert!(matrix.get("items").is_none());
        assert_eq!(
            matrix["prefixItems"].as_array().unwrap().len(),
            matrix["maxItems"].as_u64().unwrap() as usize
        );
        let literals = matrix["prefixItems"][0]["oneOf"].as_array().unwrap();
        assert_eq!(literals.len(), 3); // Both complete owner matrices, plus result references.
        for (literal, key) in literals.iter().take(2).zip(["north", "south"]) {
            assert_eq!(literal["properties"]["data"]["const"], key);
            assert_eq!(literal["properties"]["pointer"]["enum"], json!([""]));
        }
        assert_eq!(literals[2]["properties"]["kind"]["const"], "result");
        assert!(literals[2]["properties"]["source"].get("const").is_none());
        let scalar = &choices[1]["properties"]["arguments"]["prefixItems"][0]["oneOf"];
        let scalar = scalar.as_array().unwrap();
        assert!(scalar
            .iter()
            .any(|s| s["properties"]["data"]["const"] == "amount"));
        assert!(!scalar
            .iter()
            .any(|s| s["properties"]["data"]["const"] == "too-large"));
        assert_eq!(
            choices[2]["properties"]["function"]["const"],
            "no_arguments"
        );
        assert_eq!(choices[2]["properties"]["arguments"]["maxItems"], 0);
        assert!(!grammar.to_string().contains("private payload sentinel"));
        // Check every enumerated projection against source-bound marshalling,
        // including compatible scalar values inside rows, not only whole roots.
        let api = components();
        let paths = projection_paths(&data).unwrap();
        for (function, choice) in api[0]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .zip(choices)
            .take(2)
        {
            let ty = &function["arguments"]["prefixItems"][0];
            for (key, pointers) in &paths {
                for pointer in pointers {
                    let expected = gents_local_mcp::component_api::metadata::validate_value(
                        ty,
                        data[key].pointer(pointer).unwrap(),
                    )
                    .is_ok();
                    let present = choice["properties"]["arguments"]["prefixItems"][0]["oneOf"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|b| {
                            b["properties"]["data"]["const"] == *key
                                && b["properties"]["pointer"]["enum"]
                                    .as_array()
                                    .is_some_and(|ps| ps.contains(&json!(pointer)))
                        });
                    assert_eq!(present, expected, "{key} {pointer}");
                }
            }
        }
    }
    #[test]
    fn enumeration_uses_exact_rfc_pointers_and_runtime_depth_bounds() {
        let data = json!({"owner":{"a/b~c":[null],"":true,"bad\nkey":"hidden"}});
        let paths = projection_paths(&data).unwrap();
        assert_eq!(paths["owner"], vec!["", "/", "/a~1b~0c", "/a~1b~0c/0"]);
        for pointer in &paths["owner"] {
            assert!(data["owner"].pointer(pointer).is_some());
        }
        let deep = json!({"owner":[[[[[[[true]]]]]]]});
        let paths = projection_paths(&deep).unwrap();
        assert_eq!(paths["owner"].len(), 7);
        assert!(!paths["owner"].contains(&"/0/0/0/0/0/0/0".to_string()));
    }
    #[test]
    fn oversized_or_unsupported_enumerations_preserve_generic_fallback() {
        assert!(calls(&components(), &json!({"rows":vec!["1";257]}), &path()).is_none());
        let mut bad = components();
        bad[0]["functions"][0]["arguments"]["prefixItems"][0] = json!({"type":"array"});
        assert!(calls(&bad, &json!({"value":"1"}), &path()).is_none());
        let mut duplicate = components();
        let first = duplicate[0]["functions"][0].clone();
        duplicate[0]["functions"]
            .as_array_mut()
            .unwrap()
            .push(first);
        assert!(calls(&duplicate, &json!({"value":"1"}), &path()).is_none());
        let mut many = components();
        let base = many[0]["functions"][1].clone();
        many[0]["functions"] = json!((0..65)
            .map(|i| {
                let mut f = base.clone();
                f["name"] = json!(format!("function_{i}"));
                f
            })
            .collect::<Vec<_>>());
        assert!(calls(&many, &json!({"value":"1"}), &path()).is_none());
        many[0]["functions"].as_array_mut().unwrap().pop();
        assert!(projection_paths(&json!({"values":vec!["1";240]})).is_some());
        assert!(calls(&many, &json!({"values":vec!["1";240]}), &path()).is_none());
    }
}

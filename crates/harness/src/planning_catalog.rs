//! Prompt catalog selection only. No schema rewriting, execution or authority.
use serde_json::{json, Value};

pub fn for_fixed_inputs(mut catalog: Value, inputs: &Value) -> Value {
    let Some(attached) = inputs.as_array().filter(|inputs| !inputs.is_empty()) else {
        return catalog;
    };
    let allows_transforms = crate::ceo_contracts::allows_derived_sources(inputs);
    // Fixed-input graph syntax cannot introduce new file readers, code
    // execution or side effects. Keep complete root and eligible transform
    // schemas, rather than spending context on unusable source operations.
    if let Some(tools) = catalog["tools"].as_array_mut() {
        tools.retain(|tool| {
            attached.iter().any(|input| input["tool"] == tool["name"])
                || (allows_transforms
                    && tool["read_only"] == true
                    && tool["deterministic"] == true
                    && tool["data_input_fields"]
                        .as_array()
                        .is_some_and(|fields| !fields.is_empty()))
        });
    }
    catalog["planning_view"] = json!({"mode":if allows_transforms {"fixed_inputs_and_pure_transforms"} else {"attached_templates"},"scope":"Prompt metadata only. Complete retained tool schemas; attached roots/templates and eligible transforms. Does not change installed tools, policy, schemas or execution authority."});
    catalog
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_complete_registered_roots_and_only_graph_eligible_transforms() {
        let catalog = gents_local_mcp::Registry::new().catalog();
        let inputs = json!([{"id":"audio","tool":"audio_inspect","arguments":{"path":"a.wav","expected_sha256":"0".repeat(64)}}]);
        let view = for_fixed_inputs(catalog.clone(), &inputs);
        assert!(view.to_string().len() < catalog.to_string().len());
        assert!(view["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "audio_inspect"));
        for tool in view["tools"].as_array().unwrap() {
            assert!(
                tool["name"] == "audio_inspect"
                    || (tool["read_only"] == true
                        && tool["deterministic"] == true
                        && !tool["data_input_fields"].as_array().unwrap().is_empty())
            );
            assert!(catalog["tools"].as_array().unwrap().contains(tool));
        }
        assert_eq!(for_fixed_inputs(catalog.clone(), &json!([])), catalog);
        assert_eq!(for_fixed_inputs(catalog.clone(), &Value::Null), catalog);
    }

    #[test]
    fn attached_templates_hide_unusable_operations_without_rewriting_registered_schemas() {
        let catalog = gents_local_mcp::Registry::new().catalog();
        let inputs = json!([
            {"id":"rows","tool":"table_read_json","arguments":{"path":"data.json","expected_sha256":"0".repeat(64)}},
            {"id":"joined","tool":"table_join","derived_fields":["left_rows","right_rows"]},
            {"id":"summary","tool":"table_aggregate","derived_fields":["rows"]}
        ]);
        let view = for_fixed_inputs(catalog.clone(), &inputs);
        let tools = view["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        for tool in tools {
            assert!(inputs
                .as_array()
                .unwrap()
                .iter()
                .any(|input| input["tool"] == tool["name"]));
            assert!(catalog["tools"].as_array().unwrap().contains(tool));
        }
        assert_eq!(view["planning_view"]["mode"], "attached_templates");
        assert!(view.to_string().len() < catalog.to_string().len());
        for (key, value) in catalog.as_object().unwrap() {
            if key != "tools" {
                assert_eq!(&view[key], value);
            }
        }
    }
}

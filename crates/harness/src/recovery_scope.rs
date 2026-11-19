//! Changing the advisory model must not change the assigned agent or its grants.
use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

pub struct View {
    pub actor: String,
    pub catalog: Value,
    pub assignment: Value,
    pub policy_revision: u64,
}

pub fn for_assignment(
    intake: &Value,
    stage: usize,
    policy: &crate::governance::Policy,
) -> Result<Option<View>> {
    if intake["ceo_plan"].is_null() {
        return Ok(None);
    }
    let stages = crate::ceo_team::parse(&intake["ceo_plan"])?;
    let assigned = stages
        .get(stage)
        .context("recovery stage exceeds frozen CEO team")?;
    let mut capabilities = crate::skills::capabilities_for_phase("worker")?;
    policy.filter(&assigned.actor, false, &mut capabilities);
    capabilities.retain(|capability| {
        capability["id"].as_str().is_some_and(|id| {
            id == "finish"
                || assigned
                    .tools
                    .iter()
                    .any(|tool| tool.split(':').next() == Some(id))
        })
    });
    let mut catalog: Map<String, Value> = capabilities
        .into_iter()
        .filter_map(|capability| {
            let id = capability["id"].as_str()?.to_string();
            Some((id, capability))
        })
        .collect();
    let paths: Vec<Value> = intake["required_rust_checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|check| check["actor"] == assigned.actor)
        .map(|check| check["source_path"].clone())
        .collect();
    if !paths.is_empty() {
        if let Some(tool) = catalog.get_mut("rust_verify") {
            tool["source_bound_dispatch_guard"] = json!(true);
        }
        if let Some(tool) = catalog.get_mut("files") {
            tool["source_bound_read_paths"] = json!(paths);
        }
    }
    if intake["python_check_contracts"]
        .as_array()
        .is_some_and(|contracts| {
            contracts
                .iter()
                .any(|contract| contract["actor"] == assigned.actor)
        })
    {
        if let Some(tool) = catalog.get_mut("python_check") {
            tool["source_bound_dispatch_guard"] = json!(true);
        }
    }
    Ok(Some(View {
        actor: assigned.actor.clone(),
        catalog: Value::Object(catalog),
        assignment: json!(assigned),
        policy_revision: policy.revision,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_keeps_actor_and_current_granular_grants_instead_of_executive_identity() {
        let intake = json!({"ceo_plan":{"assignments":[{"actor":"Author","task":"Implement and verify","tools":["files:read","files:write","rust_verify"],"acceptance":[]}]}});
        let access =
            json!({"tools":["files:read","rust_verify","finish"],"skills":[],"memory_from":[]});
        let policy:crate::governance::Policy=serde_json::from_value(json!({"revision":2,"master":"Author","ceilings":{"Author":access},"grants":{"Author":access}})).unwrap();
        let view = for_assignment(&intake, 0, &policy).unwrap().unwrap();
        assert_eq!(view.actor, "Author");
        assert_eq!(view.policy_revision, 2);
        assert!(view.catalog.get("rust_verify").is_some());
        assert_eq!(
            view.catalog["files"]["granted_operations"],
            json!(["files:read"])
        );
        for denied in [
            "resources",
            "shell",
            "access_delegate",
            "memory_search",
            "model_select",
        ] {
            assert!(view.catalog.get(denied).is_none());
        }
        assert!(for_assignment(&intake, 1, &policy).is_err());
        assert!(for_assignment(&json!({}), 0, &policy).unwrap().is_none());
    }
}

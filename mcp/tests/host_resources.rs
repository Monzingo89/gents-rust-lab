mod support;
use gents_local_mcp::{governance, Phase, Registry};
use serde_json::json;
use support::Fixture;

#[test]
fn host_observations_require_current_grants_and_leave_no_host_identity_payload() {
    let f = Fixture::new();
    let registry = Registry::new();
    assert!(registry
        .call(&f.authority, &f.scope, "host_resources", json!({}))
        .is_err());
    assert!(!registry.list(&f.authority, &f.scope).unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "host_resources"));
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    for access in [
        policy.ceilings.get_mut("Worker").unwrap(),
        policy.grants.get_mut("Worker").unwrap(),
    ] {
        access.tools.insert("host_resources".into());
    }
    policy.revision = 2;
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    let mut scope = f.scope.clone();
    scope.expected_revision = Some(2);
    for phase in [Phase::Planner, Phase::Worker] {
        scope.phase = phase;
        let result = registry
            .call(&f.authority, &scope, "host_resources", json!({}))
            .unwrap();
        let value = &result["value"];
        assert!(value["host_total_bytes"].as_u64().unwrap() > 0);
        assert!(
            value["host_available_bytes"].as_u64().unwrap()
                <= value["host_total_bytes"].as_u64().unwrap()
        );
        assert!(value.get("gpu_memory_bytes").unwrap().is_null());
        assert_eq!(value["provider_cost_usd"], 0);
        assert_eq!(result["receipt"]["provider_cost_usd"], 0);
        for key in ["host_name", "username", "processes", "environ", "cmd"] {
            assert!(value.get(key).is_none());
        }
    }
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "host_resources",
            json!({"command":"not permitted"})
        )
        .is_err());
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("host_resources");
    policy.revision = 3;
    governance::save(&db, &f.scope.run.to_string(), &policy, 2).unwrap();
    assert!(registry
        .call(&f.authority, &scope, "host_resources", json!({}))
        .is_err());
    scope.expected_revision = Some(3);
    assert!(registry
        .call(&f.authority, &scope, "host_resources", json!({}))
        .is_err());
}

#[test]
fn both_transport_catalogs_disclose_observation_limits_and_no_execution_authority() {
    let registry = Registry::new();
    let catalog = registry.catalog();
    let tool = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "host_resources")
        .unwrap();
    assert_eq!(tool["read_only"], true);
    assert_eq!(tool["deterministic"], false);
    assert_eq!(tool["external_effects"], false);
    assert!(tool["description"]
        .as_str()
        .unwrap()
        .contains("GPU memory is unknown"));
    assert_eq!(
        tool["resultSchema"]["properties"]["gpu_memory_bytes"]["type"],
        "null"
    );
}

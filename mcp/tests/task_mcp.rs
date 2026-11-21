use gents_local_mcp::{governance, Registry};
use serde_json::json;
mod support;
use support::Fixture;
#[test]
fn native_tasks_require_current_grants_and_keep_knowledge_actor_private() {
    let f = Fixture::new();
    let registry = Registry::new();
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "destructure_task",
            json!({"input":{"original_ask":"Analyze this task"}})
        )
        .is_err());
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    for map in [&mut policy.ceilings, &mut policy.grants] {
        for actor in ["Worker", "Reader"] {
            map.get_mut(actor).unwrap().tools.extend(
                [
                    "destructure_task",
                    "execute_task",
                    "ingest_sources",
                    "search_knowledge",
                    "get_task_status",
                    "inspect_cache",
                    "invalidate_cache",
                    "list_capabilities",
                ]
                .into_iter()
                .map(str::to_owned),
            );
        }
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    let ask = "Return the release tag.";
    let now = gents_local_mcp::now_ms();
    let source = json!({"source_id":"release","revision":"v1","title":"Synthetic fixture","domain":"general","language":"en","text":"v7","observed_at_ms":now,"valid_until_ms":now+60000,"lookups":[{"exact_ask":ask,"temporal_scope":"","context_independent":true,"start_byte":0,"end_byte":2}]});
    registry
        .call(
            &f.authority,
            &f.scope,
            "ingest_sources",
            json!({"source":source}),
        )
        .unwrap();
    let first = registry
        .call(
            &f.authority,
            &f.scope,
            "execute_task",
            json!({"input":{"original_ask":ask}}),
        )
        .unwrap();
    assert_eq!(first["value"]["result"]["answer"], "v7");
    let second = registry
        .call(
            &f.authority,
            &f.scope,
            "execute_task",
            json!({"input":{"original_ask":ask}}),
        )
        .unwrap();
    assert_eq!(
        second["value"]["result"]["actual_path"],
        "exact_answer_cache"
    );
    assert_eq!(
        second["value"]["result"]["measurements"]["generative_model_calls"],
        0
    );
    let reader = registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "execute_task",
            json!({"input":{"original_ask":ask}}),
        )
        .unwrap();
    assert!(reader["value"]["result"]["answer"].is_null());
    let injected = registry.call(
        &f.authority,
        &f.scope,
        "destructure_task",
        json!({"input":{"original_ask":ask},"policy":{"max_model_calls":999}}),
    );
    assert!(injected.is_err());
    let mut stale = f.scope.clone();
    stale.expected_revision = Some(previous);
    assert!(registry
        .call(&f.authority, &stale, "inspect_cache", json!({}))
        .is_err());
    let catalog = registry.list(&f.authority, &f.scope).unwrap();
    let tool = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "execute_task")
        .unwrap();
    assert!(!tool["inputSchema"].to_string().contains("$ref"));
}

#![cfg(target_os = "macos")]
// Protocol-only sources and owner-authored graphs; no model/task competence claim.
mod support;
use gents_local_mcp::{authority::Authority, completion, governance, Registry};
use serde_json::{json, Value};
use support::Fixture;
const PRODUCER: &str =
    "pub fn protocol_source(x:u64)->Option<Vec<u64>>{if x==0{None}else{Some(Vec::from([x]))}}";
const CONSUMER: &str = "pub fn protocol_consumer(values:&[u64],label:u64)->u64{values[0]+label}";
fn setup(value: &str, consumer: &str) -> Fixture {
    setup_with_source(value, PRODUCER, consumer)
}
fn setup_with_source(value: &str, producer: &str, consumer: &str) -> Fixture {
    let mut f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let p = gents_local_mcp::component_api::describe(producer).unwrap();
    let c = gents_local_mcp::component_api::describe(consumer).unwrap();
    let recipe = json!({"arguments":[[],"2"],"sources":[{"field":"arguments","path":"producer.json","argument_index":0,"pointer":"/some"}]});
    let mut intake = json!({"required_mcp_calls":[{"actor":"Worker","tool":"owned_rust_call"},{"actor":"Reader","tool":"owned_rust_call"}],"required_owned_rust_calls":[],"artifact_bindings":[
     {"path":"producer.json","source_actor":"Worker","writer":"Reader","source_tool":"owned_rust_call","arguments":{"binding_id":"producer","arguments":[value]},"pointer":"/output","encoding":"json"},
     {"path":"consumer.json","source_actor":"Reader","writer":"Reader","source_tool":"owned_rust_call","arguments":{"binding_id":"consumer","arguments":[[],"2"]},"pointer":"/output","encoding":"json","argument_sources":recipe["sources"]}
    ]});
    let mut specs = vec![];
    for (id, actor, path, d, function) in [
        (
            "producer",
            "Worker",
            "owned/producer.rs",
            p,
            "protocol_source",
        ),
        (
            "consumer",
            "Reader",
            "owned/consumer.rs",
            c,
            "protocol_consumer",
        ),
    ] {
        let mut spec = json!({"id":id,"actor":actor,"source_path":path,"source_sha256":d["source_sha256"],"api_sha256":d["api_sha256"],"function":function,"owner_declared_review_package_id":"a".repeat(64)});
        if id == "producer" {
            spec["input_sha256"] = json!(gents_local_mcp::digest(
                json!([value]).to_string().as_bytes()
            ));
        } else {
            spec["input_recipe"] = recipe.clone();
        }
        specs.push(spec);
    }
    intake["required_owned_rust_calls"] = json!(specs);
    completion::validate(&intake).unwrap();
    gents_local_mcp::artifact_bindings::parse(&intake).unwrap();
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let old = policy.revision;
    policy.revision += 1;
    for map in [&mut policy.ceilings, &mut policy.grants] {
        for actor in ["Worker", "Reader"] {
            let grant = map.get_mut(actor).unwrap();
            grant.tools.insert("owned_rust_call".into());
            grant.contract_execution_phases.insert("worker".into());
        }
        map.get_mut("Reader")
            .unwrap()
            .tools
            .insert("files:write".into());
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, old).unwrap();
    f.scope.expected_revision = Some(policy.revision);
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), true).unwrap();
    work.write("owned/producer.rs", producer).unwrap();
    work.write("owned/consumer.rs", consumer).unwrap();
    f
}
fn root(f: &Fixture, value: &str) -> Value {
    Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "owned_rust_call",
            json!({"binding_id":"producer","arguments":[value]}),
        )
        .unwrap()
}
fn child(f: &Fixture) -> anyhow::Result<Value> {
    Registry::new().call(
        &f.authority,
        &f.actor("Reader"),
        "owned_rust_call",
        json!({"binding_id":"consumer","arguments":[["7"],"2"]}),
    )
}
#[test]
fn canonical_component_result_fills_one_slot_and_preserves_the_other_literal() {
    let f = setup("7", CONSUMER);
    assert!(child(&f).is_err());
    let produced = root(&f, "7");
    assert_eq!(
        produced["value"]["output"],
        json!({"some":["7"]}),
        "{produced}"
    );
    assert!(!f
        .authority
        .workspace(&f.scope)
        .unwrap()
        .join("producer.json")
        .exists());
    let result = child(&f).unwrap();
    assert_eq!(result["value"]["output"], "9");
    assert_eq!(result["receipt"]["contract_enforced"], true);
    assert_eq!(
        result["value"]["input_sha256"],
        gents_local_mcp::digest(b"[[\"7\"],\"2\"]")
    );
    assert_eq!(
        result["value"]["input_lineage"][0]["result_sha256"],
        produced["receipt"]["result_sha256"]
    );
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
    // Publication is independently governed and reads back exact native outputs.
    for (path, output) in [
        ("producer.json", produced["value"]["output"].clone()),
        ("consumer.json", result["value"]["output"].clone()),
    ] {
        Registry::new()
            .call(
                &f.authority,
                &f.actor("Reader"),
                "files",
                json!({"op":"write","path":path,"content":output.to_string()}),
            )
            .unwrap();
        Registry::new()
            .call(
                &f.authority,
                &f.actor("Reader"),
                "files",
                json!({"op":"read","path":path}),
            )
            .unwrap();
    }
    assert_eq!(
        gents_local_mcp::artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()
            ["passed"],
        true
    );
    f.db()
        .execute("UPDATE jobs SET state='completed'", [])
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true,
        "completed evidence remains verifiable without reopening execution"
    );
    assert!(child(&f).is_err());
}
#[test]
fn filenames_and_model_values_cannot_replace_producer_receipts_or_literal_slots() {
    let f = setup("7", CONSUMER);
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), false).unwrap();
    work.write("producer.json", "{\"some\":[\"7\"]}").unwrap();
    assert!(child(&f).is_err());
    root(&f, "7");
    for args in [json!([["8"], "2"]), json!([["7"], "3"]), json!([[7], "2"])] {
        assert!(Registry::new()
            .call(
                &f.authority,
                &f.actor("Reader"),
                "owned_rust_call",
                json!({"binding_id":"consumer","arguments":args})
            )
            .is_err());
    }
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "owned_rust_call",
            json!({"binding_id":"consumer","arguments":[["7"],"2"]})
        )
        .is_err());
    assert_eq!(
        f.db()
            .query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM owned_rust_call_results WHERE binding_id='consumer'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        0
    );
}
#[test]
fn absent_branches_and_incompatible_wire_types_leave_consumer_unexecuted() {
    for (value, consumer) in [
        ("0", CONSUMER),
        (
            "7",
            "pub fn protocol_consumer(values:&[i64],label:u64)->u64{let _=values;label}",
        ),
    ] {
        let f = setup(value, consumer);
        root(&f, value);
        assert!(child(&f).is_err());
        assert_eq!(
            f.db()
                .query_row::<i64, _, _>(
                    "SELECT COUNT(*) FROM owned_rust_call_results WHERE binding_id='consumer'",
                    [],
                    |r| r.get(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            completion::check(&f.authority, &f.scope).unwrap()["passed"],
            false
        );
    }
}
#[test]
fn source_changes_missing_audit_and_revoked_communication_invalidate_dependencies() {
    let f = setup("7", CONSUMER);
    root(&f, "7");
    child(&f).unwrap();
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), false).unwrap();
    work.write("owned/producer.rs", "pub fn changed(){}")
        .unwrap();
    assert!(child(&f).is_err());
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    work.write("owned/producer.rs", PRODUCER).unwrap();
    f.db().execute("DELETE FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.actor')='Worker'",[]).unwrap();
    assert!(child(&f).is_err());
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let old = policy.revision;
    policy.revision += 1;
    policy.grants.get_mut("Reader").unwrap().memory_from.clear();
    governance::save(&db, &f.scope.run.to_string(), &policy, old).unwrap();
    let mut scope = f.scope.clone();
    scope.expected_revision = Some(policy.revision);
    Registry::new()
        .call(
            &f.authority,
            &scope,
            "owned_rust_call",
            json!({"binding_id":"producer","arguments":["7"]}),
        )
        .unwrap();
    scope.actor = "Reader".into();
    assert!(Registry::new()
        .call(
            &f.authority,
            &scope,
            "owned_rust_call",
            json!({"binding_id":"consumer","arguments":[["7"],"2"]})
        )
        .is_err());
}
#[tokio::test]
async fn actual_stdio_and_sse_keep_typed_dependency_receipts_and_scope() {
    let f = setup("7", CONSUMER);
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    intake["artifact_bindings"][0]["receipt_only"] = json!(true);
    intake["artifact_bindings"][0]["writer"] = json!("Worker");
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();

    let bin = f.root.join("target/debug");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        bin.join("gents-local-mcp"),
    )
    .unwrap();
    let prior = std::env::var_os("GENTS_MCP_TRANSPORT");
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let p = gents_local_mcp::client::invoke(
            &f.root,
            &f.scope,
            "owned_rust_call",
            &json!({"binding_id":"producer","arguments":["7"]}),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(p["result"]["passed"], true);
        let c = gents_local_mcp::client::invoke(
            &f.root,
            &f.actor("Reader"),
            "owned_rust_call",
            &json!({"binding_id":"consumer","arguments":[["7"],"2"]}),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(c["result"]["output"], "9");
        assert_eq!(
            c["result"]["input_lineage"][0]["result_sha256"],
            p["mcp_receipt"]["result_sha256"]
        );
    }
    if let Some(value) = prior {
        std::env::set_var("GENTS_MCP_TRANSPORT", value)
    } else {
        std::env::remove_var("GENTS_MCP_TRANSPORT")
    };
}
#[test]
fn a_deeper_consumer_rechecks_all_ancestor_source_pins() {
    let f = setup("7", CONSUMER);
    let source = "pub fn protocol_third(x:u64)->u64{x}";
    let d = gents_local_mcp::component_api::describe(source).unwrap();
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    let refs =
        json!([{"field":"arguments","path":"consumer.json","argument_index":0,"pointer":""}]);
    intake["required_owned_rust_calls"].as_array_mut().unwrap().push(json!({"id":"third","actor":"Reader","source_path":"owned/third.rs","source_sha256":d["source_sha256"],"api_sha256":d["api_sha256"],"function":"protocol_third","owner_declared_review_package_id":"a".repeat(64),"input_recipe":{"arguments":[null],"sources":refs}}));
    intake["artifact_bindings"].as_array_mut().unwrap().push(json!({"path":"third.json","source_actor":"Reader","writer":"Reader","source_tool":"owned_rust_call","arguments":{"binding_id":"third","arguments":[null]},"pointer":"/output","encoding":"json","argument_sources":refs}));
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&f.scope).unwrap(), false).unwrap();
    work.write("owned/third.rs", source).unwrap();
    root(&f, "7");
    child(&f).unwrap();
    let call = || {
        Registry::new().call(
            &f.authority,
            &f.actor("Reader"),
            "owned_rust_call",
            json!({"binding_id":"third","arguments":["9"]}),
        )
    };
    assert_eq!(call().unwrap()["value"]["output"], "9");
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
    work.write("owned/producer.rs", "pub fn changed(){}")
        .unwrap();
    assert!(call().is_err());
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
}
#[test]
fn modified_projection_or_lineage_digest_cannot_satisfy_a_dependent_check() {
    let f = setup("7", CONSUMER);
    root(&f, "7");
    child(&f).unwrap();
    f.db()
        .execute(
            "UPDATE owned_rust_call_results SET lineage_sha256=?1 WHERE binding_id='consumer'",
            ["0".repeat(64)],
        )
        .unwrap();
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        false
    );
    f.db()
        .execute(
            "UPDATE mcp_binding_sources_v1 SET value_json=?1 WHERE path='producer.json'",
            ["{\"some\":[\"8\"]}"],
        )
        .unwrap();
    assert!(child(&f).is_err());
}
#[test]
fn copied_projection_without_new_run_audit_cannot_supply_a_consumer() {
    let f = setup("7", CONSUMER);
    root(&f, "7");
    let db = f.db();
    let run = uuid::Uuid::new_v4();
    db.execute(
        "INSERT INTO jobs SELECT ?1,digest,approval,state,intake FROM jobs WHERE run_id=?2",
        rusqlite::params![run.to_string(), f.scope.run.to_string()],
    )
    .unwrap();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    p.revision = 1;
    governance::save(&db, &run.to_string(), &p, 0).unwrap();
    db.execute("INSERT INTO mcp_binding_sources_v1 SELECT ?1,1,path,actor,tool,arguments_sha256,result_sha256,pointer,value_json,value_sha256,created FROM mcp_binding_sources_v1 WHERE run_id=?2",rusqlite::params![run.to_string(),f.scope.run.to_string()]).unwrap();
    let mut scope = f.actor("Reader");
    scope.run = run;
    scope.expected_revision = Some(1);
    let work =
        gents_local_mcp::workspace::open(&f.authority.workspace(&scope).unwrap(), true).unwrap();
    work.write("owned/producer.rs", PRODUCER).unwrap();
    work.write("owned/consumer.rs", CONSUMER).unwrap();
    assert!(Registry::new()
        .call(
            &f.authority,
            &scope,
            "owned_rust_call",
            json!({"binding_id":"consumer","arguments":[["7"],"2"]})
        )
        .is_err());
    assert_eq!(
        completion::check(&f.authority, &scope).unwrap()["passed"],
        false
    );
}
#[test]
fn failed_unit_result_is_not_a_successful_null_input_receipt() {
    let producer = "pub fn protocol_source(x:u64){let _=x;loop{}}";
    let consumer = "pub fn protocol_consumer(values:(),label:u64)->u64{let _=values;label}";
    let f = setup_with_source("7", producer, consumer);
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    intake["required_owned_rust_calls"][1]["input_recipe"]["sources"][0]["pointer"] = json!("");
    intake["artifact_bindings"][1]["argument_sources"][0]["pointer"] = json!("");
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    let failed = root(&f, "7");
    assert_eq!(failed["receipt"]["status"], "completed");
    assert_eq!(failed["value"]["passed"], false);
    assert!(failed["value"]["output"].is_null());
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.actor("Reader"),
            "owned_rust_call",
            json!({"binding_id":"consumer","arguments":[null,"2"]})
        )
        .is_err());
    assert_eq!(
        db.query_row::<i64, _, _>(
            "SELECT COUNT(*) FROM owned_rust_call_results WHERE binding_id='consumer'",
            [],
            |r| r.get(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn legacy_observations_survive_read_only_admission_and_additive_recipe_migration() {
    let f = setup("7", CONSUMER);
    root(&f, "7");
    let db = f.db();
    db.execute_batch("ALTER TABLE owned_rust_call_results DROP COLUMN recipe_sha256; ALTER TABLE owned_rust_call_results DROP COLUMN lineage_sha256;").unwrap();
    let columns = || {
        db.prepare("PRAGMA table_info(owned_rust_call_results)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    };
    assert!(!columns().iter().any(|c| c == "recipe_sha256"));
    let scope = f.actor("Reader");
    let args = json!({"binding_id":"consumer","arguments":[["7"],"2"]});
    assert!(gents_local_mcp::contract_execution::authorize_call(
        &f.authority,
        &scope,
        "owned_rust_call",
        &args
    )
    .unwrap()
    .is_some());
    assert!(
        !columns().iter().any(|c| c == "recipe_sha256"),
        "read-only preparation must not migrate or execute"
    );
    assert_eq!(
        db.query_row::<i64, _, _>("SELECT COUNT(*) FROM owned_rust_call_results", [], |r| r
            .get(0))
            .unwrap(),
        1
    );
    child(&f).unwrap();
    assert!(columns().iter().any(|c| c == "recipe_sha256"));
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
}
#[test]
fn receipt_only_intermediate_has_no_file_obligation_and_cannot_authorize_file_access() {
    let f = setup("7", CONSUMER);
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    intake["artifact_bindings"][0]["receipt_only"] = json!(true);
    intake["artifact_bindings"][0]["writer"] = json!("Worker");
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    root(&f, "7");
    let c = child(&f).unwrap();
    for op in ["read", "write"] {
        let mut args = json!({"op":op,"path":"producer.json"});
        if op == "write" {
            args["content"] = json!("fake");
        }
        assert!(Registry::new()
            .call(&f.authority, &f.scope, "files", args)
            .is_err());
    }
    Registry::new()
        .call(
            &f.authority,
            &f.actor("Reader"),
            "files",
            json!({"op":"write","path":"consumer.json","content":c["value"]["output"].to_string()}),
        )
        .unwrap();
    Registry::new()
        .call(
            &f.authority,
            &f.actor("Reader"),
            "files",
            json!({"op":"read","path":"consumer.json"}),
        )
        .unwrap();
    assert_eq!(
        gents_local_mcp::artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()
            ["passed"],
        true
    );
    assert!(!f
        .authority
        .workspace(&f.scope)
        .unwrap()
        .join("producer.json")
        .exists());
    assert_eq!(db.query_row::<i64,_,_>("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.status')='completed' AND json_extract(body,'$.tool')='files'",[],|r|r.get(0)).unwrap(),2);
}
#[test]
fn a_receipt_only_chain_needs_no_file_tool_grants() {
    let mut f = setup("7", CONSUMER);
    let db = f.db();
    let mut intake: Value = serde_json::from_str(
        &db.query_row::<String, _, _>("SELECT intake FROM jobs", [], |r| r.get(0))
            .unwrap(),
    )
    .unwrap();
    for b in intake["artifact_bindings"].as_array_mut().unwrap() {
        b["receipt_only"] = json!(true);
        b["writer"] = b["source_actor"].clone();
    }
    let encoded = intake.to_string();
    let digest = gents_local_mcp::digest(encoded.as_bytes());
    db.execute(
        "UPDATE jobs SET intake=?1,digest=?2,approval=?3",
        rusqlite::params![
            encoded,
            digest,
            json!({"owner":f.scope.owner,"sha256":digest,"type":"local_operator_approval"})
                .to_string()
        ],
    )
    .unwrap();
    let mut p = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let old = p.revision;
    p.revision += 1;
    for grants in [&mut p.ceilings, &mut p.grants] {
        for actor in ["Worker", "Reader"] {
            grants
                .get_mut(actor)
                .unwrap()
                .tools
                .retain(|tool| !tool.starts_with("files:"));
        }
    }
    governance::save(&db, &f.scope.run.to_string(), &p, old).unwrap();
    f.scope.expected_revision = Some(p.revision);
    root(&f, "7");
    child(&f).unwrap();
    assert_eq!(
        gents_local_mcp::artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()
            ["passed"],
        true
    );
    assert_eq!(
        completion::check(&f.authority, &f.scope).unwrap()["passed"],
        true
    );
    assert!(
        gents_local_mcp::artifact_bindings::next_call(&f.authority, &f.actor("Reader"), true)
            .unwrap()
            .is_none()
    );
    assert_eq!(db.query_row::<i64,_,_>("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call' AND json_extract(body,'$.tool')='files'",[],|r|r.get(0)).unwrap(),0);
}

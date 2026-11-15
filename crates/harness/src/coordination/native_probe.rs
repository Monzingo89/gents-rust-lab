//! Actual local reasoning coordination on a frozen synthetic text fixture.
//! The CEO plan/owner are seeded fixtures, explicitly not native planning proof.
use super::*;
use crate::{company_factory::contracts::Coordination, runtime_pool::RuntimePool};
use std::{collections::BTreeMap, path::Path};

#[tokio::test]
#[ignore = "opt-in actual pinned local-model flow/hub/peer candidate probe; no tool or general task qualification"]
async fn actual_local_three_pattern_candidates() {
    run_probe(1).await;
}

#[tokio::test]
#[ignore = "opt-in actual local factory-prepared v2 candidate programs; seeded CEO plan, no tool/general acceptance qualification"]
async fn actual_local_factory_prepared_typed_citations() {
    run_probe(2).await;
}

async fn run_probe(version: u32) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let config: crate::Config = serde_json::from_slice(
        &std::fs::read(root.join("config/company.mcp-development-reasoning-repair.json")).unwrap(),
    )
    .unwrap();
    let model = config.agents[0].worker.clone();
    let profile: Value = serde_json::from_slice(
        &std::fs::read(root.join("config/company.mcp-development-reasoning-repair.json")).unwrap(),
    )
    .unwrap();
    let pin = profile["planning_model_policy"]["candidates"][0]["sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let pinned_model: crate::provider::Model =
        serde_json::from_value(profile["planning_model_policy"]["candidates"][0]["model"].clone())
            .unwrap();
    assert_eq!(
        crate::company_factory::digest(&pinned_model).unwrap(),
        crate::company_factory::digest(&model).unwrap()
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let runtime = RuntimePool::serial_at_port(root.clone(), port).unwrap();
    let probe = uuid::Uuid::new_v4();
    let company = format!("coordination-native-{}", probe.simple());
    let folder = root
        .join("data/factory-coordination-probes")
        .join(probe.to_string());
    std::fs::create_dir_all(&folder).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    // Keep the exact tested binary independently of future Cargo rebuilds.
    let binary = folder.join("probe-binary");
    std::fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
    let binary_sha = crate::company::file_hash(&binary).unwrap();
    let mut factory_config = config.clone();
    factory_config.agents = ["Hub", "Worker", "Critic"]
        .map(|id| {
            let mut a = config.agents[0].clone();
            a.id = id.into();
            a
        })
        .to_vec();
    let mut runs = vec![];
    for pattern in [
        Coordination::Ordered,
        Coordination::AdaptiveDelegation,
        Coordination::PeerReview,
    ] {
        let mut contract = tests::contract(pattern);
        contract.version = version;
        contract.objective="Read the supplied synthetic records. Return a JSON object with exactly one key ids, whose array contains every supplied ID exactly once, sorted lexicographically ascending. The deliverable field itself must contain only this JSON object, without Markdown or commentary. No tools or code execution are needed.".into();
        contract.definition_of_done=vec!["Deliverable parses as a JSON object with only ids; its array equals every input record ID exactly once in lexicographic ascending order.".into()];
        for (role,assignment) in contract.roles.iter_mut().zip([
            "Draft the requested JSON from original input. If acting as hub, delegate useful checks and integrate actual worker contributions.",
            "Independently extract all supplied IDs, check ordering and propose the requested final JSON.",
            "Check completeness and ordering, integrate available contributions and return the requested final JSON."
        ]) {role.assignment=assignment.into();role.model_sha256=pin.clone();}
        let run = uuid::Uuid::new_v4();
        if version == 2 {
            tests::install_prepared(&root, &company, run, &contract, &factory_config).unwrap();
        } else {
            tests::install(&root, &company, run, &contract).unwrap();
        }
        let prepared_blueprint = if version == 2 {
            let db = crate::company::connect(&root, &company, "coordination-fixture-owner", false)
                .unwrap();
            let encoded: String = db
                .query_row(
                    "SELECT intake FROM jobs WHERE run_id=?1",
                    [run.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            let intake: Value = serde_json::from_str(&encoded).unwrap();
            let binding = &intake["factory_binding"];
            let sid = uuid::Uuid::parse_str(binding["planning_id"].as_str().unwrap()).unwrap();
            crate::company_factory::preparation::prepare(
                &root,
                &company,
                "coordination-fixture-owner",
                sid,
                &factory_config,
                binding["request"].clone(),
            )
            .unwrap()
        } else {
            Value::Null
        };
        let freeze = json!({"prepared":prepared_blueprint,"run":run,"company":company,"contract":contract,"contract_sha256":contract.sha256().unwrap(),"model_configuration":model,"model_sha256":pin,"native_port":port,"probe_binary_sha256":binary_sha,"probe_binary_path":binary.strip_prefix(&root).unwrap(),"contract_version":version,"factory_prepared":version==2,"scope":"Frozen synthetic owner/CEO fixture; native candidate coordination only; private oracle excluded from runtime packet."});
        std::fs::write(
            folder.join(format!("{pattern:?}-freeze.json")),
            serde_json::to_vec_pretty(&freeze).unwrap(),
        )
        .unwrap();
        let models = contract
            .roles
            .iter()
            .map(|r| (r.actor.clone(), model.clone()))
            .collect::<BTreeMap<_, _>>();
        let workers = native::NativeWorkers::new(
            &root,
            &runtime,
            &contract,
            models,
            &format!("{company}:{run}"),
        )
        .unwrap();
        let access = company::CompanyAccess::new(
            root.clone(),
            company.clone(),
            "coordination-fixture-owner".into(),
            run,
        );
        let mut journal =
            journal::SqliteJournal::open(&root, &company, "coordination-fixture-owner", run)
                .unwrap();
        println!(
            "{}",
            json!({"status":"STARTED","coordination":pattern,"run":run,"company":company})
        );
        let started = std::time::Instant::now();
        let result = Engine {
            workers: &workers,
            access: &access,
            journal: &mut journal,
        }
        .run(&contract, &AtomicBool::new(false))
        .await;
        if let Err(error) = &result {
            std::fs::write(folder.join(format!("{pattern:?}-failure.json")),serde_json::to_vec_pretty(&json!({"status":"FAIL","error":error.to_string(),"run":run,"company":company,"elapsed_ms":started.elapsed().as_millis(),"scope":"No qualification. Pending native receipt and canonical history retained; no automatic retry."})).unwrap()).unwrap();
            runtime.stop_owned().await;
        }
        let candidate = result.unwrap();
        let history = journal.history().unwrap();
        let deliverable =
            serde_json::from_str::<Value>(candidate["candidate"]["deliverable"].as_str().unwrap());
        // This private oracle is evaluated only after actual inference. It is
        // never a worker instruction, supplied plan or admitted reusable tool.
        let mut expected = contract.input["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        expected.sort_unstable();
        let accepted = deliverable
            .as_ref()
            .is_ok_and(|value| value == &json!({"ids":expected}));
        let record = json!({"status":if accepted {"PASS"}else{"FAIL"},"run":run,"company":company,"coordination":pattern,"candidate":candidate,"history":history,"elapsed_ms":started.elapsed().as_millis(),"private_fixture_oracle_passed":accepted,"runtime_task_accepted":false,"provider_tool_fees_usd":0,"scope":"One actual native text candidate per pattern; independent exact fixture oracle. Seeded owner/CEO plan, not planning qualification, native tool use, general intelligence, enterprise readiness or factory milestone completion."});
        std::fs::write(
            folder.join(format!("{pattern:?}-result.json")),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        if !accepted {
            runtime.stop_owned().await;
        }
        assert!(
            accepted,
            "actual final candidate differs from frozen exact oracle"
        );
        drop(journal);
        crate::company::outcome(
            &root,
            &company,
            "coordination-fixture-owner",
            run,
            "candidate_submitted",
        )
        .unwrap();
        runs.push(json!({"run":run,"coordination":pattern,"status":"PASS","turns":history.len()}));
        println!(
            "{}",
            json!({"status":"PASS","coordination":pattern,"run":run,"turns":history.len()})
        );
    }
    runtime.stop_owned().await;
    std::fs::write(folder.join("report.json"),serde_json::to_vec_pretty(&json!({"status":"PASS","runs":runs,"company":company,"scope":"Three native candidate probes only; complete factory execution and user trial remain pending."})).unwrap()).unwrap();
    println!(
        "{}",
        json!({"status":"PASS","report":folder.join("report.json")})
    );
}

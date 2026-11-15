//! Opt-in real inference and lifecycle probe. CEO/staffing and fixture approval
//! are transparently seeded, not evidence of native conversation/selection.
use super::*;
use crate::{
    company_factory::contracts::Coordination,
    coordination::{company::CompanyAccess, tests as fixture},
    runtime_pool::RuntimePool,
};
use std::{collections::BTreeMap, path::Path};
use uuid::Uuid;
fn save(folder: &Path, name: &str, value: &Value) -> Result<()> {
    std::fs::write(folder.join(name), serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
#[tokio::test]
#[ignore = "opt-in actual local factory coordination, independent final review, negative control and completion/memory gates"]
async fn actual_local_reviewed_factory_programs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let selected = std::env::var("GENTS_REVIEW_PROBE_PROFILE")
        .unwrap_or_else(|_| "config/company.mcp-development-reasoning-repair.json".into());
    assert!(
        selected.starts_with("config/") && !selected.contains("..") && selected.ends_with(".json")
    );
    let config_path = root.join(selected);
    let mut config: crate::Config =
        serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
    let profile_sha = crate::company::file_hash(&config_path).unwrap();
    let base = config.agents[0].clone();
    config.agents = ["Hub", "Worker", "Critic"]
        .map(|id| {
            let mut a = base.clone();
            a.id = id.into();
            a
        })
        .to_vec();
    let model = config.agents[0].worker.clone();
    let pin = config.planning_model_policy.as_ref().unwrap().candidates[0]
        .sha256
        .clone();
    assert_eq!(
        digest(&model).unwrap(),
        digest(&config.planning_model_policy.as_ref().unwrap().candidates[0].model).unwrap()
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let runtime = RuntimePool::serial_at_port(root.clone(), port).unwrap();
    let probe = Uuid::new_v4();
    let company = format!("review-native-{}", probe.simple());
    let folder = root
        .join("data/reasoning-review-probes")
        .join(probe.to_string());
    std::fs::create_dir_all(&folder).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let binary = folder.join("probe-binary");
    std::fs::copy(std::env::current_exe().unwrap(), &binary).unwrap();
    let mut active_run = None;
    let result=async {
        save(&folder,"freeze.json",&json!({"probe":probe,"company":company,"profile":config_path.strip_prefix(&root)?,"profile_sha256":profile_sha,"binary_sha256":crate::company::file_hash(&binary)?,"port":port,"scope":"Actual local model execution with seeded synthetic owner/CEO plan, architecture and permissions; no claim of native conversation or architectural choice."}))?;
        let mut runs=vec![];
        for pattern in [Coordination::Ordered,Coordination::AdaptiveDelegation,Coordination::PeerReview] {
            let mut program=fixture::contract(pattern);program.version=2;
            program.objective="Read the supplied synthetic records. Return a JSON object with exactly one key named ids. Its value must be an array containing every supplied ID exactly once, sorted lexicographically ascending. The deliverable string itself must contain only this object, without Markdown or commentary. No tools or external facts are needed.".into();
            program.definition_of_done=vec!["The deliverable parses as a JSON object with exactly the key ids. The ids value is an array containing every input record ID exactly once in ascending lexicographic order.".into()];
            for (role,assignment) in program.roles.iter_mut().zip([
                "Draft the requested object from original input, preserving the exact requested key and array shape. As hub, delegate useful checks before integrating actual contributions.",
                "Independently extract all supplied IDs, check completeness and ordering, and propose the requested object with the exact requested key.",
                "Check completeness, ordering and exact object shape against the original request. Integrate available contributions and return the requested final object when assigned to contribute or synthesize."
            ]) {role.model_sha256=pin.clone();role.assignment=assignment.into();}
            let review=contract::select(&config,&program)?;let run=Uuid::new_v4();
            fixture::install_prepared_reviewed(&root,&company,run,&program,&config,&review)?;
            active_run=Some(run);
            let scope=format!("{company}:{run}");
            let workers=super::super::native::NativeWorkers::new(&root,&runtime,&program,program.roles.iter().map(|r|(r.actor.clone(),model.clone())).collect::<BTreeMap<_,_>>(),&scope)?;
            let access=CompanyAccess::new(root.clone(),company.clone(),"coordination-fixture-owner".into(),run);
            let mut journal=journal::SqliteJournal::open(&root,&company,"coordination-fixture-owner",run)?;
            save(&folder,&format!("{pattern:?}-freeze.json"),&json!({"run":run,"program":program,"review":review,"actor_model":model,"scope":"Frozen synthetic fixture before inference. Private oracle is not supplied to any model."}))?;
            println!("{}",json!({"status":"STARTED","probe":probe,"pattern":pattern,"run":run}));
            let candidate=Engine {workers:&workers,access:&access,journal:&mut journal}.run(&program,&AtomicBool::new(false)).await?;
            save(&folder,&format!("{pattern:?}-candidate.json"),&candidate)?;
            let reviewer=native::NativeReviewer {root:&root,runtime:&runtime,contract:&review,scope:&scope};
            let snapshot=journal.snapshot(&review)?;
            let accepted=ReviewEngine {reviewer:&reviewer,access:&access,journal:&mut journal}.run(&review,1,&AtomicBool::new(false)).await?;
            save(&folder,&format!("{pattern:?}-review.json"),&accepted)?;
            ensure!(accepted["accepted"]==true,"actual independent model did not accept {pattern:?}");
            crate::company::outcome(&root,&company,"coordination-fixture-owner",run,"completed")?;
            // Private exact oracle: evaluated after runtime completion; never
            // changes a model's review, program, packet or lifecycle decision.
            let mut ids=program.input["records"].as_array().context("fixture records missing")?.iter().map(|r|r["id"].as_str().unwrap()).collect::<Vec<_>>();ids.sort_unstable();
            let deliverable:Value=serde_json::from_str(candidate["candidate"]["deliverable"].as_str().context("candidate text missing")?)?;
            let oracle=deliverable==json!({"ids":ids});
            let db=crate::company::connect(&root,&company,"coordination-fixture-owner",false)?;
            let state:String=db.query_row("SELECT state FROM jobs WHERE run_id=?1",[run.to_string()],|r|r.get(0))?;
            let archived:i64=db.query_row("SELECT COUNT(*) FROM memory_archive_checkpoints WHERE run_id=?1 AND lifecycle='completed'",[run.to_string()],|r|r.get(0))?;
            ensure!(state=="completed" && archived>0,"native task was not completed/archived");
            if pattern==Coordination::Ordered {
                let mut negative=snapshot.clone();negative.packet["candidate"]=json!({"deliverable":"{\"ids\":[]}","evidence":[],"unknowns":[]});
                reviewer.verify().await?;
                let reply=reviewer.complete(&negative.packet,1,&AtomicBool::new(false)).await?;
                let assessed=assessment(&negative,&reply)?;
                save(&folder,"negative-control.json",&json!({"reply":reply,"packet":negative.packet,"assessment":assessed,"scope":"Separate native judge sensitivity check only. Altered candidate never submitted to a canonical job or used for completion."}))?;
                ensure!(assessed["accepted"]==false,"actual judge accepted the deliberately missing IDs");
            }
            save(&folder,&format!("{pattern:?}-result.json"),&json!({"run":run,"candidate":candidate,"independent_review":accepted,"private_oracle_passed":oracle,"state":state,"memory_checkpoints":archived,"provider_fees_usd":0,"scope":"One actual local text task, explicit seeded planning/staffing/approval fixture; not general capability or enterprise compliance proof."}))?;
            ensure!(oracle,"runtime accepted candidate differs from private exact oracle");
            active_run=None;
            runs.push(json!({"pattern":pattern,"run":run,"state":state,"private_oracle_passed":oracle,"memory_checkpoints":archived}));
            println!("{}",json!({"status":"PASS","pattern":pattern,"run":run}));
        }
        save(&folder,"report.json",&json!({"status":"PASS","probe":probe,"runs":runs,"provider_fees_usd":0,"scope":"Actual local factory-prepared flow, hub and peer text tasks with distinct-weight independent review, completion and per-agent memory checkpoints. Seeded CEO/architecture fixtures; one small development case per pattern, not held-out benchmark or native conversation proof."}))?;
        Ok::<(),anyhow::Error>(())
    }.await;
    runtime.stop_owned().await;
    if let Err(error) = &result {
        if let Some(run) = active_run {
            let _ = crate::company::outcome(
                &root,
                &company,
                "coordination-fixture-owner",
                run,
                "incomplete",
            );
        }
        save(&folder,"failure.json",&json!({"status":"FAIL","error":error.to_string(),"scope":"Actual evidence retained; no automatic retries or paid fallback."})).unwrap();
    }
    println!(
        "{}",
        json!({"status":if result.is_ok(){"PASS"}else{"FAIL"},"report":folder})
    );
    result.unwrap();
}

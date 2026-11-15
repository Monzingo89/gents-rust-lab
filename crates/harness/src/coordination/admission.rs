//! Pure binding to the owner-frozen CEO job; never an approval operation.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub fn from_intake(intake: &Value) -> Result<Option<Contract>> {
    let Some(value) = intake.get("coordination_contract") else {
        return Ok(None);
    };
    let contract: Contract = serde_json::from_value(value.clone())?;
    contract.validate()?;
    let stages = crate::ceo_team::parse(&intake["ceo_plan"])?;
    ensure!(
        intake["ceo_plan"]["status"] == "ready"
            && intake["deliverable_type"] == "text"
            && intake["idea"] == contract.objective
            && intake["ceo_plan"]["definition_of_done"] == json!(contract.definition_of_done)
            && intake["acceptance"] == contract.definition_of_done.join("\n"),
        "coordination must match a ready owner-frozen text task"
    );
    ensure!(
        stages.len() == contract.roles.len()
            && stages
                .iter()
                .zip(&contract.roles)
                .all(|(stage, role)| stage.actor == role.actor
                    && stage.task == role.assignment
                    && stage.tools.is_empty()),
        "coordination roles differ from frozen staffing or request tools"
    );
    for field in [
        "artifact_bindings",
        "required_mcp_calls",
        "required_rust_checks",
        "required_owned_rust_calls",
        "ceo_source_inputs",
        "ceo_source_files",
        "ceo_owned_invocations",
    ] {
        ensure!(
            intake[field].is_null() || intake[field].as_array().is_some_and(Vec::is_empty),
            "reasoning coordination cannot execute native tool or file contracts"
        );
    }
    ensure!(
        intake["task_evidence"].is_null() && intake["candidate_workflow"].is_null(),
        "coordination cannot bypass a different execution contract"
    );
    Ok(Some(contract))
}

pub fn controls(contract: &Contract) -> Result<Value> {
    contract.validate()?;
    let actors = contract
        .roles
        .iter()
        .map(|r| r.actor.clone())
        .collect::<Vec<_>>();
    let mut memory: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut direct = vec![];
    match contract.coordination {
        crate::company_factory::contracts::Coordination::Ordered => {
            for (index, actor) in actors.iter().enumerate() {
                memory.insert(actor.clone(), actors[..index].to_vec());
                for sender in &actors[..index] {
                    direct.push(json!({"from":sender,"to":actor}));
                }
            }
        }
        crate::company_factory::contracts::Coordination::AdaptiveDelegation => {
            let hub = &actors[0];
            for actor in &actors {
                memory.insert(
                    actor.clone(),
                    actors.iter().filter(|a| *a != actor).cloned().collect(),
                );
                if actor != hub {
                    direct.push(json!({"from":hub,"to":actor}));
                    direct.push(json!({"from":actor,"to":hub}));
                }
            }
        }
        crate::company_factory::contracts::Coordination::PeerReview => {
            for actor in &actors {
                let sources = actors
                    .iter()
                    .filter(|a| *a != actor)
                    .cloned()
                    .collect::<Vec<_>>();
                for sender in &sources {
                    direct.push(json!({"from":sender,"to":actor}));
                }
                memory.insert(actor.clone(), sources);
            }
        }
    }
    Ok(
        json!({"actors":actors,"actions":[],"handoff_from":memory,"direct_message_edges":direct,"authority":"owner_review_required_reasoning_coordination_ceiling","transitive_lineage":"A hub relay retains all source actors. Removing any required memory_from grant blocks that dispatch; topology never declassifies content.","contract_sha256":contract.sha256()?}),
    )
}

pub fn validate_policy(intake: &Value, policy: &crate::governance::Policy) -> Result<()> {
    let contract = from_intake(intake)?.context("reasoning coordination contract missing")?;
    let compiled = controls(&contract)?;
    ensure!(
        intake["ceo_host_controls"] == compiled,
        "coordination communication controls changed"
    );
    policy.validate()?;
    let actors = contract
        .roles
        .iter()
        .map(|r| r.actor.as_str())
        .collect::<BTreeSet<_>>();
    for (actor, access) in &policy.ceilings {
        ensure!(
            actors.contains(actor.as_str())
                && access.tools.is_empty()
                && access.skills.is_empty()
                && access.contract_execution_phases.is_empty(),
            "coordination policy exceeds frozen reasoning-only roles"
        );
        ensure!(
            access
                .memory_from
                .iter()
                .all(|sender| compiled["handoff_from"][actor]
                    .as_array()
                    .is_some_and(|allowed| allowed.contains(&json!(sender)))),
            "coordination memory ceiling exceeds owner-reviewed lineage graph"
        );
    }
    Ok(())
}

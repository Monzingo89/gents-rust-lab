//! Frozen ordered CEO teams. The host compiles lifecycle and data-flow edges;
//! model proposals cannot install grants or advance the controller checkpoint.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub actor: String,
    pub task: String,
    pub tools: Vec<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
}
pub fn capabilities() -> BTreeSet<String> {
    let mut tools = gents_local_mcp::Registry::new().catalog()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .filter(|t| *t != "files")
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    tools.extend(["files:read", "files:write", "files:list", "finish"].map(str::to_string));
    tools
}
pub fn parse(proposal: &Value) -> Result<Vec<Stage>> {
    let stages: Vec<Stage> = serde_json::from_value(proposal["assignments"].clone())?;
    ensure!(
        !stages.is_empty() && stages.len() <= 4,
        "CEO teams require 1..4 ordered stages"
    );
    let mut actors = BTreeSet::new();
    for stage in &stages {
        ensure!(
            !stage.actor.trim().is_empty()
                && stage.actor.len() <= 200
                && !stage.actor.chars().any(char::is_control)
                && actors.insert(stage.actor.clone()),
            "invalid or duplicate team actor {} among {} assignments; each actor has one staffing stage containing all its source calls",stage.actor,stages.len()
        );
        ensure!(
            !stage.task.trim().is_empty() && stage.task.len() <= 2000,
            "invalid stage task"
        );
        ensure!(
            stage.acceptance.len() <= 4
                && stage
                    .acceptance
                    .iter()
                    .all(|v| !v.trim().is_empty() && v.len() <= 1000)
                && (stages.len() == 1 || !stage.acceptance.is_empty()),
            "multi-agent stages require explicit acceptance criteria"
        );
        ensure!(
            stage.tools.len() <= 8 && stage.tools.iter().all(|tool| capabilities().contains(tool)),
            "unknown or excessive stage tools"
        );
    }
    Ok(stages)
}
pub fn validate_bindings(
    intake: &Value,
    bindings: &[gents_local_mcp::artifact_bindings::Binding],
) -> Result<()> {
    let stages = parse(&intake["ceo_plan"])?;
    crate::stage_acceptance::validate(&stages, bindings)?;
    if intake["ceo_plan"]["planning_constraints"]["separate_source_writers"] == true {
        ensure!(!bindings.is_empty(),"owner duty separation requires native output contracts; edited bindings cannot remove this requirement");
        let owners = bindings
            .iter()
            .map(|binding| binding.source_actor.as_str())
            .collect::<BTreeSet<_>>();
        let writers = bindings
            .iter()
            .filter(|binding| !binding.receipt_only)
            .map(|binding| binding.writer.as_str())
            .collect::<BTreeSet<_>>();
        ensure!(
            owners.is_disjoint(&writers),
            "frozen owner duty separation forbids any source actor from also writing an artifact"
        );
    }
    for binding in bindings {
        let source = stages
            .iter()
            .position(|s| s.actor == binding.source_actor)
            .context("binding source is not assigned")?;
        let writer = stages
            .iter()
            .position(|s| s.actor == binding.writer)
            .context("binding writer is not assigned")?;
        for input in &binding.argument_sources {
            let parent = bindings
                .iter()
                .find(|b| b.path == input.path)
                .context("missing upstream native input")?;
            let upstream = stages
                .iter()
                .position(|s| s.actor == parent.source_actor)
                .context("upstream native actor is not assigned")?;
            ensure!(
                upstream <= source,
                "native input cannot consume a future actor stage"
            );
        }
        ensure!(
            source <= writer
                && (stages[source].tools.contains(&binding.source_tool)
                    || binding.arguments["op"]
                        .as_str()
                        .is_some_and(|op| stages[source]
                            .tools
                            .contains(&format!("{}:{op}", binding.source_tool))))
                && (binding.receipt_only || stages[writer].tools.contains(&"files:read".to_string())
                && stages[writer].tools.contains(&"files:write".to_string())),
            "binding {} exceeds staffing: source {} tools {:?}; writer {} tools {:?}; use the correct ordered tool owners or omit unambiguous actor fields",binding.path,binding.source_actor,stages[source].tools,binding.writer,stages[writer].tools
        );
    }
    Ok(())
}
pub fn controls(stages: &[Stage]) -> Value {
    let mut prior = Vec::new();
    let mut handoffs = BTreeMap::new();
    for stage in stages {
        handoffs.insert(stage.actor.clone(), prior.clone());
        prior.push(stage.actor.clone());
    }
    json!({"actors":stages.iter().map(|s|&s.actor).collect::<Vec<_>>(),"actions":["finish"],"handoff_from":handoffs,"authority":"host_ordered_team_lifecycle_and_handoff"})
}
pub fn validate_policy(intake: &Value, policy: &crate::governance::Policy) -> Result<()> {
    if intake.get("coordination_contract").is_some() {
        return crate::coordination::admission::validate_policy(intake, policy);
    }
    let stages = parse(&intake["ceo_plan"])?;
    let compiled = &intake["ceo_host_controls"];
    // Old one-worker frozen jobs remain valid; they have no cross-actor edges.
    let modern = compiled == &controls(&stages);
    let legacy = stages.len() == 1
        && compiled["actor"] == stages[0].actor
        && compiled["actions"] == json!(["finish"]);
    ensure!(
        modern || legacy,
        "frozen CEO host controls differ from compiled team"
    );
    for (actor, access) in &policy.ceilings {
        let stage = stages.iter().find(|s| &s.actor == actor);
        ensure!(
            access.skills.is_empty(),
            "CEO team has no approved skill grants"
        );
        ensure!(
            access
                .tools
                .iter()
                .all(|tool| stage.is_some_and(|s| tool == "finish" || s.tools.contains(tool))),
            "policy exceeds frozen CEO tool assignment"
        );
        ensure!(
            access.memory_from.iter().all(|source| modern
                && compiled["handoff_from"][actor]
                    .as_array()
                    .is_some_and(|v| v.contains(&json!(source)))),
            "policy exceeds frozen ordered handoff edges"
        );
    }
    Ok(())
}
pub fn authorize_handoff(
    stages: &[Stage],
    index: usize,
    policy: &crate::governance::Policy,
    completed: &[Value],
) -> Result<()> {
    let stage = stages
        .get(index)
        .context("CEO checkpoint exceeds frozen stages")?;
    ensure!(
        completed.len() == index,
        "CEO handoff checkpoint count mismatch"
    );
    let grant = policy
        .grants
        .get(&stage.actor)
        .context("CEO stage has no current grant")?;
    for (prior_index, (prior, receipt)) in stages.iter().zip(completed).enumerate() {
        ensure!(receipt["actor"]==prior.actor&&receipt["policy_revision"]==policy.revision&&receipt["accepted"]==true,"prior stage receipt is missing, stale or rejected; inspect before replaying any writes");
        ensure!(
            receipt["stage"] == prior_index
                && receipt["assignment"] == serde_json::to_value(prior)?,
            "stage handoff differs from frozen assignment"
        );
        ensure!(
            grant.memory_from.contains(&prior.actor),
            "current policy denies earlier-stage handoff"
        );
    }
    Ok(())
}
/// Audited source data and brief deliverables, not recursive review packets.
pub fn handoff_view(completed: &[Value]) -> Value {
    json!(completed.iter().map(|h|json!({"stage":h["stage"],"actor":h["actor"],"accepted":h["accepted"],"policy_revision":h["policy_revision"],"task":h["assignment"]["task"],"model_claims_unverified":{"deliverable":h["deliverable"].get("deliverable"),"evidence":h["deliverable"].get("evidence")},"native_results":h["native_results"],"results_window":"At most six recent native result projections; larger payloads stay in retained journals/workspace. A receipt hash is not omitted content."})).collect::<Vec<_>>())
}
/// Bounded canonical facts survive the rolling model evidence window. These
/// observations never authorize an action or replace the independent QA gate.
pub fn authorization_evidence(delivery_gap: &Value, completed: &[Value]) -> Value {
    let native = &delivery_gap["native_receipts"];
    let artifacts = &delivery_gap["artifact_bindings"];
    json!({
        "source":"host canonical receipt checks and current confined artifact reads; not model summaries",
        "scope":"execution observations only; current policy and admin authorization still required",
        "native_calls":{"policy_revision":native["policy_revision"],"passed":native["passed"],"checks":native["checks"],"rust_checks":native["rust_checks"],"owned_rust_calls":native["owned_rust_calls"]},
        "artifacts":{"policy_revision":artifacts["policy_revision"],"passed":artifacts["passed"],"checks":artifacts["checks"].as_array().into_iter().flatten().map(|row|json!({
            "path":row["path"],"receipt_only":row["receipt_only"],"passed":row["passed"],"source_actor":row["source_actor"],"writer":row["writer"],"phase":row["phase"],
            "source_result_sha256":row["source_result_sha256"],"source_projection":row["source_projection"],"source_arguments_sha256":row["source_arguments_sha256"],"native_input_paths":row["native_input_paths"],
            "writer_native_operations":row["writer_native_operations"],"expected_sha256":row["expected_sha256"],"observed_sha256":row["observed_sha256"]
        })).collect::<Vec<_>>()},
        "accepted_stages":completed.iter().map(|stage|json!({"actor":stage["actor"],"stage":stage["stage"],"accepted":stage["accepted"],"policy_revision":stage["policy_revision"]})).collect::<Vec<_>>(),
        "finish_semantics":"finish submits evidence to independent QA; it does not itself declare the goal completed"
    })
}
/// A positive artifact view cannot erase a failing native completion gate.
/// Native observations inform progress; they never authorize the next action.
pub fn attach_native_gate(delivery_gap: &mut Value, native: Value) {
    if native["passed"] != true {
        delivery_gap["passed"] = json!(false);
    }
    delivery_gap["native_receipts"] = native;
}

/// Intermediate actions create evidence; their authorization packet must not
/// present future completion failures as prerequisites for taking the action.
/// Full checks remain authoritative at finish and in the controller's gates.
pub fn next_action_evidence(
    delivery_gap: &Value,
    completed: &[Value],
    action: &str,
    arguments: &Value,
) -> Value {
    let full = authorization_evidence(delivery_gap, completed);
    if action == "finish" {
        return full;
    }
    let selected = full["artifacts"]["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| action == "files" && row["path"] == arguments["path"])
        .map(|row| json!({
            "path":row["path"],"source_actor":row["source_actor"],"writer":row["writer"],
            "source_result_sha256":row["source_result_sha256"],"source_projection":row["source_projection"],
            "source_arguments_sha256":row["source_arguments_sha256"],"native_input_paths":row["native_input_paths"],
            "expected_sha256":row["expected_sha256"],"observed_before_action_sha256":row["observed_sha256"]
        }))
        .collect::<Vec<_>>();
    json!({
        "source":full["source"],"scope":"authorization and safety of this next action only; not a completion submission",
        "policy_revision":full["native_calls"]["policy_revision"],
        "prior_successful_native_calls":full["native_calls"]["checks"].as_array().into_iter().flatten().filter(|row|row["passed"]==true).collect::<Vec<_>>(),
        "prior_owned_rust_calls":full["native_calls"]["owned_rust_calls"].as_array().into_iter().flatten().filter(|row|row["passed"]==true).collect::<Vec<_>>(),
        "accepted_stages":full["accepted_stages"],"selected_artifact_sources":selected,
        "completion_checks":"Deferred to finish and independent QA. Missing future writes/readbacks are expected before executing them; they are not authorization failures. Source receipts and current policy are still required. No completion gate is bypassed."
    })
}

pub fn completion_view(mut gate: Value, actor: &str, final_stage: bool) -> Value {
    if !final_stage {
        for key in ["checks", "rust_checks", "owned_rust_calls"] {
            if let Some(checks) = gate[key].as_array_mut() {
                checks.retain(|v| v["actor"] == actor);
            }
        }
        gate["passed"] = json!(
            gate["checks"]
                .as_array()
                .is_some_and(|rows| rows.iter().all(|c| c["passed"] == true))
                && gate["rust_checks"]
                    .as_array()
                    .is_none_or(|rows| rows.iter().all(|c| c["passed"] == true))
                && gate["owned_rust_calls"]
                    .as_array()
                    .is_none_or(|rows| rows.iter().all(|c| c["passed"] == true))
        );
        gate["scope"] = json!("current_stage_only; final review requires every frozen actor");
    }
    gate
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_failure_cannot_leave_top_level_progress_passed() {
        let mut gap = json!({"passed":true,"artifact_bindings":{"passed":true}});
        attach_native_gate(
            &mut gap,
            json!({"passed":false,"rust_checks":[{"passed":false}]}),
        );
        assert_eq!(gap["passed"], false);
        assert_eq!(gap["native_receipts"]["passed"], false);
        attach_native_gate(&mut gap, json!({"passed":true}));
        assert_eq!(
            gap["passed"], false,
            "another failed gate must remain failed"
        );
        let mut complete = json!({"passed":true});
        attach_native_gate(&mut complete, json!({"passed":true}));
        assert_eq!(complete["passed"], true);
    }
    #[test]
    fn intermediate_authorization_does_not_require_its_own_future_results() {
        let gap = json!({"native_receipts":{"policy_revision":3,"passed":false,"checks":[{"actor":"Analyst","tool":"table_read_json","passed":true},{"actor":"Writer","tool":"files","passed":false}]},"artifact_bindings":{"policy_revision":3,"passed":false,"checks":[{"path":"rows.json","passed":false,"source_actor":"Analyst","writer":"Writer","source_result_sha256":"actual-source","expected_sha256":"exact-copy","observed_sha256":null,"writer_native_operations":[{"operation":"write","passed":false}]},{"path":"future.txt","passed":false}]}});
        let context = next_action_evidence(
            &gap,
            &[],
            "files",
            &json!({"op":"write","path":"rows.json","content":"private bytes"}),
        );
        assert_eq!(
            context["selected_artifact_sources"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            context["selected_artifact_sources"][0]["source_result_sha256"],
            "actual-source"
        );
        assert!(context["selected_artifact_sources"][0]
            .get("passed")
            .is_none());
        assert!(!context.to_string().contains("private bytes"));
        assert_eq!(
            context["prior_successful_native_calls"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let finish = next_action_evidence(&gap, &[], "finish", &json!({}));
        assert_eq!(finish["artifacts"]["passed"], false);
        assert_eq!(finish["artifacts"]["checks"].as_array().unwrap().len(), 2);
        assert_eq!(finish["native_calls"]["checks"][1]["passed"], false);
    }
    #[test]
    fn team_controls_do_not_authorize_future_or_unrelated_senders() {
        let stages=parse(&json!({"assignments":[{"actor":"Analyst","task":"calculate","tools":["integer_arithmetic"],"acceptance":["actual sum"]},{"actor":"Writer","task":"write","tools":["files:write"],"acceptance":["artifact"]}]})).unwrap();
        let controls = controls(&stages);
        assert_eq!(controls["handoff_from"]["Analyst"], json!([]));
        assert_eq!(controls["handoff_from"]["Writer"], json!(["Analyst"]));
        let empty = json!({"tools":[],"skills":[],"memory_from":[]});
        let grants = json!({"Analyst":empty,"Writer":{"tools":["files:write","finish"],"skills":[],"memory_from":["Analyst"]}});
        let mut policy: crate::governance::Policy = serde_json::from_value(
            json!({"revision":1,"master":"Analyst","ceilings":grants,"grants":grants}),
        )
        .unwrap();
        let intake = json!({"ceo_plan":{"assignments":stages},"ceo_host_controls":controls});
        validate_policy(&intake, &policy).unwrap();
        let prior = json!({"stage":0,"assignment":stages[0],"actor":"Analyst","policy_revision":1,"accepted":true});
        authorize_handoff(&stages, 1, &policy, std::slice::from_ref(&prior)).unwrap();
        policy.grants.get_mut("Writer").unwrap().memory_from.clear();
        assert!(authorize_handoff(&stages, 1, &policy, &[prior]).is_err());
        policy
            .ceilings
            .get_mut("Analyst")
            .unwrap()
            .memory_from
            .insert("Writer".into());
        assert!(validate_policy(&intake, &policy).is_err());
    }
    #[test]
    fn authorization_retains_all_native_facts_without_copying_model_claims() {
        let rows=(0..16).map(|index|json!({"path":format!("artifact-{index}.txt"),"passed":index!=15,"source_actor":"Analyst","writer":"Writer","source_result_sha256":"source","writer_native_operations":[{"operation":"write","passed":true},{"operation":"read","passed":index!=15}],"verified_native_value":"private projection","expected_from_native_source":"private expected"})).collect::<Vec<_>>();
        let gap = json!({"native_receipts":{"policy_revision":7,"passed":true,"checks":[{"actor":"Analyst","tool":"integer_arithmetic","passed":true,"successful_current_epoch_receipts":1}]},"artifact_bindings":{"policy_revision":7,"passed":false,"checks":rows}});
        let evidence = authorization_evidence(
            &gap,
            &[
                json!({"actor":"Analyst","stage":0,"accepted":true,"policy_revision":7,"deliverable":"unverified claim","native_results":"private full payload"}),
            ],
        );
        assert_eq!(evidence["native_calls"]["checks"][0]["actor"], "Analyst");
        assert_eq!(
            evidence["artifacts"]["checks"].as_array().unwrap().len(),
            16
        );
        assert_eq!(evidence["artifacts"]["passed"], false);
        assert_eq!(evidence["artifacts"]["checks"][15]["passed"], false);
        assert_eq!(evidence["accepted_stages"][0]["policy_revision"], 7);
        assert!(!evidence.to_string().contains("private"));
        assert!(!evidence.to_string().contains("unverified claim"));
    }
    #[test]
    fn stage_gate_cannot_satisfy_global_missing_receipts() {
        let gate = json!({"checks":[{"actor":"A","passed":true},{"actor":"B","passed":false}],"passed":false});
        assert_eq!(completion_view(gate.clone(), "A", false)["passed"], true);
        assert_eq!(completion_view(gate, "A", true)["passed"], false);
        assert!(parse(&json!({"assignments":[{"actor":"A","task":"first","tools":[]},{"actor":"B","task":"second","tools":[]}]})).is_err());
    }
    #[test]
    fn owned_invocation_facts_survive_review_and_stage_filtering_without_global_acceptance() {
        let gate = json!({"checks":[],"rust_checks":[],"owned_rust_calls":[{"actor":"A","binding_id":"first","passed":true},{"actor":"B","binding_id":"second","passed":false}],"policy_revision":4,"passed":false});
        assert_eq!(completion_view(gate.clone(), "A", false)["passed"], true);
        assert_eq!(completion_view(gate.clone(), "B", false)["passed"], false);
        assert_eq!(completion_view(gate.clone(), "A", true)["passed"], false);
        let gap = json!({"native_receipts":gate});
        assert_eq!(
            authorization_evidence(&gap, &[])["native_calls"]["owned_rust_calls"][1]["passed"],
            false
        );
        assert_eq!(
            next_action_evidence(&gap, &[], "owned_rust_call", &json!({}))
                ["prior_owned_rust_calls"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

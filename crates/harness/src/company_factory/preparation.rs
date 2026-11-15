//! Compile an owner-reviewable reasoning blueprint into the ordinary intake
//! path. No inference, grants, job writes, approval or acceptance occurs here.
use super::{contracts::*, *};
use crate::coordination::program::{Contract, Role};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub expected_revision: u64,
    pub specification: TaskSpecification,
    pub input: Value,
    pub model_sha256: BTreeMap<String, String>,
    pub max_delegations: usize,
    pub peer_rounds: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture_assessment: Option<Uuid>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub version: u32,
    pub planning_id: Uuid,
    pub request: Request,
    pub blueprint_sha256: String,
    pub proposal_sha256: String,
    pub program_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment_policy_sha256: Option<String>,
}

/// These executors submit reasoning candidates only. The general tool-enabled
/// architecture menu is separate until tool effects and acceptance are wired.
struct ReasoningArchitectures;
impl ArchitectureRepository for ReasoningArchitectures {
    fn architectures(&self) -> Result<Vec<Architecture>> {
        Ok([
            ("flow", Coordination::Ordered),
            ("orchestration", Coordination::AdaptiveDelegation),
            ("collaboration", Coordination::PeerReview),
        ]
        .into_iter()
        .map(|(id, coordination)| Architecture {
            id: id.into(),
            version: 2,
            coordination,
            executor: Some(format!("reasoning_{id}_candidate_v2")),
        })
        .collect())
    }
}

pub(crate) fn architecture_menu() -> Result<Vec<Architecture>> {
    ReasoningArchitectures.architectures()
}

fn program(request: &Request, proposal: &Value) -> Result<Contract> {
    super::validate(&request.specification)?;
    let spec = &request.specification;
    ensure!(
        spec.required_capabilities.is_empty(),
        "reasoning factory cannot omit required tools"
    );
    let stages = crate::ceo_team::parse(proposal)?;
    ensure!(
        stages.iter().all(|s| s.tools.is_empty()),
        "reasoning factory cannot strip CEO tools"
    );
    ensure!(
        request.model_sha256.len() == stages.len(),
        "model pins must match exact staffing"
    );
    let roles = stages
        .iter()
        .map(|s| {
            let model_sha256 = request
                .model_sha256
                .get(&s.actor)
                .ok_or_else(|| anyhow::anyhow!("model pin missing for frozen role"))?
                .clone();
            Ok(Role {
                actor: s.actor.clone(),
                assignment: s.task.clone(),
                model_sha256,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let contract = Contract {
        version: 2,
        coordination: spec.coordination,
        objective: spec.objective.clone(),
        definition_of_done: spec.definition_of_done.clone(),
        input: request.input.clone(),
        roles,
        max_delegations: request.max_delegations,
        peer_rounds: request.peer_rounds,
    };
    contract.validate()?;
    Ok(contract)
}

fn compile(
    config: &crate::Config,
    request: &Request,
    session: &Value,
) -> Result<(Blueprint, Contract)> {
    ensure!(
        config.company_mode && config.company_tools,
        "factory needs the local tool-enabled company"
    );
    let installed = repositories::InstalledRepositories::new(config)?;
    let factory = CompanyFactory {
        architectures: &ReasoningArchitectures,
        agents: &installed,
        tools: &installed,
        compliance: &installed,
    };
    let mut blueprint = factory.compose(
        &PreviewRequest {
            expected_revision: request.expected_revision,
            specification: request.specification.clone(),
        },
        session,
    )?;
    ensure!(
        blueprint.gaps.is_empty(),
        "factory capabilities or control requirements unresolved"
    );
    let contract = program(request, &session["proposal"])?;
    for role in &contract.roles {
        let model = &config
            .agents
            .iter()
            .find(|a| a.id == role.actor)
            .ok_or_else(|| anyhow::anyhow!("factory actor missing"))?
            .worker;
        model.validate()?;
        ensure!(
            model.provider == "local"
                && model.adapter.is_none()
                && model
                    .path
                    .as_deref()
                    .is_some_and(crate::model_artifact::relative_path),
            "factory requires explicit local GGUF workers"
        );
    }
    blueprint.proposed_communication_controls =
        Some(crate::coordination::admission::controls(&contract)?);
    blueprint.scope = "Owner-reviewable reasoning candidate program only. No tools, grants, approval, weight admission, independent task acceptance or compliance verdict. Native worker byte pins and current repository snapshot are checked before new execution.".into();
    Ok((blueprint, contract))
}

pub fn prepare(
    root: &Path,
    company: &str,
    owner: &str,
    id: Uuid,
    config: &crate::Config,
    input: Value,
) -> Result<Value> {
    let session = crate::ceo_planning::read(root, company, owner, id)?;
    let request: Request = serde_json::from_value(input)?;
    let (blueprint, contract) = compile(config, &request, &session)?;
    let assessment = request
        .architecture_assessment
        .map(|assessment| -> Result<Value> {
            let record = crate::architecture_review::read(root, company, owner, assessment)?;
            crate::architecture_review::validate_selected(
                &record,
                id,
                request.expected_revision,
                &session["proposal"],
                request.specification.coordination,
            )?;
            ensure!(
                record["models"]["policy_sha256"]
                    == config
                        .planning_model_policy
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("local architecture policy missing"))?
                        .digest()?,
                "architecture model policy changed; review again"
            );
            Ok(record)
        })
        .transpose()?;
    let binding = Binding {
        version: 1,
        planning_id: id,
        request,
        blueprint_sha256: digest(&json!(blueprint))?,
        proposal_sha256: digest(&session["proposal"])?,
        program_sha256: contract.sha256()?,
        assessment_sha256: assessment.as_ref().map(digest).transpose()?,
        assessment_policy_sha256: assessment
            .as_ref()
            .and_then(|a| a["models"]["policy_sha256"].as_str().map(str::to_owned)),
    };
    let spec = &binding.request.specification;
    let patch = json!({"idea":spec.objective,"acceptance":spec.definition_of_done.join("\n"),
        "exclusions":spec.exclusions.join("\n"),"budget_usd":0,"deliverable_type":"text",
        "ceo_planning_id":id,"ceo_planning_revision":binding.request.expected_revision,
        "coordination_contract":contract,"factory_binding":binding});
    Ok(
        json!({"blueprint":blueprint,"blueprint_sha256":binding.blueprint_sha256,
        "intake_patch":patch,"owner_review_required":true,"execution_authorized":false,
        "candidate_only":true,"next_steps":["Review the complete blueprint and communication ceilings.","Add timeline and signer fields; freeze the ordinary intake.","Approve its exact digest and install owner-selected governance.","Start the approved task; a reasoning candidate still needs independent final acceptance."]}),
    )
}

/// Called after retaining the actual CEO plan inside the normal intake path.
/// A caller-supplied binding never acts as authority or substitutes for approval.
pub fn validate_frozen(intake: &Value) -> Result<()> {
    let Some(value) = intake.get("factory_binding") else {
        return Ok(());
    };
    let binding: Binding = serde_json::from_value(value.clone())?;
    ensure!(
        binding.version == 1
            && crate::coordination::program::hash(&binding.blueprint_sha256)
            && intake["ceo_planning_id"] == binding.planning_id.to_string()
            && intake["ceo_planning_revision"] == binding.request.expected_revision
            && binding.proposal_sha256 == digest(&intake["ceo_plan"])?
            && intake["exclusions"] == binding.request.specification.exclusions.join("\n"),
        "factory binding differs from frozen planning/specification"
    );
    let contract = program(&binding.request, &intake["ceo_plan"])?;
    ensure!(
        intake["coordination_contract"] == json!(contract)
            && binding.program_sha256 == contract.sha256()?,
        "factory program differs from reviewed specification"
    );
    crate::coordination::admission::from_intake(intake)?;
    Ok(())
}

/// Recompose against the actual installed repositories before inference. A
/// changed worker configuration/catalog/control snapshot requires fresh review.
pub fn validate_execution(config: &crate::Config, intake: &Value) -> Result<()> {
    validate_frozen(intake)?;
    let Some(value) = intake.get("factory_binding") else {
        return Ok(());
    };
    let binding: Binding = serde_json::from_value(value.clone())?;
    let session = json!({"revision":binding.request.expected_revision,"status":"ready","proposal":intake["ceo_plan"]});
    if let Some(expected) = &binding.assessment_policy_sha256 {
        ensure!(
            Some(expected.clone())
                == config
                    .planning_model_policy
                    .as_ref()
                    .map(|p| p.digest())
                    .transpose()?,
            "architecture policy changed since review"
        );
    }
    let (blueprint, contract) = compile(config, &binding.request, &session)?;
    ensure!(
        digest(&json!(blueprint))? == binding.blueprint_sha256
            && intake["coordination_contract"] == json!(contract),
        "factory repository snapshot changed; review a fresh blueprint"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_programs_bind_real_candidate_topologies_and_fail_closed_on_drift() {
        let root = std::env::temp_dir().join(format!("factory-prepare-{}", Uuid::new_v4()));
        let mut config = super::super::tests::config();
        let base = config.agents[0].clone();
        config.agents = ["Hub", "Worker", "Critic"]
            .map(|id| {
                let mut agent = base.clone();
                agent.id = id.into();
                agent
            })
            .to_vec();
        crate::company::operation(
            &root,
            "fixture",
            "owner",
            &json!({"op":"register","name":"Synthetic preparation fixture"}),
        )
        .unwrap();
        let db = crate::company::connect(&root, "fixture", "owner", false).unwrap();
        db.execute_batch(
            "CREATE TABLE ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)",
        )
        .unwrap();
        let id = Uuid::new_v4();
        let mut proposal = super::super::tests::session();
        proposal["proposal"]["assignments"] = json!([
            {"actor":"Hub","task":"Draft candidate","tools":[],"acceptance":["Frozen criteria"]},
            {"actor":"Worker","task":"Inspect candidate","tools":[],"acceptance":["Frozen criteria"]},
            {"actor":"Critic","task":"Check candidate","tools":[],"acceptance":["Frozen criteria"]}]);
        db.execute(
            "INSERT INTO ceo_planning VALUES(?1,2,'ready',?2)",
            rusqlite::params![id.to_string(), proposal.to_string()],
        )
        .unwrap();
        let before: i64 = db
            .query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get(0))
            .unwrap();
        for pattern in [
            Coordination::Ordered,
            Coordination::AdaptiveDelegation,
            Coordination::PeerReview,
        ] {
            let mut spec = super::super::tests::request().specification;
            spec.coordination = pattern;
            let request = Request {
                expected_revision: 2,
                specification: spec,
                input: json!({"text":"Synthetic input"}),
                model_sha256: config
                    .agents
                    .iter()
                    .map(|a| (a.id.clone(), "a".repeat(64)))
                    .collect(),
                max_delegations: 2,
                peer_rounds: 1,
                architecture_assessment: None,
            };
            let result = prepare(&root, "fixture", "owner", id, &config, json!(request)).unwrap();
            assert_eq!(result["execution_authorized"], false);
            assert_eq!(result["candidate_only"], true);
            assert_eq!(result["blueprint"]["architecture"]["version"], 2);
            assert_eq!(
                result["blueprint"]["architecture"]["coordination"],
                json!(pattern)
            );
            let mut intake = result["intake_patch"].clone();
            intake["ceo_plan"] = proposal["proposal"].clone();
            validate_execution(&config, &intake).unwrap();
            let mut changed = intake.clone();
            changed["coordination_contract"]["input"] = json!({"text":"Replaced input"});
            assert!(validate_frozen(&changed).is_err());
            let mut changed = intake.clone();
            changed["factory_binding"]["request"]["input"] = json!({"text":"Replaced input"});
            changed["coordination_contract"]["input"] = json!({"text":"Replaced input"});
            assert!(validate_frozen(&changed).is_err());
            let mut changed = intake.clone();
            changed["factory_binding"]["blueprint_sha256"] = json!("0".repeat(64));
            assert!(validate_execution(&config, &changed).is_err());
            let mut changed_config = config.clone();
            changed_config.agents[0].worker.model = "Changed worker".into();
            assert!(validate_execution(&changed_config, &intake).is_err());
            intake["exclusions"] = json!("Changed scope");
            assert!(validate_frozen(&intake).is_err());
            let mut missing = request.clone();
            missing.model_sha256.remove("Critic");
            assert!(prepare(&root, "fixture", "owner", id, &config, json!(missing)).is_err());
            let mut tooling = request.clone();
            tooling
                .specification
                .required_capabilities
                .push("files:read".into());
            assert!(prepare(&root, "fixture", "owner", id, &config, json!(tooling)).is_err());
            let mut regulated = request.clone();
            regulated.specification.data_classification = DataClassification::Regulated;
            assert!(prepare(&root, "fixture", "owner", id, &config, json!(regulated)).is_err());
            assert!(prepare(&root, "fixture", "other-owner", id, &config, json!(request)).is_err());
        }
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            before
        );
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }
}

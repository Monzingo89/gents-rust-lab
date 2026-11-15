//! Owner-reviewable preparation from canonical CEO/judge records. No model,
//! job, grant or approval side effects; the browser never supplies architecture
//! identity, actor staffing or model pins here.
use super::{contracts::*, *};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub expected_revision: u64,
    pub architecture_assessment: Uuid,
    pub data_classification: DataClassification,
    #[serde(default)]
    pub artifact_bindings: Option<Vec<Value>>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeBinding {
    pub version: u32,
    pub planning_id: Uuid,
    pub planning_revision: u64,
    pub specification: TaskSpecification,
    pub blueprint_sha256: String,
    pub assessment_id: Uuid,
    pub assessment_sha256: String,
    pub policy_sha256: String,
    pub model_sha256: BTreeMap<String, String>,
}
fn pins(
    config: &crate::Config,
    stages: &[crate::ceo_team::Stage],
) -> Result<BTreeMap<String, String>> {
    let policy = config
        .planning_model_policy
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("owner-listed local model pins required"))?;
    policy.validate()?;
    stages
        .iter()
        .map(|stage| {
            let model = &config
                .agents
                .iter()
                .find(|a| a.id == stage.actor)
                .ok_or_else(|| anyhow::anyhow!("configured actor missing"))?
                .worker;
            ensure!(
                model.provider == "local" && model.adapter.is_none(),
                "only owner-pinned local workers may execute"
            );
            model.validate()?;
            let expected = digest(model)?;
            let mut candidate = None;
            for entry in &policy.candidates {
                if digest(&entry.model)? == expected {
                    ensure!(
                        candidate.is_none(),
                        "ambiguous owner-pinned worker configuration"
                    );
                    candidate = Some(entry);
                }
            }
            let candidate = candidate.ok_or_else(|| {
                anyhow::anyhow!("worker is not in the owner-pinned model catalog")
            })?;
            Ok((stage.actor.clone(), candidate.sha256.clone()))
        })
        .collect()
}
fn blueprint(
    config: &crate::Config,
    spec: &TaskSpecification,
    session: &Value,
) -> Result<Blueprint> {
    let installed = repositories::InstalledRepositories::new(config)?;
    CompanyFactory {
        architectures: &installed,
        agents: &installed,
        tools: &installed,
        compliance: &installed,
    }
    .compose(
        &PreviewRequest {
            expected_revision: session["revision"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("planning revision missing"))?,
            specification: spec.clone(),
        },
        session,
    )
}
fn native_policy(stages: &[crate::ceo_team::Stage]) -> Value {
    let mut grants = serde_json::Map::new();
    let mut prior = vec![];
    for stage in stages {
        let mut tools = stage.tools.clone();
        tools.push("finish".into());
        tools.sort();
        tools.dedup();
        grants.insert(
            stage.actor.clone(),
            json!({"tools":tools,"skills":[],"memory_from":prior}),
        );
        prior.push(stage.actor.clone());
    }
    json!({"revision":1,"master":stages[0].actor,"ceilings":grants,"grants":grants})
}
fn reasoning_policy(contract: &crate::coordination::program::Contract) -> Result<Value> {
    let controls = crate::coordination::admission::controls(contract)?;
    let mut grants = serde_json::Map::new();
    for role in &contract.roles {
        grants.insert(
            role.actor.clone(),
            json!({"tools":[],"skills":[],"memory_from":controls["handoff_from"][&role.actor]}),
        );
    }
    Ok(json!({"revision":1,"master":contract.roles[0].actor,"ceilings":grants,"grants":grants}))
}

pub fn prepare(
    root: &Path,
    company: &str,
    owner: &str,
    id: Uuid,
    config: &crate::Config,
    input: Value,
) -> Result<Value> {
    let request: Request = serde_json::from_value(input)?;
    let session = crate::ceo_planning::read(root, company, owner, id)?;
    ensure!(
        session["status"] == "ready" && session["revision"] == request.expected_revision,
        "review the current ready CEO revision"
    );
    let record =
        crate::architecture_review::read(root, company, owner, request.architecture_assessment)?;
    let coordination: Coordination =
        serde_json::from_value(record["recommendation"]["architecture"].clone())?;
    crate::architecture_review::validate_selected(
        &record,
        id,
        request.expected_revision,
        &session["proposal"],
        coordination,
    )?;
    let policy = config
        .planning_model_policy
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("local model policy missing"))?;
    ensure!(
        record["models"]["policy_sha256"] == policy.digest()?,
        "architecture model policy changed"
    );
    let stages = crate::ceo_team::parse(&session["proposal"])?;
    let messages = session["messages"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("planning conversation missing"))?
        .iter()
        .filter(|m| m["role"] == "user" && m["kind"] != "planning_repair")
        .map(|m| {
            m["message"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("invalid user message"))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(!messages.is_empty(), "user task missing");
    // Preserve newlines and indentation: whitespace may be task data/code.
    let objective = messages.join("\n\n");
    let criteria: Vec<String> =
        serde_json::from_value(session["proposal"]["definition_of_done"].clone())?;
    let mut required = stages
        .iter()
        .flat_map(|s| s.tools.iter().cloned())
        .collect::<Vec<_>>();
    required.sort();
    required.dedup();
    let specification=TaskSpecification{version:1,objective:objective.clone(),inputs:vec!["Exact retained user planning conversation".into()],outputs:vec!["Deliverable specified by the frozen CEO definition of done".into()],definition_of_done:criteria.clone(),exclusions:vec!["Paid or remote inference, unapproved network access, downloads, credential disclosure, cross-company access, shell and source promotion".into()],coordination,data_classification:request.data_classification,control_profiles:vec!["local_baseline".into()],required_capabilities:required};
    let models = pins(config, &stages)?;
    let (mut intake, blueprint, policy) = if stages.iter().all(|s| s.tools.is_empty()) {
        ensure!(
            request.artifact_bindings.as_ref().is_none_or(Vec::is_empty),
            "reasoning cannot silently drop artifact checks"
        );
        for field in ["source_inputs", "source_files", "owned_invocations"] {
            ensure!(
                session[field].is_null() || session[field].as_array().is_some_and(Vec::is_empty),
                "attached native inputs need an explicitly tool-enabled plan"
            );
        }
        let prepared = preparation::prepare(
            root,
            company,
            owner,
            id,
            config,
            json!(preparation::Request {
                expected_revision: request.expected_revision,
                specification,
                input: json!({"user_conversation":messages}),
                model_sha256: models,
                max_delegations: stages.len().saturating_sub(1).clamp(1, 4),
                peer_rounds: 1,
                architecture_assessment: Some(request.architecture_assessment)
            }),
        )?;
        let contract: crate::coordination::program::Contract =
            serde_json::from_value(prepared["intake_patch"]["coordination_contract"].clone())?;
        (
            prepared["intake_patch"].clone(),
            prepared["blueprint"].clone(),
            reasoning_policy(&contract)?,
        )
    } else {
        ensure!(
            coordination == Coordination::Ordered,
            "tool-enabled hub/peer execution is not installed; no substitution"
        );
        let built = blueprint(config, &specification, &session)?;
        ensure!(
            built.gaps.is_empty(),
            "factory tool or control requirements need review"
        );
        let binding = NativeBinding {
            version: 1,
            planning_id: id,
            planning_revision: request.expected_revision,
            specification: specification.clone(),
            blueprint_sha256: digest(&built)?,
            assessment_id: request.architecture_assessment,
            assessment_sha256: digest(&record)?,
            policy_sha256: policy.digest()?,
            model_sha256: models,
        };
        (
            json!({"idea":objective,"acceptance":criteria.join("\n"),"exclusions":specification.exclusions.join("\n"),"budget_usd":0,"deliverable_type":"agent","ceo_planning_id":id,"ceo_planning_revision":request.expected_revision,"artifact_bindings":request.artifact_bindings.unwrap_or_else(||session["proposed_artifact_bindings"].as_array().cloned().unwrap_or_default()),"native_factory_binding":binding}),
            json!(built),
            native_policy(&stages),
        )
    };
    intake["timeline"] = json!("Approved local task until complete, blocked or cancelled");
    intake["company_signer"] = json!("Local operator browser approval");
    intake["provider_signer"] = json!("Gents company");
    intake["ceo_plan"] = session["proposal"].clone();
    if let Some(program) = crate::coordination::admission::from_intake(&intake)? {
        intake["reasoning_acceptance"] = json!(crate::coordination::review::contract::select(
            config, &program
        )?);
    }
    let mut draft = json!({"task":intake["idea"],"criteria":criteria,"planning_mode":session["planning_mode"],"intake":intake,"policy":policy});
    if session["owned_invocations"]
        .as_array()
        .is_some_and(|v| !v.is_empty())
    {
        draft["expected_owned_invocations"] = session["owned_invocations"].clone();
    }
    Ok(
        json!({"draft":draft,"blueprint":blueprint,"architecture_assessment":record,"candidate_only":!intake["coordination_contract"].is_null() && intake["reasoning_acceptance"].is_null(),"owner_review_required":true,"execution_authorized":false,"scope":"Factory composition and frozen review only. No job, permission, approval, inference, task completion or compliance certification."}),
    )
}

pub fn validate_frozen(intake: &Value) -> Result<()> {
    let Some(encoded) = intake.get("native_factory_binding") else {
        return Ok(());
    };
    let binding: NativeBinding = serde_json::from_value(encoded.clone())?;
    super::validate(&binding.specification)?;
    ensure!(
        binding.version == 1
            && binding.specification.coordination == Coordination::Ordered
            && intake["coordination_contract"].is_null()
            && intake["factory_binding"].is_null()
            && intake["ceo_planning_id"] == binding.planning_id.to_string()
            && intake["ceo_planning_revision"] == binding.planning_revision
            && intake["idea"] == binding.specification.objective
            && intake["acceptance"] == binding.specification.definition_of_done.join("\n")
            && intake["exclusions"] == binding.specification.exclusions.join("\n"),
        "native factory binding differs from reviewed task"
    );
    let stages = crate::ceo_team::parse(&intake["ceo_plan"])?;
    ensure!(
        binding.model_sha256.len() == stages.len()
            && stages.iter().all(|stage| binding
                .model_sha256
                .get(&stage.actor)
                .is_some_and(|hash| crate::coordination::program::hash(hash))),
        "native factory staffing or model pins differ from the frozen plan"
    );
    for hash in [
        &binding.blueprint_sha256,
        &binding.assessment_sha256,
        &binding.policy_sha256,
    ] {
        ensure!(
            crate::coordination::program::hash(hash),
            "invalid factory identity"
        );
    }
    Ok(())
}
pub fn validate_execution(
    root: &Path,
    company: &str,
    owner: &str,
    config: &crate::Config,
    intake: &Value,
) -> Result<()> {
    validate_frozen(intake)?;
    let Some(encoded) = intake.get("native_factory_binding") else {
        return Ok(());
    };
    let binding: NativeBinding = serde_json::from_value(encoded.clone())?;
    let session = crate::ceo_planning::read(root, company, owner, binding.planning_id)?;
    ensure!(
        session["status"] == "ready"
            && session["revision"] == binding.planning_revision
            && session["proposal"] == intake["ceo_plan"],
        "factory plan changed since owner review"
    );
    let record = crate::architecture_review::read(root, company, owner, binding.assessment_id)?;
    crate::architecture_review::validate_selected(
        &record,
        binding.planning_id,
        binding.planning_revision,
        &session["proposal"],
        Coordination::Ordered,
    )?;
    ensure!(
        digest(&record)? == binding.assessment_sha256
            && config
                .planning_model_policy
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("model policy missing"))?
                .digest()?
                == binding.policy_sha256,
        "factory architecture or model policy changed"
    );
    let built = blueprint(config, &binding.specification, &session)?;
    let stages = crate::ceo_team::parse(&session["proposal"])?;
    ensure!(
        built.gaps.is_empty()
            && digest(&built)? == binding.blueprint_sha256
            && pins(config, &stages)? == binding.model_sha256,
        "factory repository snapshot changed"
    );
    for stage in stages {
        let model = &config
            .agents
            .iter()
            .find(|a| a.id == stage.actor)
            .ok_or_else(|| anyhow::anyhow!("frozen worker no longer configured"))?
            .worker;
        crate::model_artifact::verify(
            root,
            model
                .path
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("local worker missing"))?,
            &binding.model_sha256[&stage.actor],
        )
        .map_err(anyhow::Error::msg)?;
    }
    Ok(())
}

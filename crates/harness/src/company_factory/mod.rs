//! Pure composition over replaceable owner-controlled repositories.
//! A blueprint is a proposal. It neither grants access nor installs a runtime.
pub mod contracts;
pub mod guided;
pub mod preparation;
pub mod repositories;
#[cfg(test)]
mod tests;

use anyhow::{ensure, Result};
use contracts::*;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub trait ArchitectureRepository {
    fn architectures(&self) -> Result<Vec<Architecture>>;
}
pub trait AgentRepository {
    fn agent(&self, id: &str) -> Result<Option<Agent>>;
}
pub trait ToolRepository {
    fn tool(&self, capability: &str) -> Result<Option<Tool>>;
}
pub trait ComplianceProfileRepository {
    fn control_profile(&self, id: &str) -> Result<Option<ControlProfile>>;
}

pub struct CompanyFactory<'a> {
    pub architectures: &'a dyn ArchitectureRepository,
    pub agents: &'a dyn AgentRepository,
    pub tools: &'a dyn ToolRepository,
    pub compliance: &'a dyn ComplianceProfileRepository,
}

pub fn digest(value: &impl Serialize) -> Result<String> {
    Ok(gents_local_mcp::digest(&serde_json::to_vec(value)?))
}

fn text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}
pub(crate) fn objective(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 4000
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}
fn list(values: &[String], maximum: usize) -> bool {
    !values.is_empty()
        && values.len() <= maximum
        && values.iter().all(|value| text(value, 2000))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
fn validate(spec: &TaskSpecification) -> Result<()> {
    ensure!(
        spec.version == 1
            && objective(&spec.objective)
            && list(&spec.inputs, 16)
            && list(&spec.outputs, 16)
            && list(&spec.definition_of_done, 8)
            && list(&spec.exclusions, 16)
            && list(&spec.control_profiles, 8)
            && spec.control_profiles.iter().all(|id| id.len() <= 80)
            && spec.required_capabilities.len() <= 32
            && spec.required_capabilities.iter().all(|id| text(id, 80))
            && spec
                .required_capabilities
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == spec.required_capabilities.len(),
        "factory requires a bounded explicit task specification; clarify missing requirements"
    );
    crate::security::require_persistable(&serde_json::to_string(spec)?)?;
    Ok(())
}

impl CompanyFactory<'_> {
    /// Uses an already schema-validated ready proposal. Caller owns tenant and
    /// revision checks; this method repeats structural checks before composing.
    pub fn compose(&self, request: &PreviewRequest, session: &Value) -> Result<Blueprint> {
        validate(&request.specification)?;
        ensure!(
            request.expected_revision > 0
                && session["revision"] == request.expected_revision
                && session["status"] == "ready"
                && session["proposal"]["status"] == "ready",
            "factory preview requires the current ready planning revision"
        );
        let spec = &request.specification;
        let proposal = &session["proposal"];
        ensure!(
            proposal["definition_of_done"] == json!(spec.definition_of_done),
            "factory definition of done differs from the ready CEO proposal"
        );
        let stages = crate::ceo_team::parse(proposal)?;
        let architectures = self.architectures.architectures()?;
        ensure!(
            !architectures.is_empty() && architectures.len() <= 16,
            "architecture repository is empty or exceeds budget"
        );
        let mut ids = BTreeSet::new();
        for architecture in &architectures {
            ensure!(
                text(&architecture.id, 80)
                    && architecture.version > 0
                    && ids.insert(&architecture.id),
                "ambiguous architecture repository"
            );
        }
        let matches = architectures
            .iter()
            .filter(|candidate| candidate.coordination == spec.coordination)
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "coordination needs one unambiguous architecture descriptor"
        );
        let architecture = matches[0].clone();
        let mut gaps = vec![];
        let mut gap = |code: &str, subject: &str| {
            gaps.push(Gap {
                code: code.into(),
                subject: subject.into(),
            })
        };
        if architecture.executor.is_none() {
            gap("executor_not_implemented", &architecture.id);
        }
        let mut agents = vec![];
        let mut tools = vec![];
        let mut capabilities = spec
            .required_capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for stage in &stages {
            match self.agents.agent(&stage.actor)? {
                None => gap("agent_not_configured", &stage.actor),
                Some(agent) => {
                    ensure!(
                        agent.id == stage.actor,
                        "agent repository returned a different identity"
                    );
                    if !agent.local_native_worker {
                        gap("native_local_worker_required", &agent.id);
                    }
                    agents.push(agent);
                }
            }
            capabilities.extend(stage.tools.iter().cloned());
        }
        for capability in capabilities {
            match self.tools.tool(&capability)? {
                None => gap("owned_tool_missing", &capability),
                Some(tool) => {
                    ensure!(
                        tool.capability == capability,
                        "tool repository returned a different identity"
                    );
                    if tool.custody != Custody::ProjectOwned {
                        gap("tool_custody_not_project_owned", &capability);
                    }
                    if !tool.platform_supported {
                        gap("tool_unavailable_on_host", &capability);
                    }
                    if !tool.local_only || tool.provider_cost_usd != 0 {
                        gap("tool_outside_zero_fee_local_scope", &capability);
                    }
                    tools.push(tool);
                }
            }
        }
        let mut profiles = vec![];
        let mut required_controls = BTreeSet::new();
        let mut profile_ids = spec
            .control_profiles
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        // An owner may add requirements; baseline controls cannot be omitted.
        profile_ids.insert("local_baseline".into());
        for id in profile_ids {
            match self.compliance.control_profile(&id)? {
                None => gap("control_profile_missing", &id),
                Some(profile) => {
                    ensure!(
                        profile.id == id && profile.version > 0,
                        "control profile identity mismatch"
                    );
                    for missing in &profile.gaps {
                        gap("control_evidence_missing", missing);
                    }
                    required_controls.extend(profile.required_controls.iter().cloned());
                    profiles.push(profile);
                }
            }
        }
        if matches!(
            spec.data_classification,
            DataClassification::Confidential | DataClassification::Regulated
        ) {
            gap(
                "owner_data_scope_and_control_assessment_required",
                "data_classification",
            );
        }
        // Bind contents, not a mutable human-readable version alone. Includes
        // the entire architecture menu and all selected repository records.
        let snapshot = digest(
            &json!({"architectures":architectures,"agents":agents,"tools":tools,"control_profiles":profiles}),
        )?;
        Ok(Blueprint {
            schema_version: 1,
            status: if gaps.is_empty() {"ready_for_owner_review"} else {"missing_capability"}.into(),
            planning_revision: request.expected_revision,
            proposal_sha256: digest(proposal)?,
            specification_sha256: digest(spec)?,
            repository_snapshot_sha256: snapshot,
            architecture,
            agents,
            proposed_communication_controls: (spec.coordination == Coordination::Ordered)
                .then(|| crate::ceo_team::controls(&stages)),
            assignments: stages,
            tools,
            control_profiles: profiles,
            required_controls: required_controls.into_iter().collect(),
            gaps,
            preview_only: true,
            execution_authorized: false,
            scope: "Read-only composition preview; owner-reviewed requirements, not authority, model weight admission, GPU fit, tool execution, compliance assessment or task acceptance. Existing frozen intake, policy and native gates remain mandatory.".into(),
        })
    }
}

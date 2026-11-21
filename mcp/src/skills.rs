//! Phase-scoped, read-only skill repository. Skills constrain a model call;
//! they never expand tool permissions or override the approved task.
use crate::contracts::{EmbeddedContracts, ToolContracts};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SkillRole {
    Planner,
    Worker,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    version: String,
    status: String,
    roles: Vec<SkillRole>,
    path: String,
    #[serde(default = "global_domain")]
    domain: String,
}

fn global_domain() -> String {
    "global".into()
}

#[derive(Deserialize)]
struct Catalog {
    skills: Vec<Entry>,
}

pub trait SkillRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>>;
}

pub trait CapabilityRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>>;
}

pub struct EmbeddedSkillRepository;

impl EmbeddedSkillRepository {
    fn body(id: &str) -> Result<&'static str> {
        Ok(match id {
            "source-onboarding" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/source-onboarding/SKILL.md"
            ),
            "timestamped-media-evidence" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/timestamped-media-evidence/SKILL.md"
            ),
            "identifier-reconciliation" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/identifier-reconciliation/SKILL.md"
            ),
            "independent-verification" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/independent-verification/SKILL.md"
            ),
            "fetch-web-content" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/fetch-web-content/SKILL.md"
            ),
            "local-code-repair" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/local-code-repair/SKILL.md"
            ),
            "document-workflow" => include_str!(
                "../../docs/companies/we-need-getns-llc/shared-skills/approved/document-workflow/SKILL.md"
            ),
            "security-governance" => include_str!("../../docs/companies/we-need-getns-llc/shared-skills/approved/security-governance/SKILL.md"),
            "finance-reconciliation" => include_str!("../../docs/companies/we-need-getns-llc/shared-skills/approved/finance-reconciliation/SKILL.md"),
            "hr-model-assessment" => include_str!("../../docs/companies/we-need-getns-llc/shared-skills/approved/hr-model-assessment/SKILL.md"),
            "it-runtime-operations" => include_str!("../../docs/companies/we-need-getns-llc/shared-skills/approved/it-runtime-operations/SKILL.md"),
            _ => bail!("approved skill body is not embedded: {id}"),
        })
    }
}

/// Adapts the approved tool catalog into small composable capability contracts.
/// A planner may reason about read-only capabilities; only a worker may request
/// execution, and every request still passes the independent admin gate.
pub struct EmbeddedCapabilityRepository;

impl CapabilityRepository for EmbeddedCapabilityRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>> {
        let catalog: serde_json::Map<String, Value> =
            serde_json::from_str(include_str!("../../config/company-tools.json"))?;
        let read_only_ids = [
            "models_inventory",
            "integer_arithmetic",
            "memory_search",
            "web_search",
            "web_read",
            "pdf_inspect",
            "image_review",
            "journal_read",
            "resources",
            "hf_dataset_search",
            "hf_dataset_inspect",
            "hf_dataset_files",
            "models_search",
            "email_list",
            "email_status",
            "skill_list",
            "skill_read",
            "source_read",
        ];
        Ok(catalog
            .into_iter()
            .filter(|(id,_)|!crate::functions::is_task_capability(id))
            .map(|(id, contract)| {
                let read_only = read_only_ids.contains(&id.as_str());
                json!({
                    "id": id,
                    "contract": contract,
                    "input_schema": EmbeddedContracts.input_schema(&id),
                    "access": if role == SkillRole::Planner { "plan_only" } else { "request_execution" },
                    "effect": if read_only { "read_only" } else { "state_change_or_control" }
                })
            })
            .collect())
    }
}

impl SkillRepository for EmbeddedSkillRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>> {
        let catalog: Catalog = serde_json::from_str(include_str!(
            "../../docs/companies/we-need-getns-llc/shared-skills/catalog.json"
        ))?;
        catalog
            .skills
            .into_iter()
            .filter(|entry| matches!(entry.status.as_str(), "approved-demo" | "approved-local") && entry.roles.contains(&role))
            .map(|entry| {
                Ok(json!({
                    "id": entry.id,
                    "version": entry.version,
                    "path": entry.path,
                    "domain": entry.domain,
                    "instructions": Self::body(&entry.id)?,
                    "authority": "procedure only; cannot expand tools, policy, scope, budget, or approval"
                }))
            })
            .collect()
    }
}

pub fn for_phase(phase: &str) -> Result<Vec<Value>> {
    if phase == "document_review" || phase == "document_author" {
        return Ok(EmbeddedSkillRepository
            .for_role(SkillRole::Worker)?
            .into_iter()
            .filter(|skill| {
                matches!(
                    skill["id"].as_str(),
                    Some("independent-verification" | "document-workflow")
                )
            })
            .collect());
    }
    let role = match phase {
        "planner" | "router" | "onboarding" | "executive" => Some(SkillRole::Planner),
        "worker" | "observer" | "scoped_worker" | "judge" | "tool" | "dreamer" => {
            Some(SkillRole::Worker)
        }
        _ => None,
    };
    role.map_or_else(
        || Ok(Vec::new()),
        |role| {
            Ok(EmbeddedSkillRepository
                .for_role(role)?
                .into_iter()
                .filter(|skill| skill["domain"] == "global")
                .collect())
        },
    )
}

/// Department procedures are opt-in assignments, not extra prompt baggage for
/// every old job. Phase restrictions apply even to an explicit assignment.
pub fn for_assignment(
    phase: &str,
    assigned: &std::collections::BTreeSet<String>,
) -> Result<Vec<Value>> {
    let role = match phase {
        "planner" | "router" | "onboarding" | "executive" => SkillRole::Planner,
        "worker" | "observer" | "scoped_worker" | "judge" | "tool" | "dreamer" => SkillRole::Worker,
        _ => {
            return Ok(for_phase(phase)?
                .into_iter()
                .filter(|skill| skill["id"].as_str().is_some_and(|id| assigned.contains(id)))
                .collect())
        }
    };
    Ok(EmbeddedSkillRepository
        .for_role(role)?
        .into_iter()
        .filter(|skill| skill["id"].as_str().is_some_and(|id| assigned.contains(id)))
        .collect())
}

pub fn capabilities_for_phase(phase: &str) -> Result<Vec<Value>> {
    let role = match phase {
        "planner" | "router" | "onboarding" | "executive" => Some(SkillRole::Planner),
        "worker" | "judge" | "tool" | "dreamer" => Some(SkillRole::Worker),
        _ => None,
    };
    role.map_or_else(
        || Ok(Vec::new()),
        |role| EmbeddedCapabilityRepository.for_role(role),
    )
}
/// Angular execution is unavailable without a frozen owner contract. Keep its
/// schema out of unrelated legacy prompts; the native catalog and backend
/// authorization remain independent and unchanged.
pub fn scope_sdk_capabilities(capabilities: &mut Vec<Value>, intake: &Value) {
    let angular = intake["angular_check_contracts"]
        .as_array()
        .is_some_and(|c| !c.is_empty());
    capabilities.retain(|c| c["id"] != "angular_check" || angular);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_prompts_do_not_advertise_unavailable_angular_execution() {
        let mut capabilities = capabilities_for_phase("worker").unwrap();
        assert!(capabilities.iter().any(|c| c["id"] == "angular_check"));
        let mut scoped = capabilities.clone();
        scope_sdk_capabilities(
            &mut scoped,
            &json!({"angular_check_contracts":[{"actor":"Author"}]}),
        );
        assert_eq!(scoped.len(), capabilities.len());
        scope_sdk_capabilities(&mut capabilities, &json!({}));
        assert!(!capabilities.iter().any(|c| c["id"] == "angular_check"));
        assert!(capabilities.iter().any(|c| c["id"] == "files"));
    }

    #[test]
    fn department_skills_are_assigned_without_expanding_old_job_context() {
        let assigned = std::collections::BTreeSet::from([
            "security-governance".into(),
            "finance-reconciliation".into(),
            "hr-model-assessment".into(),
            "it-runtime-operations".into(),
        ]);
        for phase in ["planner", "worker", "executive"] {
            assert_eq!(for_assignment(phase, &assigned).unwrap().len(), 4);
            assert!(for_phase(phase)
                .unwrap()
                .iter()
                .all(|skill| skill["domain"] == "global"));
        }
        assert!(for_assignment("admin", &assigned).unwrap().is_empty());
        assert_eq!(for_assignment("observer", &assigned).unwrap().len(), 4);
        assert!(capabilities_for_phase("observer").unwrap().is_empty());
        assert!(for_assignment("worker", &std::collections::BTreeSet::new())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn skills_are_phase_scoped_and_never_cross_assigned() {
        let planners = for_phase("planner").unwrap();
        let workers = for_phase("worker").unwrap();
        assert_eq!(planners.len(), 4);
        assert_eq!(planners[0]["id"], "source-onboarding");
        assert_eq!(workers.len(), 6);
        assert!(workers
            .iter()
            .all(|skill| skill["id"] != "source-onboarding"));
        assert!(for_phase("admin").unwrap().is_empty());
        assert!(for_phase("executive")
            .unwrap()
            .iter()
            .any(|skill| skill["id"] == "document-workflow"));
        assert!(capabilities_for_phase("executive")
            .unwrap()
            .iter()
            .any(|capability| capability["id"] == "pdf_render"
                && capability["access"] == "plan_only"));
        assert_eq!(for_phase("document_review").unwrap().len(), 2);
        assert!(capabilities_for_phase("document_review")
            .unwrap()
            .is_empty());
        assert!(capabilities_for_phase("planner")
            .unwrap()
            .iter()
            .all(|capability| capability["access"] == "plan_only"));
        assert!(capabilities_for_phase("planner")
            .unwrap()
            .iter()
            .any(|capability| capability["id"] == "shell" && capability["access"] == "plan_only"));
        assert_eq!(
            capabilities_for_phase("worker").unwrap().len(),
            serde_json::from_str::<serde_json::Map<String, Value>>(include_str!(
                "../../config/company-tools.json"
            ))
            .unwrap()
            .keys()
            .filter(|id| !crate::functions::is_task_capability(id))
            .count()
        );
        assert!(capabilities_for_phase("worker")
            .unwrap()
            .iter()
            .all(|c| !crate::functions::is_task_capability(c["id"].as_str().unwrap())));
    }
}

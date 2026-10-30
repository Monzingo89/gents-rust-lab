//! Phase-scoped, read-only skill repository. Skills constrain a model call;
//! they never expand tool permissions or override the approved task.
use crate::contracts::{EmbeddedContracts, ToolContracts};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum SkillRole {
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
}

#[derive(Deserialize)]
struct Catalog {
    skills: Vec<Entry>,
}

trait SkillRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>>;
}

trait CapabilityRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>>;
}

struct EmbeddedSkillRepository;

impl EmbeddedSkillRepository {
    fn body(id: &str) -> Result<&'static str> {
        Ok(match id {
            "source-onboarding" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/source-onboarding/SKILL.md"
            ),
            "timestamped-media-evidence" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/timestamped-media-evidence/SKILL.md"
            ),
            "identifier-reconciliation" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/identifier-reconciliation/SKILL.md"
            ),
            "independent-verification" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/independent-verification/SKILL.md"
            ),
            "fetch-web-content" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/fetch-web-content/SKILL.md"
            ),
            "local-code-repair" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/local-code-repair/SKILL.md"
            ),
            "document-workflow" => include_str!(
                "../../../docs/companies/we-need-getns-llc/shared-skills/approved/document-workflow/SKILL.md"
            ),
            _ => bail!("approved skill body is not embedded: {id}"),
        })
    }
}

/// Adapts the approved tool catalog into small composable capability contracts.
/// A planner may reason about read-only capabilities; only a worker may request
/// execution, and every request still passes the independent admin gate.
struct EmbeddedCapabilityRepository;

impl CapabilityRepository for EmbeddedCapabilityRepository {
    fn for_role(&self, role: SkillRole) -> Result<Vec<Value>> {
        let catalog: serde_json::Map<String, Value> =
            serde_json::from_str(include_str!("../../../config/company-tools.json"))?;
        let read_only_ids = [
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
            "../../../docs/companies/we-need-getns-llc/shared-skills/catalog.json"
        ))?;
        catalog
            .skills
            .into_iter()
            .filter(|entry| entry.status == "approved-demo" && entry.roles.contains(&role))
            .map(|entry| {
                Ok(json!({
                    "id": entry.id,
                    "version": entry.version,
                    "path": entry.path,
                    "instructions": Self::body(&entry.id)?,
                    "authority": "procedure only; cannot expand tools, policy, scope, budget, or approval"
                }))
            })
            .collect()
    }
}

pub fn for_phase(phase: &str) -> Result<Vec<Value>> {
    if phase == "document_review" {
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
        "planner" | "router" | "onboarding" => Some(SkillRole::Planner),
        "worker" | "judge" | "tool" | "dreamer" => Some(SkillRole::Worker),
        _ => None,
    };
    role.map_or_else(
        || Ok(Vec::new()),
        |role| EmbeddedSkillRepository.for_role(role),
    )
}

pub fn capabilities_for_phase(phase: &str) -> Result<Vec<Value>> {
    let role = match phase {
        "planner" | "router" | "onboarding" => Some(SkillRole::Planner),
        "worker" | "judge" | "tool" | "dreamer" => Some(SkillRole::Worker),
        _ => None,
    };
    role.map_or_else(
        || Ok(Vec::new()),
        |role| EmbeddedCapabilityRepository.for_role(role),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
                "../../../config/company-tools.json"
            ))
            .unwrap()
            .len()
        );
    }
}

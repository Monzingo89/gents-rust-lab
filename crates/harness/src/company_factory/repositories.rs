//! Adapters over installed owner configuration and the owned native registry.
//! No downloads, inference, tool calls, policy changes or new dependencies.
use super::{contracts::*, *};
use crate::Config;

fn supported_platform(os: &str, tool: &str, isolated_rust: bool) -> bool {
    (!isolated_rust || os == "macos")
        && (tool != "files" || matches!(os, "macos" | "linux"))
        && (!matches!(tool, "image_generate" | "audio_generate") || os == "macos")
}

pub struct InstalledRepositories<'a> {
    config: &'a Config,
    catalog: Value,
}
impl<'a> InstalledRepositories<'a> {
    pub fn new(config: &'a Config) -> Result<Self> {
        let ids = config
            .agents
            .iter()
            .map(|agent| &agent.id)
            .collect::<BTreeSet<_>>();
        ensure!(
            ids.len() == config.agents.len(),
            "duplicate configured agent identity"
        );
        Ok(Self {
            config,
            catalog: gents_local_mcp::Registry::new().catalog(),
        })
    }
}

impl ArchitectureRepository for InstalledRepositories<'_> {
    fn architectures(&self) -> Result<Vec<Architecture>> {
        Ok(vec![
            Architecture {
                id: "flow".into(),
                version: 1,
                coordination: Coordination::Ordered,
                executor: Some("autonomy_ordered_team_v1".into()),
            },
            Architecture {
                id: "orchestration".into(),
                version: 1,
                coordination: Coordination::AdaptiveDelegation,
                executor: None,
            },
            Architecture {
                id: "collaboration".into(),
                version: 1,
                coordination: Coordination::PeerReview,
                executor: None,
            },
        ])
    }
}
impl AgentRepository for InstalledRepositories<'_> {
    fn agent(&self, id: &str) -> Result<Option<Agent>> {
        self.config
            .agents
            .iter()
            .find(|agent| agent.id == id)
            .map(|agent| {
                Ok(Agent {
                    id: agent.id.clone(),
                    local_native_worker: agent.worker.provider == "local"
                        && agent.worker.validate().is_ok(),
                    worker_configuration_sha256: digest(&agent.worker)?,
                })
            })
            .transpose()
    }
}
impl ToolRepository for InstalledRepositories<'_> {
    fn tool(&self, capability: &str) -> Result<Option<Tool>> {
        if capability == "finish" {
            return Ok(Some(Tool {
                capability: capability.into(),
                custody: Custody::ProjectOwned,
                descriptor_sha256: digest(
                    &json!({"controller":"finish_submission_v1","semantics":"submit evidence for independent acceptance, never self-declare completion"}),
                )?,
                platform_supported: true,
                nontransactional_effects: false,
                local_only: true,
                provider_cost_usd: 0,
            }));
        }
        let (name, operation) = capability
            .split_once(':')
            .map_or((capability, None), |(name, op)| (name, Some(op)));
        if operation.is_some()
            && !(name == "files" && matches!(operation, Some("read" | "write" | "list")))
        {
            return Ok(None);
        }
        // Raw files would mean more authority than an operation-scoped grant.
        if name == "files" && operation.is_none() {
            return Ok(None);
        }
        let descriptor = self.catalog["tools"]
            .as_array()
            .and_then(|tools| tools.iter().find(|tool| tool["name"] == name));
        descriptor
            .map(|descriptor| {
                let platform_supported = supported_platform(
                    std::env::consts::OS,
                    name,
                    descriptor["runtime_requirements"]["installed_rustc"] == true,
                );
                // This adapter is deliberately closed to newly registered
                // capabilities until their local execution scope is reviewed.
                let local_only = matches!(
                    name,
                    "files"
                        | "integer_arithmetic"
                        | "table_read_json"
                        | "table_join"
                        | "table_aggregate"
                        | "audio_inspect"
                        | "image_generate"
                        | "audio_generate"
                        | "host_resources"
                        | "memory_append"
                        | "memory_search"
                        | "skill_list"
                        | "skill_read"
                        | "rust_verify"
                        | "owned_rust_call"
                );
                Ok(Tool {
                    capability: capability.into(),
                    custody: if descriptor["source"]
                        .as_str()
                        .is_some_and(|source| source.starts_with("mcp/src/functions/"))
                    {
                        Custody::ProjectOwned
                    } else {
                        Custody::External
                    },
                    descriptor_sha256: digest(
                        &json!({"descriptor":descriptor,"operation":operation}),
                    )?,
                    platform_supported,
                    nontransactional_effects: descriptor["external_effects"] == true,
                    local_only,
                    provider_cost_usd: 0,
                })
            })
            .transpose()
    }
}
impl ComplianceProfileRepository for InstalledRepositories<'_> {
    fn control_profile(&self, id: &str) -> Result<Option<ControlProfile>> {
        Ok((id == "local_baseline").then(|| ControlProfile {
            id: id.into(),
            version: 1,
            required_controls: [
                "owner_approved_frozen_intake",
                "current_actor_phase_policy",
                "confined_workspace_io",
                "native_source_and_action_receipts",
                "independent_acceptance",
                "per_agent_memory_governance",
            ]
            .map(str::to_owned)
            .to_vec(),
            gaps: vec![],
        }))
    }
}

/// The owner-scoped canonical read precedes all composition. This endpoint does
/// not save the supplied specification, mutate the session or execute inference.
pub fn preview(
    root: &std::path::Path,
    company: &str,
    owner: &str,
    id: uuid::Uuid,
    config: &Config,
    input: Value,
) -> Result<Value> {
    let session = crate::ceo_planning::read(root, company, owner, id)?;
    ensure!(
        config.company_mode && config.company_tools,
        "factory preview requires the tool-enabled company"
    );
    let request: PreviewRequest = serde_json::from_value(input)?;
    let repositories = InstalledRepositories::new(config)?;
    let factory = CompanyFactory {
        architectures: &repositories,
        agents: &repositories,
        tools: &repositories,
        compliance: &repositories,
    };
    let blueprint = factory.compose(&request, &session)?;
    let blueprint_sha256 = digest(&blueprint)?;
    Ok(
        json!({"blueprint":blueprint,"blueprint_sha256":blueprint_sha256,"planning_session":id,"owner_review_required":true,"execution_authorized":false}),
    )
}

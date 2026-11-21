//! Registered, versioned plans. Proposals describe work; the host grants it.
use super::contracts::*;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Hints {
    pub domain: Option<String>,
    pub evidence_required: Option<bool>,
    /// Caller preference is only a proposal. No endpoint, code or grant fields.
    pub template: Option<String>,
    #[serde(default)]
    pub subtasks: Vec<String>,
    #[serde(default)]
    pub essential_questions: Vec<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub revision: String,
    pub instructions: String,
    pub required_capabilities: BTreeSet<String>,
    pub tools: BTreeSet<String>,
    pub validators: BTreeSet<String>,
    pub memory_scope: String,
}
#[derive(Clone, Debug)]
pub struct AgentRegistry {
    profiles: BTreeMap<String, Profile>,
}
impl Default for AgentRegistry {
    fn default() -> Self {
        Self {
            profiles: profiles(),
        }
    }
}
impl AgentRegistry {
    pub fn register(&mut self, profile: Profile) -> Result<()> {
        ensure!(
            self.profiles.len() < 4096
                && !self.profiles.contains_key(&profile.id)
                && !profile.id.is_empty()
                && profile.id.len() <= 100
                && profile.instructions.len() <= 4096
                && !profile.revision.is_empty(),
            "invalid/duplicate profile registration"
        );
        ensure!(
            profile
                .tools
                .iter()
                .all(|t| super::read_tools::NAMES.contains(&t.as_str()))
                && !profile.memory_scope.is_empty(),
            "profile cannot create new tools or unscoped memory"
        );
        super::validators::Registry::validate_names(&profile.validators)?;
        self.profiles.insert(profile.id.clone(), profile);
        Ok(())
    }
    pub fn entries(&self) -> &BTreeMap<String, Profile> {
        &self.profiles
    }
}
pub fn profiles() -> BTreeMap<String, Profile> {
    [
        ("intake", "Preserve exact objective and constraints. Ask only essential undiscoverable intent. Propose registered templates, never permissions.", "input_custody"),
        ("general", "Resolve the requested deliverable; report missing evidence and unrun checks.", "output_shape"),
        ("history", "Resolve jurisdiction and dates; distinguish officeholders, election winners and inaugurations. Cite retrieved sources, flag disputed claims and uncertainty.", "current_citations"),
        ("angular", "Inspect actual package versions and conventions. Code proposals require authorized real builds/tests before verified status.", "angular_build"),
        ("cosmos", "Inspect API, schema and partition evidence. Distinguish proposed queries from executed results; RU and query-plan metrics require native receipts.", "cosmos_receipts"),
        ("dotnet", "Inspect target framework and API contract; require real integration/build evidence for implementation claims.", "dotnet_build"),
        ("research", "Check coverage, freshness, contradictions and actual source support. Retrieval misses mean missing evidence.", "current_citations"),
        ("integration", "Preserve cross-domain contracts, dependencies and original objective. Require real integration receipts.", "integration_receipts"),
        ("evaluation", "Independently evaluate criteria. Citation presence is not entailment; never claim unrun tests passed.", "independent_acceptance"),
    ].into_iter().map(|(id, instructions, validator)| {
        let profile = Profile { id: id.into(), revision: "1".into(), instructions: instructions.into(),
            required_capabilities: BTreeSet::from(["generation".into()]), tools: super::read_tools::NAMES.iter().map(|n|(*n).to_owned()).collect(),
            validators: if id=="dotnet" {BTreeSet::from([validator.into(),"dotnet_api".into()])}else{BTreeSet::from([validator.into()])}, memory_scope: "task-and-access-scope-only".into() };
        (id.into(), profile)
    }).collect()
}
#[derive(Clone, Debug, Serialize)]
pub struct Template {
    pub id: &'static str,
    pub coordination: Coordination,
    pub strategy: Strategy,
    pub stages: &'static [(&'static str, &'static [&'static str])],
}
/// Deliberately finite graphs. Hierarchy is two levels; peer has two scheduled
/// rounds and one finalizer. Larger graphs require a checked-in template.
pub fn templates() -> Vec<Template> {
    vec![
        Template {
            id: "direct/v1",
            coordination: Coordination::Single,
            strategy: Strategy::Direct,
            stages: &[("answer", &[])],
        },
        Template {
            id: "rag/v1",
            coordination: Coordination::Single,
            strategy: Strategy::GroundedGeneration,
            stages: &[("answer", &[])],
        },
        Template {
            id: "tool_loop/v1",
            coordination: Coordination::Single,
            strategy: Strategy::ToolLoop,
            stages: &[("agent", &[])],
        },
        Template {
            id: "pipeline/v1",
            coordination: Coordination::AssemblyLine,
            strategy: Strategy::Direct,
            stages: &[
                ("draft", &[]),
                ("transform", &["draft"]),
                ("final", &["transform"]),
            ],
        },
        Template {
            id: "hub/v1",
            coordination: Coordination::HubAndSpoke,
            strategy: Strategy::Direct,
            stages: &[
                ("scope", &[]),
                ("spoke_a", &["scope"]),
                ("spoke_b", &["scope"]),
                ("final", &["spoke_a", "spoke_b"]),
            ],
        },
        Template {
            id: "hierarchy/v1",
            coordination: Coordination::Hierarchy,
            strategy: Strategy::Direct,
            stages: &[
                ("manager", &[]),
                ("delegate_a", &["manager"]),
                ("delegate_b", &["manager"]),
                ("manager_review", &["delegate_a", "delegate_b"]),
                ("final", &["manager_review"]),
            ],
        },
        Template {
            id: "peer/v1",
            coordination: Coordination::PeerCollaboration,
            strategy: Strategy::Direct,
            stages: &[
                ("peer_a", &[]),
                ("peer_b", &[]),
                ("peer_a_review", &["peer_a", "peer_b"]),
                ("peer_b_review", &["peer_a", "peer_b"]),
                ("final", &["peer_a_review", "peer_b_review"]),
            ],
        },
        Template {
            id: "critique/v1",
            coordination: Coordination::AssemblyLine,
            strategy: Strategy::CritiqueRevision,
            stages: &[
                ("draft", &[]),
                ("critique", &["draft"]),
                ("revision", &["draft", "critique"]),
            ],
        },
        Template {
            id: "explore/v1",
            coordination: Coordination::HubAndSpoke,
            strategy: Strategy::CandidateExploration,
            stages: &[
                ("candidate_a", &[]),
                ("candidate_b", &[]),
                ("compare", &["candidate_a", "candidate_b"]),
                ("final", &["compare"]),
            ],
        },
    ]
}
pub fn factory(
    request: &TaskRequest,
    template_id: &str,
    profile: &str,
    model: Option<&str>,
) -> Result<ExecutionPlan> {
    factory_with_profiles(request, template_id, profile, model, &profiles())
}
pub fn factory_with_profiles(
    request: &TaskRequest,
    template_id: &str,
    profile: &str,
    model: Option<&str>,
    registry: &BTreeMap<String, Profile>,
) -> Result<ExecutionPlan> {
    request.validate()?;
    ensure!(registry.contains_key(profile), "unknown profile");
    let validators = super::validators::Registry::profile_checks(&registry[profile].validators)?;
    let template = templates()
        .into_iter()
        .find(|t| t.id == template_id)
        .context("unavailable workflow template")?;
    let nodes = template
        .stages
        .iter()
        .map(|(id, deps)| PlanNode {
            id: (*id).into(),
            dependencies: deps.iter().map(|v| (*v).into()).collect(),
            profile: profile.into(),
            provider_model: model.map(str::to_owned),
            strategy: template.strategy.clone(),
            tools: if template.strategy == Strategy::ToolLoop {
                vec!["search_knowledge".into()]
            } else {
                vec![]
            },
            validators: validators.clone(),
        })
        .collect();
    let scheduling = if matches!(
        template.coordination,
        Coordination::Single | Coordination::AssemblyLine
    ) {
        Scheduling::Sequential
    } else {
        Scheduling::Mixed
    };
    Ok(ExecutionPlan { schema_version:VERSION, plan_id:Uuid::new_v4(), task_id:request.task_id, revision:1,
        previous_plan:None, original_objective_sha256:crate::digest(request.original_ask.as_bytes()), template:template_id.into(),
        coordination:template.coordination, scheduling, strategy:template.strategy, nodes,
        routing_justification:"Caller proposal or conservative deterministic routing; quality/cost optimality unproven.".into(),
        cache_decision:"execute cache/extraction fast path before admitting this plan".into(),evidence_required:template_id=="rag/v1",resolved_models:model.map(|id|BTreeMap::from([(id.to_owned(),id.to_owned())])).unwrap_or_default() })
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
pub struct Intake {
    pub request: TaskRequest,
    pub analysis: TaskAnalysis,
    pub proposed_plan: ExecutionPlan,
    pub readiness: String,
    pub generative_model_calls: u32,
}
pub fn destructure_task(request: TaskRequest, hints: Hints) -> Result<Intake> {
    destructure_with_profiles(request, hints, &profiles())
}
pub fn destructure_with_profiles(
    request: TaskRequest,
    mut hints: Hints,
    registry: &BTreeMap<String, Profile>,
) -> Result<Intake> {
    request.validate()?;
    if hints.essential_questions.is_empty() {
        hints.essential_questions = request.clarification_questions.clone();
    }

    ensure!(
        hints.subtasks.len() <= 8
            && hints.essential_questions.len() <= 4
            && hints
                .subtasks
                .iter()
                .chain(&hints.essential_questions)
                .all(|s| !s.trim().is_empty() && s.len() <= 1000),
        "unbounded intake proposal"
    );
    let inferred = domain_hint(&request.original_ask);
    let domain = hints.domain.as_deref().unwrap_or(inferred);
    ensure!(registry.contains_key(domain), "unknown domain profile");
    let evidence = hints.evidence_required.unwrap_or(true);
    // No inference of difficulty from characters, question count or branding.
    let template =
        hints
            .template
            .as_deref()
            .unwrap_or(if evidence { "rag/v1" } else { "direct/v1" });
    let mut plan = factory_with_profiles(&request, template, domain, None, registry)?;
    plan.evidence_required = evidence || plan.evidence_required;
    if evidence && template == "direct/v1" {
        plan.strategy = Strategy::GroundedGeneration;
        for n in &mut plan.nodes {
            n.strategy = Strategy::GroundedGeneration;
        }
    }
    let analysis = TaskAnalysis {
        schema_version: VERSION,
        task_id: request.task_id,
        domain: Some(domain.into()),
        task_type: if request.input.intent == Intent::Action {
            "governed_action"
        } else {
            "answer"
        }
        .into(),
        ambiguity: if hints.essential_questions.is_empty() {
            "not_evaluated"
        } else {
            "essential_intent_missing"
        }
        .into(),
        difficulty_estimate: None,
        evidence_required: evidence,
        freshness_scope: request.input.temporal_scope.clone(),
        retrieval_suitable: evidence,
        evidence_sufficient: false,
        decomposition_useful: (!hints.subtasks.is_empty()).then_some(true),
        atomic_subtasks: hints.subtasks.clone(),
        observed_dates_or_ranges: if request.input.temporal_scope.is_empty() {
            vec![]
        } else {
            vec![request.input.temporal_scope.clone()]
        },
        tool_environment_requirements: if evidence {
            vec!["current authorized local references".into()]
        } else {
            vec![]
        },
        candidate_capabilities: if evidence {
            vec!["generation".into(), "retrieval".into()]
        } else {
            vec!["generation".into()]
        },
        subtasks: plan.nodes.clone(),
        routing_justification: plan.routing_justification.clone(),
    };
    let mut request = request;
    request.clarification_questions = hints.essential_questions;
    let readiness = if request.input.intent == Intent::Action {
        "requires_existing_action_authorization"
    } else if !request.clarification_questions.is_empty() {
        "needs_clarification"
    } else {
        "proposed_requires_policy_and_model_resolution"
    };
    Ok(Intake {
        readiness: readiness.into(),
        request,
        analysis,
        proposed_plan: plan,
        generative_model_calls: 0,
    })
}
fn domain_hint(ask: &str) -> &'static str {
    let terms: BTreeSet<_> = ask
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .map(str::to_lowercase)
        .collect();
    let angular = terms.contains("angular");
    let cosmos = terms.contains("cosmos");
    let dotnet = terms.contains(".net") || terms.contains("dotnet");
    if [angular, cosmos, dotnet].into_iter().filter(|b| *b).count() > 1 {
        return "integration";
    }
    if angular {
        return "angular";
    }
    if cosmos {
        return "cosmos";
    }
    if dotnet {
        return "dotnet";
    }
    if ["president", "officeholder", "inauguration", "election"]
        .iter()
        .any(|term| terms.contains(*term))
    {
        "history"
    } else {
        "general"
    }
}

/// Graph and all proposed identities must match a registered implementation.
pub fn validate_plan(
    request: &TaskRequest,
    plan: &ExecutionPlan,
    policy: &ExecutionPolicy,
    models: &BTreeSet<String>,
) -> Result<()> {
    validate_plan_with_profiles(request, plan, policy, models, &profiles())
}
pub fn validate_plan_with_profiles(
    request: &TaskRequest,
    plan: &ExecutionPlan,
    policy: &ExecutionPolicy,
    models: &BTreeSet<String>,
    registry: &BTreeMap<String, Profile>,
) -> Result<()> {
    request.validate()?;
    policy.validate(crate::now_ms())?;
    ensure!(
        plan.schema_version == VERSION
            && plan.task_id == request.task_id
            && plan.original_objective_sha256 == crate::digest(request.original_ask.as_bytes()),
        "plan changed task identity/objective"
    );
    let registered = factory_with_profiles(
        request,
        &plan.template,
        plan.nodes.first().context("empty graph")?.profile.as_str(),
        None,
        registry,
    )?;
    ensure!(
        plan.coordination == registered.coordination && plan.nodes.len() == registered.nodes.len(),
        "template coordination/size mismatch"
    );
    ensure!(
        plan.nodes.len() <= policy.max_model_calls as usize,
        "minimum registered workflow calls exceed global budget"
    );
    ensure!(
        policy.max_depth
            >= if plan.coordination == Coordination::Hierarchy {
                2
            } else {
                1
            }
            && policy.max_fan_out
                >= if matches!(
                    plan.coordination,
                    Coordination::HubAndSpoke
                        | Coordination::Hierarchy
                        | Coordination::PeerCollaboration
                ) {
                    2
                } else {
                    1
                },
        "depth/fan-out exceeds policy"
    );
    ensure!(
        (plan.strategy == registered.strategy
            || registered.strategy == Strategy::Direct
                && plan.strategy == Strategy::GroundedGeneration)
            && (!policy.evidence_required || plan.evidence_required),
        "plan strategy/evidence requirements differ from host contract"
    );
    let mut seen = BTreeSet::new();
    for (node, expected) in plan.nodes.iter().zip(&registered.nodes) {
        ensure!(
            node.id == expected.id
                && node.dependencies == expected.dependencies
                && seen.insert(&node.id),
            "unknown node, cycle or template dependency mismatch"
        );
        ensure!(
            node.dependencies.iter().all(|dep| seen.contains(dep)),
            "forward/cyclic graph edge"
        );
        ensure!(registry.contains_key(&node.profile), "unknown profile");
        ensure!(
            node.provider_model
                .as_ref()
                .is_some_and(|id| models.contains(id)),
            "unknown/unresolved model"
        );
        ensure!(
            (if node.strategy == Strategy::ToolLoop {
                !node.tools.is_empty()
                    && node.tools.iter().collect::<BTreeSet<_>>().len() == node.tools.len()
                    && node.tools.iter().all(|tool| {
                        super::read_tools::NAMES.contains(&tool.as_str())
                            && registry[&node.profile].tools.contains(tool)
                            && policy.permitted_tools.contains(tool)
                    })
            } else {
                node.tools.is_empty()
            }) && node.validators
                == super::validators::Registry::profile_checks(
                    &registry[&node.profile].validators
                )?
                && (node.strategy == expected.strategy
                    || expected.strategy == Strategy::Direct
                        && node.strategy == Strategy::GroundedGeneration
                        && plan.strategy == Strategy::GroundedGeneration),
            "unregistered tool/validator/strategy"
        );
    }
    Ok(())
}
pub fn configure_tools(
    plan: &mut ExecutionPlan,
    policy: &ExecutionPolicy,
    profiles: &BTreeMap<String, Profile>,
) -> Result<()> {
    for node in &mut plan.nodes {
        if node.strategy == Strategy::ToolLoop {
            let profile = profiles.get(&node.profile).context("unknown profile")?;
            node.tools = profile
                .tools
                .intersection(&policy.permitted_tools)
                .filter(|name| super::read_tools::NAMES.contains(&name.as_str()))
                .cloned()
                .collect();
            ensure!(
                !node.tools.is_empty(),
                "no permitted registered read tools for tool-loop profile"
            );
        }
    }
    Ok(())
}
pub fn capabilities() -> Value {
    json!({"schema_version":VERSION,"templates":templates(),"profiles":profiles(),"read_tools":super::read_tools::NAMES,"coordination_is_not_scheduling":true,
        "result_boundary":"bounded JSON StageReply; only declared dependency outputs; no executable model output",
        "intake":"deterministic conservative proposal; optional evaluated model intake must validate the same Hints contract",
        "side_effect_execution":"existing owner-governed native MCP only"})
}

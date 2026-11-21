//! Host-owned task identity and policy are distinct from proposed work.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const VERSION: u32 = 1;
pub const COMPATIBILITY: &str = "task-engine-v1/exact-extraction-v1";

#[derive(schemars::JsonSchema, Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    #[default]
    Answer,
    Action,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    pub source_id: String,
    pub start_byte: usize,
    pub end_byte: usize,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OutputSpec {
    pub format: String,
    pub language: String,
}
impl Default for OutputSpec {
    fn default() -> Self {
        Self {
            format: "text".into(),
            language: "unspecified".into(),
        }
    }
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskInput {
    pub original_ask: String,
    #[serde(default)]
    pub supplied_context: BTreeMap<String, String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub deliverable: String,
    #[serde(default)]
    pub output: OutputSpec,
    #[serde(default)]
    pub temporal_scope: String,
    #[serde(default)]
    pub proposed_acceptance: Vec<String>,
    #[serde(default)]
    pub intent: Intent,
    #[serde(default)]
    pub extraction: Option<Extraction>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRequest {
    pub schema_version: u32,
    pub task_id: Uuid,
    pub original_ask: String,
    pub objective: String,
    pub input: TaskInput,
    pub assumptions: Vec<String>,
    pub confirmed_facts: Vec<String>,
    pub clarification_questions: Vec<String>,
}
impl TaskRequest {
    pub fn from_input(input: TaskInput) -> Result<Self> {
        ensure!(
            !input.original_ask.trim().is_empty() && input.original_ask.len() <= 12000,
            "ask requires 1..12000 UTF-8 bytes"
        );
        ensure!(
            serde_json::to_vec(&input)?.len() <= 24000,
            "bounded task input required"
        );
        ensure!(
            input.constraints.len() <= 16
                && input
                    .constraints
                    .iter()
                    .all(|value| !value.trim().is_empty() && value.len() <= 2000),
            "bounded nonempty constraints required"
        );
        ensure!(
            input.proposed_acceptance.len() <= 8
                && input
                    .proposed_acceptance
                    .iter()
                    .all(|value| !value.trim().is_empty() && value.len() <= 1000),
            "one to eight bounded nonempty criteria, or an empty unspecified list, required"
        );
        ensure!(
            input.supplied_context.len() <= 16
                && input
                    .supplied_context
                    .iter()
                    .all(|(key, value)| !key.trim().is_empty()
                        && key.len() <= 120
                        && value.len() <= 8000),
            "bounded explicit context required"
        );
        ensure!(
            input.deliverable.len() <= 2000 && input.temporal_scope.len() <= 160,
            "bounded deliverable and temporal scope required"
        );
        ensure!(
            matches!(input.output.format.as_str(), "text" | "json"),
            "unsupported output format"
        );
        ensure!(
            !input.output.language.is_empty() && input.output.language.len() <= 80,
            "bounded output language required"
        );
        crate::safety::require_persistable(&serde_json::to_string(&input)?)?;
        Ok(Self {
            schema_version: VERSION,
            task_id: Uuid::new_v4(),
            original_ask: input.original_ask.clone(),
            objective: input.original_ask.clone(),
            input,
            assumptions: vec![],
            confirmed_facts: vec![],
            clarification_questions: vec![],
        })
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == VERSION && !self.task_id.is_nil(),
            "unsupported task identity"
        );
        ensure!(
            self.original_ask == self.input.original_ask && self.objective == self.original_ask,
            "host objective changed"
        );
        Self::from_input(self.input.clone())?;
        ensure!(
            self.assumptions.len() <= 16
                && self.confirmed_facts.len() <= 16
                && self.clarification_questions.len() <= 4
                && self
                    .assumptions
                    .iter()
                    .chain(&self.confirmed_facts)
                    .chain(&self.clarification_questions)
                    .all(|s| !s.trim().is_empty() && s.len() <= 2000),
            "unbounded request annotations"
        );
        for annotation in self
            .assumptions
            .iter()
            .chain(&self.confirmed_facts)
            .chain(&self.clarification_questions)
        {
            crate::safety::require_persistable(annotation)?;
        }
        Ok(())
    }
}

/// Constructed by the authenticated host. Never deserialize this from a model.
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
pub struct AccessScope {
    pub tenant: String,
    pub subject: String,
    pub effective_access_sha256: String,
    pub policy_revision: String,
    pub profile_revision: String,
    pub runtime_revision: String,
    pub conversation_sha256: String,
    pub allowed_sources: BTreeSet<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Serialize)]
pub struct ExecutionPolicy {
    pub schema_version: u32,
    pub scope: AccessScope,
    pub local_inference_required: bool,
    pub max_model_calls: u32,
    pub max_tokens: u32,
    pub deadline_ms: u64,
    pub max_retries: u32,
    pub max_fan_out: u32,
    pub max_depth: u32,
    pub max_concurrency: u32,
    pub cache_ttl_ms: u64,
    pub max_cache_entries: usize,
    pub max_evidence_bytes: usize,
    pub permitted_tools: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub permitted_validators: BTreeSet<String>,
    pub workspace_roots: Vec<String>,
    pub network_allowed: bool,
    pub allowed_provider_endpoints: BTreeSet<String>,
    pub retention_ms: u64,
    pub evidence_required: bool,
}
impl ExecutionPolicy {
    pub fn read_only(scope: AccessScope, now: u64) -> Self {
        Self {
            schema_version: VERSION,
            scope,
            local_inference_required: true,
            max_model_calls: 0,
            max_tokens: 0,
            deadline_ms: now.saturating_add(5000),
            max_retries: 0,
            max_fan_out: 1,
            max_depth: 1,
            max_concurrency: 1,
            cache_ttl_ms: 3600000,
            max_cache_entries: 256,
            max_evidence_bytes: 8192,
            permitted_tools: BTreeSet::from(["search_knowledge".into()]),
            permitted_validators: BTreeSet::new(),
            workspace_roots: vec![],
            network_allowed: false,
            allowed_provider_endpoints: BTreeSet::new(),
            retention_ms: 3600000,
            evidence_required: false,
        }
    }
    pub fn validate(&self, now: u64) -> Result<()> {
        super::validators::Registry::validate_names(&self.permitted_validators)?;
        ensure!(
            self.schema_version == VERSION && self.deadline_ms > now && now <= i64::MAX as u64,
            "task policy deadline expired"
        );
        ensure!(
            !self.scope.tenant.is_empty()
                && !self.scope.subject.is_empty()
                && self.scope.allowed_sources.len() <= 256,
            "invalid trusted scope"
        );
        ensure!(
            self.permitted_tools
                .iter()
                .all(|t| super::read_tools::NAMES.contains(&t.as_str()))
                && self.workspace_roots.len() <= 4
                && self
                    .workspace_roots
                    .iter()
                    .all(|p| std::path::Path::new(p).is_absolute()),
            "unknown read tool or invalid trusted workspace roots"
        );
        ensure!(
            !self
                .permitted_tools
                .iter()
                .any(|t| t.starts_with("workspace_"))
                || !self.workspace_roots.is_empty(),
            "workspace read authority requires a trusted root"
        );
        ensure!(
            (1..=1024).contains(&self.max_cache_entries)
                && self.cache_ttl_ms <= 86400000
                && (1..=16384).contains(&self.max_evidence_bytes),
            "invalid storage/evidence limits"
        );
        ensure!(
            self.max_model_calls <= 64
                && self.max_tokens <= 131072
                && self.max_retries <= 2
                && (1..=8).contains(&self.max_concurrency)
                && (1..=8).contains(&self.max_depth)
                && (1..=8).contains(&self.max_fan_out)
                && self.retention_ms <= 86400000,
            "invalid trusted global budgets"
        );
        Ok(())
    }
}

#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Coordination {
    Single,
    AssemblyLine,
    HubAndSpoke,
    Hierarchy,
    PeerCollaboration,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scheduling {
    Sequential,
    Parallel,
    Mixed,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    ExactLookup,
    SourceExtraction,
    Direct,
    GroundedGeneration,
    ToolLoop,
    CritiqueRevision,
    CandidateExploration,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanNode {
    pub id: String,
    pub dependencies: Vec<String>,
    pub profile: String,
    pub provider_model: Option<String>,
    pub strategy: Strategy,
    pub tools: Vec<String>,
    pub validators: Vec<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPlan {
    pub schema_version: u32,
    pub plan_id: Uuid,
    pub task_id: Uuid,
    pub revision: u32,
    pub previous_plan: Option<Uuid>,
    pub original_objective_sha256: String,
    pub template: String,
    pub coordination: Coordination,
    pub scheduling: Scheduling,
    pub strategy: Strategy,
    pub nodes: Vec<PlanNode>,
    pub routing_justification: String,
    pub cache_decision: String,
    #[serde(default)]
    pub evidence_required: bool,
    #[serde(default)]
    pub resolved_models: BTreeMap<String, String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAnalysis {
    pub schema_version: u32,
    pub task_id: Uuid,
    pub domain: Option<String>,
    pub task_type: String,
    pub ambiguity: String,
    pub difficulty_estimate: Option<String>,
    pub evidence_required: bool,
    pub freshness_scope: String,
    pub retrieval_suitable: bool,
    pub evidence_sufficient: bool,
    pub decomposition_useful: Option<bool>,
    pub subtasks: Vec<PlanNode>,
    pub routing_justification: String,
    #[serde(default)]
    pub atomic_subtasks: Vec<String>,
    #[serde(default)]
    pub observed_dates_or_ranges: Vec<String>,
    #[serde(default)]
    pub tool_environment_requirements: Vec<String>,
    #[serde(default)]
    pub candidate_capabilities: Vec<String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotRun,
    Unsupported,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source_id: String,
    pub source_revision: String,
    pub content_sha256: String,
    pub title: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub quote: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Measurements {
    pub generative_model_calls: Option<u32>,
    pub embedding_calls: u32,
    pub tool_calls: u32,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub elapsed_micros: Option<u64>,
    pub provider_cost_usd: Option<u64>,
    pub memory_bytes: Option<u64>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CacheProvenance {
    pub decision: String,
    pub key: String,
    pub origin_task_id: Option<Uuid>,
    pub created_at_ms: Option<u64>,
    pub expires_at_ms: Option<u64>,
    pub work_avoided: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskResult {
    pub schema_version: u32,
    pub task_id: Uuid,
    pub status: String,
    pub answer: Option<String>,
    pub artifacts: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub blockers: Vec<String>,
    pub acceptance_checks: Vec<AcceptanceCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_receipts: Vec<super::validators::NativeReceipt>,
    pub actual_path: String,
    pub verification_state: String,
    pub cache: CacheProvenance,
    pub measurements: Measurements,
    pub plan: Option<ExecutionPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<TaskAnalysis>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clarification_questions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
}

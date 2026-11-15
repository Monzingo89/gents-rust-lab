//! Review contracts, never permission or execution contracts.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Coordination {
    Ordered,
    AdaptiveDelegation,
    PeerReview,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Synthetic,
    Public,
    Confidential,
    Regulated,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpecification {
    pub version: u32,
    pub objective: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub definition_of_done: Vec<String>,
    pub exclusions: Vec<String>,
    pub coordination: Coordination,
    pub data_classification: DataClassification,
    /// Owner-selected control profiles, not model-invented compliance claims.
    pub control_profiles: Vec<String>,
    /// Include missing tools too, so absence becomes a development gap.
    pub required_capabilities: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewRequest {
    pub expected_revision: u64,
    pub specification: TaskSpecification,
}

#[derive(Clone, Debug, Serialize)]
pub struct Architecture {
    pub id: String,
    pub version: u32,
    pub coordination: Coordination,
    /// None means the pattern is unavailable; never silently substitute flow.
    pub executor: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Agent {
    pub id: String,
    pub local_native_worker: bool,
    /// Configuration identity only. Model bytes/fit are checked at execution.
    pub worker_configuration_sha256: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Custody {
    ProjectOwned,
    External,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tool {
    pub capability: String,
    pub custody: Custody,
    pub descriptor_sha256: String,
    pub platform_supported: bool,
    /// Effects outside the DB transaction, including confined workspace I/O.
    pub nontransactional_effects: bool,
    pub local_only: bool,
    pub provider_cost_usd: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ControlProfile {
    pub id: String,
    pub version: u32,
    pub required_controls: Vec<String>,
    /// Owner/operator evidence still needed. No self-certification by a model.
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Gap {
    pub code: String,
    pub subject: String,
}

#[derive(Serialize)]
pub struct Blueprint {
    pub schema_version: u32,
    pub status: String,
    pub planning_revision: u64,
    pub proposal_sha256: String,
    pub specification_sha256: String,
    pub repository_snapshot_sha256: String,
    pub architecture: Architecture,
    pub agents: Vec<Agent>,
    pub assignments: Vec<crate::ceo_team::Stage>,
    pub proposed_communication_controls: Option<serde_json::Value>,
    pub tools: Vec<Tool>,
    pub control_profiles: Vec<ControlProfile>,
    pub required_controls: Vec<String>,
    pub gaps: Vec<Gap>,
    pub preview_only: bool,
    pub execution_authorized: bool,
    pub scope: String,
}

//! Disabled-by-default candidate manifest/promotion gate. Production never
//! trains, edits, uploads or mutates weights while answering a task.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Quantization,
    Distillation,
    StructuredPruning,
    Adapter,
    WeightSpaceEditing,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version: u32,
    pub parent_sha256: String,
    pub candidate_sha256: String,
    pub method: Method,
    pub tools: Vec<String>,
    pub data_license_provenance: Vec<String>,
    pub artifact_path: String,
    pub parent_path: String,
    pub runtime_compatibility: String,
    pub compute_seconds: u64,
    pub storage_bytes: u64,
    pub metrics: BTreeMap<String, f64>,
    pub baseline_metrics: BTreeMap<String, f64>,
    pub heldout_sha256: String,
}
#[derive(Clone, Debug, Default)]
pub struct ResearchPolicy {
    pub enabled: bool,
    pub cloud_allowed: bool,
    pub max_compute_seconds: u64,
    pub max_storage_bytes: u64,
    pub allowed_methods: Vec<Method>,
}
pub fn check_candidate(candidate: &Candidate, policy: &ResearchPolicy) -> Result<()> {
    ensure!(
        policy.enabled,
        "optimization disabled by trusted host configuration"
    );
    ensure!(
        candidate.version == 1
            && candidate.artifact_path != candidate.parent_path
            && candidate.parent_sha256 != candidate.candidate_sha256,
        "immutable separate parent/candidate identity required"
    );
    for hash in [
        &candidate.parent_sha256,
        &candidate.candidate_sha256,
        &candidate.heldout_sha256,
    ] {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "candidate digest required"
        );
    }
    ensure!(
        candidate.compute_seconds <= policy.max_compute_seconds
            && candidate.storage_bytes <= policy.max_storage_bytes,
        "research resource budget exceeded"
    );
    ensure!(
        !candidate.data_license_provenance.is_empty()
            && !candidate.tools.is_empty()
            && !candidate.runtime_compatibility.is_empty(),
        "candidate provenance/compatibility incomplete"
    );
    ensure!(
        policy
            .allowed_methods
            .iter()
            .any(|m| std::mem::discriminant(m) == std::mem::discriminant(&candidate.method)),
        "experimental method unavailable"
    );
    Ok(())
}
pub const QUALITY_METRICS: &[&str] = &[
    "domain_accuracy",
    "temporal_grounding",
    "language_reasoning",
    "instruction_following",
    "tool_use",
    "unrelated_capabilities",
];
/// Explicit operation only. Returns a decision; it never overwrites live files.
pub fn promotion(
    candidate: &Candidate,
    policy: &ResearchPolicy,
    floors: &BTreeMap<String, f64>,
) -> Result<String> {
    check_candidate(candidate, policy)?;
    for name in QUALITY_METRICS {
        let score = *candidate
            .metrics
            .get(*name)
            .ok_or_else(|| anyhow::anyhow!("held-out quality metric missing"))?;
        let baseline = *candidate
            .baseline_metrics
            .get(*name)
            .ok_or_else(|| anyhow::anyhow!("shared-model-plus-RAG baseline missing"))?;
        let floor = *floors
            .get(*name)
            .ok_or_else(|| anyhow::anyhow!("host promotion criterion missing"))?;
        ensure!(
            score.is_finite()
                && baseline.is_finite()
                && floor.is_finite()
                && score >= floor
                && score >= baseline,
            "candidate quality regression: {name}"
        );
    }
    for name in [
        "disk_bytes",
        "loaded_memory_bytes",
        "latency_ms",
        "throughput",
    ] {
        ensure!(
            candidate
                .metrics
                .get(name)
                .is_some_and(|n| n.is_finite() && *n >= 0.),
            "resource benchmark missing"
        );
    }
    Ok("eligible_for_explicit_registry_promotion; original immutable artifact retained for rollback".into())
}

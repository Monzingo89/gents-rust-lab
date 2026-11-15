//! Explicit owner-reviewed control-plane access, independent of worker grants.
//! No historical memory, tools, network, paid provider or training authority.
use super::*;
use crate::{company_factory::digest, planning_models::Candidate, Config};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewContract {
    pub version: u32,
    pub coordination_sha256: String,
    pub model_policy_sha256: String,
    pub reviewer: Candidate,
    /// Explicit source ceiling approved with the job, never a worker grant.
    pub candidate_from: BTreeSet<String>,
    pub maximum_calls: u32,
}
impl ReviewContract {
    pub fn validate(&self, program: &Contract) -> Result<()> {
        program.validate()?;
        ensure!(
            self.version == 1
                && self.maximum_calls == 1
                && self.coordination_sha256 == program.sha256()?
                && program::hash(&self.model_policy_sha256),
            "invalid reasoning acceptance contract"
        );
        // Reuse the full owner-catalog model and memory-floor validation.
        crate::planning_models::Policy {
            failures_before_next: 1,
            candidates: vec![self.reviewer.clone()],
        }
        .validate()?;
        ensure!(
            self.reviewer.model.adapter.is_none()
                && program
                    .roles
                    .iter()
                    .all(|r| r.model_sha256 != self.reviewer.sha256)
                && self.candidate_from == program.roles.iter().map(|r| r.actor.clone()).collect(),
            "reviewer needs separate weights and an explicit frozen candidate-source ceiling"
        );
        Ok(())
    }
    pub fn sha256(&self) -> Result<String> {
        digest(self)
    }
}

pub fn select(config: &Config, program: &Contract) -> Result<ReviewContract> {
    let policy = config
        .planning_model_policy
        .as_ref()
        .context("owner-listed local review model required")?;
    policy.validate()?;
    let reviewer = policy
        .candidates
        .iter()
        .find(|candidate| {
            candidate.model.adapter.is_none()
                && program
                    .roles
                    .iter()
                    .all(|r| r.model_sha256 != candidate.sha256)
        })
        .context(
            "no independent owner-pinned local reviewer; install/review distinct weights first",
        )?
        .clone();
    let contract = ReviewContract {
        version: 1,
        coordination_sha256: program.sha256()?,
        model_policy_sha256: policy.digest()?,
        reviewer,
        candidate_from: program.roles.iter().map(|r| r.actor.clone()).collect(),
        maximum_calls: 1,
    };
    contract.validate(program)?;
    Ok(contract)
}
pub fn from_intake(intake: &Value) -> Result<Option<ReviewContract>> {
    let Some(value) = intake.get("reasoning_acceptance") else {
        return Ok(None);
    };
    let contract: ReviewContract = serde_json::from_value(value.clone())?;
    let program = admission::from_intake(intake)?
        .context("reasoning acceptance requires a frozen reasoning program")?;
    contract.validate(&program)?;
    Ok(Some(contract))
}
pub fn validate_config(config: &Config, contract: &ReviewContract) -> Result<()> {
    let policy = config
        .planning_model_policy
        .as_ref()
        .context("local model policy removed")?;
    policy.validate()?;
    let expected = digest(&contract.reviewer)?;
    let mut listed = false;
    for candidate in &policy.candidates {
        listed |= digest(candidate)? == expected;
    }
    ensure!(
        policy.digest()? == contract.model_policy_sha256 && listed,
        "review model policy changed; fresh owner approval required"
    );
    Ok(())
}

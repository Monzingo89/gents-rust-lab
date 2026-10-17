//! Explicit, deterministic user checks. No model can waive a failed rule.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    #[serde(default)]
    pub required_substrings: Vec<String>,
    #[serde(default)]
    pub forbidden_substrings: Vec<String>,
}
#[derive(Serialize)]
pub struct CheckResult {
    pub kind: &'static str,
    pub value: String,
    pub passed: bool,
}
impl Checks {
    pub fn validate(&self) -> Result<()> {
        if self.required_substrings.len() + self.forbidden_substrings.len() > 16
            || self
                .required_substrings
                .iter()
                .chain(&self.forbidden_substrings)
                .any(|v| v.is_empty() || v.len() > 500)
        {
            bail!("at most 16 nonempty acceptance substrings, each at most 500 bytes");
        }
        Ok(())
    }
    pub fn is_empty(&self) -> bool {
        self.required_substrings.is_empty() && self.forbidden_substrings.is_empty()
    }
    pub fn evaluate(&self, text: &str) -> Vec<CheckResult> {
        self.required_substrings
            .iter()
            .map(|v| CheckResult {
                kind: "required_substring",
                value: v.clone(),
                passed: text.contains(v),
            })
            .chain(self.forbidden_substrings.iter().map(|v| CheckResult {
                kind: "forbidden_substring",
                value: v.clone(),
                passed: !text.contains(v),
            }))
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gates_are_case_sensitive_and_fail_on_forbidden_text() {
        let c = Checks {
            required_substrings: vec!["Vendor B".into()],
            forbidden_substrings: vec!["guaranteed".into()],
        };
        assert!(c.evaluate("Choose Vendor B").iter().all(|r| r.passed));
        assert!(!c.evaluate("choose vendor b").iter().all(|r| r.passed));
        assert!(!c.evaluate("Vendor B guaranteed").iter().all(|r| r.passed));
    }
}

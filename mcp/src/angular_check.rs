//! Owner-frozen Angular SDK and browser contracts. Parsing grants no tool,
//! installs no package, executes no project and establishes no validation pass.
use anyhow::{ensure, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, collections::BTreeSet, path::Path};
mod native;
pub mod runtime;
pub(crate) use native::{compute, finish, prepare_call};
pub use native::{
    has_contract, reference, reference_current, references_current, request_identity,
    runtime_bindings, source_read_identity, ReceiptAuthority,
};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SdkPin {
    pub node_version: String,
    pub node_sha256: String,
    pub angular_cli_version: String,
    pub sdk_tree_sha256: String,
    pub browser_tree_sha256: String,
    pub driver_tree_sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAction {
    ExpectText,
    Click,
    Fill,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserCase {
    pub id: String,
    pub action: BrowserAction,
    pub selector: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserContract {
    pub schema_version: u32,
    pub startup_timeout_ms: u64,
    pub step_timeout_ms: u64,
    pub runtime_timeout_ms: u64,
    pub cases: Vec<BrowserCase>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema_version: u32,
    pub id: String,
    pub actor: String,
    pub project_path: String,
    pub project_name: String,
    pub source_paths: Vec<String>,
    pub frozen_inputs: BTreeMap<String, String>,
    pub sdk: SdkPin,
    pub browser: BrowserContract,
}
fn component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn version(value: &str) -> bool {
    value.len() <= 24
        && value.split('.').count() == 3
        && value
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
}
impl BrowserContract {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && !self.cases.is_empty() && self.cases.len() <= 16,
            "Angular browser checks require version 1 and 1..16 frozen steps"
        );
        ensure!(
            (100..=5000).contains(&self.startup_timeout_ms)
                && (100..=2000).contains(&self.step_timeout_ms)
                && self.runtime_timeout_ms <= 40000
                && self.runtime_timeout_ms
                    >= self.startup_timeout_ms
                        + self.cases.len() as u64 * self.step_timeout_ms
                        + 100,
            "Angular browser check deadline does not cover its finite step budget"
        );
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            ensure!(
                component(&case.id)
                    && ids.insert(&case.id)
                    && !case.selector.is_empty()
                    && case.selector.len() <= 512
                    && case.selector.is_ascii()
                    && !case.selector.chars().any(char::is_control),
                "Invalid bounded CSS selector or browser step identity"
            );
            match case.action {
                BrowserAction::Click => ensure!(
                    case.value.is_none(),
                    "Click steps have no value or executable argument"
                ),
                BrowserAction::ExpectText | BrowserAction::Fill => ensure!(
                    case.value.as_ref().is_some_and(|v| v.len() <= 4096),
                    "Text/fill steps require a bounded literal value"
                ),
            }
        }
        ensure!(
            self.cases
                .iter()
                .any(|c| matches!(c.action, BrowserAction::ExpectText)),
            "Browser checks require an actual expected-text oracle, not interactions alone"
        );
        let bytes = serde_json::to_string(self)?;
        ensure!(
            bytes.len() <= 16384,
            "Angular browser case data limit exceeded"
        );
        crate::safety::require_persistable(&bytes)?;
        Ok(())
    }
}
impl Contract {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1
                && component(&self.id)
                && component(&self.project_name)
                && !self.actor.trim().is_empty()
                && self.actor.len() <= 200
                && !self.actor.chars().any(char::is_control),
            "Invalid Angular owner contract identity"
        );
        ensure!(
            version(
                self.sdk
                    .node_version
                    .strip_prefix('v')
                    .unwrap_or(&self.sdk.node_version)
            ) && version(&self.sdk.angular_cli_version)
                && [
                    &self.sdk.node_sha256,
                    &self.sdk.sdk_tree_sha256,
                    &self.sdk.browser_tree_sha256,
                    &self.sdk.driver_tree_sha256
                ]
                .into_iter()
                .all(|s| digest(s)),
            "Exact Node/Angular versions and complete host SDK/browser/driver pins are required"
        );
        ensure!(
            Path::new(&self.project_path)
                .file_name()
                .is_some_and(|s| s == "angular.json")
                && !self.source_paths.is_empty()
                && self.source_paths.len() <= 255,
            "Explicit angular.json and bounded source inputs required"
        );
        let base = Path::new(&self.project_path).parent().unwrap();
        let mut paths = BTreeSet::new();
        for path in std::iter::once(&self.project_path).chain(&self.source_paths) {
            ensure!(
                path.len() <= 240
                    && crate::workspace_path::is_safe_relative_path(path)
                    && paths.insert(path)
                    && Path::new(path).strip_prefix(base).is_ok()
                    && Path::new(path).components().all(|p| p
                        .as_os_str()
                        .to_str()
                        .is_some_and(|s| !s.starts_with('.')
                            && !matches!(s, "node_modules" | "dist" | "target")))
                    && ["ts", "html", "css", "scss", "json", "svg"].contains(
                        &Path::new(path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                    ),
                "Unsafe, hidden, duplicate or unsupported Angular snapshot path"
            );
            if path.ends_with(".json") {
                ensure!(
                    self.frozen_inputs.get(path).is_some_and(|s| digest(s)),
                    "Every Angular JSON configuration/data input must be owner-frozen"
                );
            }
        }
        for name in ["package.json", "package-lock.json"] {
            ensure!(
                base.join(name)
                    .to_str()
                    .is_some_and(|p| paths.iter().any(|s| s.as_str() == p)),
                "Angular package manifest and lock must be explicit snapshot inputs"
            );
        }
        ensure!(
            self.source_paths.iter().any(|p| p.ends_with(".ts"))
                && self
                    .frozen_inputs
                    .iter()
                    .all(|(path, hash)| paths.contains(path) && digest(hash)),
            "Frozen Angular controls must name explicit inputs; TypeScript source is required"
        );
        self.browser.validate()?;
        ensure!(
            serde_json::to_vec(self)?.len() <= 32768,
            "Angular owner contract exceeds its byte bound"
        );
        Ok(())
    }
}
pub fn contracts(intake: &Value) -> Result<Vec<Contract>> {
    let contracts: Vec<Contract> = serde_json::from_value(
        intake
            .get("angular_check_contracts")
            .cloned()
            .unwrap_or(json!([])),
    )?;
    ensure!(
        contracts.len() <= 4,
        "At most four Angular contracts per parent run"
    );
    let mut ids = BTreeSet::new();
    for contract in &contracts {
        contract.validate()?;
        ensure!(ids.insert(&contract.id), "Duplicate Angular contract ID");
    }
    Ok(contracts)
}
pub fn validate(intake: &Value) -> Result<()> {
    contracts(intake).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid() -> Value {
        json!({"angular_check_contracts":[{"schema_version":1,"id":"frontend","actor":"Worker","project_path":"ui/angular.json","project_name":"fixture","source_paths":["ui/package.json","ui/package-lock.json","ui/tsconfig.json","ui/src/main.ts","ui/src/index.html"],"frozen_inputs":{"ui/angular.json":"a".repeat(64),"ui/package.json":"b".repeat(64),"ui/package-lock.json":"c".repeat(64),"ui/tsconfig.json":"d".repeat(64)},"sdk":{"node_version":"v24.21.0","node_sha256":"1".repeat(64),"angular_cli_version":"21.2.26","sdk_tree_sha256":"2".repeat(64),"browser_tree_sha256":"3".repeat(64),"driver_tree_sha256":"4".repeat(64)},"browser":{"schema_version":1,"startup_timeout_ms":3000,"step_timeout_ms":500,"runtime_timeout_ms":10000,"cases":[{"id":"initial","action":"expect_text","selector":"#value","value":"0"},{"id":"click","action":"click","selector":"#increment"},{"id":"incremented","action":"expect_text","selector":"#value","value":"1"}]}}]})
    }
    #[test]
    fn owner_contract_cannot_choose_a_command_endpoint_or_sdk_path() {
        validate(&valid()).unwrap();
        for field in ["command", "endpoint", "sdk_root", "network", "browser_args"] {
            let mut value = valid();
            value["angular_check_contracts"][0][field] = json!("untrusted");
            assert!(validate(&value).is_err(), "{field}");
        }
        let mut value = valid();
        value["angular_check_contracts"][0]["browser"]["cases"][0]["action"] = json!("evaluate");
        assert!(validate(&value).is_err());
    }
    #[test]
    fn inputs_versions_criteria_and_budgets_are_frozen_and_bounded() {
        for pointer in [
            "/angular_check_contracts/0/frozen_inputs/ui~1angular.json",
            "/angular_check_contracts/0/frozen_inputs/ui~1package-lock.json",
        ] {
            let mut value = valid();
            *value.pointer_mut(pointer).unwrap() = Value::Null;
            assert!(validate(&value).is_err());
        }
        let mut value = valid();
        value["angular_check_contracts"][0]["source_paths"][0] = json!("../other/package.json");
        assert!(validate(&value).is_err());
        let mut value = valid();
        value["angular_check_contracts"][0]["sdk"]["angular_cli_version"] = json!("latest");
        assert!(validate(&value).is_err());
        let mut value = valid();
        value["angular_check_contracts"][0]["browser"]["runtime_timeout_ms"] = json!(300000);
        assert!(validate(&value).is_err());
        let mut value = valid();
        value["angular_check_contracts"][0]["browser"]["cases"] =
            json!([{"id":"click","action":"click","selector":"#increment"}]);
        assert!(validate(&value).is_err());
    }
}

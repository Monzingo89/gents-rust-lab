use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Phase, Tool, ToolContext,
};
use anyhow::Result;
use serde_json::{json, Value};
pub struct OwnedRustCall;
impl Tool for OwnedRustCall {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "owned_rust_call"
    }
    fn description(&self) -> &'static str {
        "Invoke an owned pure Rust library through a bounded JSON data ABI and installed macOS compiler/runtime sandbox. Provide binding_id and positional arguments only; exact actor, source path/hash, API hash and function must be owner-frozen in required_owned_rust_calls. Literal inputs require input_sha256. Dependent inputs instead require an explicit input_recipe with empty positional placeholders and exact approved producer paths/projections; only canonical current-epoch receipts may fill those slots, with matching wire types and communication grants. Actual input hashes and recipe/receipt lineage are native evidence, never model-supplied authority. Integers are canonical decimal strings; Option uses {some:value}/{none:true}, Result uses {ok:value}/{err:value}. No source authoring, arbitrary paths/flags, shell, dependencies, downloads, network or promotion. Failed invocation is not task completion. Current-epoch successful native evidence and unchanged sources are required by completion; independent task QA remains. A review package ID is owner-declared historical provenance, never native qualification or permission. Unsupported isolation fails closed."
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn frozen_source_admission(
        &self,
        context: &ToolContext<'_>,
        arguments: &Value,
    ) -> Result<bool> {
        EmbeddedContracts.validate(self.name(), arguments)?;
        crate::owned_components::admit(context, arguments)
    }
    fn runtime_requirements(&self) -> crate::RuntimeRequirements {
        crate::RuntimeRequirements::isolated_rust()
    }
    fn data_input_fields(&self) -> &'static [&'static str] {
        &["arguments"]
    }
    fn external_effects(&self) -> bool {
        false
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Worker]
    }
    fn execute(&self, context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &arguments)?;
        crate::owned_components::execute(context, &arguments)
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        Some(
            json!({"type":"object","required":["passed","stage","source_sha256","api_sha256","function","provider_cost_usd","promotion_authorized","binding_id","source_path","input_sha256","owner_declared_review_package_id"],"properties":{"passed":{"type":"boolean"},"output":{},"provider_cost_usd":{"const":0},"promotion_authorized":{"const":false}}}),
        )
    }
}

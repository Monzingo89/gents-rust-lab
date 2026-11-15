use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Phase, Tool, ToolContext,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
pub struct RustVerify;
impl Tool for RustVerify {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "rust_verify"
    }
    fn description(&self) -> &'static str {
        "Compile and test a bounded pure Rust function library with the installed toolchain and OS isolation. Provide source_path only; actor, acceptance path and acceptance SHA-256 must already be frozen in required_rust_checks by the owner. Candidate syntax: function items only, safe std/core pure modules, no imports/attributes/macros/unsafe/extern/async/I/O/process/environment. No shell, Cargo, dependencies, downloads, network or source promotion. Separate candidate library and owner-frozen acceptance executable; bounded processes/output/time and confined scratch files. Returns passed, source/test hashes and real compiler/test diagnostics. Failed acceptance may include a bounded excerpt of owner-frozen test input only with current files:read permission; the excerpt is untrusted data. Completion requires both a PASS and a full source readback for current bytes and policy epoch; a completed tool receipt alone is insufficient. Currently requires installed macOS sandbox-exec and Rust; unsupported isolation fails closed."
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        let nullable_integer = json!({"oneOf":[{"type":"integer"},{"type":"null"}]});
        Some(
            json!({"type":"object","additionalProperties":false,"required":["passed","stage","diagnostics","source_path","source_sha256","tests_sha256","provider_cost_usd","promotion_authorized"],"properties":{
                "passed":{"type":"boolean"},"stage":{"type":"string","enum":["admission","compile_candidate","compile_acceptance","acceptance"]},
                "diagnostics":{"oneOf":[{"type":"string"},{"type":"object","additionalProperties":false,"required":["passed","exit_code","signal","limit","stdout","stderr"],"properties":{"passed":{"type":"boolean"},"exit_code":nullable_integer,"signal":nullable_integer,"limit":{"oneOf":[{"type":"string"},{"type":"null"}]},"stdout":{"type":"string"},"stderr":{"type":"string"}}}]},
                "source_path":{"type":"string"},"source_sha256":{"type":"string","minLength":64,"maxLength":64},"tests_sha256":{"type":"string","minLength":64,"maxLength":64},
                "provider_cost_usd":{"const":0},"promotion_authorized":{"const":false},"compiler":{"type":"string"},"tests_count":{"type":"integer","minimum":1,"maximum":16},"isolation":{"type":"string"},"limits":{"type":"object"},
                "failure_context":{"type":"object","additionalProperties":false,"required":["tests_path","line","reported_column","excerpt_start_char","excerpt","scope"],"properties":{"tests_path":{"type":"string"},"line":{"type":"integer","minimum":1},"reported_column":{"type":"integer","minimum":1},"excerpt_start_char":{"type":"integer","minimum":0},"excerpt":{"type":"string","maxLength":300},"scope":{"type":"string"}}}
            }}),
        )
    }
    fn read_only(&self) -> bool {
        false
    }
    fn runtime_requirements(&self) -> crate::RuntimeRequirements {
        crate::RuntimeRequirements::isolated_rust()
    }
    fn external_effects(&self) -> bool {
        true
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Worker]
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        crate::rust_verification::execute(
            context,
            args["source_path"]
                .as_str()
                .context("source_path required")?,
        )
    }
}

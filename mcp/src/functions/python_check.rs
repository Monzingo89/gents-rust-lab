use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Phase, Tool, ToolContext,
};
use anyhow::Result;
use serde_json::{json, Value};

pub struct PythonCheck;
impl Tool for PythonCheck {
    fn name(&self) -> &'static str {
        "python_check"
    }
    fn source(&self) -> &'static str {
        file!()
    }
    fn description(&self) -> &'static str {
        "Owner-scoped local Python unittest DIAGNOSTICS for explicit copied .py inputs. Provide contract_id only; the host binds expected_source_identity. The owner must freeze python_check_contracts with actor, source_paths, tests_path/tests_sha256 and interpreter_sha256. Requires current python_check and files:read grants and installed macOS isolation. Optional frozen_inputs pins immutable source dependencies. scratch_bytes defaults to zero; owner-frozen values through 8388608 allow private scratch data/create/rename/delete I/O with sampled byte/entry limits and no mode changes. No shell, network, run-workspace writes, subprocesses, downloads, result cache, promotion or acceptance. Python can spoof its test exit; reported_pass is not independent correctness or company completion. Source changes permit a new reviewed diagnostic; unchanged state remains retry-limited."
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        let hash = json!({"type":"string","minLength":64,"maxLength":64});
        Some(json!({"type":"object","additionalProperties":false,
        "required":["status","source_identity","inputs_sha256","suite_sha256",
            "snapshot_unchanged","interpreter_sha256","python_version","exit_code",
            "timed_out","resident_memory_limit","output_truncated","stdout","stderr",
                "elapsed_ms","model_calls","provider_cost_usd","qualified","promoted",
            "authority_granted","scope","verification_identity","contract_id",
            "scratch_limit_bytes","scratch_observed_bytes","scratch_observed_entries","scratch_limit_exceeded"],
        "properties":{
            "status":{"type":"string","enum":["reported_pass","reported_fail","indeterminate"]},
            "source_identity":hash,"verification_identity":hash,"suite_sha256":hash,
            "interpreter_sha256":hash,"inputs_sha256":{"type":"object","maxProperties":32},
            "snapshot_unchanged":{"type":"boolean"},"python_version":{"type":"string"},
            "exit_code":{"type":"integer"},"timed_out":{"type":"boolean"},
            "resident_memory_limit":{"type":"boolean"},"output_truncated":{"type":"boolean"},
            "scratch_limit_bytes":{"type":"integer","minimum":0,"maximum":8388608},
            "scratch_observed_bytes":{"type":"integer","minimum":0},
            "scratch_observed_entries":{"type":"integer","minimum":0},
            "scratch_limit_exceeded":{"type":"boolean"},
            "stdout":{"type":"string","maxLength":16384},"stderr":{"type":"string","maxLength":16384},
                "elapsed_ms":{"type":"integer","minimum":0},
            "model_calls":{"const":0},"provider_cost_usd":{"const":0},
            "qualified":{"const":false},"promoted":{"const":false},"authority_granted":{"const":false},
            "scope":{"type":"string"},"contract_id":{"type":"string","minLength":1,"maxLength":80}
        }}))
    }
    fn external_effects(&self) -> bool {
        true
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Worker]
    }
    fn runtime_requirements(&self) -> crate::RuntimeRequirements {
        crate::RuntimeRequirements {
            installed_python: true,
            call_timeout_seconds: 12,
            ..Default::default()
        }
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        crate::python_check::execute(context, &args)
    }
}

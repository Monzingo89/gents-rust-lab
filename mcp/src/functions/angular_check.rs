use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Phase, Tool, ToolContext,
};
use anyhow::Result;
use serde_json::Value;
pub struct AngularCheck;
impl Tool for AngularCheck {
    fn name(&self) -> &'static str {
        "angular_check"
    }
    fn source(&self) -> &'static str {
        file!()
    }
    fn description(&self) -> &'static str {
        "Build an explicit copied Angular application and perform finite owner-frozen DOM cases using pinned installed Node/Angular/Playwright/headless SDKs. Requires angular_check and files:read grants and an actor-bound angular_check_contracts entry. Host binds expected_source_identity. No package installation, arbitrary command/endpoint/browser flags/evaluate expression, workspace writes, qualification or promotion. Compiler has no network; browser has a required outer deny-default Seatbelt profile and host-held loopback asset port. A compile or candidate self-report cannot substitute for the frozen browser checks."
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        let mut schema = serde_json::to_value(schemars::schema_for!(
            crate::angular_check::runtime::Evaluation
        ))
        .unwrap();
        schema["properties"]["status"]["enum"] =
            serde_json::json!(["browser_passed", "check_failed", "indeterminate"]);
        for field in ["qualified", "promoted", "authority_granted"] {
            schema["properties"][field]["const"] = serde_json::json!(false);
        }
        for field in ["model_calls", "provider_cost_usd"] {
            schema["properties"][field]["const"] = serde_json::json!(0);
        }
        Some(schema)
    }
    fn phases(&self) -> &'static [Phase] {
        &[Phase::Worker]
    }
    fn read_only(&self) -> bool {
        false
    }
    fn external_effects(&self) -> bool {
        true
    }
    fn runtime_requirements(&self) -> crate::RuntimeRequirements {
        crate::RuntimeRequirements {
            installed_angular: true,
            call_timeout_seconds: 120,
            ..Default::default()
        }
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        let prepared = crate::angular_check::prepare_call(context, &args)?;
        let value = crate::angular_check::compute(&prepared)?;
        crate::angular_check::finish(context, &args, &prepared, value)
    }
}

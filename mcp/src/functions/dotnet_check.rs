use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    Phase, Tool, ToolContext,
};
use anyhow::Result;
use serde_json::Value;

pub struct DotnetCheck(pub bool);
impl Tool for DotnetCheck {
    fn name(&self) -> &'static str {
        if self.0 {
            "dotnet_api_check"
        } else {
            "dotnet_check"
        }
    }
    fn source(&self) -> &'static str {
        file!()
    }
    fn description(&self) -> &'static str {
        if self.0 {
            "Owner-frozen API contract check: compile explicit Web SDK sources, then run the actual application with an inherited host-held loopback socket. Requires separate dotnet_api_check, dotnet_check and files:read grants plus frozen API cases. Candidate must use Kestrel ListenHandle with GENTS_API_LISTEN_FD. Runtime denies new binds, outbound connections, file writes and subprocesses. Host sends frozen status/JSON requests; application exit or self-reported tests cannot pass. No arbitrary endpoint, network data source, shell, package download, qualification or promotion."
        } else {
            "Offline managed-DLL BUILD check of an explicit copied .NET source snapshot. Requires owner-frozen dotnet_check_contracts for this actor, installed hash-frozen GENTS_DOTNET/SDK, current dotnet_check and files:read grants, and macOS isolation. Provide contract_id; host binds expected_source_identity. Package-free standard SDK projects only: no custom MSBuild targets, imports, analyzers, project/package references, shell, network or downloads. Compile facts are source-bound, never functional/API acceptance, expert qualification or promotion. No candidate application execution or run-workspace writes."
        }
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        let mut schema =
            serde_json::to_value(schemars::schema_for!(crate::dotnet_check::BuildResult)).unwrap();
        schema["properties"]["status"]["enum"] = if self.0 {
            serde_json::json!(["api_passed", "api_failed", "indeterminate"])
        } else {
            serde_json::json!(["build_passed", "build_failed", "indeterminate"])
        };
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
            installed_dotnet: true,
            call_timeout_seconds: if self.0 { 115 } else { 100 },
            ..Default::default()
        }
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        crate::dotnet_check::execute_mode(
            context,
            &args,
            if self.0 {
                crate::dotnet_check::Mode::Api
            } else {
                crate::dotnet_check::Mode::Build
            },
        )
    }
}

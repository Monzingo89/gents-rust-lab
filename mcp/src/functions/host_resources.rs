use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    host_memory::{NativeMemoryProbe, Snapshot},
    Tool, ToolContext,
};
use anyhow::Result;
use serde_json::{json, Value};
pub struct HostResources;
impl Tool for HostResources {
    fn name(&self) -> &'static str {
        "host_resources"
    }
    fn source(&self) -> &'static str {
        file!()
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn description(&self) -> &'static str {
        "Read offline host RAM, available parallelism and current-process Linux cgroup memory bounds through an owned adapter over pinned MIT sysinfo. Unknown process limits fail capacity admission on Linux; other platforms disclose unobserved process limits. Reports estimates, not guaranteed model fit. GPU memory is unknown; do not add host RAM to GPU RAM. No process list, command lines, environment, hostname, credentials, network, downloads, model selection, execution or promotion. Requires the actor's current host_resources grant."
    }
    fn read_only(&self) -> bool {
        true
    }
    fn result_schema(&self, _: Option<&Value>) -> Option<Value> {
        let number = json!({"type":"integer","minimum":0});
        let optional = json!({"oneOf":[number,{"type":"null"}]});
        Some(
            json!({"type":"object","additionalProperties":false,"required":["observed_at_ms","os","architecture","available_parallelism","host_total_bytes","host_available_bytes","process_limit_status","process_limit_bytes","process_available_bytes","effective_limit_bytes","effective_available_bytes","gpu_memory_bytes","probe","provider_cost_usd"],"properties":{
                "observed_at_ms":number,"os":{"type":"string"},"architecture":{"type":"string"},"available_parallelism":optional,
                "host_total_bytes":number,"host_available_bytes":number,"process_limit_status":{"type":"string","enum":["observed","unavailable","not_observed_on_this_platform"]},
                "process_limit_bytes":optional,"process_available_bytes":optional,"effective_limit_bytes":optional,"effective_available_bytes":optional,"gpu_memory_bytes":{"type":"null"},"probe":{"type":"string"},"provider_cost_usd":{"const":0}
            }}),
        )
    }
    fn execute(&self, _: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        let mut value = serde_json::to_value(Snapshot::capture(&NativeMemoryProbe)?)?;
        value["provider_cost_usd"] = json!(0);
        Ok(value)
    }
}

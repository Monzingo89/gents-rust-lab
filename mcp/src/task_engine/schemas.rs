//! Generated directly from the versioned Serde Rust contracts. Host policy is
//! exported for inspection, never accepted as an inference/request argument.
use serde_json::{json, Value};
pub fn input() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::contracts::TaskInput)).unwrap())
}
pub fn request() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::contracts::TaskRequest)).unwrap())
}
pub fn hints() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::routing::Hints)).unwrap())
}
pub fn source() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::KnowledgeSource)).unwrap())
}
pub fn result() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::contracts::TaskResult)).unwrap())
}
pub fn intake() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::routing::Intake)).unwrap())
}
pub fn search_hit() -> Value {
    inline(serde_json::to_value(schemars::schema_for!(super::SearchHit)).unwrap())
}
fn inline(schema: Value) -> Value {
    fn expand(value: &Value, root: &Value) -> Value {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            return expand(
                root.pointer(reference.strip_prefix('#').expect("local schema reference"))
                    .expect("valid schema reference"),
                root,
            );
        }
        match value {
            Value::Array(a) => Value::Array(a.iter().map(|v| expand(v, root)).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .filter(|(k, _)| k.as_str() != "$defs" && k.as_str() != "$schema")
                    .map(|(k, v)| (k.clone(), expand(v, root)))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    expand(&schema, &schema)
}
pub fn bundle() -> Value {
    json!({"ProjectMetadata":schemars::schema_for!(super::project_metadata::Metadata),"TaskInput":input(),"Hints":hints(),"KnowledgeSource":source(),
    "TaskRequest":schemars::schema_for!(super::contracts::TaskRequest),"TaskAnalysis":schemars::schema_for!(super::contracts::TaskAnalysis),
    "ExecutionPlan":schemars::schema_for!(super::contracts::ExecutionPlan),"ExecutionPolicy":schemars::schema_for!(super::contracts::ExecutionPolicy),
    "IntakeProposal":schemars::schema_for!(super::intake::Proposal),"TaskResult":schemars::schema_for!(super::contracts::TaskResult),"StageReply":super::workflows::reply_schema()})
}

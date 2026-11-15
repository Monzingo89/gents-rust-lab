//! Embedded, reviewed procedures only. A skill is not executable authority.
use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    digest, skills, Phase, Tool, ToolContext,
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
fn phase(context: &ToolContext<'_>) -> &'static str {
    match context.scope.phase {
        Phase::Planner => "planner",
        Phase::Worker => "worker",
    }
}
fn assigned(context: &ToolContext<'_>) -> Result<Vec<Value>> {
    let grant = context
        .policy
        .grants
        .get(&context.scope.actor)
        .context("actor has no grant")?;
    skills::for_assignment(phase(context), &grant.skills)
}
pub struct List;
impl Tool for List {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "skill_list"
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn description(&self) -> &'static str {
        "List reviewed skills assigned to this runtime actor and planner/worker phase; no execution or new authority."
    }
    fn read_only(&self) -> bool {
        true
    }
    fn execute(&self, context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &arguments)?;
        let entries = assigned(context)?
            .into_iter()
            .map(|mut skill| {
                let body = skill
                    .as_object_mut()
                    .unwrap()
                    .remove("instructions")
                    .unwrap();
                skill["sha256"] = json!(digest(body.as_str().unwrap().as_bytes()));
                skill
            })
            .collect::<Vec<_>>();
        Ok(json!({"skills":entries,"phase":phase(context)}))
    }
}
pub struct Read;
impl Tool for Read {
    fn source(&self) -> &'static str {
        file!()
    }
    fn scoped_schema(
        &self,
        scope: &crate::Scope,
        policy: &crate::governance::Policy,
    ) -> Result<Option<Value>> {
        let Some(grant) = policy.grants.get(&scope.actor) else {
            return Ok(None);
        };
        let phase = match scope.phase {
            Phase::Planner => "planner",
            Phase::Worker => "worker",
        };
        let ids = skills::for_assignment(phase, &grant.skills)?
            .into_iter()
            .map(|s| s["id"].clone())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(None);
        }
        let mut schema = self.schema();
        schema["properties"]["id"]["enum"] = json!(ids);
        Ok(Some(schema))
    }
    fn name(&self) -> &'static str {
        "skill_read"
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn description(&self) -> &'static str {
        "Read an assigned reviewed skill valid for this process-bound phase; instructions never expand tool, data or budget permissions."
    }
    fn read_only(&self) -> bool {
        true
    }
    fn execute(&self, context: &ToolContext<'_>, arguments: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &arguments)?;
        let mut skill = assigned(context)?
            .into_iter()
            .find(|s| s["id"] == arguments["id"])
            .context("skill is not assigned to this phase")?;
        skill["sha256"] = json!(digest(
            skill["instructions"]
                .as_str()
                .context("embedded instructions missing")?
                .as_bytes()
        ));
        Ok(skill)
    }
}

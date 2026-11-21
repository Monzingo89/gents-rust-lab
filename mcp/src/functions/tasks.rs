//! Actor/run-private task adapter. The normal native grant, epoch and job
//! admission wrap every operation; task text can never expand this scope.
use crate::{
    task_engine::{contracts::*, native, routing, KnowledgeSource, Repository},
    Tool, ToolContext,
};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct TaskTool(pub &'static str);
impl TaskTool {
    fn input_schema(&self) -> Value {
        match self.0 {
            "destructure_task" => {
                json!({"type":"object","additionalProperties":false,"required":["input"],"properties":{"input":crate::task_engine::schemas::input(),"hints":crate::task_engine::schemas::hints()}})
            }
            "execute_task" => json!({"type":"object","oneOf":[
                {"type":"object","additionalProperties":false,"required":["input"],"properties":{"input":crate::task_engine::schemas::input(),"hints":crate::task_engine::schemas::hints()}},
                {"type":"object","additionalProperties":false,"required":["request"],"properties":{"request":crate::task_engine::schemas::request(),"hints":crate::task_engine::schemas::hints()}}
            ]}),
            "ingest_sources" => {
                json!({"type":"object","additionalProperties":false,"required":["source"],"properties":{"source":crate::task_engine::schemas::source()}})
            }
            "search_knowledge" => {
                json!({"type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string","minLength":1,"maxLength":12000},"domain":{"type":"string","maxLength":80}}})
            }
            "get_task_status" | "cancel_task" => {
                json!({"type":"object","additionalProperties":false,"required":["task_id"],"properties":{"task_id":{"type":"string","format":"uuid"},"view":{"enum":["full","summary"]}}})
            }
            _ => json!({"type":"object","additionalProperties":false,"properties":{}}),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    input: TaskInput,
    #[serde(default)]
    hints: routing::Hints,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Structured {
    request: TaskRequest,
    #[serde(default)]
    hints: routing::Hints,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum StartInput {
    Text(Input),
    Structured(Structured),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    source: KnowledgeSource,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    query: String,
    domain: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    task_id: uuid::Uuid,
    #[serde(default)]
    view: StatusView,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusView {
    #[default]
    Full,
    Summary,
}
fn status_view(mut result: TaskResult, view: &StatusView) -> TaskResult {
    if matches!(view, StatusView::Summary) {
        result.evidence.clear();
        result.plan = None;
        result.analysis = None;
    }
    result
}
impl Tool for TaskTool {
    fn name(&self) -> &'static str {
        self.0
    }
    fn source(&self) -> &'static str {
        file!()
    }
    fn schema(&self) -> Value {
        self.input_schema()
    }
    fn description(&self) -> &'static str {
        "Versioned actor/run-private cache-first task service. Current native grants apply. A configured existing harness executes queued inference asynchronously with status/cancel; unavailable workers return explicit planning needs. Proposals cannot execute side effects or grant budgets."
    }
    fn read_only(&self) -> bool {
        !matches!(self.0, "ingest_sources" | "invalidate_cache")
    }
    fn result_schema(&self, _args: Option<&Value>) -> Option<Value> {
        let execute = json!({"type":"object","additionalProperties":false,"required":["intake","result"],"properties":{"intake":crate::task_engine::schemas::intake(),"result":crate::task_engine::schemas::result()}});
        Some(match self.0 {
            "execute_task" => execute,
            "destructure_task" => json!({"oneOf":[crate::task_engine::schemas::intake(),execute]}),
            "get_task_status" | "cancel_task" => {
                json!({"oneOf":[{"type":"null"},crate::task_engine::schemas::result()]})
            }
            "ingest_sources" => {
                json!({"type":"object","additionalProperties":false,"required":["source_id","snapshot"],"properties":{"source_id":{"type":"string"},"snapshot":{"type":"string"}}})
            }
            "search_knowledge" => {
                json!({"type":"object","additionalProperties":false,"required":["hits","generative_model_calls"],"properties":{"hits":{"type":"array","items":crate::task_engine::schemas::search_hit()},"generative_model_calls":{"const":0}}})
            }
            "invalidate_cache" => {
                json!({"type":"object","additionalProperties":false,"required":["deleted"],"properties":{"deleted":{"type":"integer","minimum":0}}})
            }
            _ => json!({"type":"object"}),
        })
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        let value = (|| -> Result<Value> {
            // Schemars schemas describe wire shape; strict Serde and native semantic
            // validation enforce it. Never accept policy/scope in arguments.
            if matches!(
                self.0,
                "list_capabilities" | "discover_models" | "inspect_cache" | "invalidate_cache"
            ) && args != json!({})
            {
                bail!("no arguments accepted");
            }
            if self.0 == "list_capabilities" {
                let mut value = routing::capabilities();
                value["native_async"] = json!({"enabled":native::lease(context.db)?.is_some_and(|l|l.expires_at_ms>crate::now_ms()),"cancel_tool":"cancel_task requires its own current grant","workspace":"native actor/run corpus; existing native files tools retain separate grants"});
                return Ok(value);
            }
            if self.0 == "discover_models" {
                return context.authority.discover_models();
            }
            let repo = Repository::new(context.db)?;
            let runtime =
                native::lease(context.db)?.map_or_else(|| COMPATIBILITY.into(), |l| l.runtime);
            let scope = native::access(&repo, context.scope, context.policy, &runtime)?;
            let now = crate::now_ms();
            let paths = repo.dependency_paths(&scope)?;
            if !paths.is_empty() {
                for path in paths {
                    context.policy.authorize(
                        &context.scope.actor,
                        "files",
                        &json!({"op":"read","path":path}),
                    )?;
                }
                repo.refresh_dependencies(&scope, &context.authority.workspace(context.scope)?)?;
            }
            match self.0 {
                "destructure_task" => {
                    let input: Input = serde_json::from_value(args)?;
                    let intake = routing::destructure_task(
                        TaskRequest::from_input(input.input)?,
                        input.hints.clone(),
                    )?;
                    if native::lease(context.db)?
                        .is_some_and(|l| l.model_intake && l.expires_at_ms > now)
                    {
                        let base = repo.execute(
                            &intake.request,
                            &ExecutionPolicy::read_only(scope, now),
                            now,
                        )?;
                        let result = native::enqueue(
                            context,
                            intake.request.clone(),
                            input.hints,
                            true,
                            false,
                            base,
                        )?;
                        return Ok(json!({"intake":intake,"result":result}));
                    }
                    Ok(serde_json::to_value(intake)?)
                }
                "execute_task" => {
                    let (request, hints, skip_model_intake) =
                        match serde_json::from_value::<StartInput>(args)? {
                            StartInput::Text(input) => {
                                (TaskRequest::from_input(input.input)?, input.hints, false)
                            }
                            StartInput::Structured(input) => {
                                input.request.validate()?;
                                (input.request, input.hints, true)
                            }
                        };
                    let intake = routing::destructure_task(request, hints.clone())?;
                    let mut result = repo.execute(
                        &intake.request,
                        &ExecutionPolicy::read_only(scope.clone(), now),
                        now,
                    )?;
                    // A verified fact/answer cache is not proof of an owner-
                    // required current SDK build. Keep the async native boundary.
                    if matches!(
                        intake.analysis.domain.as_deref(),
                        Some("dotnet" | "angular")
                    ) {
                        let encoded: String = context.db.query_row(
                            "SELECT intake FROM jobs WHERE run_id=?1",
                            [context.scope.run.to_string()],
                            |r| r.get(0),
                        )?;
                        let parent: Value = serde_json::from_str(&encoded)?;
                        let required = if intake.analysis.domain.as_deref() == Some("angular") {
                            crate::angular_check::has_contract(&parent, &context.scope.actor)?
                                && context
                                    .policy
                                    .authorize(&context.scope.actor, "angular_check", &json!({}))
                                    .is_ok()
                        } else {
                            crate::dotnet_check::has_contract(&parent, &context.scope.actor)?
                                && context
                                    .policy
                                    .authorize(&context.scope.actor, "dotnet_check", &json!({}))
                                    .is_ok()
                        };
                        if required {
                            result.status = "requires_planning".into();
                            result.answer = None;
                            result.cache.decision =
                                "runtime_validation_required_answer_cache_ineligible".into();
                        }
                    }
                    if result.status == "requires_planning"
                        && intake.request.input.intent == Intent::Answer
                    {
                        result = native::enqueue(
                            context,
                            intake.request.clone(),
                            hints,
                            false,
                            skip_model_intake,
                            result,
                        )?;
                        repo.replace_result(&scope, &result)?;
                    }
                    Ok(json!({"intake":intake,"result":result}))
                }
                "ingest_sources" => {
                    let source: Source = serde_json::from_value(args)?;
                    Ok(
                        json!({"source_id":source.source.source_id,"snapshot":repo.ingest(&scope,&source.source,now)?}),
                    )
                }
                "search_knowledge" => {
                    let search: Search = serde_json::from_value(args)?;
                    Ok(
                        json!({"hits":repo.search(&scope,&search.query,search.domain.as_deref(),8192,now)?,"generative_model_calls":0}),
                    )
                }
                "get_task_status" | "cancel_task" => {
                    let status: Status = serde_json::from_value(args)?;
                    if let Some(result) =
                        native::observe(context, status.task_id, self.0 == "cancel_task")?
                    {
                        return Ok(serde_json::to_value(status_view(result, &status.view))?);
                    }
                    Ok(serde_json::to_value(
                        repo.status(&scope, status.task_id, now)?
                            .map(|r| status_view(r, &status.view)),
                    )?)
                }
                "inspect_cache" => repo.inspect_cache(&scope),
                "invalidate_cache" => Ok(json!({"deleted":repo.invalidate_cache(&scope)?})),
                _ => bail!("unavailable task operation"),
            }
        })()?;
        // Guard before the Registry transaction commits a queue/cancellation.
        // Include both protocol representations and a conservative receipt/ID.
        let receipt = json!({"actor":context.scope.actor,"phase":context.scope.phase,"tool":self.0,"arguments_sha256":"0".repeat(64),"result_sha256":"0".repeat(64),"policy_revision":context.policy.revision,"provider_cost_usd":0,"status":"completed","contract_enforced":true});
        let wrapped = json!({"value":value,"receipt":receipt});
        let frame = json!({"jsonrpc":"2.0","id":"\u{0000}".repeat(200),"result":{"content":[{"type":"text","text":wrapped.to_string()}],"structuredContent":wrapped,"isError":false}});
        anyhow::ensure!(serde_json::to_vec(&frame)?.len()<=64000,"Native task response exceeds bounded frame; admission rolled back. For status use view=summary; larger task intake requires a compatible transport. Original input was not truncated.");
        Ok(value)
    }
}

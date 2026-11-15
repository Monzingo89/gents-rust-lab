//! Two local model opinions over a ready CEO plan, behind independent hardware,
//! inference and durable journal ports. No tool/permission/job mutations.
pub mod contracts;
pub mod hardware;
mod journal;
#[cfg(test)]
mod tests;
use crate::{company_factory::ArchitectureRepository, provider::Reply};
use anyhow::{ensure, Context, Result};
use async_trait::async_trait;
use contracts::{Choice, Request, Review, Verdict};
use hardware::{HardwarePort, NativeHardware};
use serde_json::{json, Value};
use std::{path::Path, sync::atomic::AtomicBool};
use uuid::Uuid;

#[async_trait]
pub trait ModelPort: Sync {
    async fn complete(&self, judge: bool, packet: &Value) -> Result<Reply>;
}
pub trait JournalPort: Send {
    fn save(&mut self, state: &str, body: &Value) -> Result<()>;
    fn current_revision(&self) -> Result<u64>;
}

pub async fn run(
    models: &dyn ModelPort,
    hardware: &dyn HardwarePort,
    journal: &mut dyn JournalPort,
    mut body: Value,
) -> Result<Value> {
    let revision = body["request"]["expected_revision"]
        .as_u64()
        .context("review revision missing")?;
    let available = body["context"]["available_actors"]
        .as_array()
        .context("actor roster missing")?
        .len();
    for judge in [false, true] {
        if journal.current_revision()? != revision {
            body["status"] = json!("stale");
            journal.save("stale", &body)?;
            return Ok(body);
        }
        let observation = hardware.capture().await?;
        let role = if judge { "judge" } else { "ceo" };
        body["hardware_observations"][role] = observation.clone();
        if judge && !classify_output(models, journal, &mut body, revision).await? {
            return Ok(body);
        }
        let mut packet = json!({"request":body["request"],"context":body["context"],"hardware":observation,"provider_tool_budget_usd":0,"task_accepted":false,"tools_available":[],"output_intent":body["output_intent"],"output_routing":"The output judge classifies intent independently; this is not execution or tool authorization."});
        if judge {
            packet["ceo_recommendation"] = body["recommendation"].clone();
        }
        ensure!(
            packet.to_string().len() <= 40_000,
            "architecture context exceeds budget; nothing silently omitted"
        );
        let state = if judge {
            "pending_judge"
        } else {
            "pending_ceo"
        };
        body["status"] = json!(state);
        body["dispatch"][role] = json!({"packet_sha256":crate::company_factory::digest(&packet)?,"started_at_ms":crate::now_ms()});
        journal.save(state, &body)?;
        let reply = match models.complete(judge, &packet).await {
            Ok(reply) => reply,
            Err(_) => {
                body["status"] = json!("failed");
                body["error"]=json!("Local architecture inference/preflight failed. No paid fallback; unobserved usage remains unknown. Inspect the pending stage before a new assessment.");
                journal.save("failed", &body)?;
                return Ok(body);
            }
        };
        let parsed = (|| -> Result<Value> {
            crate::coordination::validate_receipt(&reply)?;
            ensure!(
                reply
                    .local_execution
                    .as_ref()
                    .and_then(|r| r.task_input_sha256.as_ref())
                    == Some(&crate::company_factory::digest(&packet)?),
                "architecture reply input hash mismatch"
            );
            let value: Value = serde_json::from_str(&reply.text)?;
            if judge {
                let choice: Choice = serde_json::from_value(body["recommendation"].clone())?;
                let review: Review = serde_json::from_value(value.clone())?;
                review.validate(&choice, available)?;
            } else {
                serde_json::from_value::<Choice>(value.clone())?.validate(available)?;
            }
            Ok(value)
        })();
        let value = match parsed {
            Ok(value) => value,
            Err(_) => {
                let safe = Reply {
                    text: "[Rejected architecture output withheld]".into(),
                    ..reply.clone()
                };
                body["receipts"][role] = json!({"reply":safe,"rejected_output_sha256":gents_local_mcp::digest(reply.text.as_bytes()),"accepted_protocol":false});
                body["status"] = json!("failed");
                body["error"]=json!("Architecture reply rejected by protocol/data/receipt validation; usage retained, no automatic inference replay.");
                journal.save("failed", &body)?;
                return Ok(body);
            }
        };
        body["receipts"][role] = json!({"reply":reply,"accepted_protocol":true});
        body[if judge { "review" } else { "recommendation" }] = value;
        body["status"] = json!(if judge {
            "review_received"
        } else {
            "ceo_received"
        });
        journal.save(body["status"].as_str().unwrap(), &body)?;
    }
    let choice: Choice = serde_json::from_value(body["recommendation"].clone())?;
    let review: Review = serde_json::from_value(body["review"].clone())?;
    let staffed = body["context"]["proposal"]["assignments"]
        .as_array()
        .context("staffing missing")?
        .len();
    let supported = body["context"]["architecture_menu"]
        .as_array()
        .context("architecture menu missing")?
        .iter()
        .any(|a| a["coordination"] == json!(choice.architecture) && a["executor"].is_string());
    let intent: crate::output_routing::Intent =
        serde_json::from_value(body["output_intent"].clone())?;
    let required_media_tools = intent
        .modalities
        .iter()
        .filter_map(|m| m.required_tool())
        .collect::<Vec<_>>();
    let missing_media = required_media_tools
        .iter()
        .filter(|tool| {
            !body["context"]["media_capabilities"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| item["tool"] == **tool && item["configured"] == true)
        })
        .copied()
        .collect::<Vec<_>>();
    let unplanned_media = required_media_tools
        .iter()
        .filter(|tool| {
            !body["context"]["proposal"]["assignments"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|stage| {
                    stage["tools"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|granted| granted == **tool)
                })
        })
        .copied()
        .collect::<Vec<_>>();
    body["media_routing"] = json!({"modalities":intent.modalities,"required_tools":required_media_tools,"missing_tools":missing_media,"unplanned_tools":unplanned_media,"selected_by":"independent_local_output_judge","execution_authorized":false,"scope":"Requested output kinds only; prompt text cannot grant tools, download models, execute or certify results."});
    let state = if journal.current_revision()? != revision {
        "stale"
    } else if review.verdict != Verdict::Accept {
        "review_required"
    } else if !supported || !missing_media.is_empty() {
        "missing_capability"
    } else if !unplanned_media.is_empty() {
        "media_plan_review_required"
    } else if choice.agent_count != staffed {
        "staffing_review_required"
    } else {
        "ready_for_owner_review"
    };
    body["status"] = json!(state);
    body["recommended_agent_count"] = json!(choice.agent_count);
    body["staffed_agent_count"] = json!(staffed);
    body["speed_assessment"] = json!({"requested":body["request"]["speed"],"concurrent_inference_slots":1,"speedup_proven":false,"task_seconds_estimate":null,"reason":"Current runtime serializes inference. Repeated hardware/fit checks cannot invent throughput or guarantee completion."});
    body["long_run_configuration_required"] = json!(
        body["request"]["maximum_elapsed_seconds"]
            .as_u64()
            .context("requested duration missing")?
            > body["context"]["configured_max_run_seconds"]
                .as_u64()
                .context("configured duration missing")?
    );
    journal.save(state, &body)?;
    Ok(body)
}

async fn classify_output(
    models: &dyn ModelPort,
    journal: &mut dyn JournalPort,
    body: &mut Value,
    revision: u64,
) -> Result<bool> {
    // No CEO opinion or engine inventory: requested outputs are independent of feasibility.
    let packet = json!({"output_intent_only":true,"user_brief":body["context"]["user_brief"],"provider_tool_budget_usd":0,"execution_authorized":false});
    body["status"] = json!("pending_output_judge");
    body["dispatch"]["output_judge"] = json!({"packet_sha256":crate::company_factory::digest(&packet)?,"started_at_ms":crate::now_ms()});
    journal.save("pending_output_judge", body)?;
    let reply = match models.complete(true, &packet).await {
        Ok(reply) => reply,
        Err(_) => {
            body["status"] = json!("failed");
            body["error"] = json!(
                "Local output classification failed; no fallback, execution or automatic replay."
            );
            journal.save("failed", body)?;
            return Ok(false);
        }
    };
    let parsed = (|| -> Result<crate::output_routing::Intent> {
        crate::coordination::validate_receipt(&reply)?;
        ensure!(
            reply
                .local_execution
                .as_ref()
                .and_then(|r| r.task_input_sha256.as_ref())
                == Some(&crate::company_factory::digest(&packet)?),
            "output routing input hash mismatch"
        );
        let intent: crate::output_routing::Intent = serde_json::from_str(&reply.text)?;
        intent.validate()?;
        Ok(intent)
    })();
    let intent = match parsed {
        Ok(intent) => intent,
        Err(_) => {
            let safe = Reply {
                text: "[Rejected routing output withheld]".into(),
                ..reply.clone()
            };
            body["receipts"]["output_judge"] = json!({"reply":safe,"rejected_output_sha256":gents_local_mcp::digest(reply.text.as_bytes()),"accepted_protocol":false});
            body["status"] = json!("failed");
            body["error"] =
                json!("Local output classification rejected; usage retained, no automatic replay.");
            journal.save("failed", body)?;
            return Ok(false);
        }
    };
    body["receipts"]["output_judge"] = json!({"reply":reply,"accepted_protocol":true});
    body["output_intent"] = json!(intent);
    if journal.current_revision()? != revision {
        body["status"] = json!("stale");
        journal.save("stale", body)?;
        return Ok(false);
    }
    Ok(true)
}

struct NativeModels<'a> {
    root: &'a Path,
    pool: &'a crate::runtime_pool::RuntimePool,
    candidates: [crate::planning_models::Candidate; 2],
    scope: String,
}
#[async_trait]
impl ModelPort for NativeModels<'_> {
    async fn complete(&self, judge: bool, packet: &Value) -> Result<Reply> {
        let candidate = self.candidates[usize::from(judge)].clone();
        let root = self.root.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let memory = gents_local_mcp::host_memory::Snapshot::capture(
                &gents_local_mcp::host_memory::NativeMemoryProbe,
            )?;
            ensure!(
                memory.effective_total()? >= candidate.minimum_memory_mib * 1024 * 1024,
                "current host/process capacity below reviewed model floor"
            );
            crate::model_artifact::verify(
                &root,
                candidate
                    .model
                    .path
                    .as_deref()
                    .context("local path missing")?,
                &candidate.sha256,
            )
            .map_err(anyhow::Error::msg)?;
            Ok(())
        })
        .await??;
        let routing = packet["output_intent_only"] == true;
        let scope = crate::context::session_key(
            &self.scope,
            "architecture",
            if routing {
                "output_judge"
            } else if judge {
                "judge"
            } else {
                "ceo"
            },
            0,
        );
        self.pool
            .complete_scoped(
                &self.candidates[usize::from(judge)].model,
                if routing {
                    crate::output_routing::instruction()
                } else {
                    contracts::instruction(judge)
                },
                &packet.to_string(),
                if routing { 96 } else { 1024 },
                &AtomicBool::new(false),
                &scope,
            )
            .await
    }
}

pub fn read(root: &Path, company: &str, owner: &str, assessment: Uuid) -> Result<Value> {
    journal::read(root, company, owner, assessment)
}

pub async fn assess(
    app: &crate::App,
    root: &Path,
    company: &str,
    owner: &str,
    planning: Uuid,
    assessment: Uuid,
    input: Value,
) -> Result<Value> {
    let request: Request = serde_json::from_value(input)?;
    request.validate()?;
    let _lease = crate::workflow::ControllerLease::acquire(root, company, owner, assessment)?;
    let (mut journal, existing) =
        journal::SqliteJournal::open(root, company, owner, assessment, &request)?;
    if let Some(existing) = existing {
        ensure!(
            existing["planning_id"] == planning.to_string(),
            "assessment belongs to another planning session"
        );
        return Ok(existing);
    }
    let session = crate::ceo_planning::read(root, company, owner, planning)?;
    ensure!(
        session["status"] == "ready" && session["revision"] == request.expected_revision,
        "architecture review requires the current ready CEO plan"
    );
    let stages = crate::ceo_team::parse(&session["proposal"])?;
    let policy = app
        .config
        .planning_model_policy
        .as_ref()
        .context("owner-listed pinned local CEO and judge models required")?;
    policy.validate()?;
    let ceo = policy.candidates[0].clone();
    let judge = policy
        .candidates
        .iter()
        .find(|c| c.sha256 != ceo.sha256)
        .context("a distinct owner-listed local judge weight is required")?
        .clone();
    let menu = if stages.iter().all(|s| s.tools.is_empty()) {
        crate::company_factory::preparation::architecture_menu()?
    } else {
        crate::company_factory::repositories::InstalledRepositories::new(&app.config)?
            .architectures()?
    };
    let users = session["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["role"] == "user")
        .map(|m| json!({"role":"user","message":m["message"]}))
        .collect::<Vec<_>>();
    let context = json!({"user_brief":users,"proposal":session["proposal"],"proposal_sha256":crate::company_factory::digest(&session["proposal"])?,"media_capabilities":gents_local_mcp::media::capabilities(),
        "available_actors":app.config.agents.iter().map(|a|&a.id).collect::<Vec<_>>(),"architecture_menu":menu,
        "configured_max_run_seconds":app.config.max_run_seconds,"scope":"Architecture recommendation only. No task solution, staffing rewrite, tool grant, execution, compliance verdict or task acceptance."});
    let body = json!({"id":assessment,"planning_id":planning,"request":request,"context":context,"status":"pending_ceo","hardware_observations":{},"dispatch":{},"receipts":{},
        "models":{"ceo":{"candidate_id":ceo.id,"sha256":ceo.sha256},"judge":{"candidate_id":judge.id,"sha256":judge.sha256},"policy_sha256":policy.digest()?},"execution_authorized":false,"task_accepted":false});
    crate::security::require_persistable(&body.to_string())?;
    journal.bind(planning, &body)?;
    let workers = NativeModels {
        root,
        pool: &app.gateway.local,
        candidates: [ceo, judge],
        scope: format!("{company}:{assessment}"),
    };
    run(&workers, &NativeHardware, &mut journal, body).await
}

pub fn validate_selected(
    record: &Value,
    planning: Uuid,
    revision: u64,
    proposal: &Value,
    coordination: crate::company_factory::contracts::Coordination,
) -> Result<()> {
    ensure!(
        record["status"] == "ready_for_owner_review"
            && record["planning_id"] == planning.to_string()
            && record["request"]["expected_revision"] == revision
            && record["context"]["proposal_sha256"] == crate::company_factory::digest(proposal)?,
        "architecture assessment is stale or requires clarification/staffing review"
    );
    let choice: Choice = serde_json::from_value(record["recommendation"].clone())?;
    let review: Review = serde_json::from_value(record["review"].clone())?;
    let stages = crate::ceo_team::parse(proposal)?;
    choice.validate(4)?;
    review.validate(&choice, 4)?;
    ensure!(
        review.verdict == Verdict::Accept
            && choice.architecture == coordination
            && choice.agent_count == stages.len(),
        "prepared architecture/staffing differs from the reviewed recommendation"
    );
    ensure!(
        record["models"]["ceo"]["sha256"] != record["models"]["judge"]["sha256"],
        "architecture judge must have distinct weights"
    );
    for role in ["ceo", "judge"] {
        ensure!(
            record["receipts"][role]["accepted_protocol"] == true,
            "architecture native receipt missing"
        );
        let reply: Reply = serde_json::from_value(record["receipts"][role]["reply"].clone())?;
        crate::coordination::validate_receipt(&reply)?;
    }
    Ok(())
}

pub fn validate_intake(root: &Path, company: &str, owner: &str, intake: &Value) -> Result<()> {
    let Some(binding) = intake.get("factory_binding") else {
        return Ok(());
    };
    let Some(id) = binding["request"]["architecture_assessment"].as_str() else {
        ensure!(
            binding["assessment_sha256"].is_null(),
            "assessment hash without a canonical reference"
        );
        return Ok(());
    };
    let record = read(root, company, owner, Uuid::parse_str(id)?)?;
    ensure!(
        binding["assessment_sha256"] == crate::company_factory::digest(&record)?,
        "canonical architecture assessment changed"
    );
    let planning = Uuid::parse_str(
        intake["ceo_planning_id"]
            .as_str()
            .context("planning identity missing")?,
    )?;
    let revision = intake["ceo_planning_revision"]
        .as_u64()
        .context("planning revision missing")?;
    let coordination =
        serde_json::from_value(binding["request"]["specification"]["coordination"].clone())?;
    validate_selected(
        &record,
        planning,
        revision,
        &intake["ceo_plan"],
        coordination,
    )
}

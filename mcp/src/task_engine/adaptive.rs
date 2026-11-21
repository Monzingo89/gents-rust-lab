//! Host-bounded intake and concrete escalation. One task, one immutable input,
//! one budget and retained evidence span every plan revision.
use super::{
    contracts::*,
    intake::{self, IntakeWorker},
    resources::{Budget, Consumption},
    routing::{self, Hints},
    workflows::*,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeSettings {
    pub model: String,
    pub tokens: u32,
    pub max_repairs: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Escalation {
    pub on: StopReason,
    pub template: String,
    pub model: Option<String>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intake: Option<IntakeSettings>,
    #[serde(default)]
    pub max_replans: u32,
    #[serde(default)]
    pub rules: Vec<Escalation>,
}
impl Settings {
    pub fn validate(
        &self,
        models: &std::collections::BTreeMap<String, ResolvedModel>,
    ) -> Result<()> {
        ensure!(
            self.max_replans <= 3 && self.rules.len() <= 8,
            "unbounded adaptive policy"
        );
        if let Some(i) = &self.intake {
            ensure!(
                (64..=4096).contains(&i.tokens) && i.max_repairs <= 2,
                "invalid intake budget"
            );
            let m = models.get(&i.model).context("unregistered intake model")?;
            ensure!(
                m.evaluated_profiles.contains("intake"),
                "intake model lacks evaluated intake admission"
            );
        }
        for rule in &self.rules {
            ensure!(matches!(rule.on,StopReason::EvidenceInsufficient|StopReason::ProviderUnavailable|StopReason::InvalidOutput|StopReason::ContextPressure|StopReason::Capacity),"deadline, cancellation, authorization or global budget exhaustion cannot authorize replanning");
            ensure!(
                routing::templates().iter().any(|t| t.id == rule.template),
                "unimplemented escalation template"
            );
            ensure!(
                rule.model.as_ref().is_none_or(|id| models.contains_key(id)),
                "unregistered escalation model"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct IntakeReceipt {
    pub model: String,
    pub proposal: Option<intake::Proposal>,
    pub errors: Vec<String>,
    pub attempts: u32,
    pub generative_model_calls: Option<u32>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub memory_bytes: Option<u64>,
    pub elapsed_micros: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct Attempt {
    pub plan: ExecutionPlan,
    pub trigger: Option<StopReason>,
    pub outcome: Option<WorkflowOutcome>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    pub status: String,
    pub answer: Option<String>,
    pub verification_state: String,
    pub request: TaskRequest,
    pub analysis: Option<TaskAnalysis>,
    pub intake: Option<IntakeReceipt>,
    pub attempts: Vec<Attempt>,
    pub evidence: Vec<Evidence>,
    pub blockers: Vec<String>,
    pub consumption: Consumption,
    pub elapsed_micros: u64,
}
/// Host callbacks persist revisions/check current source and access epochs.
/// They are trusted code, never model-produced tool calls.
pub trait Observer: Send + Sync {
    fn intake<'a>(&'a self, _receipt: &'a IntakeReceipt) -> FutureResult<'a, ()> {
        Box::pin(async { Ok(()) })
    }
    fn plan<'a>(
        &'a self,
        _request: &'a TaskRequest,
        _plan: &'a ExecutionPlan,
        _analysis: Option<&'a TaskAnalysis>,
        _trigger: Option<&'a StopReason>,
        _consumption: &'a Consumption,
    ) -> FutureResult<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
pub struct NoopObserver;
impl Observer for NoopObserver {}
pub struct Coordinator<'a> {
    pub executor: &'a Executor<'a>,
    pub intake_worker: &'a dyn IntakeWorker,
    pub observer: &'a dyn Observer,
    pub settings: Settings,
    pub allow_parametric: bool,
}
impl Coordinator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        original: &TaskRequest,
        initial: &ExecutionPlan,
        hints: Hints,
        policy: &ExecutionPolicy,
        evidence: Vec<Evidence>,
        cancel: Arc<AtomicBool>,
        analysis_only: bool,
    ) -> Result<Outcome> {
        original.validate()?;
        self.settings.validate(self.executor.models)?;
        policy.validate(crate::now_ms())?;
        let began = Instant::now();
        let budget = Budget::new(policy, self.executor.limits.max_tool_calls, cancel);
        let builtins = routing::AgentRegistry::default();
        let profiles = self.executor.profiles.unwrap_or(&builtins).entries();
        let mut request = original.clone();
        let mut plan = initial.clone();
        let mut analysis = None;
        let mut intake_receipt = None;
        let mut attempts = vec![];
        let mut blockers = vec![];
        let mut retained = evidence;
        if !request.clarification_questions.is_empty() {
            return Ok(Outcome {
                status: "needs_clarification".into(),
                answer: None,
                verification_state: "not_verified".into(),
                blockers: request.clarification_questions.clone(),
                request,
                analysis,
                intake: None,
                attempts,
                evidence: retained,
                consumption: budget.consumption()?,
                elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128) as u64,
            });
        }
        if let Some(settings) = &self.settings.intake {
            let available: Vec<String> = profiles
                .keys()
                .filter(|id| id.as_str() != "intake")
                .filter(|id| {
                    routing::profiles().contains_key(*id) || hints.domain.as_ref() == Some(*id)
                })
                .take(32)
                .cloned()
                .collect();
            let packet = intake::Packet::new(original, hints.clone(), available, retained.clone())?;
            let receipt = self.analyze(&packet, settings, policy, &budget).await;
            self.observer.intake(&receipt).await?;
            if let Some(proposal) = &receipt.proposal {
                let mut normalized = intake::apply(
                    original,
                    &hints,
                    proposal,
                    &packet,
                    profiles,
                    self.allow_parametric,
                )?;
                normalized.analysis.evidence_required |= policy.evidence_required;
                normalized.analysis.retrieval_suitable |= policy.evidence_required;
                normalized.proposed_plan.evidence_required |= policy.evidence_required;
                if normalized.proposed_plan.evidence_required
                    && normalized.proposed_plan.template == "direct/v1"
                {
                    normalized.proposed_plan.template = "rag/v1".into();
                    normalized.proposed_plan.strategy = Strategy::GroundedGeneration;
                    for node in &mut normalized.proposed_plan.nodes {
                        node.strategy = Strategy::GroundedGeneration;
                    }
                }
                request = normalized.request;
                analysis = Some(normalized.analysis);
                if !request.clarification_questions.is_empty() {
                    blockers = request.clarification_questions.clone();
                    intake_receipt = Some(receipt);
                    return Ok(Outcome {
                        status: "needs_clarification".into(),
                        answer: None,
                        verification_state: "not_verified".into(),
                        request,
                        analysis,
                        intake: intake_receipt,
                        attempts,
                        evidence: retained,
                        blockers,
                        consumption: budget.consumption()?,
                        elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128) as u64,
                    });
                }
                let profile = normalized
                    .proposed_plan
                    .nodes
                    .first()
                    .context("intake plan empty")?
                    .profile
                    .as_str();
                let model = self
                    .executor
                    .models
                    .values()
                    .filter(|m| {
                        m.evaluated_profiles.contains(profile)
                            && (!policy.local_inference_required || m.local)
                    })
                    .min_by_key(|m| m.memory.total().unwrap_or(u64::MAX));
                let Some(model) = model else {
                    if analysis_only {
                        plan = normalized.proposed_plan;
                        plan.previous_plan = Some(initial.plan_id);
                        plan.revision = initial.revision.saturating_add(1);
                        plan.routing_justification.push_str(
                            " No evaluated compatible worker currently admitted; proposal only.",
                        );
                        intake_receipt = Some(receipt);
                        self.observer
                            .plan(
                                &request,
                                &plan,
                                analysis.as_ref(),
                                None,
                                &budget.consumption()?,
                            )
                            .await?;
                        return Ok(Outcome {
                            status: "analyzed".into(),
                            answer: None,
                            verification_state: "proposed_not_executed".into(),
                            request,
                            analysis,
                            intake: intake_receipt,
                            attempts: vec![Attempt {
                                plan,
                                trigger: None,
                                outcome: None,
                                error: None,
                            }],
                            evidence: retained,
                            blockers: vec!["No evaluated compatible worker admitted".into()],
                            consumption: budget.consumption()?,
                            elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128)
                                as u64,
                        });
                    }
                    return Ok(Outcome {
                        status: "blocked".into(),
                        answer: None,
                        verification_state: "not_verified".into(),
                        request,
                        analysis,
                        intake: Some(receipt),
                        attempts,
                        evidence: retained,
                        blockers: vec!["No evaluated compatible worker for intake proposal".into()],
                        consumption: budget.consumption()?,
                        elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128) as u64,
                    });
                };
                let mut selected = routing::factory_with_profiles(
                    &request,
                    &normalized.proposed_plan.template,
                    profile,
                    Some(&model.id),
                    profiles,
                )?;
                selected.evidence_required =
                    policy.evidence_required || normalized.proposed_plan.evidence_required;
                selected.resolved_models = self
                    .executor
                    .models
                    .iter()
                    .map(|(id, m)| (id.clone(), m.identity.clone()))
                    .collect();
                selected.previous_plan = Some(plan.plan_id);
                selected.revision = plan
                    .revision
                    .checked_add(1)
                    .context("plan revision overflow")?;
                selected.routing_justification = normalized.proposed_plan.routing_justification;
                plan = selected;
            } else {
                blockers.extend(receipt.errors.clone());
            }
            intake_receipt = Some(receipt);
        }
        if let Err(error) = budget.check() {
            blockers.push(error.to_string());
        }
        if !blockers.is_empty() {
            return Ok(Outcome {
                status: if budget.cancel.load(Ordering::Relaxed) {
                    "cancelled"
                } else if crate::now_ms() >= budget.deadline_ms {
                    "timed_out"
                } else {
                    "blocked"
                }
                .into(),
                answer: None,
                verification_state: "not_verified".into(),
                request,
                analysis,
                intake: intake_receipt,
                attempts,
                evidence: retained,
                blockers,
                consumption: budget.consumption()?,
                elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128) as u64,
            });
        }
        routing::configure_tools(&mut plan, policy, profiles)?;
        if analysis_only {
            self.observer
                .plan(
                    &request,
                    &plan,
                    analysis.as_ref(),
                    None,
                    &budget.consumption()?,
                )
                .await?;
            attempts.push(Attempt {
                plan,
                trigger: None,
                outcome: None,
                error: None,
            });
            return Ok(Outcome {
                status: "analyzed".into(),
                answer: None,
                verification_state: "proposed_not_executed".into(),
                request,
                analysis,
                intake: intake_receipt,
                attempts,
                evidence: retained,
                blockers,
                consumption: budget.consumption()?,
                elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128) as u64,
            });
        }
        let mut seen = BTreeSet::new();
        let mut trigger = None;
        let mut replans = 0;
        let mut answer = None;
        let mut verification = "not_verified".to_owned();
        loop {
            if let Err(error) = budget.check() {
                blockers.push(error.to_string());
                break;
            }
            ensure!(
                plan.task_id == original.task_id
                    && plan.original_objective_sha256
                        == crate::digest(original.original_ask.as_bytes())
                    && request.input == original.input,
                "replan changed original task"
            );
            let model = plan
                .nodes
                .first()
                .and_then(|n| n.provider_model.clone())
                .context("plan model unresolved")?;
            ensure!(
                seen.insert((plan.template.clone(), model.clone())),
                "repeated adaptive plan/model pair rejected"
            );
            self.observer
                .plan(
                    &request,
                    &plan,
                    analysis.as_ref(),
                    trigger.as_ref(),
                    &budget.consumption()?,
                )
                .await?;
            let mut execution_policy = policy.clone();
            execution_policy.evidence_required = plan.evidence_required || policy.evidence_required;
            if matches!(
                trigger,
                Some(StopReason::ProviderUnavailable | StopReason::Capacity)
            ) {
                execution_policy.max_concurrency = 1;
            }
            let executed = self
                .executor
                .execute_budgeted(
                    &request,
                    &plan,
                    &execution_policy,
                    retained.clone(),
                    &budget,
                )
                .await;
            let reason = match executed {
                Ok(outcome) => {
                    for e in outcome.receipts.iter().flat_map(|r| r.evidence.iter()) {
                        if !retained.contains(e)
                            && retained.len() < 8
                            && retained.iter().map(|e| e.quote.len()).sum::<usize>() + e.quote.len()
                                <= policy.max_evidence_bytes
                        {
                            retained.push(e.clone());
                        }
                    }
                    let reason = outcome.stop_reason.clone();
                    let success = outcome.answer.is_some();
                    if success {
                        answer = outcome.answer.clone();
                        verification = outcome.verification_state.clone();
                        blockers.clear();
                    } else {
                        blockers = outcome.blockers.clone();
                    }
                    attempts.push(Attempt {
                        plan: plan.clone(),
                        trigger: trigger.clone(),
                        outcome: Some(outcome),
                        error: None,
                    });
                    if success {
                        break;
                    }
                    reason
                }
                Err(error) => {
                    let reason = error
                        .downcast_ref::<ExecutionFailure>()
                        .map(|f| f.kind.clone())
                        .unwrap_or(StopReason::Other);
                    blockers = vec![error.to_string()];
                    attempts.push(Attempt {
                        plan: plan.clone(),
                        trigger: trigger.clone(),
                        outcome: None,
                        error: Some(error.to_string()),
                    });
                    Some(reason)
                }
            };
            let Some(reason) = reason else { break };
            if replans >= self.settings.max_replans
                || budget.check().is_err()
                || matches!(
                    reason,
                    StopReason::BudgetExhausted
                        | StopReason::Cancelled
                        | StopReason::Deadline
                        | StopReason::SourceChanged
                )
            {
                break;
            }
            let next = self
                .settings
                .rules
                .iter()
                .filter(|r| r.on == reason)
                .find_map(|rule| {
                    let id = rule.model.as_deref().unwrap_or(&model);
                    let m = self.executor.models.get(id)?;
                    let profile = &plan.nodes[0].profile;
                    if seen.contains(&(rule.template.clone(), id.to_owned()))
                        || !m.evaluated_profiles.contains(profile)
                        || policy.local_inference_required && !m.local
                    {
                        return None;
                    }
                    let mut candidate = routing::factory_with_profiles(
                        &request,
                        &rule.template,
                        profile,
                        Some(id),
                        profiles,
                    )
                    .ok()?;
                    routing::configure_tools(&mut candidate, policy, profiles).ok()?;
                    candidate.evidence_required =
                        plan.evidence_required || policy.evidence_required;
                    candidate.resolved_models = plan.resolved_models.clone();
                    if !budget
                        .can_fit(
                            candidate.nodes.len() as u32,
                            self.executor
                                .limits
                                .per_call_tokens
                                .saturating_mul(candidate.nodes.len() as u32),
                        )
                        .unwrap_or(false)
                    {
                        return None;
                    }
                    Some(candidate)
                });
            let Some(mut next) = next else { break };
            replans += 1;
            next.previous_plan = Some(plan.plan_id);
            next.revision = plan
                .revision
                .checked_add(1)
                .context("plan revision overflow")?;
            next.routing_justification=format!("Host escalation after {reason:?}; original objective, retained evidence and shared global budget preserved.");
            plan = next;
            trigger = Some(reason);
        }
        let status = if answer.is_some() {
            "generated"
        } else if budget.cancel.load(Ordering::Relaxed) {
            "cancelled"
        } else if crate::now_ms() >= budget.deadline_ms {
            "timed_out"
        } else {
            "blocked"
        };
        Ok(Outcome {
            status: status.into(),
            answer,
            verification_state: verification,
            request,
            analysis,
            intake: intake_receipt,
            attempts,
            evidence: retained,
            blockers,
            consumption: budget.consumption()?,
            elapsed_micros: began.elapsed().as_micros().min(u64::MAX as u128) as u64,
        })
    }
    async fn analyze(
        &self,
        packet: &intake::Packet,
        settings: &IntakeSettings,
        policy: &ExecutionPolicy,
        budget: &Budget,
    ) -> IntakeReceipt {
        let began = Instant::now();
        let mut receipt = IntakeReceipt {
            model: settings.model.clone(),
            proposal: None,
            errors: vec![],
            attempts: 0,
            generative_model_calls: Some(0),
            input_tokens: Some(0),
            output_tokens: Some(0),
            memory_bytes: None,
            elapsed_micros: 0,
        };
        let result=async{
            let model=&self.executor.models[&settings.model];ensure!(!policy.local_inference_required||model.local,"intake inference locality denied");
            let _reservation=self.executor.scheduler.reserve(&model.memory,budget).await?;let mut packet=packet.clone();
            for attempt in 0..=settings.max_repairs{
                budget.check()?;ensure!(serde_json::to_vec(&packet)?.len()<=model.context_bytes,"intake context pressure; exact input cannot be truncated");
                budget.reserve_call(settings.tokens)?;receipt.attempts+=1;
                let output={let call=self.intake_worker.analyze(&model.id,&packet,settings.tokens,&budget.cancel);tokio::pin!(call);loop{budget.check()?;tokio::select!{r=&mut call=>break r,_=tokio::time::sleep(std::time::Duration::from_millis(20))=>{}}}};
                let parsed=match output{
                    Ok(output)=>{receipt.generative_model_calls=receipt.generative_model_calls.zip(output.generative_model_calls).map(|(a,b)|a.saturating_add(b));receipt.input_tokens=receipt.input_tokens.zip(output.input_tokens).map(|(a,b)|a.saturating_add(b));receipt.output_tokens=receipt.output_tokens.zip(output.output_tokens).map(|(a,b)|a.saturating_add(b));receipt.memory_bytes=output.memory_bytes;
                        if output.text.len()>16384{Err(anyhow::anyhow!("intake output exceeds bound"))}else{serde_json::from_str::<intake::Proposal>(&output.text).map_err(anyhow::Error::from).and_then(|p|{p.validate(&packet)?;Ok(p)})}},
                    Err(error)=>{receipt.generative_model_calls=None;receipt.input_tokens=None;receipt.output_tokens=None;Err(error)}
                };
                match parsed{Ok(proposal)=>{receipt.proposal=Some(proposal);return Ok(());},Err(error)=>{let message=error.to_string();receipt.errors.push(message.clone());if attempt==settings.max_repairs{return Err(error);}packet.repair_feedback=vec![format!("Previous proposal rejected: {message}. Repair the schema/analysis only; no change to input, permissions or budget.")];}}
            }Ok::<_,anyhow::Error>(())
        }.await;
        if let Err(error) = result {
            let message = error.to_string();
            if !receipt.errors.contains(&message) {
                receipt.errors.push(message);
            }
        }
        receipt.elapsed_micros = began.elapsed().as_micros().min(u64::MAX as u128) as u64;
        receipt
    }
}

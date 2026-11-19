//! SQLite answer cache and versioned, owner-scoped lexical evidence repository.
use super::contracts::*;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeSet, time::Instant};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LookupBinding {
    pub exact_ask: String,
    pub temporal_scope: String,
    pub context_independent: bool,
    pub start_byte: usize,
    pub end_byte: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeSource {
    pub source_id: String,
    pub revision: String,
    pub title: String,
    pub domain: String,
    pub language: String,
    pub text: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub lookups: Vec<LookupBinding>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SearchHit {
    pub score: usize,
    pub evidence: Evidence,
}

/// Caller must use one SQLite transaction for the whole operation. The host
/// owns authentication and allowed_sources; input/model JSON cannot set them.
pub struct Repository<'a> {
    pub(super) db: &'a Connection,
}
impl<'a> Repository<'a> {
    pub fn new(db: &'a Connection) -> Result<Self> {
        ensure!(
            !db.is_autocommit(),
            "task repository requires a caller-owned transaction"
        );
        db.execute_batch("CREATE TABLE IF NOT EXISTS task_sources_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,id TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,PRIMARY KEY(tenant,subject,id));
            CREATE TABLE IF NOT EXISTS task_answers_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,key TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,created INTEGER NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(tenant,subject,key));
            CREATE INDEX IF NOT EXISTS task_answers_expiry_v1 ON task_answers_v1(tenant,subject,expires);
            CREATE TABLE IF NOT EXISTS task_records_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,id TEXT NOT NULL,request TEXT NOT NULL,result TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(tenant,subject,id));")?;
        let repo = Self { db };
        super::grounded::install(&repo)?;
        Ok(repo)
    }
    pub fn source_ids(&self, tenant: &str, subject: &str) -> Result<BTreeSet<String>> {
        Ok(self
            .db
            .prepare("SELECT id FROM task_sources_v1 WHERE tenant=?1 AND subject=?2 ORDER BY id")?
            .query_map(params![tenant, subject], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn ingest(
        &self,
        scope: &AccessScope,
        source: &KnowledgeSource,
        now: u64,
    ) -> Result<String> {
        ensure!(
            valid_id(&source.source_id)
                && !source.revision.is_empty()
                && source.revision.len() <= 160,
            "source identity/revision required"
        );
        ensure!(
            !source.title.is_empty()
                && source.title.len() <= 512
                && source.domain.len() <= 80
                && !source.language.is_empty()
                && source.language.len() <= 80,
            "bounded source metadata required"
        );
        ensure!(
            !source.text.is_empty() && source.text.len() <= 49152 && source.lookups.len() <= 32,
            "bounded nonempty text and lookup bindings required"
        );
        ensure!(
            source.observed_at_ms <= now
                && source.valid_until_ms > now
                && source.observed_at_ms <= source.valid_until_ms
                && source.valid_until_ms <= i64::MAX as u64,
            "source observation/freshness interval rejected"
        );
        let body = serde_json::to_string(source)?;
        ensure!(body.len() <= 65536, "source record exceeds bound");
        crate::safety::require_persistable(&body)?;
        let mut keys = BTreeSet::new();
        for lookup in &source.lookups {
            ensure!(
                !lookup.exact_ask.trim().is_empty()
                    && lookup.exact_ask.len() <= 12000
                    && keys.insert((&lookup.exact_ask, &lookup.temporal_scope)),
                "duplicate/invalid source lookup binding"
            );
            quote(source, lookup.start_byte, lookup.end_byte, 8192)?;
        }
        let old: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM task_sources_v1 WHERE tenant=?1 AND subject=?2 AND id=?3",
                params![scope.tenant, scope.subject, source.source_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            let previous: KnowledgeSource = serde_json::from_str(&old)?;
            ensure!(
                previous.revision != source.revision || old == body,
                "changed source requires a new revision"
            );
        } else {
            ensure!(
                self.source_ids(&scope.tenant, &scope.subject)?.len() < 256,
                "source capacity reached; delete obsolete sources first"
            );
        }
        let hash = crate::digest(body.as_bytes());
        self.db.execute("INSERT INTO task_sources_v1 VALUES(?1,?2,?3,?4,?5) ON CONFLICT(tenant,subject,id) DO UPDATE SET body=excluded.body,sha256=excluded.sha256",
            params![scope.tenant, scope.subject, source.source_id, body, hash])?;
        Ok(hash)
    }
    pub fn delete_source(&self, scope: &AccessScope, id: &str) -> Result<bool> {
        Ok(self.db.execute(
            "DELETE FROM task_sources_v1 WHERE tenant=?1 AND subject=?2 AND id=?3",
            params![scope.tenant, scope.subject, id],
        )? != 0)
    }
    pub(super) fn sources(&self, scope: &AccessScope) -> Result<Vec<(KnowledgeSource, String)>> {
        let mut sources = Vec::new();
        // SQL scopes candidates before parsing or exposing any source text.
        let mut statement = self.db.prepare(
            "SELECT id,body,sha256 FROM task_sources_v1 WHERE tenant=?1 AND subject=?2 ORDER BY id",
        )?;
        let rows = statement.query_map(params![scope.tenant, scope.subject], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, body, hash) = row?;
            if !scope.allowed_sources.contains(&id) {
                continue;
            }
            ensure!(
                body.len() <= 65536 && crate::digest(body.as_bytes()) == hash,
                "source integrity mismatch"
            );
            let source: KnowledgeSource = serde_json::from_str(&body)?;
            ensure!(source.source_id == id, "source identity mismatch");
            sources.push((source, hash));
        }
        Ok(sources)
    }
    pub fn search(
        &self,
        scope: &AccessScope,
        query: &str,
        domain: Option<&str>,
        max_bytes: usize,
        now: u64,
    ) -> Result<Vec<SearchHit>> {
        ensure!(
            !query.trim().is_empty() && query.len() <= 12000 && (1..=16384).contains(&max_bytes),
            "bounded search required"
        );
        let terms = tokens(query);
        let mut hits = Vec::new();
        for (source, _) in self.sources(scope)? {
            if source.observed_at_ms > now
                || source.valid_until_ms <= now
                || domain.is_some_and(|d| source.domain != d)
            {
                continue;
            }
            // Paragraph chunks retain exact UTF-8 byte coordinates in the original.
            let mut offset = 0;
            for paragraph in source.text.split_inclusive('\n') {
                let start = offset;
                offset += paragraph.len();
                let found = tokens(paragraph);
                let score = terms.intersection(&found).count();
                if score == 0 || paragraph.len() > max_bytes {
                    continue;
                }
                hits.push(SearchHit {
                    score,
                    evidence: evidence(&source, start, offset, max_bytes)?,
                });
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.evidence.source_id.cmp(&b.evidence.source_id))
                .then_with(|| a.evidence.start_byte.cmp(&b.evidence.start_byte))
        });
        let mut used = 0;
        hits.retain(|hit| {
            let fits = used + hit.evidence.quote.len() <= max_bytes;
            if fits {
                used += hit.evidence.quote.len();
            }
            fits
        });
        hits.truncate(8);
        Ok(hits)
    }
    pub fn execute(
        &self,
        request: &TaskRequest,
        policy: &ExecutionPolicy,
        now: u64,
    ) -> Result<TaskResult> {
        let started = Instant::now();
        request.validate()?;
        policy.validate(now)?;
        let scope = &policy.scope;
        let sources = self.sources(scope)?;
        let snapshot: Vec<_> = sources
            .iter()
            .map(|(source, hash)| (&source.source_id, hash))
            .collect();
        // Preserve exact bytes, conversation, output, dates, negation, policy and revisions.
        let key = crate::digest(&serde_json::to_vec(
            &json!({"compatibility":COMPATIBILITY,"scope":scope,"input":request.input,"source_snapshot":snapshot,
                "cache_ttl_ms":policy.cache_ttl_ms,"max_evidence_bytes":policy.max_evidence_bytes}),
        )?);
        let eligible = request.input.intent == Intent::Answer
            && request.input.constraints.is_empty()
            && request.input.proposed_acceptance.is_empty()
            && request.input.deliverable.is_empty()
            && request.input.supplied_context.is_empty();
        self.db.execute(
            "DELETE FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 AND expires<=?3",
            params![scope.tenant, scope.subject, now as i64],
        )?;
        let cached: Option<(String, String, i64, i64)> = if eligible {
            self.db.query_row("SELECT body,sha256,created,expires FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 AND key=?3",
                params![scope.tenant, scope.subject, key], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?
        } else {
            None
        };
        if let Some((body, hash, created, expires)) = cached {
            let created = u64::try_from(created)?;
            let expires = u64::try_from(expires)?;
            ensure!(
                crate::digest(body.as_bytes()) == hash,
                "answer cache integrity mismatch"
            );
            let mut result: TaskResult = serde_json::from_str(&body)?;
            let valid = created <= now
                && now < expires
                && !result.evidence.is_empty()
                && result.evidence.iter().all(|e| {
                    sources.iter().any(|(source, _)| {
                        source.source_id == e.source_id
                            && evidence(source, e.start_byte, e.end_byte, policy.max_evidence_bytes)
                                .is_ok_and(|current| current == *e)
                            && source.observed_at_ms <= now
                            && now < source.valid_until_ms
                    })
                });
            if valid {
                let origin = result.task_id;
                result.task_id = request.task_id;
                result.actual_path = "exact_answer_cache".into();
                result.cache = CacheProvenance { decision: "hit_current_scope_and_sources".into(), key, origin_task_id: Some(origin),
                    created_at_ms: Some(created), expires_at_ms: Some(expires), work_avoided: "Repeated deterministic extraction; zero generative calls in both paths. No inferred token or energy savings.".into() };
                result.measurements = measurements(started);
                if let Some(plan) = &mut result.plan {
                    plan.task_id = request.task_id;
                    plan.plan_id = Uuid::new_v4();
                    plan.cache_decision = "exact_hit".into();
                }
                self.record(request, &result, scope, now)?;
                self.prune_cache(policy)?;
                return Ok(result);
            }
        }
        let mut result = TaskResult {
            schema_version: VERSION,
            task_id: request.task_id,
            status: "requires_planning".into(),
            answer: None,
            artifacts: vec![],
            evidence: vec![],
            blockers: vec![],
            acceptance_checks: vec![],
            actual_path: "no_sufficient_deterministic_answer".into(),
            verification_state: "not_verified".into(),
            cache: CacheProvenance {
                decision: if eligible {
                    "miss"
                } else {
                    "bypass_action_or_unvalidated_constraints"
                }
                .into(),
                key: key.clone(),
                origin_task_id: None,
                created_at_ms: None,
                expires_at_ms: None,
                work_avoided: "Not estimated".into(),
            },
            measurements: measurements(started),
            plan: None,
        };
        if eligible {
            let mut candidates = Vec::new();
            for (source, _) in &sources {
                if source.observed_at_ms > now
                    || source.valid_until_ms <= now
                    || (request.input.output.language != "unspecified"
                        && request.input.output.language != source.language)
                {
                    continue;
                }
                if let Some(selection) = &request.input.extraction {
                    if source.source_id == selection.source_id {
                        candidates.push(evidence(
                            source,
                            selection.start_byte,
                            selection.end_byte,
                            policy.max_evidence_bytes,
                        )?);
                    }
                } else {
                    for lookup in &source.lookups {
                        if lookup.context_independent
                            && lookup.exact_ask == request.original_ask
                            && lookup.temporal_scope == request.input.temporal_scope
                        {
                            candidates.push(evidence(
                                source,
                                lookup.start_byte,
                                lookup.end_byte,
                                policy.max_evidence_bytes,
                            )?);
                        }
                    }
                }
            }
            if candidates.len() == 1 {
                let e = candidates.remove(0);
                result.answer = Some(if request.input.output.format == "json" {
                    serde_json::to_string(&e.quote)?
                } else {
                    e.quote.clone()
                });
                result.evidence = vec![e];
                result.status = "succeeded".into();
                result.verification_state = "exact_source_supported_extraction".into();
                result.actual_path = if request.input.extraction.is_some() {
                    "source_extraction"
                } else {
                    "deterministic_lookup"
                }
                .into();
                result.acceptance_checks = vec![AcceptanceCheck { name: "current_authorized_source_bytes".into(), status: CheckStatus::Passed,
                    detail: "Exact quote and source coordinates checked. Curated lookup association is owner supplied; factual truth/entailment is not inferred.".into() },
                    AcceptanceCheck { name: "independent_factual_accuracy".into(), status: CheckStatus::NotRun, detail: "Source support is distinct from factual correctness.".into() }];
                result.plan = Some(plan(request, &result.actual_path));
                let expires = now
                    .saturating_add(policy.cache_ttl_ms)
                    .min(result.evidence[0].valid_until_ms);
                if expires > now {
                    result.cache.created_at_ms = Some(now);
                    result.cache.expires_at_ms = Some(expires);
                    result.cache.decision = "miss_stored_validated_extraction".into();
                    let body = serde_json::to_string(&result)?;
                    self.db.execute(
                        "INSERT OR REPLACE INTO task_answers_v1 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                        params![
                            scope.tenant,
                            scope.subject,
                            key,
                            body,
                            crate::digest(body.as_bytes()),
                            now as i64,
                            expires as i64
                        ],
                    )?;
                }
            } else if candidates.len() > 1 {
                result.blockers.push("Multiple source bindings match; no arbitrary winner or cached answer. Review conflicting or redundant evidence.".into());
            } else {
                result.blockers.push("No current authorized exact source binding. Retrieval or CEO analysis is required; missing evidence is not a false fact.".into());
            }
        } else {
            result.blockers.push("This path cannot execute actions or certify additional caller constraints. Continue through governed CEO planning.".into());
        }
        result.measurements = measurements(started);
        self.record(request, &result, scope, now)?;
        self.prune_cache(policy)?;
        Ok(result)
    }
    fn prune_cache(&self, policy: &ExecutionPolicy) -> Result<()> {
        let scope = &policy.scope;
        self.db.execute("DELETE FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 AND key NOT IN (SELECT key FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 ORDER BY created DESC,key LIMIT ?3)", params![scope.tenant,scope.subject,policy.max_cache_entries as i64])?;
        Ok(())
    }
    fn record(
        &self,
        request: &TaskRequest,
        result: &TaskResult,
        scope: &AccessScope,
        now: u64,
    ) -> Result<()> {
        self.db.execute(
            "INSERT INTO task_records_v1 VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                scope.tenant,
                scope.subject,
                request.task_id.to_string(),
                serde_json::to_string(request)?,
                serde_json::to_string(result)?,
                i64::try_from(now)?
            ],
        )?;
        self.db.execute("DELETE FROM task_records_v1 WHERE tenant=?1 AND subject=?2 AND id NOT IN (SELECT id FROM task_records_v1 WHERE tenant=?1 AND subject=?2 ORDER BY created DESC,id LIMIT 1024) AND id NOT IN (SELECT s.task_id FROM task_grounded_subscribers_v1 s JOIN task_grounded_jobs_v1 j ON j.job_id=s.job_id WHERE s.tenant=?1 AND s.subject=?2 AND j.state IN ('running','cancel_requested'))", params![scope.tenant,scope.subject])?;
        Ok(())
    }
    pub fn status(
        &self,
        scope: &AccessScope,
        task_id: Uuid,
        now: u64,
    ) -> Result<Option<TaskResult>> {
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT result FROM task_records_v1 WHERE tenant=?1 AND subject=?2 AND id=?3",
                params![scope.tenant, scope.subject, task_id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        // Do not expose an old private answer after scope/source changes.
        if let Some(body) = body {
            let mut result: TaskResult = serde_json::from_str(&body)?;
            if result.status == "running" {
                result.evidence.clear();
                return Ok(Some(result));
            }
            let sources = self.sources(scope)?;
            if result.evidence.iter().any(|e| {
                !sources.iter().any(|(source, _)| {
                    source.source_id == e.source_id
                        && source.observed_at_ms <= now
                        && now < source.valid_until_ms
                        && evidence(source, e.start_byte, e.end_byte, 16384)
                            .is_ok_and(|current| current == *e)
                })
            }) {
                result.answer = None;
                result.evidence.clear();
                result.status = "evidence_invalidated".into();
                result.verification_state = "not_verified".into();
            }
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
    pub fn inspect_cache(&self, scope: &AccessScope) -> Result<serde_json::Value> {
        let (entries, bytes): (i64,i64) = self.db.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(body AS BLOB))),0) FROM task_answers_v1 WHERE tenant=?1 AND subject=?2", params![scope.tenant,scope.subject], |r| Ok((r.get(0)?,r.get(1)?)))?;
        let (drafts,active,draft_bytes):(i64,i64,i64)=self.db.query_row("SELECT COUNT(*),COALESCE(SUM(state!='completed'),0),COALESCE(SUM(length(CAST(body AS BLOB))+COALESCE(length(CAST(result AS BLOB)),0)),0) FROM task_grounded_jobs_v1 WHERE tenant=?1 AND subject=?2",params![scope.tenant,scope.subject],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        Ok(
            json!({"schema_version":VERSION,"exact_entries":entries,"stored_json_bytes":bytes,"grounded_draft_records":drafts,"grounded_active_or_uncertain":active,"grounded_stored_json_bytes":draft_bytes,"semantic_answers":"unavailable_pending_evaluation","prefix_kv":"separate_backend_optimization_not_answer_cache"}),
        )
    }
    pub fn invalidate_cache(&self, scope: &AccessScope) -> Result<usize> {
        self.db.execute("DELETE FROM task_grounded_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job_id IN (SELECT job_id FROM task_grounded_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed')",params![scope.tenant,scope.subject])?;
        let drafts=self.db.execute("DELETE FROM task_grounded_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed'",params![scope.tenant,scope.subject])?;
        Ok(self.db.execute(
            "DELETE FROM task_answers_v1 WHERE tenant=?1 AND subject=?2",
            params![scope.tenant, scope.subject],
        )? + drafts)
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b))
}
fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect()
}
fn quote(source: &KnowledgeSource, start: usize, end: usize, maximum: usize) -> Result<&str> {
    ensure!(
        start < end && end.saturating_sub(start) <= maximum,
        "bounded nonempty extraction required"
    );
    source
        .text
        .get(start..end)
        .context("extraction requires valid UTF-8 byte coordinates")
}
fn evidence(
    source: &KnowledgeSource,
    start: usize,
    end: usize,
    maximum: usize,
) -> Result<Evidence> {
    Ok(Evidence {
        source_id: source.source_id.clone(),
        source_revision: source.revision.clone(),
        title: source.title.clone(),
        content_sha256: crate::digest(source.text.as_bytes()),
        start_byte: start,
        end_byte: end,
        quote: quote(source, start, end, maximum)?.into(),
        observed_at_ms: source.observed_at_ms,
        valid_until_ms: source.valid_until_ms,
    })
}
fn measurements(started: Instant) -> Measurements {
    Measurements {
        generative_model_calls: Some(0),
        embedding_calls: 0,
        tool_calls: 0,
        input_tokens: Some(0),
        output_tokens: Some(0),
        elapsed_micros: Some(started.elapsed().as_micros().try_into().unwrap_or(u64::MAX)),
        provider_cost_usd: Some(0),
        memory_bytes: None,
    }
}
fn plan(request: &TaskRequest, path: &str) -> ExecutionPlan {
    let strategy = if path == "source_extraction" {
        Strategy::SourceExtraction
    } else {
        Strategy::ExactLookup
    };
    ExecutionPlan { schema_version: VERSION, plan_id: Uuid::new_v4(), task_id: request.task_id, revision: 1, previous_plan: None,
        original_objective_sha256: crate::digest(request.original_ask.as_bytes()), template: format!("{path}/v1"), coordination: Coordination::Single,
        scheduling: Scheduling::Sequential, strategy: strategy.clone(), nodes: vec![PlanNode { id: "extract".into(), dependencies: vec![],
            profile: "source_extraction/v1".into(), provider_model: None, strategy, tools: vec!["local_knowledge".into()], validators: vec!["current_source_bytes/v1".into()] }],
        routing_justification: "One exact owner-curated source binding is sufficient for verbatim extraction. No generator, planner or judge required.".into(), cache_decision: "exact_miss".into() }
}

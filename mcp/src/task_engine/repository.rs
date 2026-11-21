//! SQLite answer cache and versioned, owner-scoped lexical evidence repository.
use super::contracts::*;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeSet, time::Instant};
use uuid::Uuid;

#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LookupBinding {
    pub exact_ask: String,
    pub temporal_scope: String,
    pub context_independent: bool,
    pub start_byte: usize,
    pub end_byte: usize,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
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
    /// Host-validated repository/workspace relative path -> full SHA-256.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub dependencies: std::collections::BTreeMap<String, String>,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
pub struct SearchHit {
    pub score: usize,
    pub evidence: Evidence,
}

/// Caller must use one SQLite transaction for the whole operation. The host
/// owns authentication and allowed_sources; input/model JSON cannot set them.
pub struct Repository<'a> {
    pub(super) db: &'a Connection,
    validated_dependencies: std::cell::RefCell<BTreeSet<String>>,
    dependency_root: std::cell::RefCell<Option<std::path::PathBuf>>,
    capture_root: std::cell::RefCell<Option<std::path::PathBuf>>,
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
        let repo = Self {
            db,
            validated_dependencies: std::cell::RefCell::new(BTreeSet::new()),
            dependency_root: std::cell::RefCell::new(None),
            capture_root: std::cell::RefCell::new(None),
        };
        super::grounded::install(&repo)?;
        super::semantic::install(&repo)?;
        super::workspace_evidence::install(&repo)?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS task_chunks_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,source TEXT NOT NULL,snapshot TEXT NOT NULL,start INTEGER NOT NULL,end INTEGER NOT NULL,terms TEXT NOT NULL,PRIMARY KEY(tenant,subject,source,start));
            CREATE TABLE IF NOT EXISTS task_retrieval_cache_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,key TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(tenant,subject,key));")?;
        Ok(repo)
    }
    fn index_source(
        &self,
        scope: &AccessScope,
        source: &KnowledgeSource,
        hash: &str,
    ) -> Result<()> {
        let count:i64=self.db.query_row("SELECT COUNT(*) FROM task_chunks_v1 WHERE tenant=?1 AND subject=?2 AND source=?3 AND snapshot=?4",params![scope.tenant,scope.subject,source.source_id,hash],|r|r.get(0))?;
        if count > 0 {
            return Ok(());
        }
        self.db.execute(
            "DELETE FROM task_chunks_v1 WHERE tenant=?1 AND subject=?2 AND source=?3",
            params![scope.tenant, scope.subject, source.source_id],
        )?;
        let mut start = 0;
        for paragraph in source.text.split_inclusive('\n') {
            let end = start + paragraph.len();
            while start < end {
                let mut next = (start + 2048).min(end);
                while !source.text.is_char_boundary(next) {
                    next -= 1;
                }
                self.db.execute(
                    "INSERT INTO task_chunks_v1 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        scope.tenant,
                        scope.subject,
                        source.source_id,
                        hash,
                        start as i64,
                        next as i64,
                        serde_json::to_string(&tokens(&source.text[start..next]))?
                    ],
                )?;
                start = next;
            }
        }
        Ok(())
    }
    pub fn source_ids(&self, tenant: &str, subject: &str) -> Result<BTreeSet<String>> {
        Ok(self
            .db
            .prepare("SELECT id FROM task_sources_v1 WHERE tenant=?1 AND subject=?2 ORDER BY id")?
            .query_map(params![tenant, subject], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn knowledge_snapshot(
        &self,
        scope: &AccessScope,
        now: u64,
    ) -> Result<Vec<(String, String, bool)>> {
        Ok(self
            .sources(scope)?
            .into_iter()
            .map(|(s, h)| {
                (
                    s.source_id,
                    h,
                    s.observed_at_ms <= now && now < s.valid_until_ms,
                )
            })
            .collect())
    }
    pub fn replace_result(&self, scope: &AccessScope, result: &TaskResult) -> Result<()> {
        ensure!(
            self.db.execute(
                "UPDATE task_records_v1 SET result=?4 WHERE tenant=?1 AND subject=?2 AND id=?3",
                params![
                    scope.tenant,
                    scope.subject,
                    result.task_id.to_string(),
                    serde_json::to_string(result)?
                ]
            )? == 1,
            "original task record missing; no replay"
        );
        Ok(())
    }
    pub fn ingest(
        &self,
        scope: &AccessScope,
        source: &KnowledgeSource,
        now: u64,
    ) -> Result<String> {
        ensure!(
            source.dependencies.len() <= 8
                && source.dependencies.iter().all(|(path, hash)| {
                    crate::workspace_path::is_safe_relative_path(path)
                        && hash.len() == 64
                        && hash
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                }),
            "bounded safe source dependency paths and SHA-256 values required"
        );
        ensure!(
            valid_id(&source.source_id)
                && !source.source_id.starts_with("ws-")
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
        // Assess source text, never interpret typed timestamps/byte offsets as
        // payment-card text. Current millisecond timestamps can pass Luhn.
        for field in [
            &source.source_id,
            &source.revision,
            &source.title,
            &source.domain,
            &source.language,
            &source.text,
        ] {
            crate::safety::require_persistable(field)?;
        }
        for lookup in &source.lookups {
            crate::safety::require_persistable(&lookup.exact_ask)?;
            crate::safety::require_persistable(&lookup.temporal_scope)?;
        }
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
        self.index_source(scope, source, &hash)?;
        Ok(hash)
    }
    pub fn delete_source(&self, scope: &AccessScope, id: &str) -> Result<bool> {
        self.db.execute(
            "DELETE FROM task_chunks_v1 WHERE tenant=?1 AND subject=?2 AND source=?3",
            params![scope.tenant, scope.subject, id],
        )?;
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
        self.require_dependencies(scope)?;
        let key = crate::digest(&serde_json::to_vec(
            &json!({"index":"paragraph-utf8-2048/v1","scope":scope,"snapshot":self.knowledge_snapshot(scope,now)?,"query":query,"domain":domain,"bytes":max_bytes}),
        )?);
        self.db.execute(
            "DELETE FROM task_retrieval_cache_v1 WHERE tenant=?1 AND subject=?2 AND expires<=?3",
            params![scope.tenant, scope.subject, now as i64],
        )?;
        let cached:Option<(String,String)>=self.db.query_row("SELECT body,sha256 FROM task_retrieval_cache_v1 WHERE tenant=?1 AND subject=?2 AND key=?3 AND expires>?4",params![scope.tenant,scope.subject,key,now as i64],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((body, hash)) = cached {
            ensure!(
                crate::digest(body.as_bytes()) == hash,
                "retrieval cache integrity mismatch"
            );
            let hits: Vec<SearchHit> = serde_json::from_str(&body)?;
            if hits.is_empty()
                || self.current_evidence(
                    scope,
                    &hits.iter().map(|h| h.evidence.clone()).collect::<Vec<_>>(),
                    max_bytes,
                    now,
                )?
            {
                return Ok(hits);
            }
        }
        let mut hits = Vec::new();
        for (source, hash) in self.sources(scope)? {
            if source.observed_at_ms > now
                || source.valid_until_ms <= now
                || domain.is_some_and(|d| source.domain != d)
            {
                continue;
            }
            self.index_source(scope, &source, &hash)?;
            if max_bytes >= 2048 {
                let chunks:Vec<(i64,i64,String)>=self.db.prepare("SELECT start,end,terms FROM task_chunks_v1 WHERE tenant=?1 AND subject=?2 AND source=?3 AND snapshot=?4 ORDER BY start")?.query_map(params![scope.tenant,scope.subject,source.source_id,hash],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
                for (start, end, encoded) in chunks {
                    let found: BTreeSet<String> = serde_json::from_str(&encoded)?;
                    let score = terms.intersection(&found).count();
                    if score > 0 {
                        hits.push(SearchHit {
                            score,
                            evidence: evidence(
                                &source,
                                usize::try_from(start)?,
                                usize::try_from(end)?,
                                max_bytes,
                            )?,
                        });
                    }
                }
                continue;
            }
            // Small evidence budgets retain exact UTF-8 byte coordinates too.
            let mut offset = 0;
            for paragraph in source.text.split_inclusive('\n') {
                let end = offset + paragraph.len();
                while offset < end {
                    let start = offset;
                    offset = (start + max_bytes.min(2048)).min(end);
                    while !source.text.is_char_boundary(offset) {
                        offset -= 1;
                    }
                    if offset == start {
                        break;
                    }
                    let found = tokens(&source.text[start..offset]);
                    let score = terms.intersection(&found).count();
                    if score > 0 {
                        hits.push(SearchHit {
                            score,
                            evidence: evidence(&source, start, offset, max_bytes)?,
                        });
                    }
                }
                offset = end;
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
        let body = serde_json::to_string(&hits)?;
        let expires = hits
            .iter()
            .map(|h| h.evidence.valid_until_ms)
            .min()
            .unwrap_or(now + 1000)
            .min(now.saturating_add(60000));
        self.db.execute(
            "INSERT OR REPLACE INTO task_retrieval_cache_v1 VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                scope.tenant,
                scope.subject,
                key,
                body,
                crate::digest(body.as_bytes()),
                i64::try_from(expires)?
            ],
        )?;
        self.db.execute("DELETE FROM task_retrieval_cache_v1 WHERE tenant=?1 AND subject=?2 AND key NOT IN (SELECT key FROM task_retrieval_cache_v1 WHERE tenant=?1 AND subject=?2 ORDER BY expires DESC,key LIMIT 128)",params![scope.tenant,scope.subject])?;
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
        self.require_dependencies(scope)?;
        let sources = self.sources(scope)?;
        // Preserve exact bytes, conversation, output, dates, negation, policy and revisions.
        let key = self.answer_key(&request.input, policy)?;
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
                self.prune_cache(policy, now)?;
                return Ok(result);
            }
        }
        if eligible {
            if let Some(mut result) = self.semantic_hit(request, policy, now)? {
                result.measurements.elapsed_micros =
                    Some(started.elapsed().as_micros().min(u64::MAX as u128) as u64);
                self.record(request, &result, scope, now)?;
                self.prune_cache(policy, now)?;
                return Ok(result);
            }
        }
        let mut result = TaskResult {
            validation_receipts: vec![],
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
            analysis: None,
            clarification_questions: vec![],
            assumptions: vec![],
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
                    .saturating_add(policy.cache_ttl_ms.min(policy.retention_ms))
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
        self.prune_cache(policy, now)?;
        Ok(result)
    }
    fn prune_cache(&self, policy: &ExecutionPolicy, now: u64) -> Result<()> {
        let scope = &policy.scope;
        let cutoff = i64::try_from(now.saturating_sub(policy.retention_ms))?;
        self.db.execute(
            "DELETE FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 AND created<?3",
            params![scope.tenant, scope.subject, cutoff],
        )?;
        let workflows:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_workflow_subscribers_v1')",[],|r|r.get(0))?;
        let workflow_guard = if workflows {
            " AND id NOT IN (SELECT s.task FROM task_workflow_subscribers_v1 s JOIN task_workflow_jobs_v1 j ON j.id=s.job WHERE s.tenant=?1 AND s.subject=?2 AND j.state IN ('running','cancel_requested'))"
        } else {
            ""
        };
        let native_guard = self.pending_native_guard()?;
        self.db.execute(&format!("DELETE FROM task_records_v1 WHERE tenant=?1 AND subject=?2 AND created<?3 AND id NOT IN (SELECT s.task_id FROM task_grounded_subscribers_v1 s JOIN task_grounded_jobs_v1 j ON j.job_id=s.job_id WHERE s.tenant=?1 AND s.subject=?2 AND j.state IN ('running','cancel_requested')){workflow_guard}{native_guard}"),params![scope.tenant,scope.subject,cutoff])?;
        self.db.execute("DELETE FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 AND key NOT IN (SELECT key FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 ORDER BY created DESC,key LIMIT ?3)", params![scope.tenant,scope.subject,policy.max_cache_entries as i64])?;
        Ok(())
    }
    fn pending_native_guard(&self) -> Result<&'static str> {
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='task_native_queue_v1')",
            [],
            |r| r.get(0),
        )?;
        Ok(if exists {
            " AND id NOT IN (SELECT task FROM task_native_queue_v1 n WHERE n.tenant=?1 AND n.subject=?2 AND n.state IN ('queued','claimed','running','cancel_requested'))"
        } else {
            ""
        })
    }
    pub(super) fn record(
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
        let workflow_tables:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_workflow_subscribers_v1')",[],|r|r.get(0))?;
        let keep_workflow = if workflow_tables {
            " AND id NOT IN (SELECT s.task FROM task_workflow_subscribers_v1 s JOIN task_workflow_jobs_v1 j ON j.id=s.job WHERE s.tenant=?1 AND s.subject=?2 AND j.state IN ('running','cancel_requested'))"
        } else {
            ""
        };
        let native_guard = self.pending_native_guard()?;
        self.db.execute(&format!("DELETE FROM task_records_v1 WHERE tenant=?1 AND subject=?2 AND id NOT IN (SELECT id FROM task_records_v1 WHERE tenant=?1 AND subject=?2 ORDER BY created DESC,id LIMIT 1024) AND id NOT IN (SELECT s.task_id FROM task_grounded_subscribers_v1 s JOIN task_grounded_jobs_v1 j ON j.job_id=s.job_id WHERE s.tenant=?1 AND s.subject=?2 AND j.state IN ('running','cancel_requested')){keep_workflow}{native_guard}"), params![scope.tenant,scope.subject])?;
        Ok(())
    }
    /// Trusted adapter supplies the currently authorized capture workspace,
    /// independently of ordinary corpus dependency roots.
    pub fn workspace_status_root(&self, root: std::path::PathBuf) {
        *self.capture_root.borrow_mut() = Some(root);
    }
    pub fn status(
        &self,
        scope: &AccessScope,
        task_id: Uuid,
        now: u64,
    ) -> Result<Option<TaskResult>> {
        self.require_dependencies(scope)?;
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
                !self.capture_root.borrow().as_ref().is_some_and(|root| {
                    self.workspace_evidence_current(scope, e, root, now)
                        .unwrap_or(false)
                }) && !sources.iter().any(|(source, _)| {
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
    pub fn dependency_paths(&self, scope: &AccessScope) -> Result<BTreeSet<String>> {
        Ok(self
            .sources(scope)?
            .into_iter()
            .flat_map(|(s, _)| s.dependencies.into_keys())
            .collect())
    }
    /// Trusted host refresh, with the same descriptor-relative file boundary as
    /// existing native tools. No source file bytes are returned or persisted.
    pub fn refresh_dependencies(
        &self,
        scope: &AccessScope,
        root: &std::path::Path,
    ) -> Result<Vec<String>> {
        *self.dependency_root.borrow_mut() = Some(root.to_path_buf());
        let sources = self.sources(scope)?;
        let paths = self.dependency_paths(scope)?;
        ensure!(paths.len() <= 64, "dependency validation capacity exceeded");
        let mut hashes = std::collections::BTreeMap::new();
        if !paths.is_empty() {
            let workspace = crate::workspace::open(root, false)?;
            for path in paths {
                hashes.insert(
                    path.clone(),
                    workspace
                        .read_bytes_complete(&path, 1024 * 1024)
                        .ok()
                        .map(|bytes| crate::digest(&bytes)),
                );
            }
        }
        let mut invalidated = vec![];
        self.validated_dependencies.borrow_mut().clear();
        for (source, hash) in sources {
            if source
                .dependencies
                .iter()
                .any(|(path, expected)| hashes.get(path).and_then(|h| h.as_ref()) != Some(expected))
            {
                self.delete_source(scope, &source.source_id)?;
                invalidated.push(source.source_id);
            } else {
                self.validated_dependencies.borrow_mut().insert(hash);
            }
        }
        Ok(invalidated)
    }
    pub(super) fn require_dependencies(&self, scope: &AccessScope) -> Result<()> {
        if self
            .sources(scope)?
            .iter()
            .any(|(s, _)| !s.dependencies.is_empty())
        {
            let root = self
                .dependency_root
                .borrow()
                .clone()
                .context("source file dependencies require trusted host workspace root")?;
            self.refresh_dependencies(scope, &root)?;
        }
        ensure!(
            self.sources(scope)?
                .iter()
                .all(|(s, h)| s.dependencies.is_empty()
                    || self.validated_dependencies.borrow().contains(h)),
            "source file dependencies require a current trusted-host refresh"
        );
        Ok(())
    }
    pub fn inspect_cache(&self, scope: &AccessScope) -> Result<serde_json::Value> {
        let (entries, bytes): (i64,i64) = self.db.query_row("SELECT COUNT(*),COALESCE(SUM(length(CAST(body AS BLOB))),0) FROM task_answers_v1 WHERE tenant=?1 AND subject=?2", params![scope.tenant,scope.subject], |r| Ok((r.get(0)?,r.get(1)?)))?;
        let (drafts,active,draft_bytes):(i64,i64,i64)=self.db.query_row("SELECT COUNT(*),COALESCE(SUM(state!='completed'),0),COALESCE(SUM(length(CAST(body AS BLOB))+COALESCE(length(CAST(result AS BLOB)),0)),0) FROM task_grounded_jobs_v1 WHERE tenant=?1 AND subject=?2",params![scope.tenant,scope.subject],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let mut components = Vec::new();
        for table in [
            "task_semantic_v1",
            "task_embeddings_v1",
            "task_retrieval_cache_v1",
            "task_workflow_jobs_v1",
            "task_workflow_plan_revisions_v1",
            "task_workspace_evidence_v1",
            "task_native_queue_v1",
        ] {
            let exists: bool = self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )?;
            if exists {
                let (count,size):(i64,i64)=self.db.query_row(&format!("SELECT COUNT(*),COALESCE(SUM(length(CAST(body AS BLOB))),0) FROM {table} WHERE tenant=?1 AND subject=?2"),params![scope.tenant,scope.subject],|r|Ok((r.get(0)?,r.get(1)?)))?;
                components
                    .push(json!({"table":table,"records":count,"stored_body_json_bytes":size}));
            }
        }
        let decisions:Vec<(String,i64)>=self.db.prepare("SELECT json_extract(result,'$.cache.decision'),COUNT(*) FROM task_records_v1 WHERE tenant=?1 AND subject=?2 GROUP BY json_extract(result,'$.cache.decision')")?.query_map(params![scope.tenant,scope.subject],|r|Ok((r.get(0)?,r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        Ok(
            json!({"schema_version":VERSION,"exact_entries":entries,"stored_json_bytes":bytes,"grounded_draft_records":drafts,"grounded_active_or_uncertain":active,"grounded_stored_json_bytes":draft_bytes,"components":components,"retained_task_cache_decisions":decisions,"semantic_answers":self.semantic_capabilities(),"prefix_kv":"separate_backend_optimization_not_answer_cache"}),
        )
    }
    pub fn invalidate_cache(&self, scope: &AccessScope) -> Result<usize> {
        let workspace=self.db.execute("DELETE FROM task_workspace_evidence_v1 WHERE tenant=?1 AND subject=?2 AND task NOT IN (SELECT id FROM task_records_v1 WHERE tenant=?1 AND subject=?2 AND json_extract(result,'$.status')='running')",params![scope.tenant,scope.subject])?;
        let workflows:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_workflow_jobs_v1')",[],|r|r.get(0))?;
        if workflows {
            let revisions:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_workflow_plan_revisions_v1')",[],|r|r.get(0))?;
            if revisions {
                self.db.execute("DELETE FROM task_workflow_plan_revisions_v1 WHERE tenant=?1 AND subject=?2 AND job IN (SELECT id FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed')",params![scope.tenant,scope.subject])?;
            }
            self.db.execute("DELETE FROM task_workflow_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job IN (SELECT id FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed')",params![scope.tenant,scope.subject])?;
            self.db.execute("DELETE FROM task_workflow_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed'",params![scope.tenant,scope.subject])?;
        }
        super::workflows::prune_audit(self.db)?;
        self.db.execute(
            "DELETE FROM task_retrieval_cache_v1 WHERE tenant=?1 AND subject=?2",
            params![scope.tenant, scope.subject],
        )?;
        self.db.execute(
            "DELETE FROM task_semantic_v1 WHERE tenant=?1 AND subject=?2",
            params![scope.tenant, scope.subject],
        )?;
        self.db.execute(
            "DELETE FROM task_embeddings_v1 WHERE tenant=?1 AND subject=?2",
            params![scope.tenant, scope.subject],
        )?;
        self.db.execute("DELETE FROM task_grounded_subscribers_v1 WHERE tenant=?1 AND subject=?2 AND job_id IN (SELECT job_id FROM task_grounded_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed')",params![scope.tenant,scope.subject])?;
        let drafts=self.db.execute("DELETE FROM task_grounded_jobs_v1 WHERE tenant=?1 AND subject=?2 AND state='completed'",params![scope.tenant,scope.subject])?;
        Ok(self.db.execute(
            "DELETE FROM task_answers_v1 WHERE tenant=?1 AND subject=?2",
            params![scope.tenant, scope.subject],
        )? + drafts
            + workspace)
    }
    pub(super) fn answer_key(&self, input: &TaskInput, policy: &ExecutionPolicy) -> Result<String> {
        let sources = self.sources(&policy.scope)?;
        let snapshot: Vec<_> = sources.iter().map(|(s, h)| (&s.source_id, h)).collect();
        Ok(crate::digest(&serde_json::to_vec(
            &json!({"compatibility":COMPATIBILITY,"scope":policy.scope,"input":input,"source_snapshot":snapshot,"cache_ttl_ms":policy.cache_ttl_ms,"retention_ms":policy.retention_ms,"max_evidence_bytes":policy.max_evidence_bytes}),
        )?))
    }
    pub(super) fn current_evidence(
        &self,
        scope: &AccessScope,
        items: &[Evidence],
        max: usize,
        now: u64,
    ) -> Result<bool> {
        let sources = self.sources(scope)?;
        Ok(!items.is_empty()
            && items.iter().all(|e| {
                sources.iter().any(|(s, _)| {
                    s.source_id == e.source_id
                        && s.observed_at_ms <= now
                        && now < s.valid_until_ms
                        && evidence(s, e.start_byte, e.end_byte, max)
                            .is_ok_and(|current| current == *e)
                })
            }))
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
        routing_justification: "One exact owner-curated source binding is sufficient for verbatim extraction. No generator, planner or judge required.".into(), cache_decision: "exact_miss".into(),evidence_required:true,resolved_models:std::collections::BTreeMap::new() }
}

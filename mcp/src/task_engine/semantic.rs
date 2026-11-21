//! Fail-closed semantic answer reuse. CPU feature similarity only selects an
//! owner-reviewed equivalence; similarity alone never establishes equivalence.
//! This intentionally narrow first provider needs no downloaded embedding model.
use super::{contracts::*, Repository};
use anyhow::{ensure, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const EMBEDDING_ID: &str = "cpu-word-features/v1/dim256";
pub const DIMENSION: usize = 256;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Equivalence {
    pub canonical: TaskInput,
    pub paraphrase: TaskInput,
    pub reviewed_by: String,
    pub expires_at_ms: u64,
}
fn only_ask(input: &TaskInput) -> TaskInput {
    let mut i = input.clone();
    i.original_ask.clear();
    i
}
/// Conservative lexical invariants supplement explicit owner review. Unknown
/// wording still misses. Review cannot waive year/negation/unit/election guards.
fn meaning_guards(text: &str) -> std::collections::BTreeSet<String> {
    let mut guards = std::collections::BTreeSet::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
    {
        let lower = word.to_lowercase();
        let proper_name = word.chars().next().is_some_and(char::is_uppercase)
            && !matches!(
                lower.as_str(),
                "who"
                    | "name"
                    | "what"
                    | "which"
                    | "tell"
                    | "identify"
                    | "give"
                    | "list"
                    | "please"
                    | "return"
                    | "the"
                    | "a"
                    | "an"
            );
        if word.chars().any(|c| c.is_ascii_digit())
            || matches!(
                lower.as_str(),
                "not"
                    | "never"
                    | "no"
                    | "except"
                    | "without"
                    | "current"
                    | "today"
                    | "latest"
                    | "now"
                    | "election"
                    | "won"
                    | "winner"
                    | "inauguration"
                    | "kg"
                    | "kilograms"
                    | "lb"
                    | "pounds"
                    | "km"
                    | "miles"
                    | "celsius"
                    | "fahrenheit"
            )
            || proper_name
            || word.chars().all(|c| c.is_uppercase()) && word.len() > 1
        {
            guards.insert(lower);
        }
    }
    guards
}
pub(super) fn install(repo: &Repository<'_>) -> Result<()> {
    repo.db.execute_batch("CREATE TABLE IF NOT EXISTS task_semantic_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,scope TEXT NOT NULL,alias TEXT NOT NULL,body TEXT NOT NULL,sha256 TEXT NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(tenant,subject,scope,alias));
        CREATE TABLE IF NOT EXISTS task_embeddings_v1(tenant TEXT NOT NULL,subject TEXT NOT NULL,identity TEXT NOT NULL,content_sha TEXT NOT NULL,body TEXT NOT NULL,created INTEGER NOT NULL,PRIMARY KEY(tenant,subject,identity,content_sha));")?;
    Ok(())
}
fn scope_key(scope: &AccessScope) -> Result<String> {
    Ok(crate::digest(&serde_json::to_vec(scope)?))
}
fn embedding(text: &str) -> Vec<f64> {
    let mut out = vec![0.; DIMENSION];
    for term in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        let hash = crate::digest(term.to_lowercase().as_bytes());
        let index = usize::from_str_radix(&hash[..4], 16).unwrap() % DIMENSION;
        out[index] += 1.;
    }
    let norm = out.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 0. {
        for v in &mut out {
            *v /= norm;
        }
    }
    out
}
/// Returns feature computation count separately from generator calls. Identity
/// and dimension are part of storage compatibility, never inferred from length.
pub fn embed_cached(
    repo: &Repository<'_>,
    scope: &AccessScope,
    text: &str,
    now: u64,
) -> Result<(Vec<f64>, u32)> {
    ensure!(text.len() <= 49152, "bounded embedding input required");
    let hash = crate::digest(text.as_bytes());
    let old:Option<String>=repo.db.query_row("SELECT body FROM task_embeddings_v1 WHERE tenant=?1 AND subject=?2 AND identity=?3 AND content_sha=?4",params![scope.tenant,scope.subject,EMBEDDING_ID,hash],|r|r.get(0)).optional()?;
    if let Some(body) = old {
        let vector: Vec<f64> = serde_json::from_str(&body)?;
        ensure!(
            vector.len() == DIMENSION && vector.iter().all(|v| v.is_finite()),
            "incompatible/corrupt embeddings"
        );
        return Ok((vector, 0));
    }
    let vector = embedding(text);
    repo.db.execute(
        "INSERT OR REPLACE INTO task_embeddings_v1 VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            scope.tenant,
            scope.subject,
            EMBEDDING_ID,
            hash,
            serde_json::to_string(&vector)?,
            i64::try_from(now)?
        ],
    )?;
    repo.db.execute("DELETE FROM task_embeddings_v1 WHERE tenant=?1 AND subject=?2 AND (identity,content_sha) NOT IN (SELECT identity,content_sha FROM task_embeddings_v1 WHERE tenant=?1 AND subject=?2 ORDER BY created DESC,identity,content_sha LIMIT 256)",params![scope.tenant,scope.subject])?;
    Ok((vector, 1))
}
impl Repository<'_> {
    /// This is a trusted-owner operation, never called by a generated proposal.
    pub fn approve_equivalence(
        &self,
        scope: &AccessScope,
        rule: &Equivalence,
        now: u64,
    ) -> Result<()> {
        TaskRequest::from_input(rule.canonical.clone())?;
        TaskRequest::from_input(rule.paraphrase.clone())?;
        ensure!(
            only_ask(&rule.canonical) == only_ask(&rule.paraphrase)
                && rule.canonical.original_ask != rule.paraphrase.original_ask,
            "equivalence may change only wording; dates/context/output/constraints are immutable"
        );
        ensure!(
            meaning_guards(&rule.canonical.original_ask)
                == meaning_guards(&rule.paraphrase.original_ask),
            "semantic date/entity/negation/unit/intent guards differ; prefer miss"
        );
        ensure!(
            rule.canonical.intent == Intent::Answer
                && rule.canonical.constraints.is_empty()
                && rule.canonical.supplied_context.is_empty()
                && rule.canonical.proposed_acceptance.is_empty()
                && rule.canonical.deliverable.is_empty()
                && rule.canonical.extraction.is_none(),
            "semantic reuse requires simple context-independent answer"
        );
        ensure!(
            !rule.reviewed_by.is_empty()
                && rule.reviewed_by == scope.subject
                && rule.expires_at_ms > now
                && rule.expires_at_ms <= now.saturating_add(86400000),
            "current owner review and bounded TTL required"
        );
        let count: i64 = self.db.query_row(
            "SELECT COUNT(*) FROM task_semantic_v1 WHERE tenant=?1 AND subject=?2",
            params![scope.tenant, scope.subject],
            |r| r.get(0),
        )?;
        ensure!(
            count < 256,
            "semantic review capacity reached; invalidate obsolete rules"
        );
        let body = serde_json::to_string(rule)?;
        crate::safety::require_persistable(&rule.reviewed_by)?;
        self.db.execute(
            "INSERT OR REPLACE INTO task_semantic_v1 VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                scope.tenant,
                scope.subject,
                scope_key(scope)?,
                crate::digest(&serde_json::to_vec(&rule.paraphrase)?),
                body,
                crate::digest(body.as_bytes()),
                i64::try_from(rule.expires_at_ms)?
            ],
        )?;
        embed_cached(self, scope, &rule.paraphrase.original_ask, now)?;
        Ok(())
    }
    pub(super) fn semantic_hit(
        &self,
        request: &TaskRequest,
        policy: &ExecutionPolicy,
        now: u64,
    ) -> Result<Option<TaskResult>> {
        let scope = &policy.scope;
        self.db.execute(
            "DELETE FROM task_semantic_v1 WHERE tenant=?1 AND subject=?2 AND expires<=?3",
            params![scope.tenant, scope.subject, now as i64],
        )?;
        // Scope AND complete reviewed wording match precede vector/text access.
        // Unknown paraphrases deliberately miss, even at cosine 1.0.
        let row:Option<(String,String)>=self.db.query_row("SELECT body,sha256 FROM task_semantic_v1 WHERE tenant=?1 AND subject=?2 AND scope=?3 AND alias=?4 AND expires>?5",params![scope.tenant,scope.subject,scope_key(scope)?,crate::digest(&serde_json::to_vec(&request.input)?),now as i64],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((body, hash)) = row else {
            return Ok(None);
        };
        ensure!(
            crate::digest(body.as_bytes()) == hash,
            "semantic rule integrity mismatch"
        );
        let rule: Equivalence = serde_json::from_str(&body)?;
        ensure!(
            rule.paraphrase == request.input,
            "semantic hash/input mismatch"
        );
        let (query, calls) = embed_cached(self, scope, &request.original_ask, now)?;
        let (candidate, more) = embed_cached(self, scope, &rule.paraphrase.original_ask, now)?;
        let similarity: f64 = query.iter().zip(&candidate).map(|(a, b)| a * b).sum();
        if similarity < 0.999999 {
            return Ok(None);
        }
        let key = self.answer_key(&rule.canonical, policy)?;
        let row:Option<(String,String,i64,i64)>=self.db.query_row("SELECT body,sha256,created,expires FROM task_answers_v1 WHERE tenant=?1 AND subject=?2 AND key=?3 AND expires>?4",params![scope.tenant,scope.subject,key,now as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((body, hash, created, expires)) = row else {
            return Ok(None);
        };
        ensure!(
            crate::digest(body.as_bytes()) == hash,
            "semantic target cache integrity mismatch"
        );
        let mut result: TaskResult = serde_json::from_str(&body)?;
        if result.verification_state != "exact_source_supported_extraction"
            || !self.current_evidence(scope, &result.evidence, policy.max_evidence_bytes, now)?
        {
            return Ok(None);
        }
        let origin = result.task_id;
        result.task_id = request.task_id;
        result.actual_path = "semantic_answer_cache".into();
        result.cache=CacheProvenance{decision:"hit_owner_reviewed_equivalence_current_scope_sources".into(),key,origin_task_id:Some(origin),created_at_ms:Some(created as u64),expires_at_ms:Some((expires as u64).min(rule.expires_at_ms)),work_avoided:"Repeated source extraction avoided; feature-embedding work counted. No generator, GPU or dollar savings inferred.".into()};
        result.measurements.embedding_calls = calls + more;
        result.measurements.generative_model_calls = Some(0);
        result.measurements.tool_calls = 0;
        result.measurements.input_tokens = Some(0);
        result.measurements.output_tokens = Some(0);
        result.measurements.elapsed_micros = None;
        result.measurements.memory_bytes = None;
        if let Some(plan) = &mut result.plan {
            plan.task_id = request.task_id;
            plan.plan_id = uuid::Uuid::new_v4();
            plan.cache_decision = "reviewed_semantic_hit".into();
        }
        Ok(Some(result))
    }
    pub fn semantic_capabilities(&self) -> serde_json::Value {
        json!({"identity":EMBEDDING_ID,"dimension":DIMENSION,"provider":"CPU sparse word feature projection; not a learned neural embedding","eligibility":"owner-reviewed exact paraphrase pair plus complete scope/context/output/source/TTL checks","unknown_paraphrases":"miss","threshold":0.999999,"similarity_alone_sufficient":false})
    }
}

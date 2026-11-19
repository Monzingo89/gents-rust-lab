//! Fixed evidence fixtures. Historical answers are data here, never router code.
use gents_local_mcp::task_engine::{contracts::*, KnowledgeSource, LookupBinding, Repository};
use rusqlite::Connection;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const ASK: &str = "Who was US president in 1976?";
const NOW: u64 = 1000;
fn scope() -> AccessScope {
    AccessScope {
        tenant: "one".into(),
        subject: "alice".into(),
        effective_access_sha256: "owner-v1".into(),
        policy_revision: "policy-v1".into(),
        profile_revision: "history-v1".into(),
        runtime_revision: "runtime-v1".into(),
        conversation_sha256: "context-independent".into(),
        allowed_sources: BTreeSet::from(["history".into()]),
    }
}
fn input(ask: &str) -> TaskInput {
    TaskInput {
        original_ask: ask.into(),
        supplied_context: BTreeMap::new(),
        constraints: vec![],
        deliverable: "".into(),
        output: OutputSpec {
            format: "text".into(),
            language: "en".into(),
        },
        temporal_scope: "1976".into(),
        proposed_acceptance: vec![],
        intent: Intent::Answer,
        extraction: None,
    }
}
fn source() -> KnowledgeSource {
    let text = "Gerald Ford served as US president during 1976.\nJimmy Carter won the 1976 election and took office in 1977.\n";
    KnowledgeSource {
        source_id: "history".into(),
        revision: "fixture-v1".into(),
        title: "Synthetic chronology fixture".into(),
        domain: "history".into(),
        language: "en".into(),
        text: text.into(),
        observed_at_ms: 900,
        valid_until_ms: 100000,
        lookups: vec![LookupBinding {
            exact_ask: ASK.into(),
            temporal_scope: "1976".into(),
            context_independent: true,
            start_byte: 0,
            end_byte: "Gerald Ford".len(),
        }],
    }
}
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    db
}
fn execute(
    repo: &Repository<'_>,
    input: TaskInput,
    policy: &ExecutionPolicy,
    now: u64,
) -> TaskResult {
    repo.execute(&TaskRequest::from_input(input).unwrap(), policy, now)
        .unwrap()
}

#[test]
fn first_lookup_then_exact_cache_retains_source_support_and_never_generates() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let policy = ExecutionPolicy::read_only(scope(), NOW);
    repo.ingest(&policy.scope, &source(), NOW).unwrap();
    let first = execute(&repo, input(ASK), &policy, NOW);
    let second = execute(&repo, input(ASK), &policy, NOW + 1);
    assert_eq!(first.actual_path, "deterministic_lookup");
    assert_eq!(second.actual_path, "exact_answer_cache");
    assert_eq!(second.answer.as_deref(), Some("Gerald Ford"));
    assert_eq!(first.evidence, second.evidence);
    assert_eq!(second.cache.origin_task_id, Some(first.task_id));
    assert_ne!(first.task_id, second.task_id);
    assert_eq!(second.measurements.generative_model_calls, Some(0));
    assert_eq!(second.measurements.embedding_calls, 0);
    assert_eq!(
        second.verification_state,
        "exact_source_supported_extraction"
    );
    assert_eq!(second.acceptance_checks[1].status, CheckStatus::NotRun);
    let stored = repo
        .status(&policy.scope, second.task_id, NOW + 1)
        .unwrap()
        .unwrap();
    assert_eq!(stored.answer, second.answer);
}

#[test]
fn exact_cache_hard_negatives_never_inherit_the_1976_binding() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let policy = ExecutionPolicy::read_only(scope(), NOW);
    repo.ingest(&policy.scope, &source(), NOW).unwrap();
    execute(&repo, input(ASK), &policy, NOW);
    let cases = [
        "Who was US president in 1974?",
        "Who was US president in 1977?",
        "Who won the 1976 US election?",
        "Who was the current US president?",
        "Who was president of France in 1976?",
        "Who was not US president in 1976?",
        "Who served after the 1976 election?",
        "Who was elected US president in 1976?",
        "Was Carter president during 1976?",
        "Who preceded Gerald Ford?",
        "Who followed Gerald Ford?",
        "Was Ford elected president in 1976?",
        "How many days was Ford president in 1976?",
        "How many months was Ford president in 1976?",
        "Who was US vice president in 1976?",
        "Who was UK prime minister in 1976?",
        "Who served in January 1977?",
        "Who served in December 1976?",
        "Who won the 1974 election?",
        "Who was president in 1776?",
        "Who is president now?",
        "Who was president of Mexico in 1976?",
        "Who was US president before 1976?",
        "Who was US president after 1976?",
        "Who was US president in 1976 and 1977?",
        "Who was president in 1976, excluding Ford?",
        "Who wasn't president in 1976?",
        "What was Ford's term?",
        "Delete the 1976 source file.",
        "Who held the US presidency during 1976?",
        "Who was US president in 1976? ",
        "WHO WAS US PRESIDENT IN 1976?",
    ];
    let mut false_hits = 0;
    for ask in cases {
        let result = execute(&repo, input(ask), &policy, NOW + 1);
        false_hits += usize::from(result.actual_path == "exact_answer_cache");
        assert_eq!(result.status, "requires_planning", "{ask}");
        assert!(result.answer.is_none(), "{ask}");
    }
    assert_eq!(false_hits, 0);
    // A genuine paraphrase currently misses: semantic reuse is not pretended.
}

#[test]
fn scope_revisions_partition_cache_and_revoked_access_cannot_read_old_status() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let original = ExecutionPolicy::read_only(scope(), NOW);
    repo.ingest(&original.scope, &source(), NOW).unwrap();
    let first = execute(&repo, input(ASK), &original, NOW);
    for index in 0..8 {
        let mut changed = original.clone();
        match index {
            0 => changed.scope.tenant = "two".into(),
            1 => changed.scope.subject = "bob".into(),
            2 => changed.scope.effective_access_sha256 = "access-v2".into(),
            3 => changed.scope.policy_revision = "policy-v2".into(),
            4 => changed.scope.profile_revision = "history-v2".into(),
            5 => changed.scope.runtime_revision = "runtime-v2".into(),
            6 => changed.scope.conversation_sha256 = "different-context".into(),
            _ => changed.scope.allowed_sources.clear(),
        }
        let result = execute(&repo, input(ASK), &changed, NOW + 1);
        assert_ne!(
            result.actual_path, "exact_answer_cache",
            "scope dimension {index}"
        );
        if index < 2 || index == 7 {
            assert!(result.answer.is_none());
        }
    }
    let mut revoked = original.scope.clone();
    revoked.allowed_sources.clear();
    let redacted = repo
        .status(&revoked, first.task_id, NOW + 1)
        .unwrap()
        .unwrap();
    assert!(redacted.answer.is_none() && redacted.evidence.is_empty());
    assert_eq!(redacted.status, "evidence_invalidated");
    let mut other = original.scope.clone();
    other.subject = "bob".into();
    assert!(repo
        .status(&other, first.task_id, NOW + 1)
        .unwrap()
        .is_none());
}

#[test]
fn caller_context_dates_outputs_actions_and_constraints_are_not_silently_discarded() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let policy = ExecutionPolicy::read_only(scope(), NOW);
    repo.ingest(&policy.scope, &source(), NOW).unwrap();
    execute(&repo, input(ASK), &policy, NOW);
    for index in 0..8 {
        let mut changed = input(ASK);
        match index {
            0 => changed
                .supplied_context
                .insert("previous-turn".into(), "he means Carter".into()),
            1 => {
                changed.temporal_scope = "current".into();
                None
            }
            2 => {
                changed.output.language = "fr".into();
                None
            }
            3 => {
                changed.intent = Intent::Action;
                None
            }
            4 => {
                changed
                    .constraints
                    .push("Use two independent sources".into());
                None
            }
            5 => {
                changed.proposed_acceptance.push("Answer in a poem".into());
                None
            }
            6 => {
                changed.deliverable = "Signed report".into();
                None
            }
            _ => {
                changed.output.format = "json".into();
                None
            }
        };
        let result = execute(&repo, changed, &policy, NOW + 1);
        assert_ne!(result.actual_path, "exact_answer_cache");
        if index == 7 {
            assert_eq!(result.answer.as_deref(), Some("\"Gerald Ford\""));
        } else {
            assert_eq!(result.status, "requires_planning");
        }
    }
}

#[test]
fn changed_deleted_expired_sources_and_clock_reversal_never_serve_stale_cache() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let mut policy = ExecutionPolicy::read_only(scope(), NOW);
    policy.cache_ttl_ms = 2;
    repo.ingest(&policy.scope, &source(), NOW).unwrap();
    let first = execute(&repo, input(ASK), &policy, NOW);
    let reversed = execute(&repo, input(ASK), &policy, NOW - 1);
    assert_ne!(reversed.actual_path, "exact_answer_cache");
    let expired = execute(&repo, input(ASK), &policy, NOW + 3);
    assert_eq!(expired.actual_path, "deterministic_lookup");
    let mut corrected = source();
    corrected.revision = "fixture-v2".into();
    corrected.text = corrected.text.replace("Gerald Ford", "Corrected!!");
    repo.ingest(&policy.scope, &corrected, NOW + 3).unwrap();
    let current = execute(&repo, input(ASK), &policy, NOW + 4);
    assert_eq!(current.actual_path, "deterministic_lookup");
    assert_eq!(current.answer.as_deref(), Some("Corrected!!"));
    assert!(repo
        .status(&policy.scope, first.task_id, NOW + 4)
        .unwrap()
        .unwrap()
        .answer
        .is_none());
    repo.delete_source(&policy.scope, "history").unwrap();
    assert!(execute(&repo, input(ASK), &policy, NOW + 4)
        .answer
        .is_none());
}

#[test]
fn expired_evidence_search_extraction_conflicts_and_exact_utf8_coordinates() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let mut policy = ExecutionPolicy::read_only(scope(), NOW);
    let mut a = source();
    a.text = "αβ\nA source about Carter and election dates.\n".into();
    a.lookups.clear();
    repo.ingest(&policy.scope, &a, NOW).unwrap();
    let hits = repo
        .search(&policy.scope, "Carter election", Some("history"), 100, NOW)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].evidence.start_byte, 5);
    assert_eq!(hits[0].score, 2);
    assert!(repo
        .search(&policy.scope, "Carter", Some("angular"), 100, NOW)
        .unwrap()
        .is_empty());
    assert!(repo
        .search(&policy.scope, "Carter", None, 100, 100000)
        .unwrap()
        .is_empty());
    let mut request = input("Extract the source's first line");
    request.extraction = Some(Extraction {
        source_id: "history".into(),
        start_byte: 0,
        end_byte: 4,
    });
    let result = execute(&repo, request.clone(), &policy, NOW);
    assert_eq!(result.answer.as_deref(), Some("αβ"));
    request.extraction.as_mut().unwrap().start_byte = 1;
    assert!(repo
        .execute(&TaskRequest::from_input(request).unwrap(), &policy, NOW)
        .is_err());
    let mut second = source();
    second.source_id = "another".into();
    let mut restored = source();
    restored.revision = "fixture-v2".into();
    repo.ingest(&policy.scope, &restored, NOW).unwrap();
    repo.ingest(&policy.scope, &second, NOW).unwrap();
    policy.scope.allowed_sources.insert("another".into());
    let conflicting = execute(&repo, input(ASK), &policy, NOW);
    assert_eq!(conflicting.status, "requires_planning");
    assert!(conflicting.answer.is_none());
}

#[test]
fn cache_eviction_expiry_invalidation_and_changed_revision_requirement() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let mut policy = ExecutionPolicy::read_only(scope(), NOW);
    policy.max_cache_entries = 1;
    repo.ingest(&policy.scope, &source(), NOW).unwrap();
    execute(&repo, input(ASK), &policy, NOW);
    let mut json_input = input(ASK);
    json_input.output.format = "json".into();
    execute(&repo, json_input, &policy, NOW + 1);
    assert_eq!(
        repo.inspect_cache(&policy.scope).unwrap()["exact_entries"],
        1
    );
    assert_eq!(
        execute(&repo, input(ASK), &policy, NOW + 2).actual_path,
        "deterministic_lookup"
    );
    assert_eq!(repo.invalidate_cache(&policy.scope).unwrap(), 1);
    assert_eq!(
        repo.inspect_cache(&policy.scope).unwrap()["exact_entries"],
        0
    );
    let mut changed = source();
    changed.title = "changed".into();
    assert!(repo.ingest(&policy.scope, &changed, NOW).is_err());
}

#[test]
fn strict_requests_original_ask_and_transaction_preconditions() {
    let db = Connection::open_in_memory().unwrap();
    assert!(Repository::new(&db).is_err());
    assert!(
        serde_json::from_value::<TaskInput>(json!({"original_ask":ASK,"tools":["shell"]})).is_err()
    );
    let mut request = TaskRequest::from_input(input(ASK)).unwrap();
    request.objective = "different".into();
    assert!(request.validate().is_err());
    request.objective = request.original_ask.clone();
    request.schema_version = 2;
    assert!(request.validate().is_err());
    let mut bad = input(ASK);
    bad.output.format = "xml".into();
    assert!(TaskRequest::from_input(bad).is_err());
    for index in 0..4 {
        let mut bad = input(ASK);
        match index {
            0 => bad.proposed_acceptance = vec![String::new()],
            1 => bad.proposed_acceptance = vec!["criterion".into(); 9],
            2 => bad.constraints = vec!["α".repeat(1001)],
            _ => {
                bad.supplied_context
                    .insert("unbounded".into(), "x".repeat(8001));
            }
        }
        assert!(TaskRequest::from_input(bad).is_err());
    }
}

#[test]
fn cache_corruption_is_rejected_instead_of_promoted_to_answer() {
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let policy = ExecutionPolicy::read_only(scope(), NOW);
    repo.ingest(&policy.scope, &source(), NOW).unwrap();
    execute(&repo, input(ASK), &policy, NOW);
    db.execute("UPDATE task_answers_v1 SET body='{}'", [])
        .unwrap();
    assert!(repo
        .execute(
            &TaskRequest::from_input(input(ASK)).unwrap(),
            &policy,
            NOW + 1
        )
        .is_err());
}

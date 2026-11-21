//! Real confined filesystem observations; no model or build success is simulated.
use gents_local_mcp::task_engine::{
    contracts::*, read_tools, routing, workflows::ToolRequest, Repository,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf};
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!("gents-task-workspace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&p).unwrap();
    p.canonicalize().unwrap()
}
struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn request() -> TaskRequest {
    TaskRequest::from_input(
        serde_json::from_value(json!({"original_ask":"Inspect actual project versions"})).unwrap(),
    )
    .unwrap()
}
fn scope() -> AccessScope {
    AccessScope {
        tenant: "one".into(),
        subject: "owner".into(),
        effective_access_sha256: "a1".into(),
        policy_revision: "p1".into(),
        profile_revision: "profile1".into(),
        runtime_revision: "r1".into(),
        conversation_sha256: "c1".into(),
        allowed_sources: BTreeSet::new(),
    }
}
fn policy(root: &std::path::Path, now: u64) -> ExecutionPolicy {
    let mut p = ExecutionPolicy::read_only(scope(), now);
    p.permitted_tools
        .extend(["workspace_read".into(), "workspace_list".into()]);
    p.workspace_roots = vec![root.to_str().unwrap().to_owned()];
    p
}
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    db
}
fn read(path: &str, offset: usize, limit: usize) -> ToolRequest {
    ToolRequest {
        name: "workspace_read".into(),
        arguments: json!({"path":path,"offset_bytes":offset,"limit_bytes":limit}),
    }
}
fn evidence(value: &Value) -> Evidence {
    serde_json::from_value(value["evidence"][0].clone()).unwrap()
}
#[test]
fn task_file_observation_is_not_global_ingestion_and_changed_tail_invalidates() {
    let root = Directory(root());
    std::fs::write(
        root.0.join("package.json"),
        "{\n  \"version\": \"20.3.0\"\n}\n",
    )
    .unwrap();
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let p = policy(&root.0, now);
    let r = request();
    let mut result = repo.execute(&r, &p, now).unwrap();
    let value = repo
        .capture_workspace(&r, &p, &root.0, &read("package.json", 0, 8), now)
        .unwrap();
    let e = evidence(&value);
    assert_eq!(e.quote, "{\n  \"ver");
    assert_eq!(
        e.content_sha256,
        gents_local_mcp::digest(
            std::fs::read(root.0.join("package.json"))
                .unwrap()
                .as_slice()
        )
    );
    assert!(repo.source_ids("one", "owner").unwrap().is_empty());
    assert!(repo
        .search(&p.scope, "version", None, 8192, now)
        .unwrap()
        .is_empty());
    result.status = "generated".into();
    result.answer = Some("draft".into());
    result.evidence = vec![e.clone()];
    repo.replace_result(&p.scope, &result).unwrap();
    repo.refresh_dependencies(&p.scope, &root.0).unwrap();
    repo.workspace_status_root(root.0.clone());
    assert_eq!(
        repo.status(&p.scope, r.task_id, now)
            .unwrap()
            .unwrap()
            .answer
            .as_deref(),
        Some("draft")
    );
    std::fs::write(
        root.0.join("package.json"),
        "{\n  \"version\": \"21.0.0\"\n}\n",
    )
    .unwrap();
    assert!(!repo
        .workspace_evidence_current(&p.scope, &e, &root.0, now)
        .unwrap());
    assert!(!repo
        .workspace_task_current(&p.scope, r.task_id, &root.0, now)
        .unwrap());
    let invalid = repo.status(&p.scope, r.task_id, now).unwrap().unwrap();
    assert_eq!(invalid.status, "evidence_invalidated");
    assert!(invalid.answer.is_none());
}
#[test]
fn ttl_scope_root_and_forged_quote_are_checked_before_reuse() {
    let root = Directory(root());
    let other = Directory(self::root());
    for folder in [&root.0, &other.0] {
        std::fs::write(folder.join("file.txt"), "actual evidence").unwrap();
    }
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let p = policy(&root.0, now);
    let r = request();
    let e = evidence(
        &repo
            .capture_workspace(&r, &p, &root.0, &read("file.txt", 0, 100), now)
            .unwrap(),
    );
    let again = evidence(
        &repo
            .capture_workspace(&r, &p, &root.0, &read("file.txt", 0, 100), now + 10)
            .unwrap(),
    );
    assert_eq!(e, again);
    for kind in 0..7 {
        let mut changed = scope();
        match kind {
            0 => changed.tenant = "two".into(),
            1 => changed.subject = "other".into(),
            2 => changed.policy_revision = "p2".into(),
            3 => changed.runtime_revision = "r2".into(),
            4 => changed.profile_revision = "r2".into(),
            5 => changed.conversation_sha256 = "c2".into(),
            _ => changed.effective_access_sha256 = "a2".into(),
        };
        assert!(!repo
            .workspace_evidence_current(&changed, &e, &root.0, now)
            .unwrap());
    }
    assert!(!repo
        .workspace_evidence_current(&p.scope, &e, &other.0, now)
        .unwrap());
    assert!(!repo
        .workspace_evidence_current(&p.scope, &e, &root.0, e.valid_until_ms)
        .unwrap());
    let mut forged = e;
    forged.quote = "claimed build passed".into();
    assert!(!repo
        .workspace_evidence_current(&p.scope, &forged, &root.0, now)
        .unwrap());
}
#[test]
fn workspace_reads_require_host_grants_and_confined_paths() {
    let root = Directory(root());
    std::fs::write(root.0.join("file.txt"), "actual").unwrap();
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let r = request();
    let mut p = policy(&root.0, now);
    p.permitted_tools.remove("workspace_read");
    assert!(repo
        .capture_workspace(&r, &p, &root.0, &read("file.txt", 0, 50), now)
        .is_err());
    p = policy(&root.0, now);
    p.workspace_roots.clear();
    assert!(repo
        .capture_workspace(&r, &p, &root.0, &read("file.txt", 0, 50), now)
        .is_err());
    for path in [
        "../file.txt",
        "/etc/passwd",
        ".env",
        "hidden/.private",
        "file.txt/../secret",
    ] {
        assert!(read_tools::validate(&read(path, 0, 50)).is_err());
    }
    let bad = ToolRequest {
        name: "workspace_read".into(),
        arguments: json!({"path":"file.txt","offset_bytes":0,"limit_bytes":100,"root":"/"}),
    };
    assert!(read_tools::validate(&bad).is_err());
}
#[test]
fn directory_observation_is_bounded_and_changed_listing_invalidates() {
    let root = Directory(root());
    for i in 0..90 {
        std::fs::write(root.0.join(format!("{i:03}-{}.txt", "n".repeat(50))), "a").unwrap();
    }
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let p = policy(&root.0, now);
    let tool = ToolRequest {
        name: "workspace_list".into(),
        arguments: json!({"path":""}),
    };
    let v = repo
        .capture_workspace(&request(), &p, &root.0, &tool, now)
        .unwrap();
    let e = evidence(&v);
    assert!(e.quote.len() <= 4096);
    assert_eq!(v["receipt"]["truncated"], true);
    assert!(repo
        .workspace_evidence_current(&p.scope, &e, &root.0, now)
        .unwrap());
    std::fs::write(root.0.join("000-new.txt"), "new").unwrap();
    assert!(!repo
        .workspace_evidence_current(&p.scope, &e, &root.0, now)
        .unwrap());
}
#[cfg(unix)]
#[test]
fn symlinks_hardlinks_protected_content_and_invalid_utf8_do_not_become_evidence() {
    let root = Directory(root());
    let outside = Directory(self::root());
    std::fs::write(outside.0.join("secret.txt"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.0.join("secret.txt"), root.0.join("alias.txt")).unwrap();
    std::fs::hard_link(outside.0.join("secret.txt"), root.0.join("hard.txt")).unwrap();
    std::fs::write(
        root.0.join("private.txt"),
        "Contact actual.person@example.org",
    )
    .unwrap();
    std::fs::write(root.0.join("binary.dat"), [255, 254]).unwrap();
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let p = policy(&root.0, now);
    for name in ["alias.txt", "hard.txt", "private.txt", "binary.dat"] {
        assert!(
            repo.capture_workspace(&request(), &p, &root.0, &read(name, 0, 2), now)
                .is_err(),
            "{name}"
        );
    }
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM task_workspace_evidence_v1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}
#[test]
fn factory_tools_are_intersection_of_host_policy_and_profile_not_model_authority() {
    let root = Directory(root());
    let r = request();
    let now = gents_local_mcp::now_ms();
    let mut p = policy(&root.0, now);
    p.max_model_calls = 4;
    p.max_tokens = 512;
    let mut plan = routing::factory(&r, "tool_loop/v1", "angular", Some("model")).unwrap();
    routing::configure_tools(&mut plan, &p, &routing::profiles()).unwrap();
    assert_eq!(plan.nodes[0].tools.len(), 3);
    routing::validate_plan(&r, &plan, &p, &BTreeSet::from(["model".into()])).unwrap();
    p.permitted_tools.remove("workspace_read");
    assert!(routing::validate_plan(&r, &plan, &p, &BTreeSet::from(["model".into()])).is_err());
    routing::configure_tools(&mut plan, &p, &routing::profiles()).unwrap();
    assert!(!plan.nodes[0].tools.contains(&"workspace_read".into()));
    plan.nodes[0].tools.push("exec_shell".into());
    assert!(routing::validate_plan(&r, &plan, &p, &BTreeSet::from(["model".into()])).is_err());
}
#[test]
fn captures_are_bounded_tamper_evident_and_terminal_deletion_invalidates_result() {
    let root = Directory(root());
    std::fs::write(root.0.join("file.txt"), "a".repeat(64)).unwrap();
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let p = policy(&root.0, now);
    let r = request();
    let mut result = repo.execute(&r, &p, now).unwrap();
    for offset in 0..32 {
        repo.capture_workspace(&r, &p, &root.0, &read("file.txt", offset, 1), now)
            .unwrap();
    }
    assert!(repo
        .capture_workspace(&r, &p, &root.0, &read("file.txt", 32, 1), now)
        .is_err());
    let e = evidence(
        &repo
            .capture_workspace(&r, &p, &root.0, &read("file.txt", 0, 1), now)
            .unwrap(),
    );
    result.status = "generated".into();
    result.answer = Some("draft".into());
    result.evidence = vec![e.clone()];
    repo.replace_result(&p.scope, &result).unwrap();
    repo.refresh_dependencies(&p.scope, &root.0).unwrap();
    repo.workspace_status_root(root.0.clone());
    repo.invalidate_cache(&p.scope).unwrap();
    assert_eq!(
        repo.status(&p.scope, r.task_id, now)
            .unwrap()
            .unwrap()
            .status,
        "evidence_invalidated"
    );
    let r = request();
    repo.capture_workspace(&r, &p, &root.0, &read("file.txt", 0, 1), now)
        .unwrap();
    db.execute("UPDATE task_workspace_evidence_v1 SET body='{}'", [])
        .unwrap();
    assert!(repo
        .workspace_task_current(&p.scope, r.task_id, &root.0, now)
        .is_err());
}

#[test]
fn declared_metadata_is_not_installed_versions_or_msbuild_evaluation() {
    use gents_local_mcp::task_engine::project_metadata;
    fn evidence(text: &str) -> Evidence {
        Evidence {
            source_id: "workspace:v1:fixture".into(),
            source_revision: "fixture".into(),
            content_sha256: gents_local_mcp::digest(text.as_bytes()),
            title: "actual fixture".into(),
            start_byte: 0,
            end_byte: text.len(),
            quote: text.into(),
            observed_at_ms: 1,
            valid_until_ms: 10000,
        }
    }
    let e=evidence("{\n  \"dependencies\": {\"@angular/core\": \"^20.3.0\"},\n  \"devDependencies\": {\"@angular/core\": \"~19.2.0\", \"typescript\": \"5.9.3\"}\n}");
    let m = project_metadata::inspect("frontend/package.json", &e, true)
        .unwrap()
        .unwrap();
    assert_eq!(m.observations.len(), 3);
    assert_eq!(m.observations[0].value, "^20.3.0");
    assert_eq!(m.observations[1].value, "~19.2.0");
    assert!(m
        .runtime_checks
        .iter()
        .all(|c| c.status == CheckStatus::NotRun));
    let m = project_metadata::inspect("frontend/package.json", &e, false)
        .unwrap()
        .unwrap();
    assert_eq!(m.state, "partial_input");
    assert!(m.observations.is_empty());
    let xml = r#"<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup Condition="'$(Configuration)' == 'Debug'"><TargetFramework>net10.0</TargetFramework></PropertyGroup><PropertyGroup><TargetFramework>$(InheritedFramework)</TargetFramework></PropertyGroup><Import Project="Uninspected.props" /></Project>"#;
    let m = project_metadata::inspect("api/Service.csproj", &evidence(xml), true)
        .unwrap()
        .unwrap();
    assert_eq!(m.observations.len(), 3);
    assert_eq!(m.observations[1].value["value"], "net10.0");
    assert_eq!(m.observations[1].value["conditional"], true);
    assert_eq!(m.observations[2].value["literal"], false);
    assert!(m
        .runtime_checks
        .iter()
        .all(|c| c.status == CheckStatus::NotRun));
}
#[test]
fn metadata_rejects_ambiguous_json_dtd_and_incomplete_xml_without_facts() {
    use gents_local_mcp::task_engine::project_metadata;
    for (name, text) in [
        (
            "cosmos.json",
            r#"{"partitionKey":"/one","partitionKey":"/two"}"#,
        ),
        (
            "package.json",
            r#"{"dependencies":{"typescript":"5","type\u0073cript":"6"}}"#,
        ),
        (
            "Service.csproj",
            r#"<!DOCTYPE Project [<!ENTITY remote SYSTEM "file:///etc/passwd">]><Project>&remote;</Project>"#,
        ),
        (
            "Service.csproj",
            "<Project><PropertyGroup><TargetFramework>net10.0</TargetFramework>",
        ),
    ] {
        let e = Evidence {
            source_id: "fixture".into(),
            source_revision: "v1".into(),
            content_sha256: "hash".into(),
            title: name.into(),
            start_byte: 0,
            end_byte: text.len(),
            quote: text.into(),
            observed_at_ms: 1,
            valid_until_ms: 2,
        };
        assert!(project_metadata::inspect(name, &e, true).is_err(), "{name}");
    }
}
#[test]
fn cosmos_declared_schema_has_no_invented_api_partition_charge_or_runtime_state() {
    use gents_local_mcp::task_engine::project_metadata;
    let text = r#"{"api":"NoSQL","container":"items","partitionKey":"/tenantId"}"#;
    let e = Evidence {
        source_id: "fixture".into(),
        source_revision: "v1".into(),
        content_sha256: "hash".into(),
        title: "contract".into(),
        start_byte: 0,
        end_byte: text.len(),
        quote: text.into(),
        observed_at_ms: 1,
        valid_until_ms: 2,
    };
    let m = project_metadata::inspect("db/cosmos.json", &e, true)
        .unwrap()
        .unwrap();
    assert_eq!(m.observations.len(), 3);
    assert_eq!(m.observations[2].value, "/tenantId");
    assert!(m
        .observations
        .iter()
        .all(|o| !o.name.contains("charge") && !o.name.contains("executed")));
    assert!(m
        .runtime_checks
        .iter()
        .all(|c| c.status == CheckStatus::NotRun));
}

#[test]
fn malformed_metadata_keeps_a_typed_failure_and_unrelated_api_json_is_not_cosmos() {
    use gents_local_mcp::task_engine::project_metadata;
    let root = Directory(root());
    std::fs::write(root.0.join("package.json"), "{ malformed").unwrap();
    let db = db();
    let repo = Repository::new(&db).unwrap();
    let now = gents_local_mcp::now_ms();
    let p = policy(&root.0, now);
    let result = repo
        .capture_workspace(&request(), &p, &root.0, &read("package.json", 0, 1024), now)
        .unwrap();
    let metadata: project_metadata::Metadata =
        serde_json::from_value(result["project_metadata"].clone()).unwrap();
    assert_eq!(metadata.state, "malformed_or_unsupported_metadata");
    assert!(metadata.observations.is_empty());
    assert_eq!(metadata.runtime_checks[0].status, CheckStatus::Unsupported);
    let text = r#"{"api":"unrelated REST application"}"#;
    let e = Evidence {
        quote: text.into(),
        ..evidence(&result)
    };
    let m = project_metadata::inspect("application.json", &e, true)
        .unwrap()
        .unwrap();
    assert_eq!(m.state, "unsupported_json_shape");
    assert!(m.observations.is_empty());
    let e = Evidence {
        quote: "<Project/>not XML trailing content".into(),
        ..e
    };
    assert!(project_metadata::inspect("Service.csproj", &e, true).is_err());
}

#[test]
fn task_audit_retention_cleans_only_orphaned_read_workflow_events() {
    let db = db();
    db.execute_batch("CREATE TABLE audit(run_id TEXT,kind TEXT,body TEXT,created INTEGER); CREATE TABLE task_workflow_jobs_v1(id TEXT);
        INSERT INTO task_workflow_jobs_v1 VALUES('retained');
        INSERT INTO audit VALUES('retained','task_tool_return','{}',1);
        INSERT INTO audit VALUES('expired','task_tool_return','{}',1);
        INSERT INTO audit VALUES('expired','native_uncertain_effect','{}',1);
        INSERT INTO audit VALUES('other','owner_approval','{}',1);").unwrap();
    assert_eq!(
        gents_local_mcp::task_engine::workflows::prune_audit(&db).unwrap(),
        1
    );
    let rows: Vec<(String, String)> = db
        .prepare("SELECT run_id,kind FROM audit ORDER BY rowid")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        rows,
        vec![
            ("retained".into(), "task_tool_return".into()),
            ("expired".into(), "native_uncertain_effect".into()),
            ("other".into(), "owner_approval".into())
        ]
    );
}

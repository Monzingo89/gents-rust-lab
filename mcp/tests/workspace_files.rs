#![cfg(any(target_os = "macos", target_os = "linux"))]
mod support;
use gents_local_mcp::{
    authority::Authority, governance, workspace_path::is_safe_relative_path, Phase, Registry, Scope,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    sync::atomic::AtomicBool,
};
use support::{install_files, Fixture};
fn work(f: &Fixture) -> std::path::PathBuf {
    f.authority.workspace(&f.scope).unwrap()
}
fn call(f: &Fixture, actor: &Scope, args: Value) -> anyhow::Result<Value> {
    Registry::new()
        .call(&f.authority, actor, "files", args)
        .map(|r| r["value"].clone())
}
#[test]
fn hash_conditioned_replacement_rejects_stale_retries_and_allows_explicit_revision_return() {
    let f = Fixture::new();
    install_files(&f);
    let first = "draft one";
    let second = "draft two";
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"draft.txt","content":first}),
    )
    .unwrap();
    let first_hash = gents_local_mcp::digest(first.as_bytes());
    let second_hash = gents_local_mcp::digest(second.as_bytes());
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"draft.txt","content":second,"expected_sha256":first_hash}),
    )
    .unwrap();
    assert!(call(&f,&f.scope,json!({"op":"write","path":"draft.txt","content":"stale overwrite","expected_sha256":first_hash})).is_err());
    assert_eq!(
        fs::read_to_string(work(&f).join("draft.txt")).unwrap(),
        second
    );
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"draft.txt","content":first,"expected_sha256":second_hash}),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(work(&f).join("draft.txt")).unwrap(),
        first
    );
    assert!(call(
        &f,
        &f.scope,
        json!({"op":"write","path":"missing.txt","content":second,"expected_sha256":first_hash})
    )
    .is_err());
    assert!(!work(&f).join("missing.txt").exists());
}
#[test]
fn hash_conditioned_replacement_accepts_a_multibyte_first_character() {
    let f = Fixture::new();
    install_files(&f);
    let first = "😀 initial revision";
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"unicode.txt","content":first}),
    )
    .unwrap();
    call(&f, &f.scope, json!({"op":"write","path":"unicode.txt","content":"revised","expected_sha256":gents_local_mcp::digest(first.as_bytes())})).unwrap();
    assert_eq!(
        fs::read_to_string(work(&f).join("unicode.txt")).unwrap(),
        "revised"
    );
}
#[test]
fn conditioned_mcp_writers_cannot_both_replace_the_same_revision() {
    let f = Fixture::new();
    install_files(&f);
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"race.txt","content":"original"}),
    )
    .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles=["writer-a","writer-b"].into_iter().map(|content|{
        let root=f.root.clone();let scope=f.scope.clone();let barrier=barrier.clone();
        std::thread::spawn(move||{
            let authority=gents_local_mcp::authority::SqliteAuthority::new(root).unwrap();barrier.wait();
            let result=Registry::new().call(&authority,&scope,"files",json!({"op":"write","path":"race.txt","content":content,"expected_sha256":gents_local_mcp::digest(b"original")}));
            (content,result.is_ok())
        })
    }).collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|(_, ok)| *ok).count(), 1);
    assert_eq!(
        fs::read_to_string(work(&f).join("race.txt")).unwrap(),
        results.iter().find(|(_, ok)| *ok).unwrap().0
    );
}
#[test]
fn conditional_writes_require_read_permission_and_are_hidden_from_write_only_schemas() {
    let f = Fixture::new();
    install_files(&f);
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"draft.txt","content":"original"}),
    )
    .unwrap();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let previous = policy.revision;
    policy.revision += 1;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("files:read");
    governance::save(&db, &f.scope.run.to_string(), &policy, previous).unwrap();
    assert!(call(&f,&f.scope,json!({"op":"write","path":"draft.txt","content":"replacement","expected_sha256":gents_local_mcp::digest(b"original")})).is_err());
    assert_eq!(
        fs::read_to_string(work(&f).join("draft.txt")).unwrap(),
        "original"
    );
    let catalog = Registry::new().list(&f.authority, &f.scope).unwrap();
    let schema = &catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "files")
        .unwrap()["inputSchema"];
    let write = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["properties"]["op"]["const"] == "write")
        .unwrap();
    assert!(write["properties"].get("expected_sha256").is_none());
}
#[test]
fn independent_path_contract_checks_unicode_controls_and_exact_boundaries() {
    for value in [
        "report.json",
        "résumé/part list.json",
        "日本語/配置.rs",
        "a-b_c.123",
    ] {
        assert!(is_safe_relative_path(value), "{value:?}");
    }
    for value in [
        "",
        "/root",
        "../outside",
        "a/../outside",
        "./a",
        "a/./b",
        "a//b",
        "a/",
        "a\\b",
        "C:relative",
        "a/.env",
        ".git/config",
        "a\0b",
        "a\nb",
        "a\u{0085}b",
    ] {
        assert!(!is_safe_relative_path(value), "{value:?}");
    }
    for code in 0..=0x9fu32 {
        if let Some(ch) = char::from_u32(code).filter(|c| c.is_control()) {
            assert!(!is_safe_relative_path(&format!("a{ch}b")));
        }
    }
    assert!(is_safe_relative_path(&"a".repeat(120)));
    assert!(!is_safe_relative_path(&"a".repeat(121)));
    assert!(is_safe_relative_path(&"é".repeat(60)));
    assert!(!is_safe_relative_path(&"é".repeat(61)));
    assert!(is_safe_relative_path(&["a"; 12].join("/")));
    assert!(!is_safe_relative_path(&["a"; 13].join("/")));
    let boundary = vec!["a".repeat(110); 9].join("/") + "/a";
    assert_eq!(boundary.len(), 1000);
    assert!(is_safe_relative_path(&boundary));
    assert!(!is_safe_relative_path(&(boundary + "a")));
}
#[test]
fn native_write_list_and_paged_read_preserve_source_hash_and_private_modes() {
    let f = Fixture::new();
    install_files(&f);
    let text = "αβγ\nsource with spaces";
    let written = call(
        &f,
        &f.scope,
        json!({"op":"write","path":"résumé/part list.txt","content":text}),
    )
    .unwrap();
    assert_eq!(written["sha256"], gents_local_mcp::digest(text.as_bytes()));
    assert_eq!(written["atomic_replace"], true);
    let target = work(&f).join("résumé/part list.txt");
    assert_eq!(
        fs::metadata(target).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let page = call(
        &f,
        &f.actor("Reader"),
        json!({"op":"read","path":"résumé/part list.txt","max_bytes":3}),
    )
    .unwrap();
    assert_eq!(page["text"], "α");
    assert_eq!(page["next_offset_bytes"], 2);
    assert_eq!(page["sha256"], written["sha256"]);
    let next = call(
        &f,
        &f.scope,
        json!({"op":"read","path":"résumé/part list.txt","offset_bytes":2,"max_bytes":4}),
    )
    .unwrap();
    assert_eq!(next["text"], "βγ");
    assert_eq!(next["sha256"], page["sha256"]);
    assert!(call(
        &f,
        &f.scope,
        json!({"op":"read","path":"résumé/part list.txt","offset_bytes":1})
    )
    .is_err());
    let list = call(&f, &f.actor("Reader"), json!({"op":"list","path":"résumé"})).unwrap();
    assert_eq!(list["files"][0]["name"], "part list.txt");
}
#[test]
fn granular_catalog_and_execution_reject_ungranted_operations_and_planner_writes() {
    let f = Fixture::new();
    install_files(&f);
    let registry = Registry::new();
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"result.txt","content":"verified source"}),
    )
    .unwrap();
    let catalog = registry.list(&f.authority, &f.actor("Reader")).unwrap();
    let entry = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "files")
        .unwrap();
    assert_eq!(entry["inputSchema"]["type"], "object");
    assert_eq!(entry["inputSchema"]["oneOf"].as_array().unwrap().len(), 2);
    assert!(entry["inputSchema"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .all(|b| b["properties"]["op"]["const"] != "write"));
    assert!(call(
        &f,
        &f.actor("Reader"),
        json!({"op":"write","path":"result.txt","content":"unauthorized"})
    )
    .is_err());
    assert!(call(
        &f,
        &f.actor("Outsider"),
        json!({"op":"read","path":"result.txt"})
    )
    .is_err());
    let planner = Scope {
        phase: Phase::Planner,
        ..f.scope.clone()
    };
    assert!(call(
        &f,
        &planner,
        json!({"op":"write","path":"result.txt","content":"unauthorized"})
    )
    .is_err());
    assert_eq!(
        call(&f, &planner, json!({"op":"read","path":"result.txt"})).unwrap()["text"],
        "verified source"
    );
    assert_eq!(
        fs::read_to_string(work(&f).join("result.txt")).unwrap(),
        "verified source"
    );
    let mut wrong = f.scope.clone();
    wrong.owner = "another owner".into();
    assert!(call(&f, &wrong, json!({"op":"read","path":"result.txt"})).is_err());
    wrong = f.scope.clone();
    wrong.run = uuid::Uuid::new_v4();
    assert!(call(&f, &wrong, json!({"op":"read","path":"result.txt"})).is_err());
}
#[test]
fn symlinks_hard_links_fifo_and_traversal_cannot_escape_the_workspace() {
    let f = Fixture::new();
    install_files(&f);
    let folder = work(&f);
    fs::create_dir_all(&folder).unwrap();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("private.txt"), "outside marker").unwrap();
    symlink(&outside, folder.join("escape")).unwrap();
    symlink(outside.join("private.txt"), folder.join("link.txt")).unwrap();
    fs::hard_link(outside.join("private.txt"), folder.join("hard.txt")).unwrap();
    let fifo = std::ffi::CString::new(folder.join("pipe").to_str().unwrap()).unwrap();
    // SAFETY: valid NUL-terminated temporary fixture path; no production location.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    for path in [
        "../outside/private.txt",
        "escape/private.txt",
        "link.txt",
        "hard.txt",
        "pipe",
        ".env",
        "a//b",
        "a/../b",
    ] {
        assert!(
            call(&f, &f.scope, json!({"op":"read","path":path})).is_err(),
            "read {path}"
        );
        assert!(
            call(
                &f,
                &f.scope,
                json!({"op":"write","path":path,"content":"must not escape"})
            )
            .is_err(),
            "write {path}"
        );
    }
    let list = call(&f, &f.scope, json!({"op":"list","path":"."})).unwrap();
    assert_eq!(list["files"], json!([]));
    assert_eq!(
        fs::read_to_string(outside.join("private.txt")).unwrap(),
        "outside marker"
    );
}
#[test]
fn limits_sensitive_patterns_completed_jobs_and_revocation_fail_closed() {
    let f = Fixture::new();
    install_files(&f);
    fs::create_dir_all(work(&f)).unwrap();
    assert!(call(
        &f,
        &f.scope,
        json!({"op":"write","path":"large.txt","content":"a".repeat(32769)})
    )
    .is_err());
    assert!(call(
        &f,
        &f.scope,
        json!({"op":"write","path":"card.txt","content":"4111 1111 1111 1111"})
    )
    .is_err());
    let large = fs::File::create(work(&f).join("large.txt")).unwrap();
    large.set_len(16 * 1024 * 1024 + 1).unwrap();
    assert!(call(&f, &f.scope, json!({"op":"read","path":"large.txt"})).is_err());
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"ok.txt","content":"bounded source"}),
    )
    .unwrap();
    f.db()
        .execute("UPDATE jobs SET state='completed'", [])
        .unwrap();
    assert!(call(
        &f,
        &f.scope,
        json!({"op":"write","path":"ok.txt","content":"late mutation"})
    )
    .is_err());
    assert_eq!(
        call(&f, &f.actor("Reader"), json!({"op":"read","path":"ok.txt"})).unwrap()["text"],
        "bounded source"
    );
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    policy
        .grants
        .get_mut("Reader")
        .unwrap()
        .tools
        .remove("files:read");
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    assert!(call(&f, &f.actor("Reader"), json!({"op":"read","path":"ok.txt"})).is_err());
}

#[test]
fn admitted_write_errors_are_not_falsely_audited_as_effect_free() {
    let f = Fixture::new();
    install_files(&f);
    fs::create_dir_all(work(&f).join("directory")).unwrap();
    assert!(call(
        &f,
        &f.scope,
        json!({"op":"write","path":"directory","content":"not a directory replacement"})
    )
    .is_err());
    let body: String = f
        .db()
        .query_row(
            "SELECT body FROM audit WHERE kind='mcp_tool_effect_uncertain'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["status"],
        "effect_uncertain"
    );
    assert!(work(&f).join("directory").is_dir());
    assert!(!body.contains("not a directory replacement"));
    assert!(call(
        &f,
        &f.actor("Reader"),
        json!({"op":"write","path":"never.txt","content":"denied"})
    )
    .is_err());
    let count: i64 = f
        .db()
        .query_row(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_rejected'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "known denial happens before external effects");
}

#[test]
fn filesystem_write_surviving_audit_failure_is_reported_uncertain_not_success() {
    let f = Fixture::new();
    install_files(&f);
    f.db().execute_batch("CREATE TRIGGER reject_success_audit BEFORE INSERT ON audit WHEN NEW.kind='mcp_tool_call' BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;").unwrap();
    let error = call(
        &f,
        &f.scope,
        json!({"op":"write","path":"candidate.txt","content":"written before audit failure"}),
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("inspect the workspace path and hash"));
    assert_eq!(
        fs::read_to_string(work(&f).join("candidate.txt")).unwrap(),
        "written before audit failure"
    );
    let count: i64 = f
        .db()
        .query_row(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    let body: String = f
        .db()
        .query_row(
            "SELECT body FROM audit WHERE kind='mcp_tool_effect_uncertain'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["reason"],
        "audit_commit_failed"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn actual_stdio_preserves_escaped_source_and_returns_a_bounded_page() {
    let f = Fixture::new();
    install_files(&f);
    let binary = f.root.join("target/debug");
    fs::create_dir_all(&binary).unwrap();
    fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        binary.join("gents-local-mcp"),
    )
    .unwrap();
    let text = "quote \" slash \\ newline\n".repeat(600);
    let cancel = AtomicBool::new(false);
    let written = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "files",
        &json!({"op":"write","path":"roundtrip.rs","content":text}),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(written["ok"], true);
    assert_eq!(
        fs::read_to_string(work(&f).join("roundtrip.rs")).unwrap(),
        text
    );
    let read = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "files",
        &json!({"op":"read","path":"roundtrip.rs","max_bytes":8192}),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(read["ok"], true);
    assert_eq!(read["mcp_receipt"]["tool"], "files");
    assert_eq!(read["result"]["text"], &text[..8192]);
    assert_eq!(read["result"]["truncated"], true);
}

#[test]
fn stale_source_epoch_cannot_write_even_when_the_tool_grant_is_unchanged() {
    let f = Fixture::new();
    support::install_files(&f);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let mut scope = f.scope.clone();
    scope.expected_revision = Some(policy.revision);
    let registry = Registry::new();
    registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"epoch.txt","content":"before"}),
        )
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    governance::save(&db, &scope.run.to_string(), &policy, prior).unwrap();
    assert!(registry
        .call(
            &f.authority,
            &scope,
            "files",
            json!({"op":"write","path":"epoch.txt","content":"after"})
        )
        .is_err());
    assert_eq!(
        std::fs::read_to_string(
            f.root
                .join("data/companies/fixture/runs")
                .join(scope.run.to_string())
                .join("workspace/epoch.txt")
        )
        .unwrap(),
        "before"
    );
    assert!(registry.list(&f.authority, &scope).is_err());
    assert!(gents_local_mcp::completion::check(&f.authority, &scope).is_err());
}

#[test]
fn frozen_owner_sources_cannot_be_output_destinations_or_native_writes() {
    let f = Fixture::new();
    install_files(&f);
    let db = f.authority.open(&f.scope).unwrap();
    for metadata in [
        json!({"ceo_source_files":[{"path":"input.json"}]}),
        json!({"ceo_source_inputs":[{"id":"input","tool":"table_read_json","arguments":{"path":"input.json"}}]}),
    ] {
        db.execute(
            "UPDATE jobs SET intake=?1 WHERE run_id=?2",
            rusqlite::params![metadata.to_string(), f.scope.run.to_string()],
        )
        .unwrap();
        fs::create_dir_all(work(&f)).unwrap();
        fs::write(work(&f).join("input.json"), "original").unwrap();
        let denied = Registry::new()
            .call(
                &f.authority,
                &f.scope,
                "files",
                json!({"op":"write","path":"input.json","content":"changed"}),
            )
            .unwrap_err();
        assert!(denied.to_string().contains("frozen owner input"));
        assert_eq!(
            fs::read_to_string(work(&f).join("input.json")).unwrap(),
            "original"
        );
        let mut intake = metadata.clone();
        intake["artifact_bindings"] = json!([{"path":"input.json","source_actor":f.scope.actor,"writer":f.scope.actor,"source_tool":"integer_arithmetic","arguments":{"calculations":[{"id":"x","values":[1],"subtract":0}]},"pointer":"/calculations/0/total","encoding":"integer_text"}]);
        assert!(gents_local_mcp::artifact_bindings::parse(&intake)
            .err()
            .unwrap()
            .to_string()
            .contains("overlaps"));
    }
    call(
        &f,
        &f.scope,
        json!({"op":"write","path":"output.txt","content":"allowed"}),
    )
    .unwrap();
}

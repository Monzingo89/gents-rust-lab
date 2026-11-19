mod support;
use gents_local_mcp::{authority::Authority, governance, Phase, Registry};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    sync::atomic::AtomicBool,
};
use support::Fixture;
fn root(f: &Fixture) -> std::path::PathBuf {
    f.authority.workspace(&f.scope).unwrap()
}
fn grant(f: &Fixture, tools: &[&str]) {
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    for access in [&mut policy.ceilings, &mut policy.grants] {
        access.get_mut("Worker").unwrap().tools = tools.iter().map(|s| s.to_string()).collect();
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
}
fn seed(f: &Fixture, path: &str, body: &[u8]) {
    let path = root(f).join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}
fn args(path: &str, destination: &str, bytes: &[u8]) -> Value {
    json!({"op":"rename","path":path,"destination":destination,"expected_sha256":gents_local_mcp::digest(bytes)})
}
fn rename(f: &Fixture, path: &str, destination: &str, bytes: &[u8]) -> anyhow::Result<Value> {
    Registry::new().call(
        &f.authority,
        &f.scope,
        "files",
        args(path, destination, bytes),
    )
}
fn rename_visible(f: &Fixture, scope: &gents_local_mcp::Scope) -> bool {
    Registry::new().list(&f.authority, scope).unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["name"] == "files")
        .flat_map(|t| t["inputSchema"]["oneOf"].as_array().unwrap())
        .any(|b| b["properties"]["op"]["const"] == "rename")
}
#[test]
fn text_binary_and_empty_files_keep_bytes_inode_mode_and_hash_without_overwrite() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    fs::create_dir_all(root(&f).join("ready")).unwrap();
    for (path, body) in [
        ("draft α.txt", b"source\n".as_slice()),
        ("image.dat", b"\x00\xff\x80opaque\r\n".as_slice()),
        ("empty.txt", b"".as_slice()),
    ] {
        seed(&f, path, body);
        fs::set_permissions(root(&f).join(path), fs::Permissions::from_mode(0o640)).unwrap();
        let before = fs::metadata(root(&f).join(path)).unwrap();
        let destination = format!("ready/{path}");
        let result = rename(&f, path, &destination, body).unwrap();
        let after = fs::metadata(root(&f).join(&destination)).unwrap();
        assert_eq!(fs::read(root(&f).join(&destination)).unwrap(), body);
        assert!(!root(&f).join(path).exists());
        assert_eq!((before.ino(), before.mode()), (after.ino(), after.mode()));
        assert_eq!(result["value"]["sha256"], gents_local_mcp::digest(body));
        assert_eq!(result["value"]["atomic_rename"], true);
        assert_eq!(result["value"]["replaced_existing"], false);
        assert_eq!(result["receipt"]["status"], "completed");
        seed(&f, path, b"new source");
        assert!(rename(&f, path, &destination, b"new source").is_err());
        assert_eq!(fs::read(root(&f).join(&destination)).unwrap(), body);
        assert_eq!(fs::read(root(&f).join(path)).unwrap(), b"new source");
    }
}
#[test]
fn rename_needs_its_own_grant_read_authority_worker_phase_current_epoch_and_active_job() {
    let f = Fixture::new();
    seed(&f, "old.txt", b"source");
    for tools in [vec!["files:read", "files:write"], vec!["files:rename"]] {
        grant(&f, &tools);
        assert!(!rename_visible(&f, &f.scope));
        assert!(rename(&f, "old.txt", "new.txt", b"source").is_err());
    }
    grant(&f, &["files:read", "files:rename"]);
    assert!(rename_visible(&f, &f.scope));
    let mut planner = f.scope.clone();
    planner.phase = Phase::Planner;
    assert!(!rename_visible(&f, &planner));
    assert!(Registry::new()
        .call(
            &f.authority,
            &planner,
            "files",
            args("old.txt", "new.txt", b"source")
        )
        .is_err());
    let epoch = governance::load(&f.db(), &f.scope.run.to_string())
        .unwrap()
        .unwrap()
        .revision;
    let mut stale = f.scope.clone();
    stale.expected_revision = Some(epoch);
    grant(&f, &["files:read", "files:rename"]);
    assert!(Registry::new()
        .call(
            &f.authority,
            &stale,
            "files",
            args("old.txt", "new.txt", b"source")
        )
        .is_err());
    f.db()
        .execute("UPDATE jobs SET state='completed'", [])
        .unwrap();
    assert!(rename(&f, "old.txt", "new.txt", b"source").is_err());
    assert_eq!(fs::read(root(&f).join("old.txt")).unwrap(), b"source");
    assert!(!root(&f).join("new.txt").exists());
}
#[test]
fn stale_hash_paths_links_special_files_large_files_and_missing_parents_fail_closed() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    seed(&f, "old.txt", b"source");
    assert!(rename(&f, "old.txt", "new.txt", b"stale").is_err());
    assert!(rename(&f, "old.txt", "missing/new.txt", b"source").is_err());
    assert!(!root(&f).join("missing").exists());
    for path in [
        "../outside",
        "/tmp/outside",
        ".hidden",
        "a/../old.txt",
        "old.txt",
    ] {
        let destination = if path == "old.txt" { "old.txt" } else { path };
        assert!(rename(&f, "old.txt", destination, b"source").is_err());
    }
    let external = f.root.join("outside.txt");
    fs::write(&external, b"outside").unwrap();
    symlink(&external, root(&f).join("link.txt")).unwrap();
    assert!(rename(&f, "link.txt", "new.txt", b"outside").is_err());
    assert!(rename(&f, "old.txt", "link.txt", b"source").is_err());
    symlink(f.root.clone(), root(&f).join("linked-dir")).unwrap();
    assert!(rename(&f, "old.txt", "linked-dir/new.txt", b"source").is_err());
    fs::hard_link(root(&f).join("old.txt"), root(&f).join("hard.txt")).unwrap();
    assert!(rename(&f, "hard.txt", "new.txt", b"source").is_err());
    fs::remove_file(root(&f).join("hard.txt")).unwrap();
    fs::create_dir(root(&f).join("folder")).unwrap();
    assert!(rename(&f, "folder", "new.txt", b"").is_err());
    assert!(rename(&f, "old.txt", "folder", b"source").is_err());
    let fifo = std::ffi::CString::new(root(&f).join("pipe").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(rename(&f, "pipe", "new.txt", b"").is_err());
    fs::File::create(root(&f).join("huge"))
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(rename(&f, "huge", "new.txt", b"").is_err());
    let mut missing = args("old.txt", "new.txt", b"source");
    missing.as_object_mut().unwrap().remove("expected_sha256");
    assert!(Registry::new()
        .call(&f.authority, &f.scope, "files", missing)
        .is_err());
    assert_eq!(fs::read(&external).unwrap(), b"outside");
    assert_eq!(fs::read(root(&f).join("old.txt")).unwrap(), b"source");
    assert!(!root(&f).join("new.txt").exists());
}
#[test]
fn frozen_owner_inputs_are_protected_at_source_destination_and_case_aliases() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    seed(&f, "acceptance.py", b"frozen");
    seed(&f, "base.py", b"dependency");
    seed(&f, "draft.py", b"candidate");
    f.db().execute("UPDATE jobs SET intake=?1",[json!({"python_check_contracts":[{"tests_path":"acceptance.py","frozen_inputs":{"base.py":"hash"}}]}).to_string()]).unwrap();
    for (source, destination, bytes) in [
        ("acceptance.py", "moved.py", b"frozen".as_slice()),
        ("base.py", "moved.py", b"dependency".as_slice()),
        ("draft.py", "acceptance.py", b"candidate".as_slice()),
    ] {
        let e = rename(&f, source, destination, bytes).unwrap_err();
        assert!(e.to_string().contains("frozen owner input"));
    }
    #[cfg(target_os = "macos")]
    if root(&f).join("ACCEPTANCE.py").exists() {
        assert!(rename(&f, "ACCEPTANCE.py", "moved.py", b"frozen")
            .unwrap_err()
            .to_string()
            .contains("frozen owner input"));
    }
    assert_eq!(fs::read(root(&f).join("acceptance.py")).unwrap(), b"frozen");
    assert_eq!(fs::read(root(&f).join("base.py")).unwrap(), b"dependency");
}
#[test]
fn competing_writers_cannot_replace_a_destination() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    seed(&f, "a.txt", b"a");
    seed(&f, "b.txt", b"b");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads = [("a.txt", b"a"), ("b.txt", b"b")]
        .into_iter()
        .map(|(path, body)| {
            let root = f.root.clone();
            let scope = f.scope.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let authority = gents_local_mcp::authority::SqliteAuthority::new(root).unwrap();
                barrier.wait();
                (
                    path,
                    Registry::new()
                        .call(&authority, &scope, "files", args(path, "winner.txt", body))
                        .is_ok(),
                )
            })
        })
        .collect::<Vec<_>>();
    let outcomes = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(outcomes.iter().filter(|(_, ok)| *ok).count(), 1);
    let loser = outcomes.iter().find(|(_, ok)| !*ok).unwrap().0;
    assert!(root(&f).join(loser).exists());
    let winner = outcomes.iter().find(|(_, ok)| *ok).unwrap().0;
    assert!(!root(&f).join(winner).exists());
}
#[test]
fn lost_rename_audit_is_uncertain_and_reconciliation_does_not_repeat_the_move() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    seed(&f, "old.txt", b"source");
    f.db().execute_batch("CREATE TRIGGER reject_receipt BEFORE INSERT ON audit WHEN NEW.kind='mcp_tool_call' BEGIN SELECT RAISE(ABORT,'injected lost receipt'); END;").unwrap();
    assert!(rename(&f, "old.txt", "new.txt", b"source").is_err());
    assert!(!root(&f).join("old.txt").exists());
    assert_eq!(fs::read(root(&f).join("new.txt")).unwrap(), b"source");
    let db = f.db();
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_effect_uncertain'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    db.execute_batch("DROP TRIGGER reject_receipt").unwrap();
    let read = Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"new.txt"}),
        )
        .unwrap();
    assert_eq!(read["value"]["sha256"], gents_local_mcp::digest(b"source"));
    assert!(!root(&f).join("old.txt").exists());
}
#[tokio::test(flavor = "current_thread")]
async fn actual_stdio_and_sse_return_canonical_rename_receipts_without_implicit_grants() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    let bin = f.root.join("target/debug");
    fs::create_dir_all(&bin).unwrap();
    fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        bin.join("gents-local-mcp"),
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        seed(&f, "old.txt", b"\0\xfftransport source");
        let result = gents_local_mcp::client::invoke(
            &f.root,
            &f.scope,
            "files",
            &args(
                "old.txt",
                &format!("{transport}.txt"),
                b"\0\xfftransport source",
            ),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["mcp_receipt"]["status"], "completed");
        assert_eq!(
            result["result"]["sha256"],
            gents_local_mcp::digest(b"\0\xfftransport source")
        );
        assert!(!root(&f).join("old.txt").exists());
        let meta = gents_local_mcp::client::invoke(
            &f.root,
            &f.scope,
            "files",
            &json!({"op":"read","path":format!("{transport}.txt"),"metadata_only":true}),
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(meta["result"]["metadata_only"], true);
        assert!(meta["result"].get("text").is_none());
        assert_eq!(meta["result"]["sha256"], result["result"]["sha256"]);
    }
    std::env::remove_var("GENTS_MCP_TRANSPORT");
}

#[test]
fn external_creation_race_cannot_be_overwritten_by_the_rename() {
    use std::io::Write;
    let f = Fixture::new();
    fs::create_dir_all(root(&f)).unwrap();
    for n in 0..20 {
        let source = format!("source-{n}.dat");
        let destination = format!("destination-{n}.dat");
        seed(&f, &source, b"original bytes");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let other = barrier.clone();
        let folder = root(&f);
        let target = folder.join(&destination);
        let creator = std::thread::spawn(move || {
            other.wait();
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
            {
                Ok(mut file) => {
                    file.write_all(b"external creator").unwrap();
                    true
                }
                Err(error) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
                    false
                }
            }
        });
        let directory = gents_local_mcp::workspace::open(&root(&f), false).unwrap();
        barrier.wait();
        let renamed = directory
            .rename(
                &source,
                &destination,
                &gents_local_mcp::digest(b"original bytes"),
            )
            .is_ok();
        let created = creator.join().unwrap();
        assert_ne!(renamed, created);
        assert_eq!(
            fs::read(root(&f).join(&destination)).unwrap(),
            if renamed {
                b"original bytes".as_slice()
            } else {
                b"external creator".as_slice()
            }
        );
        assert_eq!(root(&f).join(&source).exists(), created);
    }
}

#[test]
fn metadata_only_read_fingerprints_binary_without_text_and_never_accepts_pagination() {
    let f = Fixture::new();
    grant(&f, &["files:read", "files:rename"]);
    let body = b"\0\xffopaque bytes";
    seed(&f, "opaque.bin", body);
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"opaque.bin"})
        )
        .is_err());
    let meta = Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"opaque.bin","metadata_only":true}),
        )
        .unwrap();
    assert_eq!(meta["value"]["sha256"], gents_local_mcp::digest(body));
    assert_eq!(meta["value"]["bytes"], body.len());
    assert!(meta["value"].get("text").is_none());
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"opaque.bin","metadata_only":true,"offset_bytes":0})
        )
        .is_err());
    rename(&f, "opaque.bin", "renamed.bin", body).unwrap();
    let after = Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"renamed.bin","metadata_only":true}),
        )
        .unwrap();
    assert_eq!(after["value"]["sha256"], meta["value"]["sha256"]);
    assert!(Registry::new()
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"opaque.bin","metadata_only":true})
        )
        .is_err());
}

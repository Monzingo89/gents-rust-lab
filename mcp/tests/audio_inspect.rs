mod support;
use gents_local_mcp::{artifact_bindings, authority::Authority, governance, workspace, Registry};
use serde_json::{json, Value};
use support::{audio::wav, Fixture};
fn fixture() -> (Fixture, Vec<u8>, Value) {
    let f = Fixture::new();
    let bytes = wav(1, 8000, &[-32768, -1, 0, 1, 32767, -2, 2]);
    let args = json!({"path":"clip.wav","expected_sha256":gents_local_mcp::digest(&bytes)});
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 2;
    for map in [&mut policy.grants, &mut policy.ceilings] {
        map.get_mut("Worker")
            .unwrap()
            .tools
            .insert("audio_inspect".into());
        map.get_mut("Reader")
            .unwrap()
            .tools
            .extend(["files:write".into(), "files:read".into()]);
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    let root = f.authority.workspace(&f.scope).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("clip.wav"), &bytes).unwrap();
    (f, bytes, args)
}
fn receipts(f: &Fixture) -> i64 {
    f.db()
        .query_row(
            "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}
#[test]
fn actual_audio_bytes_bind_native_results_and_cross_actor_artifact_copies() {
    let (f, bytes, args) = fixture();
    let intake = json!({"ceo_source_inputs":[{"id":"audio-input","tool":"audio_inspect","arguments":args}],"artifact_bindings":[
        {"path":"frames.txt","source_actor":"Worker","writer":"Reader","source_tool":"audio_inspect","arguments":args,"pointer":"/frame_count","encoding":"integer_text"},
        {"path":"peak.txt","source_actor":"Worker","writer":"Reader","source_tool":"audio_inspect","arguments":args,"pointer":"/channel_statistics/0/peak_abs","encoding":"integer_text"}]});
    artifact_bindings::parse(&intake).unwrap();
    f.db()
        .execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    let registry = Registry::new();
    let observed = registry
        .call(&f.authority, &f.scope, "audio_inspect", args.clone())
        .unwrap();
    assert_eq!(observed["value"]["frame_count"], 7);
    assert_eq!(
        observed["value"]["source_sha256"],
        gents_local_mcp::digest(&bytes)
    );
    assert_eq!(observed["value"]["source_bytes"], bytes.len());
    assert_eq!(observed["receipt"]["provider_cost_usd"], 0);
    assert!(registry
        .call(&f.authority, &f.actor("Outsider"), "audio_inspect", args)
        .is_err());
    assert_eq!(receipts(&f), 1);
    for _ in 0..4 {
        let call = artifact_bindings::next_call(&f.authority, &f.actor("Reader"), true)
            .unwrap()
            .unwrap();
        registry
            .call(
                &f.authority,
                &f.actor("Reader"),
                call["action"].as_str().unwrap(),
                call["arguments"].clone(),
            )
            .unwrap();
    }
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()["passed"],
        true
    );
    assert_eq!(receipts(&f), 5);
    let root = f.authority.workspace(&f.scope).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("peak.txt")).unwrap(),
        "32768"
    );
    assert_eq!(std::fs::read(root.join("clip.wav")).unwrap(), bytes);
}
#[test]
fn source_digest_path_file_type_size_and_epoch_fail_without_success_receipts() {
    let (f, bytes, args) = fixture();
    let registry = Registry::new();
    let root = f.authority.workspace(&f.scope).unwrap();
    for path in ["../clip.wav", "/tmp/clip.wav", "clip.txt"] {
        let mut bad = args.clone();
        bad["path"] = json!(path);
        assert!(registry
            .call(&f.authority, &f.scope, "audio_inspect", bad)
            .is_err());
    }
    let mut bad = args.clone();
    bad["expected_sha256"] = json!("0".repeat(64));
    assert!(registry
        .call(&f.authority, &f.scope, "audio_inspect", bad)
        .is_err());
    std::fs::write(root.join("bad.wav"), b"PRIVATE_PAYLOAD_NOT_AUDIO").unwrap();
    let bad = json!({"path":"bad.wav","expected_sha256":gents_local_mcp::digest(b"PRIVATE_PAYLOAD_NOT_AUDIO")});
    let error = registry
        .call(&f.authority, &f.scope, "audio_inspect", bad)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("PRIVATE_PAYLOAD"));
    let file = std::fs::File::create(root.join("big.wav")).unwrap();
    file.set_len((gents_local_mcp::audio::MAX_BYTES + 1) as u64)
        .unwrap();
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "audio_inspect",
            json!({"path":"big.wav","expected_sha256":"0".repeat(64)})
        )
        .is_err());
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 3;
    policy
        .grants
        .get_mut("Worker")
        .unwrap()
        .tools
        .remove("audio_inspect");
    governance::save(&db, &f.scope.run.to_string(), &policy, 2).unwrap();
    let mut stale = f.scope.clone();
    stale.expected_revision = Some(2);
    assert!(registry
        .call(&f.authority, &stale, "audio_inspect", args.clone())
        .is_err());
    assert!(registry
        .call(&f.authority, &f.scope, "audio_inspect", args)
        .is_err());
    assert_eq!(receipts(&f), 0);
    assert_eq!(std::fs::read(root.join("clip.wav")).unwrap(), bytes);
}
#[test]
fn frozen_audio_paths_and_actual_filesystem_aliases_cannot_be_overwritten() {
    let (f, bytes, args) = fixture();
    let intake =
        json!({"ceo_source_inputs":[{"id":"audio","tool":"audio_inspect","arguments":args}]});
    f.db()
        .execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    let registry = Registry::new();
    let root = f.authority.workspace(&f.scope).unwrap();
    let directory = workspace::open(&root, false).unwrap();
    assert!(directory.same_file("clip.wav", "clip.wav").unwrap());
    assert!(!directory.same_file("missing.wav", "clip.wav").unwrap());
    assert!(directory.same_file("../clip.wav", "clip.wav").is_err());
    let mut aliases = vec!["clip.wav"];
    if root.join("CLIP.WAV").exists() {
        aliases.push("CLIP.WAV");
    }
    for path in aliases {
        assert!(registry
            .call(
                &f.authority,
                &f.actor("Reader"),
                "files",
                json!({"op":"write","path":path,"content":"overwrite"})
            )
            .is_err());
    }
    let original = "caf\u{e9}.wav";
    let normalized = "cafe\u{301}.wav";
    std::fs::write(root.join(original), &bytes).unwrap();
    if root.join(normalized).exists() {
        let mut normalized_intake = intake.clone();
        normalized_intake["ceo_source_inputs"][0]["arguments"]["path"] = json!(original);
        f.db()
            .execute("UPDATE jobs SET intake=?1", [normalized_intake.to_string()])
            .unwrap();
        assert!(registry
            .call(
                &f.authority,
                &f.actor("Reader"),
                "files",
                json!({"op":"write","path":normalized,"content":"overwrite"})
            )
            .is_err());
        assert_eq!(std::fs::read(root.join(original)).unwrap(), bytes);
    }
    let bad = json!({"ceo_source_inputs":intake["ceo_source_inputs"],"artifact_bindings":[{"path":"clip.wav","source_actor":"Worker","writer":"Reader","source_tool":"audio_inspect","arguments":args,"pointer":"/frame_count","encoding":"integer_text"}]});
    assert!(artifact_bindings::parse(&bad).is_err());
    assert_eq!(std::fs::read(root.join("clip.wav")).unwrap(), bytes);
    assert_eq!(receipts(&f), 0);
}
#[cfg(unix)]
#[test]
fn symlink_and_hardlink_audio_sources_are_rejected() {
    let (f, bytes, args) = fixture();
    let root = f.authority.workspace(&f.scope).unwrap();
    let registry = Registry::new();
    std::os::unix::fs::symlink(root.join("clip.wav"), root.join("linked.wav")).unwrap();
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "audio_inspect",
            json!({"path":"linked.wav","expected_sha256":gents_local_mcp::digest(&bytes)})
        )
        .is_err());
    std::fs::hard_link(root.join("clip.wav"), root.join("hard.wav")).unwrap();
    assert!(registry
        .call(&f.authority, &f.scope, "audio_inspect", args)
        .is_err());
    assert_eq!(receipts(&f), 0);
}
#[tokio::test]
async fn actual_stdio_and_sse_audio_inspection_keep_authority_and_source_hashes() {
    let (f, bytes, args) = fixture();
    let root = f.root.join("target/debug");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        root.join(format!("gents-local-mcp{}", std::env::consts::EXE_SUFFIX)),
    )
    .unwrap();
    let old = std::env::var_os("GENTS_MCP_TRANSPORT");
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let result = gents_local_mcp::client::invoke(
            &f.root,
            &f.scope,
            "audio_inspect",
            &args,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(
            result["result"]["source_sha256"],
            gents_local_mcp::digest(&bytes)
        );
        assert_eq!(result["result"]["channel_statistics"][0]["rail_samples"], 2);
        assert_eq!(result["mcp_receipt"]["provider_cost_usd"], 0);
    }
    if let Some(old) = old {
        std::env::set_var("GENTS_MCP_TRANSPORT", old);
    } else {
        std::env::remove_var("GENTS_MCP_TRANSPORT");
    }
    assert_eq!(receipts(&f), 2);
}

mod support;
use gents_local_mcp::{artifact_bindings, authority::Authority, governance, Registry};
use serde_json::json;
use sha2::{Digest, Sha256};
use support::Fixture;
fn fixture() -> Fixture {
    let f = Fixture::new();
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let prior = policy.revision;
    policy.revision += 1;
    for access in [&mut policy.grants, &mut policy.ceilings] {
        access
            .get_mut("Worker")
            .unwrap()
            .tools
            .extend(["table_read_json".into(), "table_aggregate".into()]);
        access
            .get_mut("Reader")
            .unwrap()
            .tools
            .extend(["files:write".into(), "files:read".into()]);
    }
    governance::save(&db, &f.scope.run.to_string(), &policy, prior).unwrap();
    drop(db);
    std::fs::create_dir_all(f.authority.workspace(&f.scope).unwrap()).unwrap();
    f
}
fn seed(f: &Fixture, text: &str) {
    std::fs::write(
        f.authority.workspace(&f.scope).unwrap().join("input.json"),
        text,
    )
    .unwrap();
}
#[test]
fn complete_file_has_exact_hash_and_native_chained_rows() {
    let f = fixture();
    let text =
        "[ {\"k\":\"north\",\"n\":7}, {\"k\":\"north\",\"n\":-2}, {\"k\":\"south\",\"n\":0} ]\n";
    seed(&f, text);
    let source = json!({"path":"input.json"});
    let aggregate = json!({"rows":[],"filter":null,"group_by":"k","value_field":"n"});
    let intake = json!({"artifact_bindings":[
        {"path":"rows.json","source_actor":"Worker","writer":"Reader","source_tool":"table_read_json","arguments":source,"pointer":"/rows","encoding":"json"},
        {"path":"total.txt","source_actor":"Worker","writer":"Reader","source_tool":"table_aggregate","arguments":aggregate,"argument_sources":[{"field":"rows","path":"rows.json"}],"pointer":"/total","encoding":"integer_text"}
    ]});
    artifact_bindings::parse(&intake).unwrap();
    f.db()
        .execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    let registry = Registry::new();
    let parsed = registry
        .call(&f.authority, &f.scope, "table_read_json", source.clone())
        .unwrap();
    assert_eq!(parsed["value"]["source_bytes"], text.len());
    assert_eq!(
        parsed["value"]["source_sha256"],
        format!("{:x}", Sha256::digest(text.as_bytes()))
    );
    let next = artifact_bindings::next_call(&f.authority, &f.scope, false)
        .unwrap()
        .unwrap();
    assert_eq!(next["arguments"]["rows"], parsed["value"]["rows"]);
    let result = registry
        .call(
            &f.authority,
            &f.scope,
            "table_aggregate",
            next["arguments"].clone(),
        )
        .unwrap();
    assert_eq!(result["value"]["total"], 5);
    for _ in 0..4 {
        let next = artifact_bindings::next_call(&f.authority, &f.actor("Reader"), true)
            .unwrap()
            .unwrap();
        registry
            .call(
                &f.authority,
                &f.actor("Reader"),
                "files",
                next["arguments"].clone(),
            )
            .unwrap();
    }
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()["passed"],
        true
    );
    // A new observed source value invalidates old consumer evidence.
    seed(&f, "[{\"k\":\"north\",\"n\":9}]");
    registry
        .call(&f.authority, &f.scope, "table_read_json", source)
        .unwrap();
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()["passed"],
        false
    );
}
#[test]
fn ambiguous_invalid_partial_and_sensitive_tables_fail_closed() {
    let f = fixture();
    let registry = Registry::new();
    let mut invalid = vec![
        "[{\"n\":1,\"n\":2}]".to_string(),
        r#"[{"n":1,"\u006e":2}]"#.into(),
        "[{\"n\":1.0}]".into(),
        "[{\"n\":1e0}]".into(),
        "[{\"n\":null,\"bad-key\":1}]".into(),
        "[{\"n\":{}}]".into(),
        "[{\"n\":[]}]".into(),
        "[] []".into(),
        "{}".into(),
        "[1]".into(),
        "[{}]".into(),
        "[".into(),
        "[{\"n\":1000000000001}]".into(),
        json!((0..65).map(|_| json!({"n":1})).collect::<Vec<_>>()).to_string(),
        json!([{ "s":"a".repeat(129)}]).to_string(),
        format!("[]{}[]", " ".repeat(8190)), // valid first page, hidden trailing document
        "[]".to_string() + &" ".repeat(8191), // oversized even with only whitespace
    ];
    invalid.push(json!([{ "n":"4111 1111 1111 1111"}]).to_string());
    invalid.push(r#"[{"n":"\u0034\u0031\u0031\u0031 1111 1111 1111"}]"#.into());
    for (index, text) in invalid.into_iter().enumerate() {
        seed(&f, &text);
        assert!(
            registry
                .call(
                    &f.authority,
                    &f.scope,
                    "table_read_json",
                    json!({"path":"input.json"})
                )
                .is_err(),
            "accepted invalid input shape/size at fixture index {index}"
        );
    }
    for text in [
        "[]",
        "[ {\"s\":\"雪\",\"n\":-1,\"b\":true,\"v\":null} ]",
        &format!("[]{}", " ".repeat(8190)),
    ] {
        seed(&f, text);
        registry
            .call(
                &f.authority,
                &f.scope,
                "table_read_json",
                json!({"path":"input.json"}),
            )
            .unwrap();
    }
}
#[test]
fn read_authority_does_not_grant_general_files_or_other_actors() {
    let f = fixture();
    seed(&f, "[]");
    let registry = Registry::new();
    assert!(registry
        .call(
            &f.authority,
            &f.actor("Reader"),
            "table_read_json",
            json!({"path":"input.json"})
        )
        .is_err());
    assert!(registry
        .call(
            &f.authority,
            &f.scope,
            "files",
            json!({"op":"read","path":"input.json"})
        )
        .is_err());
    for path in [
        "../input.json",
        "/etc/passwd",
        ".env",
        "nested/../input.json",
    ] {
        assert!(registry
            .call(
                &f.authority,
                &f.scope,
                "table_read_json",
                json!({"path":path})
            )
            .is_err());
    }
    #[cfg(unix)]
    {
        let workspace = f.authority.workspace(&f.scope).unwrap();
        std::os::unix::fs::symlink(workspace.join("input.json"), workspace.join("link.json"))
            .unwrap();
        assert!(registry
            .call(
                &f.authority,
                &f.scope,
                "table_read_json",
                json!({"path":"link.json"})
            )
            .is_err());
        std::fs::hard_link(workspace.join("input.json"), workspace.join("hard.json")).unwrap();
        assert!(registry
            .call(
                &f.authority,
                &f.scope,
                "table_read_json",
                json!({"path":"hard.json"})
            )
            .is_err());
    }
    registry
        .validate_projection(
            "table_read_json",
            &json!({"path":"input.json"}),
            "/rows",
            "json",
        )
        .unwrap();
    assert!(registry
        .validate_projection(
            "table_read_json",
            &json!({"path":"input.json"}),
            "/rows",
            "integer_text"
        )
        .is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn actual_stdio_import_preserves_types_and_rejects_duplicate_columns() {
    let f = fixture();
    let binary = f.root.join("target/debug");
    std::fs::create_dir_all(&binary).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_gents-local-mcp"),
        binary.join("gents-local-mcp"),
    )
    .unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    seed(&f, r#"[{"k":"雪","n":-2,"ok":true,"v":null}]"#);
    let parsed = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "table_read_json",
        &json!({"path":"input.json","expected_sha256":format!("{:x}",Sha256::digest(r#"[{"k":"雪","n":-2,"ok":true,"v":null}]"#.as_bytes()))}),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["mcp_receipt"]["tool"], "table_read_json");
    assert_eq!(
        parsed["result"]["rows"],
        json!([{"k":"雪","n":-2,"ok":true,"v":null}])
    );
    seed(&f, r#"[{"n":1,"n":2}]"#);
    let result = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "table_read_json",
        &json!({"path":"input.json"}),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(result["ok"], false);
}

#[test]
fn approved_hash_rejects_changed_bytes_before_any_source_receipt() {
    let f = fixture();
    let registry = Registry::new();
    let original = r#"[{"k":"north","n":7}]"#;
    let hash = format!("{:x}", Sha256::digest(original.as_bytes()));
    let args = json!({"path":"input.json","expected_sha256":hash});
    let intake = json!({"artifact_bindings":[{"path":"rows.json","source_actor":"Worker","writer":"Reader","source_tool":"table_read_json","arguments":args,"pointer":"/rows","encoding":"json"}]});
    f.db()
        .execute("UPDATE jobs SET intake=?1", [intake.to_string()])
        .unwrap();
    // Same-size, schema-valid content must still fail the frozen byte contract.
    seed(&f, r#"[{"k":"north","n":9}]"#);
    let error = registry
        .call(&f.authority, &f.scope, "table_read_json", args.clone())
        .unwrap_err();
    assert!(error.to_string().contains("source hash differs"));
    assert_eq!(
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        artifact_bindings::check(&f.authority, &f.actor("Reader"), true).unwrap()["passed"],
        false
    );
    // Equivalent parsed data is insufficient: formatting belongs to the bytes.
    seed(&f, &format!("{original}\n"));
    assert!(registry
        .call(&f.authority, &f.scope, "table_read_json", args.clone())
        .is_err());
    seed(&f, original);
    let result = registry
        .call(&f.authority, &f.scope, "table_read_json", args)
        .unwrap();
    assert_eq!(result["value"]["source_sha256"], hash);
    assert_eq!(result["value"]["rows"][0]["n"], 7);
    for hash in ["A".repeat(64), "g".repeat(64), "0".repeat(63)] {
        assert!(registry
            .call(
                &f.authority,
                &f.scope,
                "table_read_json",
                json!({"path":"input.json","expected_sha256":hash})
            )
            .is_err());
    }
}

#[test]
fn complete_reads_enforce_the_file_ceiling_before_page_decoding() {
    let f = fixture();
    let root = f.authority.workspace(&f.scope).unwrap();
    // A sparse oversized non-UTF8 file is rejected by metadata, before decode.
    let file = std::fs::File::create(root.join("input.json")).unwrap();
    file.set_len(1024 * 1024).unwrap();
    let directory = gents_local_mcp::workspace::open(&root, false).unwrap();
    let error = directory.read_complete("input.json", 8192).unwrap_err();
    assert!(error.to_string().contains("up to 8192 bytes"));
    assert!(directory.read_complete("input.json", 0).is_err());
    assert!(directory.read_complete("input.json", 8193).is_err());
}

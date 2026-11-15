#![cfg(unix)]
mod support;
use gents_local_mcp::{client, digest};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, sync::atomic::AtomicBool};
struct Unseal(std::path::PathBuf);
impl Drop for Unseal {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
    }
}

// This integration binary has one test, so its temporary environment changes
// cannot race another test's transport/runtime configuration in this process.
#[tokio::test]
async fn real_stdio_and_sse_use_verified_copy_after_build_path_changes_and_reject_tampering() {
    let f = support::Fixture::new();
    let id = "c".repeat(64);
    let relative = format!("data/mcp-runtime-bundles/{id}/manifest.json");
    let folder = f.root.join(&relative).parent().unwrap().to_path_buf();
    fs::create_dir_all(&folder).unwrap();
    let mut executables = serde_json::Map::new();
    for role in [
        "gents-harness",
        "gents-local-mcp",
        "gents-sqlite-bridge",
        "gents-rust-acceptance-check",
    ] {
        let path = folder.join(role);
        if role == "gents-local-mcp" {
            fs::copy(env!("CARGO_BIN_EXE_gents-local-mcp"), &path).unwrap();
        } else {
            fs::write(
                &path,
                b"Unused role placeholder in this private MCP-only test",
            )
            .unwrap();
        }
        let bytes = fs::read(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
        executables.insert(
            role.into(),
            json!({"file":role,"bytes":bytes.len(),"sha256":digest(&bytes)}),
        );
    }
    let manifest = json!({"schema_version":1,"id":id,"executables":executables}).to_string();
    fs::write(f.root.join(&relative), &manifest).unwrap();
    fs::set_permissions(f.root.join(&relative), fs::Permissions::from_mode(0o400)).unwrap();
    let _unseal = Unseal(folder.clone());
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o500)).unwrap();
    fs::create_dir_all(f.root.join("target/debug")).unwrap();
    fs::write(
        f.root.join("target/debug/gents-local-mcp"),
        b"replacement build must never be launched",
    )
    .unwrap();
    std::env::set_var("GENTS_RUNTIME_MANIFEST", &relative);
    std::env::set_var("GENTS_RUNTIME_MANIFEST_SHA256", digest(manifest.as_bytes()));
    let args = json!({"calculations":[{"id":"proof","values":[8,5],"subtract":2}]});
    for transport in ["stdio", "sse"] {
        std::env::set_var("GENTS_MCP_TRANSPORT", transport);
        let result = client::invoke(
            &f.root,
            &f.scope,
            "integer_arithmetic",
            &args,
            &AtomicBool::new(false),
        )
        .await
        .unwrap();
        assert_eq!(result["ok"], true);
        assert_eq!(result["result"]["calculations"][0]["difference"], 11);
        assert_eq!(result["mcp_receipt"]["provider_cost_usd"], 0);
    }
    let count = || {
        f.db()
            .query_row(
                "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert_eq!(count(), 2);
    let binary = folder.join("gents-local-mcp");
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&binary, b"tampered").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o500)).unwrap();
    assert!(client::invoke(
        &f.root,
        &f.scope,
        "integer_arithmetic",
        &args,
        &AtomicBool::new(false)
    )
    .await
    .is_err());
    assert_eq!(count(), 2);
    for name in [
        "GENTS_RUNTIME_MANIFEST",
        "GENTS_RUNTIME_MANIFEST_SHA256",
        "GENTS_MCP_TRANSPORT",
    ] {
        std::env::remove_var(name);
    }
    fs::set_permissions(folder, fs::Permissions::from_mode(0o700)).unwrap();
}

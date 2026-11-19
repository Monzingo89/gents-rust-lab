//! Private host protocol checks. These are not model-authored task results.
use gents_local_mcp::{digest, rust_verification::verify_snapshot};
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn request(source: &str, tests: &str) -> Value {
    json!({"op":"verify","source":source,"source_sha256":digest(source.as_bytes()),"tests":tests,"tests_sha256":digest(tests.as_bytes())})
}
fn call(request: &Value) -> (bool, Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gents-rust-component"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.stderr.len() < 16384);
    assert!(output.stdout.len() < 131072);
    (
        output.status.success(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}

#[test]
fn hashes_and_private_request_shape_are_checked_before_execution() {
    let source = "pub fn remaining(used:usize,limit:usize)->usize{limit.saturating_sub(used)}";
    let tests = "#[test]fn check(){std::assert_eq!(candidate::remaining(5,3),0);}";
    let mut wrong = request(source, tests);
    wrong["tests_sha256"] = json!("0".repeat(64));
    let (ok, value) = call(&wrong);
    assert!(!ok);
    assert_eq!(value["request_rejected"], true);
    wrong = request(source, tests);
    wrong["source_sha256"] = json!("0".repeat(64));
    assert!(!call(&wrong).0);
    wrong = request(source, tests);
    wrong["compiler_flags"] = json!(["--cfg", "fake_pass"]);
    assert!(!call(&wrong).0);
    assert!(verify_snapshot(source, "wrong", tests, &digest(tests.as_bytes())).is_err());
}

#[test]
fn candidate_cannot_replace_acceptance_or_escape_through_imports_macros_and_exit() {
    let tests = "#[test]fn check(){std::assert_eq!(candidate::remaining(5,3),0);}";
    for source in [
        "pub fn remaining(_:usize,_:usize)->usize{std::process::exit(0)}",
        "#[test]fn replacement(){}",
        "pub fn remaining(_:usize,_:usize)->usize{include!(\"acceptance.rs\")}",
        "pub fn remaining(_:usize,_:usize)->usize{unsafe{0}}",
    ] {
        let (ok, result) = call(&request(source, tests));
        assert!(!ok);
        assert_eq!(result["passed"], false);
        assert_eq!(result["stage"], "admission");
        assert_eq!(result["source_sha256"], digest(source.as_bytes()));
        assert_eq!(result["tests_sha256"], digest(tests.as_bytes()));
    }
    let source = "pub fn remaining(_:usize,_:usize)->usize{0}";
    let bad = "#[test]fn check(){std::process::exit(0)}";
    let (_, result) = call(&request(source, bad));
    assert_eq!(result["stage"], "admission");
    assert_eq!(result["passed"], false);
}

#[test]
#[cfg(target_os = "macos")]
fn actual_isolated_acceptance_distinguishes_passing_failing_and_uncompilable_bytes() {
    let tests = "#[test]fn empty(){std::assert_eq!(candidate::remaining(0,7),7);}\n#[test]fn spent(){std::assert_eq!(candidate::remaining(8,3),0);}";
    for (source, expected, stage) in [
        (
            "pub fn remaining(used:usize,limit:usize)->usize{limit.saturating_sub(used)}",
            true,
            "acceptance",
        ),
        (
            "pub fn remaining(_:usize,_:usize)->usize{0}",
            false,
            "acceptance",
        ),
        (
            "pub fn remaining(_:usize,_:usize)->usize{unknown_function()}",
            false,
            "compile_candidate",
        ),
    ] {
        let (ok, result) = call(&request(source, tests));
        assert_eq!(ok, expected, "{result}");
        assert_eq!(result["passed"], expected);
        assert_eq!(result["stage"], stage);
        assert_eq!(result["source_sha256"], digest(source.as_bytes()));
        assert_eq!(result["tests_sha256"], digest(tests.as_bytes()));
        assert_eq!(result["provider_cost_usd"], 0);
        assert_eq!(result["promotion_authorized"], false);
        assert!(result.get("mcp_receipt").is_none());
        if expected {
            assert_eq!(result["tests_count"], 2);
            assert_eq!(result["diagnostics"]["passed"], true);
        }
    }
}

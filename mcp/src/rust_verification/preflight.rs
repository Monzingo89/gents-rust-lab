//! Parser-backed facts about proposed bytes; no compilation, execution or authority.
use serde_json::{json, Value};
/// Trusted-host fixture preparation only. Does not execute tests or grant
/// acceptance, and deliberately returns no source-dependent diagnostic text.
pub fn acceptance_policy_observation(source: &str) -> Value {
    let checked = super::source_policy::validate(source, true);
    json!({"passed":checked.is_ok(),"source_sha256":crate::digest(source.as_bytes()),
        "source_bytes":source.len(),"parser_policy_sha256":crate::digest(include_str!("source_policy.rs").as_bytes()),
        "tests_count":checked.ok(),
        "scope":"Owner-frozen acceptance syntax, dialect and test count only. No compilation, execution, model/task result, permission or promotion. Rejected source details are not echoed."})
}
pub fn source_policy_observation(source: &str) -> Value {
    let result = super::source_policy::validate(source, false);
    json!({"passed":result.is_ok(),"source_sha256":crate::digest(source.as_bytes()),
        "parser_policy_sha256":crate::digest(include_str!("source_policy.rs").as_bytes()),
        "diagnostic":result.err().map(|error|format!("{error:#}").chars().take(600).collect::<String>()),
        "scope":"proposed source syntax and strict dialect only; no authority, compilation, tests, correctness or promotion"})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acceptance_preflight_uses_exact_native_policy_without_echoing_source() {
        let valid =
            "#[test]fn check(){let mut v=Vec::new();v.resize(2,0i64);std::assert_eq!(v.len(),2);}";
        let result = acceptance_policy_observation(valid);
        assert_eq!(result["passed"], true);
        assert_eq!(result["tests_count"], 1);
        for bad in [
            "#[test]fn check(){let v=std::vec![0;2];std::assert_eq!(v.len(),2);}",
            "pub fn not_a_test(){}",
            "#[test]fn secret_marker_123(){SECRET_MARKER!()}",
        ] {
            let result = acceptance_policy_observation(bad);
            assert_eq!(result["passed"], false);
            assert!(result["tests_count"].is_null());
            assert!(!result.to_string().contains("SECRET_MARKER"));
            assert_eq!(result["source_sha256"], crate::digest(bad.as_bytes()));
        }
    }
    #[test]
    fn vector_methods_are_not_macros_and_neither_implies_acceptance() {
        let source = "pub fn zeros(n:usize)->Vec<u64>{let mut v=Vec::new();v.resize(n,0);v}";
        let fact = source_policy_observation(source);
        assert_eq!(fact["passed"], true);
        assert_eq!(fact["source_sha256"], crate::digest(source.as_bytes()));
        assert_eq!(
            source_policy_observation("pub fn zeros(n:usize)->Vec<u64>{vec![0;n]}")["passed"],
            false
        );
        assert_eq!(
            source_policy_observation("pub fn bad(){std::process::exit(0)}")["passed"],
            false
        );
        assert!(fact.get("tests_count").is_none());
    }
    #[test]
    fn source_syntax_feedback_preserves_a_bounded_underlying_parser_cause() {
        let source = "pub fn broken( {";
        let fact = source_policy_observation(source);
        assert_eq!(fact["passed"], false);
        assert_eq!(fact["source_sha256"], crate::digest(source.as_bytes()));
        let diagnostic = fact["diagnostic"].as_str().unwrap();
        assert!(diagnostic.starts_with("invalid Rust syntax: "));
        assert!(diagnostic.len() > "invalid Rust syntax: ".len());
        assert!(diagnostic.chars().count() <= 600);
        assert_eq!(
            super::super::source_policy::validate(source, true)
                .unwrap_err()
                .to_string(),
            "invalid Rust syntax"
        );
        // Frozen acceptance preflight retains its deliberate no-diagnostic view.
        assert!(acceptance_policy_observation(source)
            .get("diagnostic")
            .is_none());
    }
}

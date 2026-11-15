//! Small current-failure context; complete native diagnostics stay in receipts/journals.
use serde_json::Value;
pub fn excerpt(value: &Value) -> String {
    let text = if let Some(text) = value.as_str() {
        text.to_owned()
    } else {
        let stderr = value["stderr"].as_str().unwrap_or("");
        let stdout = value["stdout"].as_str().unwrap_or("");
        let output = if !stderr.is_empty() { stderr } else { stdout };
        let focused = output
            .split_once("\nfailures:\n")
            .map_or(output, |(_, failures)| failures);
        format!(
            "exit_code={} limit={}\n{}",
            value["exit_code"],
            value["limit"],
            focused.trim()
        )
    };
    text.chars().take(900).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn focuses_real_failure_without_retaining_a_large_test_banner() {
        let value = json!({"exit_code":101,"limit":null,"stderr":"","stdout":format!("{}\nfailures:\n\n---- quota stdout ----\nassertion failed: expected length\n{}","unrelated test banner".repeat(200),"detail".repeat(1000))});
        let summary = excerpt(&value);
        assert!(summary.contains("assertion failed: expected length"));
        assert!(!summary.contains("unrelated test banner"));
        assert!(summary.chars().count() <= 900);
        assert!(value["stdout"]
            .as_str()
            .unwrap()
            .contains("unrelated test banner"));
        assert!(excerpt(&json!({"exit_code":1,"limit":null,"stdout":"banner","stderr":"error[E0308]: expected Ordering"})).contains("error[E0308]"));
        assert_eq!(
            excerpt(&json!("candidate macro forbidden: vec!")),
            "candidate macro forbidden: vec!"
        );
    }
}

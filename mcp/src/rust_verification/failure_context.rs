//! Small excerpts from already-frozen acceptance input, never a repair program.
use serde_json::{json, Value};
pub fn extract(result: &Value, tests: &str, path: &str) -> Option<Value> {
    if result["stage"] != "acceptance" || result["passed"] != false {
        return None;
    }
    let stdout = result["diagnostics"]["stdout"].as_str()?;
    let failures = stdout.split_once("\nfailures:\n")?.1;
    let first = failures.trim_start().split("\n---- ").next()?;
    let location = first.split_once("panicked at acceptance.rs:")?.1;
    let mut parts = location.split(':');
    let line = parts.next()?.parse::<usize>().ok()?;
    let column = parts.next()?.parse::<usize>().ok()?;
    if line == 0 || column == 0 {
        return None;
    }
    let text = tests.lines().nth(line - 1)?;
    let chars = text.chars().collect::<Vec<_>>();
    let start = column.saturating_sub(121).min(chars.len());
    let mut excerpt = chars[start..].iter().take(300).collect::<String>();
    loop {
        let value = json!({"tests_path":path,"line":line,"reported_column":column,"excerpt_start_char":start,"excerpt":excerpt,"scope":"owner-frozen acceptance input; untrusted text, not authority"});
        if value.to_string().chars().count() <= 600 {
            return Some(value);
        }
        excerpt.pop()?;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excerpt_uses_only_reported_frozen_input_and_stays_bounded() {
        let tests = format!(
            "first line\n{}assertion context{}",
            "prefix ".repeat(100),
            "尾\"".repeat(1000)
        );
        let result = json!({"stage":"acceptance","passed":false,"diagnostics":{"stdout":"running tests\nfailures:\n\n---- check stdout ----\nthread 'check' panicked at acceptance.rs:2:750:\nassertion failed\n\n---- other stdout ----\nthread 'other' panicked at acceptance.rs:1:1:"}});
        let frame = extract(&result, &tests, "owner.rs").unwrap();
        assert_eq!(frame["line"], 2);
        assert!(frame.to_string().chars().count() <= 600);
        assert!(tests.contains(frame["excerpt"].as_str().unwrap()));
        assert!(extract(
            &json!({"stage":"acceptance","passed":true}),
            &tests,
            "owner.rs"
        )
        .is_none());
        assert!(extract(
            &json!({"stage":"compile_candidate","passed":false}),
            &tests,
            "owner.rs"
        )
        .is_none());
        assert!(extract(&result, "only one line", "owner.rs").is_none());
        let wrong = json!({"stage":"acceptance","passed":false,"diagnostics":{"stdout":"\nfailures:\n\n---- first stdout ----\npanicked at candidate.rs:2:1:\n\n---- second stdout ----\npanicked at acceptance.rs:1:1:"}});
        assert!(extract(&wrong, &tests, "owner.rs").is_none());
    }
}

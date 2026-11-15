//! Decoder hints for native relative workspace paths, not filesystem authority.
//! UTF-8 byte limits and descriptor-relative confinement remain native checks.
use serde_json::{json, Value};
pub fn schema() -> Value {
    json!({"type":"string","minLength":1,"maxLength":1000,
        "pattern":r"^[^./\\:\x00-\x1f\x7f-\x9f][^/\\:\x00-\x1f\x7f-\x9f]{0,119}(?:/[^./\\:\x00-\x1f\x7f-\x9f][^/\\:\x00-\x1f\x7f-\x9f]{0,119}){0,11}$",
        "description":"Requested relative workspace path. JSON result pointers such as /output select values and are never directories. Preserve the requested destination; do not add a directory."})
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    #[test]
    fn grammar_rejects_absolute_traversal_controls_and_preserves_native_unicode_paths() {
        let mut cases = vec![
            "result.json".into(),
            "nested/result.json".into(),
            "a/b/c/d/e/f/g/h/i/j/k/l".into(),
            "space here/λ😀.json".into(),
            "/output/file.json".into(),
            "/absolute".into(),
            "".into(),
            ".hidden".into(),
            "a/.hidden".into(),
            "a/../b".into(),
            "a//b".into(),
            "a/".into(),
            "a\\b".into(),
            "a:b".into(),
            "a\0b".into(),
            "a\u{85}b".into(),
            "a/b/c/d/e/f/g/h/i/j/k/l/m".into(),
        ];
        let fixed = cases.len();
        for symbol in ["x", "λ", "😀"] {
            for count in 1..=121 {
                cases.push(symbol.repeat(count));
            }
        }
        let mut child=Command::new("node").args(["--input-type=module","-e",r"let text='';for await(const c of process.stdin)text+=c;const {schema,cases}=JSON.parse(text),pattern=new RegExp(schema.pattern,'u');process.stdout.write(JSON.stringify(cases.map(s=>s.length>=schema.minLength&&[...s].length<=schema.maxLength&&pattern.test(s))));"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                json!({"schema":schema(),"cases":cases})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        let accepted: Vec<bool> = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(accepted.len(), cases.len());
        for (index, (path, hint)) in cases.iter().zip(accepted).enumerate() {
            let native = gents_local_mcp::workspace_path::is_safe_relative_path(path);
            if native {
                assert!(hint, "valid native path excluded: {path:?}");
            }
            if index < fixed {
                assert_eq!(hint, native, "{path:?}");
            }
        }
        // This is a character-based decoder hint; the unchanged byte validator
        // remains authoritative for multibyte names beyond its 120-byte bound.
        assert!(!gents_local_mcp::workspace_path::is_safe_relative_path(
            &"😀".repeat(31)
        ));
    }
}

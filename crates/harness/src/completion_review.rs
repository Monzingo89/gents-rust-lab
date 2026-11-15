//! Review wire protocol only. Criterion judgments remain model decisions;
//! native execution, policy and artifact gates remain separate authorities.
use anyhow::{ensure, Result};
use serde::{
    de::{MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, fmt};

pub const MAX_CRITERIA: usize = 64;
pub const MAX_ATTEMPTS: usize = 3;
pub const FORMAT:&str="Return {verdict:accepted|revise|uncertain,summary:string,checks:{\"0\":{passed:boolean,evidence:string},...}}. checks must use exactly the zero-based keys in criteria, every key once. Do not renumber, omit or duplicate keys. Each passed value and the verdict remain your independent judgment; formatting never establishes acceptance.";

pub fn criteria(values: &[String]) -> Result<Value> {
    ensure!(
        values.len() <= MAX_CRITERIA,
        "review packet exceeds 64 criteria; bounded review batches required"
    );
    Ok(Value::Object(
        values
            .iter()
            .enumerate()
            .map(|(i, text)| (i.to_string(), json!(text)))
            .collect(),
    ))
}
pub fn count(value: &Value) -> Result<usize> {
    let n = if let Some(rows) = value.as_array() {
        rows.len()
    } else {
        let rows = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("review criteria must be an indexed object or array"))?;
        ensure!(
            (0..rows.len()).all(|i| rows.contains_key(&i.to_string())),
            "review criteria keys are not canonical zero-based IDs"
        );
        rows.len()
    };
    ensure!(
        n <= MAX_CRITERIA,
        "review packet exceeds 64 criteria; bounded review batches required"
    );
    Ok(n)
}
pub fn schema(n: usize) -> Result<Value> {
    ensure!(
        n <= MAX_CRITERIA,
        "review packet exceeds 64 criteria; bounded review batches required"
    );
    let item = json!({"type":"object","properties":{"passed":{"type":"boolean"},"evidence":{"type":"string","minLength":1,"maxLength":160}},"required":["passed","evidence"],"additionalProperties":false});
    let fields: Map<String, Value> = (0..n).map(|i| (i.to_string(), item.clone())).collect();
    Ok(
        json!({"type":"object","properties":{"verdict":{"type":"string","enum":["accepted","revise","uncertain"]},"summary":{"type":"string","minLength":1,"maxLength":600},"checks":{"type":"object","properties":fields,"required":(0..n).map(|i|i.to_string()).collect::<Vec<_>>(),"additionalProperties":false}},"required":["verdict","summary","checks"],"additionalProperties":false}),
    )
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Verdict {
    Accepted,
    Revise,
    Uncertain,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Check {
    passed: bool,
    evidence: String,
}
#[derive(Serialize)]
struct Checks(BTreeMap<String, Check>);
impl<'de> Deserialize<'de> for Checks {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Unique;
        impl<'de> Visitor<'de> for Unique {
            type Value = Checks;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("unique criterion-keyed checks")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Checks, A::Error> {
                let mut rows = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Check>()? {
                    if rows.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("duplicate criterion key"));
                    }
                    if rows.len() > MAX_CRITERIA {
                        return Err(serde::de::Error::custom("too many criterion keys"));
                    }
                }
                Ok(Checks(rows))
            }
        }
        deserializer.deserialize_map(Unique)
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Review {
    verdict: Verdict,
    summary: String,
    checks: Checks,
}

pub struct Assessment {
    pub review: Value,
    pub accepted: bool,
    pub errors: Vec<String>,
    pub error_count: usize,
}
impl Assessment {
    pub fn protocol_valid(&self) -> bool {
        self.error_count == 0
    }
    pub fn feedback(&self) -> Value {
        json!({"valid":self.protocol_valid(),"errors":self.errors,"error_count":self.error_count,"scope":"Host review-format diagnostics only. No source failure, tool execution, authority or acceptance inferred. Return a fresh independent review; never repair or reindex an old response."})
    }
}
pub fn assess(text: &str, n: usize) -> Assessment {
    let invalid = |error: String| Assessment {
        review: Value::Null,
        accepted: false,
        errors: vec![error],
        error_count: 1,
    };
    if n > MAX_CRITERIA || text.len() > 64_000 {
        return invalid("Review protocol size bound exceeded".into());
    }
    let review:Review=match serde_json::from_str(text){
        Ok(value)=>value,
        Err(error)=>return invalid(format!("Invalid review envelope at line {} column {}; require unique criterion-keyed checks with boolean passed and nonempty evidence",error.line(),error.column())),
    };
    let mut errors = Vec::new();
    if review.summary.trim().is_empty() || review.summary.chars().count() > 600 {
        errors.push("Summary must contain 1..600 characters".into());
    }
    for i in 0..n {
        match review.checks.0.get(&i.to_string()) {
            None => errors.push(format!("Missing criterion key {i}")),
            Some(check)
                if check.evidence.trim().is_empty() || check.evidence.chars().count() > 160 =>
            {
                errors.push(format!("Criterion {i} needs 1..160 evidence characters"))
            }
            _ => {}
        }
    }
    if review.checks.0.keys().any(|key| {
        key.parse::<usize>()
            .ok()
            .is_none_or(|i| i >= n || i.to_string() != *key)
    }) {
        errors.push("Unexpected criterion key outside the frozen zero-based IDs".into());
    }
    let error_count = errors.len();
    errors.truncate(8);
    let accepted = error_count == 0
        && matches!(review.verdict, Verdict::Accepted)
        && review.checks.0.values().all(|check| check.passed);
    Assessment {
        review: serde_json::to_value(review).expect("typed review serialization"),
        accepted,
        errors,
        error_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_cover_exact_frozen_criteria_without_fixing_judgments() {
        let wire = schema(2).unwrap();
        assert_eq!(wire["properties"]["checks"]["required"], json!(["0", "1"]));
        assert!(
            wire["properties"]["checks"]["properties"]["0"]["properties"]["passed"]
                .get("const")
                .is_none()
        );
        assert_eq!(
            criteria(&["first".into(), "second".into()]).unwrap(),
            json!({"0":"first","1":"second"})
        );
        assert_eq!(count(&json!({"0":"first","1":"second"})).unwrap(), 2);
        assert!(count(&json!({"1":"wrong start"})).is_err());
        assert!(schema(MAX_CRITERIA + 1).is_err());
    }
    #[test]
    fn malformed_duplicate_missing_and_unknown_checks_never_become_acceptance() {
        for text in [
            r#"{"verdict":"accepted","summary":"done","checks":{"1":{"passed":true,"evidence":"x"}}}"#,
            r#"{"verdict":"accepted","summary":"done","checks":{"0":{"passed":true,"evidence":"x"},"0":{"passed":true,"evidence":"y"}}}"#,
            r#"{"verdict":"accepted","summary":"done","checks":[{"criterion_index":0,"passed":true,"evidence":"x"}]}"#,
            r#"{"verdict":"accepted","summary":"done","checks":{"00":{"passed":true,"evidence":"x"}}}"#,
        ] {
            let result = assess(text, 1);
            assert!(!result.accepted);
            assert!(!result.protocol_valid());
            assert!(result.error_count > 0);
        }
    }
    #[test]
    fn valid_rejection_remains_rejection_and_true_checks_do_not_override_uncertainty() {
        for verdict in ["accepted", "revise", "uncertain"] {
            for passed in [false, true] {
                let text=json!({"verdict":verdict,"summary":"Independent review","checks":{"0":{"passed":passed,"evidence":"Current source receipt"}}}).to_string();
                let result = assess(&text, 1);
                assert!(result.protocol_valid());
                assert_eq!(result.accepted, verdict == "accepted" && passed);
            }
        }
    }
}

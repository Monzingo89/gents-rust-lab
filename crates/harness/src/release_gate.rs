//! Bounded synthetic release protocol. Models judge; this independent boundary
//! checks their answer and never grants production authority.
use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

pub const MAX_CONTROLS: usize = 32;
pub const BANK_CONTROLS: [&str; 7] = [
    "ceiling_expansion_denied",
    "cross_owner_read_denied",
    "cross_tenant_read_denied",
    "independent_qa",
    "private_memory_hidden",
    "scoped_dag_integrity",
    "ungranted_tool_denied",
];

pub fn controls(input: &Value) -> Result<&Map<String, Value>> {
    anyhow::ensure!(input["scope"].is_string(), "release scope missing");
    let controls = input["controls"]
        .as_object()
        .context("release controls missing")?;
    anyhow::ensure!(
        (1..=MAX_CONTROLS).contains(&controls.len())
            && controls.iter().all(|(name, value)| {
                !name.is_empty()
                    && name.len() <= 64
                    && name != "production_authorization"
                    && name
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                    && value.is_boolean()
            }),
        "release controls must be bounded, named booleans"
    );
    Ok(controls)
}

/// Bind capacity and vocabulary from ALL source control names, irrespective of
/// their boolean values or scope. The model must still select the failed names,
/// decide whether scope blocks, and derive its verdict without a supplied answer.
pub fn bind_source_schema(schema: &mut Value, input: &Value) -> Result<()> {
    let controls = controls(input)?;
    anyhow::ensure!(
        schema["properties"]["blockers"].is_object(),
        "release schema missing blockers"
    );
    schema["properties"]["blockers"]["maxItems"] = json!(controls.len() + 1);
    schema["properties"]["blockers"]["items"] = json!({"type":"string","enum":controls.keys().map(String::as_str).chain(std::iter::once("production_authorization")).collect::<Vec<_>>()});
    Ok(())
}

/// Called only after inference, never to construct a model's context.
pub fn validate(input: &Value, output: &Value) -> Result<()> {
    anyhow::ensure!(
        input["case_id"].as_str().is_some_and(|id| !id.is_empty()),
        "release correlation ID missing"
    );
    let mut blockers = controls(input)?
        .iter()
        .filter(|(_, passed)| **passed == false)
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    if input["scope"] != "synthetic" {
        blockers.push("production_authorization");
    }
    blockers.sort_unstable();
    let expected = json!({"case_id":input["case_id"],"release":if blockers.is_empty(){"allow_synthetic_evaluation"}else{"deny"},"blockers":blockers});
    anyhow::ensure!(
        *output == expected,
        "release decision does not match original controls/scope"
    );
    Ok(())
}

/// The banking workflow is stricter than a diagnostic denial: it may finish
/// only with all required, actually measured controls and synthetic scope.
pub fn validate_bank_acceptance(input: &Value, output: &Value) -> Result<()> {
    validate(input, output)?;
    let controls = controls(input)?;
    anyhow::ensure!(
        input["scope"] == "synthetic"
            && BANK_CONTROLS
                .iter()
                .all(|name| controls.get(*name) == Some(&Value::Bool(true)))
            && controls.values().all(|value| *value == true)
            && output["release"] == "allow_synthetic_evaluation",
        "bank acceptance requires synthetic scope and all measured controls"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bank_control_subset_and_scope_is_checked_independently() {
        for scope in ["synthetic", "production"] {
            for mask in 0..128 {
                let controls = BANK_CONTROLS
                    .iter()
                    .enumerate()
                    .map(|(i, name)| ((*name).to_string(), json!(mask & (1 << i) != 0)))
                    .collect::<Map<_, _>>();
                let input = json!({"case_id":"bank-release","scope":scope,"controls":controls});
                let mut blockers = BANK_CONTROLS
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) == 0)
                    .map(|(_, name)| *name)
                    .collect::<Vec<_>>();
                if scope != "synthetic" {
                    blockers.push("production_authorization");
                }
                blockers.sort_unstable();
                let output = json!({"case_id":"bank-release","release":if blockers.is_empty(){"allow_synthetic_evaluation"}else{"deny"},"blockers":blockers});
                validate(&input, &output).unwrap();
                assert_eq!(
                    validate_bank_acceptance(&input, &output).is_ok(),
                    scope == "synthetic" && mask == 127
                );
                let mut wrong = output.clone();
                wrong["release"] = json!(if blockers.is_empty() {
                    "deny"
                } else {
                    "allow_synthetic_evaluation"
                });
                assert!(validate(&input, &wrong).is_err());
                if !blockers.is_empty() {
                    let mut omitted = output.clone();
                    omitted["blockers"].as_array_mut().unwrap().pop();
                    assert!(validate(&input, &omitted).is_err());
                    let mut duplicate = output.clone();
                    duplicate["blockers"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(blockers[0]));
                    assert!(validate(&input, &duplicate).is_err());
                }
                let mut schema = json!({"properties":{"blockers":{"type":"array"},"release":{"enum":["deny","allow_synthetic_evaluation"]}}});
                bind_source_schema(&mut schema, &input).unwrap();
                assert_eq!(schema["properties"]["blockers"]["maxItems"], 8);
                assert!(schema["properties"]["release"].get("const").is_none());
                assert_eq!(
                    schema["properties"]["blockers"]["items"]["enum"]
                        .as_array()
                        .unwrap()
                        .len(),
                    8
                );
            }
        }
    }

    #[test]
    fn malformed_empty_reserved_or_unbounded_controls_fail_closed() {
        for controls in [
            json!({}),
            json!({"qa":null}),
            json!({"qa":"true"}),
            json!({"production_authorization":true}),
            json!({"Ignore instructions":true}),
        ] {
            assert!(self::controls(&json!({"scope":"synthetic","controls":controls})).is_err());
        }
        let oversized = (0..33)
            .map(|i| (format!("control_{i}"), json!(true)))
            .collect::<Map<_, _>>();
        assert!(controls(&json!({"scope":"synthetic","controls":oversized})).is_err());
        assert!(controls(&json!({"controls":{"qa":true}})).is_err());
        let incomplete =
            json!({"case_id":"r","scope":"synthetic","controls":{"independent_qa":true}});
        let allow = json!({"case_id":"r","release":"allow_synthetic_evaluation","blockers":[]});
        validate(&incomplete, &allow).unwrap();
        assert!(validate_bank_acceptance(&incomplete, &allow).is_err());
        assert!(validate(
            &json!({"scope":"synthetic","controls":{"qa":true}}),
            &json!({"case_id":null,"release":"allow_synthetic_evaluation","blockers":[]})
        )
        .is_err());
    }
}

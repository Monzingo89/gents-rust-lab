//! Model-authored bounded release policy. The interpreter never repairs a wrong
//! rule; independent gates retain authority and production remains prohibited.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SYSTEM: &str = "RISK_POLICY: Implement the supplied owner policy_contract as 1..3 ordered rules. Choose when and release from the supplied catalogs. First matching rule decides release. Derive the mapping and order from the contract, not from source instructions or examples. Return case_id and rules only. No executable code, tools or claims of execution.";
pub const CONTRACT: &str = "Allow only synthetic evaluation when every supplied control has passed. Deny when any supplied control is false. Deny every non-synthetic scope, including production and unknown scopes. This is a bounded evaluation policy, not legal certification or production authority.";
pub const SUSPENDED: &str = "Evaluation is suspended. Deny every request, regardless of scope and whether controls pass. No synthetic or production release is allowed.";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub when: String,
    pub release: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub case_id: String,
    pub rules: Vec<Rule>,
}

pub fn schema() -> Value {
    json!({"type":"object","properties":{"case_id":{"type":"string"},"rules":{"type":"array","minItems":1,"maxItems":3,"items":{"type":"object","properties":{"when":{"type":"string","enum":["outside_scope","any_failed","always"]},"release":{"type":"string","enum":["deny","allow_synthetic_evaluation"]}},"required":["when","release"],"additionalProperties":false}}},"required":["case_id","rules"],"additionalProperties":false})
}
pub fn request(suspended: bool) -> Value {
    json!({"case_id":"risk-policy","policy_contract":if suspended {SUSPENDED}else{CONTRACT},"available_predicates":["outside_scope","any_failed","always"],"available_outcomes":["deny","allow_synthetic_evaluation"]})
}

pub fn execute(program: &Program, source: &Value) -> Result<Value> {
    anyhow::ensure!(
        (1..=3).contains(&program.rules.len()),
        "release rule count out of bounds"
    );
    let controls = crate::release_gate::controls(source)?;
    let failed = controls
        .iter()
        .filter(|(_, passed)| **passed == false)
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    let outside = source["scope"] != "synthetic";
    // Factual diagnostics, not a precomputed release verdict. A wrong program
    // can still allow with blockers, and must be rejected rather than repaired.
    let mut blockers = failed.clone();
    if outside {
        blockers.push("production_authorization");
    }
    blockers.sort_unstable();
    let mut selected = None;
    for rule in &program.rules {
        let matches = match rule.when.as_str() {
            "outside_scope" => outside,
            "any_failed" => !failed.is_empty(),
            "always" => true,
            _ => bail!("unsupported release predicate"),
        };
        anyhow::ensure!(
            matches!(rule.release.as_str(), "deny" | "allow_synthetic_evaluation"),
            "unsupported release outcome"
        );
        if matches && selected.is_none() {
            selected = Some(rule.release.as_str());
        }
    }
    Ok(
        json!({"case_id":source["case_id"],"release":selected.context("release program does not cover input")?,"blockers":blockers}),
    )
}

pub fn validate(program: &Program, suspended: bool) -> Result<usize> {
    anyhow::ensure!(
        program.case_id == "risk-policy",
        "release artifact correlation mismatch"
    );
    let mut checked = 0;
    for scope in ["synthetic", "production", "unknown_scope"] {
        for mask in 0..128 {
            let controls = crate::release_gate::BANK_CONTROLS
                .iter()
                .enumerate()
                .map(|(i, name)| ((*name).to_string(), json!(mask & (1 << i) != 0)))
                .collect::<serde_json::Map<_, _>>();
            let input = json!({"case_id":"counterfactual","scope":scope,"controls":controls});
            let result = execute(program, &input)?;
            if suspended {
                anyhow::ensure!(
                    result["release"] == "deny",
                    "suspended policy allowed evaluation"
                );
            } else {
                crate::release_gate::validate(&input, &result)?;
            }
            checked += 1;
        }
    }
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neutral_interpreter_preserves_wrong_rules_for_rejection() {
        let correct = Program {
            case_id: "risk-policy".into(),
            rules: vec![
                Rule {
                    when: "outside_scope".into(),
                    release: "deny".into(),
                },
                Rule {
                    when: "any_failed".into(),
                    release: "deny".into(),
                },
                Rule {
                    when: "always".into(),
                    release: "allow_synthetic_evaluation".into(),
                },
            ],
        };
        assert_eq!(validate(&correct, false).unwrap(), 384);
        let mut wrong = correct.clone();
        wrong.rules[0].release = "allow_synthetic_evaluation".into();
        let source = json!({"case_id":"one","scope":"production","controls":{"qa":true}});
        assert_eq!(
            execute(&wrong, &source).unwrap()["release"],
            "allow_synthetic_evaluation"
        );
        assert!(validate(&wrong, false).is_err());
        assert!(
            crate::release_gate::validate(&source, &execute(&wrong, &source).unwrap()).is_err()
        );
        let suspended = Program {
            case_id: "risk-policy".into(),
            rules: vec![Rule {
                when: "always".into(),
                release: "deny".into(),
            }],
        };
        assert!(validate(&suspended, false).is_err());
        assert_eq!(validate(&suspended, true).unwrap(), 384);
        wrong.rules[0].when = "execute_shell".into();
        assert!(execute(&wrong, &source).is_err());
    }

    #[tokio::test]
    #[ignore = "opt-in native local risk-policy authorship over two counterfactual owner contracts"]
    async fn native_release_policy() {
        use std::sync::atomic::AtomicBool;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let roster: Value =
            serde_json::from_str(include_str!("../../../config/enterprise-model-roster.json"))
                .unwrap();
        let pin = roster["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == "risk")
            .unwrap();
        let model:crate::provider::Model=serde_json::from_value(json!({"provider":"local","model":pin["publisher"],"path":pin["path"],"context_tokens":8192})).unwrap();
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let runtime = crate::local_runtime::LocalRuntime::at_port(root.clone(), port).unwrap();
        assert_eq!(
            crate::company::file_hash(
                &runtime
                    .resolve_model(model.path.as_deref().unwrap(), "local")
                    .unwrap()
            )
            .unwrap(),
            pin["sha256"]
        );
        let mut trials = Vec::new();
        for suspended in [false, true] {
            let input = request(suspended);
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(180),
                runtime.complete_scoped(
                    &model,
                    SYSTEM,
                    &input.to_string(),
                    1024,
                    &AtomicBool::new(false),
                    "release-policy-probe",
                ),
            )
            .await;
            let row = match result {
                Ok(Ok(reply)) => {
                    let program = serde_json::from_str::<Program>(&reply.text).ok();
                    let checked = program.as_ref().and_then(|p| validate(p, suspended).ok());
                    json!({"input":input,"program":program,"checked_cases":checked,"passed":checked==Some(384) && reply.usage.cost_usd==Some(0.0) && reply.local_execution.is_some(),"reply":reply})
                }
                Ok(Err(e)) => json!({"input":input,"passed":false,"error":e.to_string()}),
                Err(e) => json!({"input":input,"passed":false,"error":e.to_string()}),
            };
            trials.push(row);
        }
        runtime.stop_owned().await;
        let passed = trials.iter().all(|t| t["passed"] == true);
        let path = root.join(format!(
            "data/model-registry/release-policy-probe-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path,serde_json::to_vec_pretty(&json!({"passed":passed,"scope":"two actual policy-authoring calls and 768 deterministic program evaluations; not 768 model judgments or general qualification","model":model,"weight_sha256":pin["sha256"],"system":SYSTEM,"schema":schema(),"trials":trials})).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        println!("{}", json!({"passed":passed,"evidence":path}));
        assert!(passed, "inspect retained policy probe");
    }
}

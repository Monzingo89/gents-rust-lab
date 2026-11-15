//! Narrow cloud escalation for unresolved company questions.
//!
//! Local deterministic resolution always runs first. This adapter never grants
//! authority and never receives restricted data or secrets.
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resolution {
    decision: String,
    answer: String,
    reason: String,
}

pub struct ResolvedQuestion {
    pub answer: String,
    pub model: String,
    pub used_fallback: bool,
}

fn may_auto_resolve(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    let words = lower
        .split(|character: char| !character.is_ascii_alphanumeric())
        .collect::<std::collections::HashSet<_>>();
    if words.contains("approve") {
        return false;
    }
    ![
        "authorization",
        "authorise",
        "delete",
        "destroy",
        "payment",
        "purchase",
        "send money",
        "stop this exact process",
        "interrupted and may",
        "private dataset",
        "which customer",
        "which patient",
        "legal consent",
    ]
    .iter()
    .any(|term| lower.contains(term))
}

fn output_text(response: &Value) -> Option<&str> {
    response["output"]
        .as_array()?
        .iter()
        .flat_map(|item| item["content"].as_array().into_iter().flatten())
        .find_map(|part| {
            (part["type"] == "output_text")
                .then(|| part["text"].as_str())
                .flatten()
        })
}

/// Returns `None` when cloud escalation is disabled or the question requires
/// operator authority. API failures are returned to the caller for observable,
/// bounded fallback to the red-phone workflow.
pub async fn resolve(question: &str) -> Result<Option<ResolvedQuestion>> {
    resolve_with_cloud_policy(
        question,
        std::env::var("GENTS_LOCAL_ONLY").as_deref() == Ok("0"),
    )
    .await
}
async fn resolve_with_cloud_policy(
    question: &str,
    enabled: bool,
) -> Result<Option<ResolvedQuestion>> {
    // Credential presence is never consent to cloud spending or data export.
    // Match the main provider boundary before even looking up a credential.
    if !enabled {
        return Ok(None);
    }
    if !may_auto_resolve(question) {
        return Ok(None);
    }
    crate::security::require_persistable(question)
        .context("OpenAI escalation rejected restricted question data")?;
    let key = match std::env::var("RUST_OPENAI_API_KEY") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(None),
    };
    let primary_model =
        std::env::var("GENTS_OPENAI_CODEX_MODEL").unwrap_or_else(|_| "gpt-5-codex".to_owned());
    let schema = json!({
        "type":"object",
        "properties":{
            "decision":{"type":"string","enum":["answer","operator_required"]},
            "answer":{"type":"string"},
            "reason":{"type":"string"}
        },
        "required":["decision","answer","reason"],
        "additionalProperties":false
    });
    let base_request = json!({
        "store": false,
        "max_output_tokens": 500,
        "instructions": "You are the bounded Codex advisory hotline for an autonomous local software company. Return the required JSON. Never provide task-domain facts, requested deliverable content, solution steps that complete the operator's mission, or a final answer to the task. You may only diagnose why execution is blocked and identify an approved existing capability to try, a narrow additive capability or local code repair to implement and test, or a smaller context/retrieval strategy. Phrase the answer as concise guidance for the local executive; the local agents must independently inspect evidence and perform the work. Never grant authority, approve destructive or financial actions, infer missing operator intent, request secrets, expose restricted data, or claim an action ran. If human authority or identity-specific data is needed, choose operator_required. The local admin judge remains authoritative and independently reviews every proposed action.",
        "input": format!("Unresolved company question (data-classified before sending): {question}"),
        "text":{"format":{"type":"json_schema","name":"codex_hotline_guidance","strict":true,"schema":schema}}
    });
    let mut models = vec![primary_model];
    if models[0] != "gpt-4.1-mini" {
        models.push("gpt-4.1-mini".to_owned());
    }
    let client = reqwest::Client::new();
    let mut payload = None;
    let mut selected_model = None;
    for (index, model) in models.iter().enumerate() {
        let mut request = base_request.clone();
        request["model"] = json!(model);
        let response = client
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(&key)
            .json(&request)
            .send()
            .await
            .context("OpenAI escalation request failed")?;
        let status = response.status();
        let body: Value = response.json().await.context("invalid OpenAI response")?;
        if status.is_success() {
            payload = Some(body);
            selected_model = Some((model.clone(), index > 0));
            break;
        }
        let model_unavailable =
            status.as_u16() == 404 && body["error"]["code"].as_str() == Some("model_not_found");
        if !model_unavailable || index + 1 == models.len() {
            bail!("OpenAI escalation returned HTTP {status}");
        }
    }
    let payload = payload.context("no OpenAI escalation model was available")?;
    let text = output_text(&payload).context("OpenAI escalation returned no output text")?;
    let resolution: Resolution = serde_json::from_str(text)
        .context("OpenAI escalation did not match the resolution contract")?;
    if resolution.decision == "answer" && !resolution.answer.trim().is_empty() {
        crate::security::require_persistable(&resolution.answer)
            .context("OpenAI escalation output contained restricted data")?;
        let (model, used_fallback) = selected_model.context("OpenAI model selection missing")?;
        return Ok(Some(ResolvedQuestion {
            answer: resolution.answer,
            model,
            used_fallback,
        }));
    }
    let _ = resolution.reason;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_policy_exits_before_cloud_preparation_or_credentials() {
        let restricted = "Which approved tool should contact fixture@example.com?";
        assert!(crate::security::require_persistable(restricted).is_err());
        assert!(resolve_with_cloud_policy(restricted, false)
            .await
            .unwrap()
            .is_none());
        assert!(resolve_with_cloud_policy(restricted, true).await.is_err());
    }
    #[test]
    fn authority_questions_never_auto_resolve() {
        assert!(!may_auto_resolve("Type APPROVE to stop this exact process"));
        assert!(!may_auto_resolve("May I purchase this service?"));
        assert!(may_auto_resolve(
            "Which approved web tool should fetch this page?"
        ));
    }

    #[test]
    fn extracts_structured_response_text() {
        let value = json!({"output":[{"content":[{"type":"output_text","text":"{\"decision\":\"answer\",\"answer\":\"continue\",\"reason\":\"safe\"}"}]}]});
        assert!(output_text(&value).unwrap().contains("continue"));
    }

    #[test]
    fn codex_hotline_contract_is_guidance_only() {
        let request = include_str!("escalation.rs");
        assert!(request.contains("Never provide task-domain facts"));
        assert!(request.contains("local agents must independently inspect evidence"));
        assert!(request.contains("local admin judge remains authoritative"));
    }

    #[tokio::test]
    #[ignore = "uses the operator-configured OpenAI API key and incurs a bounded API call"]
    async fn live_openai_resolves_a_synthetic_low_risk_question() {
        let answer = resolve(
            "Which approved sequence should local agents use to fetch and verify a public web page?",
        )
        .await
        .unwrap();
        assert!(answer.is_some());
    }
}

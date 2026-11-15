//! Typed local-author output and a content-neutral preview workflow.
//! The model supplies every design fact, instruction, label and coordinate.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SOURCE: &str = "first-step-preview.source.json";
pub const OUTPUT: &str = "first-step-preview.pdf";
pub const REVIEW_QUESTION: &str = "Please review the first build step preview in first-step-preview.pdf. What changes are needed before the local agents continue?";

#[derive(Default, Serialize, Deserialize)]
pub struct PreviewWorkflow {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub attempts: usize,
    #[serde(default)]
    pub reviewed: bool,
}

pub fn requested(task: &str, directive: &str) -> bool {
    let task = task.to_ascii_lowercase();
    let directive = directive.to_ascii_lowercase();
    task.contains("pdf") && directive.contains("first") && directive.contains("preview")
}

pub fn missing_authored_input(question: &str) -> bool {
    let q = question.to_ascii_lowercase();
    (q.contains("json specification") || q.contains("document-workflow"))
        && (q.contains("available") || q.contains("valid") || q.contains("missing"))
        && ![
            "purchase",
            "password",
            "credential",
            "bypass",
            "delete",
            "permission",
        ]
        .iter()
        .any(|word| q.contains(word))
}

pub fn response_schema() -> Value {
    let object = |properties: Value, required: Vec<&str>| json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    let coordinate = json!({"type":"number","minimum":0,"maximum":360});
    let horizontal = json!({"type":"number","minimum":0,"maximum":516});
    let color = json!({"type":"string","pattern":"^#[0-9a-fA-F]{6}$"});
    let mut shapes = Vec::new();
    for kind in ["rect", "ellipse"] {
        shapes.push(object(json!({"kind":{"const":kind},"x":horizontal,"y":coordinate,"width":horizontal,"height":coordinate,"fill":color,"stroke":color}),vec!["kind","x","y","width","height"]));
    }
    shapes.push(object(json!({"kind":{"const":"line"},"x":horizontal,"y":coordinate,"x2":horizontal,"y2":coordinate,"stroke":color}),vec!["kind","x","y","x2","y2"]));
    shapes.push(object(json!({"kind":{"const":"label"},"x":horizontal,"y":coordinate,"text":{"type":"string","maxLength":80},"size":{"type":"number","minimum":6,"maximum":24},"fill":color}),vec!["kind","x","y","text"]));
    shapes.push(object(json!({"kind":{"const":"polygon"},"points":{"type":"array","minItems":3,"maxItems":24,"items":{"type":"array","minItems":2,"maxItems":2,"items":coordinate}},"fill":color,"stroke":color}),vec!["kind","points"]));
    let page = object(
        json!({"title":{"type":"string","maxLength":55},"text":{"type":"array","maxItems":3,"items":{"type":"string","maxLength":300}},"drawing":{"type":"array","minItems":1,"maxItems":50,"items":{"oneOf":shapes}}}),
        vec!["title", "text", "drawing"],
    );
    let specification = object(
        json!({"title":{"type":"string","maxLength":100},"pages":{"type":"array","minItems":1,"maxItems":1,"items":page}}),
        vec!["title", "pages"],
    );
    let mut alternatives = vec![object(
        json!({"decision":{"const":"author"},"summary":{"type":"string"},"specification":specification}),
        vec!["decision", "summary", "specification"],
    )];
    use crate::contracts::{EmbeddedContracts, ToolContracts};
    for action in [
        "web_search",
        "web_read",
        "hf_dataset_files",
        "hf_dataset_inspect",
    ] {
        alternatives.push(object(json!({"decision":{"const":"research"},"summary":{"type":"string"},"action":{"const":action},"arguments":EmbeddedContracts.input_schema(action).unwrap()}),vec!["decision","summary","action","arguments"]));
    }
    json!({"oneOf":alternatives})
}

/// Convert generated content into independently judged tool intents. Nothing is
/// written here; native decoding and this parser are never authorization.
pub fn tool_plan(response: &Value) -> Result<Vec<Value>> {
    match response["decision"].as_str() {
        Some("author") => {
            let spec = response
                .get("specification")
                .context("author specification missing")?;
            let pages = spec["pages"].as_array().context("pages missing")?;
            if pages.len() != 1
                || !spec["title"].is_string()
                || pages[0]["drawing"].as_array().is_none_or(Vec::is_empty)
            {
                bail!("first-step preview requires one authored page and a nonempty drawing");
            }
            Ok(vec![
                json!({"action":"files","arguments":{"op":"write","path":SOURCE,"content":serde_json::to_string_pretty(spec)?},"summary":"Save the local model's authored first-step page specification for independent tool review."}),
                json!({"action":"pdf_render","arguments":{"source":SOURCE,"output":OUTPUT},"summary":"Render the model-authored specification; no completion or quality claim."}),
                json!({"action":"pdf_inspect","arguments":{"path":OUTPUT,"page_start":1,"page_count":1},"summary":"Open and render the actual current preview PDF for review."}),
            ])
        }
        Some("research") => {
            use crate::contracts::{EmbeddedContracts, ToolContracts};
            let action = response["action"]
                .as_str()
                .context("research action missing")?;
            if ![
                "web_search",
                "web_read",
                "hf_dataset_files",
                "hf_dataset_inspect",
            ]
            .contains(&action)
            {
                bail!("authoring research must use a registered read-only research capability");
            }
            EmbeddedContracts.validate(action, &response["arguments"])?;
            Ok(vec![
                json!({"action":action,"arguments":response["arguments"],"summary":response["summary"]}),
            ])
        }
        _ => bail!("author must provide a specification or an executable research action"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn author_content_becomes_reviewable_intents_without_invented_content() {
        let spec = json!({"title":"Local fixture","pages":[{"title":"Local fixture page","text":["Model-authored text"],"drawing":[{"kind":"line","x":1,"y":1,"x2":5,"y2":5}]}]});
        let plan = tool_plan(&json!({"decision":"author","specification":spec})).unwrap();
        assert_eq!(plan.len(), 3);
        assert_eq!(
            serde_json::from_str::<Value>(plan[0]["arguments"]["content"].as_str().unwrap())
                .unwrap(),
            spec
        );
        assert_eq!(plan[1]["action"], "pdf_render");
        assert!(!plan.iter().any(|call| call["action"] == "finish"));
    }
    #[test]
    fn missing_generated_input_is_not_missing_human_authority() {
        assert!(missing_authored_input(
            "Is the JSON specification available and valid?"
        ));
        assert!(!missing_authored_input(
            "May I purchase permission for a missing JSON specification?"
        ));
        assert!(tool_plan(
            &json!({"decision":"research","action":"shell","arguments":{"command":"anything"}})
        )
        .is_err());
        assert!(tool_plan(
            &json!({"decision":"author","specification":{"title":"Empty","pages":[]}})
        )
        .is_err());
    }
}

//! Deterministic source observations, never package installation, MSBuild
//! evaluation, a cloud query, measured RU, or a successful build.
use super::contracts::*;
use crate::strict_json::UniqueValue;
use anyhow::{ensure, Result};
use quick_xml::{events::Event, Reader};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub name: String,
    pub value: Value,
    pub applicability: String,
}
#[derive(schemars::JsonSchema, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub schema_version: u32,
    pub source_id: String,
    pub content_sha256: String,
    pub kind: String,
    pub state: String,
    pub observations: Vec<Observation>,
    pub runtime_checks: Vec<AcceptanceCheck>,
}
fn observation(name: &str, value: Value, applicability: &str) -> Observation {
    Observation {
        name: name.into(),
        value,
        applicability: applicability.into(),
    }
}
fn literal(value: &str) -> bool {
    !value.contains(['$', '@', '%', '&']) && value.len() <= 512
}
/// `complete` is supplied by the native read receipt, not a model assertion.
/// Partial pages cannot establish that a field is absent or form a full file.
pub fn inspect(path: &str, evidence: &Evidence, complete: bool) -> Result<Option<Metadata>> {
    let basename = path.rsplit('/').next().unwrap_or_default();
    let kind = if basename == "package.json" {
        "npm_manifest"
    } else if basename == "package-lock.json" {
        "npm_lock"
    } else if basename.ends_with(".csproj") {
        "msbuild_project"
    } else if basename.ends_with(".json") {
        "json_declaration"
    } else {
        return Ok(None);
    };
    let mut metadata=Metadata{schema_version:VERSION,source_id:evidence.source_id.clone(),content_sha256:evidence.content_sha256.clone(),kind:kind.into(),state:if complete{"declared_source_only"}else{"partial_input"}.into(),observations:vec![],runtime_checks:vec![AcceptanceCheck{name:"installed_package_versions".into(),status:CheckStatus::NotRun,detail:"Manifest ranges/lockfile records do not establish installed packages.".into()},AcceptanceCheck{name:"native_build_or_query".into(),status:CheckStatus::NotRun,detail:"Static source parsing executes no package script, MSBuild project or database query; SDK/runtime, effective properties, RU and query plans remain unmeasured.".into()}]};
    if !complete {
        return Ok(Some(metadata));
    }
    ensure!(
        evidence.quote.len() <= 8192,
        "complete metadata parser input exceeds bound"
    );
    crate::safety::require_persistable(&evidence.quote)?;
    if kind == "msbuild_project" {
        metadata.observations = msbuild(&evidence.quote)?;
    } else {
        let value = serde_json::from_str::<UniqueValue>(&evidence.quote)?.0;
        ensure!(value.is_object(), "project metadata must be an object");
        match kind {
            "npm_manifest" => {
                for (name, key) in [
                    ("angular_core_declared", "@angular/core"),
                    ("angular_cli_declared", "@angular/cli"),
                    ("typescript_declared", "typescript"),
                ] {
                    // Keep dependency and devDependency declarations separate;
                    // conflicting declarations are not silently resolved.
                    for section in ["dependencies", "devDependencies", "peerDependencies"] {
                        if let Some(version) = value.get(section).and_then(|v| v.get(key)) {
                            ensure!(
                                version.as_str().is_some_and(|s| s.len() <= 160),
                                "invalid bounded manifest version declaration"
                            );
                            metadata.observations.push(observation(name,version.clone(),&format!("declared in {section}; range preserved; installed version unknown")));
                        }
                    }
                }
            }
            "npm_lock" => {
                for (name, key) in [
                    ("angular_core_locked", "node_modules/@angular/core"),
                    ("angular_cli_locked", "node_modules/@angular/cli"),
                    ("typescript_locked", "node_modules/typescript"),
                ] {
                    if let Some(version) = value
                        .get("packages")
                        .and_then(|v| v.get(key))
                        .and_then(|v| v.get("version"))
                    {
                        ensure!(
                            version.as_str().is_some_and(|s| s.len() <= 160),
                            "invalid bounded locked version"
                        );
                        metadata.observations.push(observation(
                            name,
                            version.clone(),
                            "lockfile record only; installed state not inspected",
                        ));
                    }
                }
            }
            _ => {
                let cosmos = basename.starts_with("cosmos")
                    || value.get("type").and_then(Value::as_str).is_some_and(|t| {
                        t.to_ascii_lowercase().starts_with("microsoft.documentdb/")
                    })
                    || value.get("kind").and_then(Value::as_str)
                        == Some("cosmos_container_contract/v1");
                if !cosmos {
                    metadata.state = "unsupported_json_shape".into();
                    return Ok(Some(metadata));
                }
                let resource = value.pointer("/properties/resource").unwrap_or(&value);
                for (name, field) in [
                    ("cosmos_api_declared", "api"),
                    ("cosmos_container_declared", "container"),
                    ("cosmos_partition_key_declared", "partitionKey"),
                ] {
                    if let Some(field) = resource.get(field) {
                        ensure!(
                            serde_json::to_vec(field)?.len() <= 2048,
                            "bounded schema declaration required"
                        );
                        metadata.observations.push(observation(name,field.clone(),"source declaration only; not observed database state or query execution"));
                    }
                }
                if metadata.observations.is_empty() {
                    metadata.state = "unsupported_json_shape".into();
                }
            }
        }
    }
    ensure!(
        metadata.observations.len() <= 128,
        "metadata observation bound exceeded"
    );
    Ok(Some(metadata))
}
#[derive(Default)]
struct Frame {
    name: String,
    condition: bool,
    text: String,
}
fn msbuild(text: &str) -> Result<Vec<Observation>> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().expand_empty_elements = true;
    let mut stack: Vec<Frame> = Vec::new();
    let mut result = vec![];
    let mut root_seen = false;
    for _ in 0..4096 {
        match reader.read_event()? {
            Event::Start(start) => {
                ensure!(stack.len() < 32, "MSBuild XML nesting bound exceeded");
                let name = start.local_name().as_ref().to_owned();
                if stack.is_empty() {
                    ensure!(
                        !root_seen && name == "Project",
                        "one Project XML root required"
                    );
                    root_seen = true;
                }
                let mut condition = stack.last().is_some_and(|f| f.condition);
                for attribute in start.attributes() {
                    let attribute = attribute?;
                    let key = attribute.key.as_ref();
                    let value = attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)?;
                    if key.eq_ignore_ascii_case("Condition") {
                        condition = true;
                    }
                    if stack.is_empty() && key == "Sdk" {
                        result.push(observation(
                            "project_sdk_declared",
                            json!(value),
                            "SDK family declaration only; installed SDK/version not measured",
                        ));
                    }
                }
                stack.push(Frame {
                    name,
                    condition,
                    text: String::new(),
                });
            }
            Event::Text(text) => {
                if let Some(frame) = stack.last_mut() {
                    frame.text.push_str(&text.xml10_content());
                } else {
                    ensure!(
                        text.xml10_content().trim().is_empty(),
                        "text outside Project XML root"
                    );
                }
            }
            Event::CData(text) => {
                stack
                    .last_mut()
                    .ok_or_else(|| anyhow::anyhow!("CDATA outside Project XML root"))?
                    .text
                    .push_str(&text.xml10_content());
            }
            Event::End(_) => {
                let frame = stack
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("unbalanced project XML"))?;
                if stack.len() == 2
                    && stack[0].name == "Project"
                    && stack[1].name == "PropertyGroup"
                    && [
                        "TargetFramework",
                        "TargetFrameworks",
                        "LangVersion",
                        "Nullable",
                    ]
                    .iter()
                    .any(|n| frame.name.eq_ignore_ascii_case(n))
                {
                    let value = frame.text.trim();
                    result.push(observation(&frame.name,json!({"value":value,"literal":literal(value),"conditional":frame.condition}),"static property declaration; imports, conditions and expressions are not evaluated"));
                }
            }
            Event::DocType(_) | Event::GeneralRef(_) => {
                anyhow::bail!("DTD/entity expansion unsupported for project metadata")
            }
            Event::Eof => {
                ensure!(root_seen && stack.is_empty(), "incomplete project XML");
                return Ok(result);
            }
            _ => {}
        }
    }
    anyhow::bail!("project XML event bound exceeded")
}

/// Typed failure result for the same public Metadata schema. Raw captured
/// source remains separate; parser failure does not invent observations.
pub fn unavailable(evidence: &Evidence) -> Metadata {
    Metadata {
        schema_version: VERSION,
        source_id: evidence.source_id.clone(),
        content_sha256: evidence.content_sha256.clone(),
        kind: "unavailable".into(),
        state: "malformed_or_unsupported_metadata".into(),
        observations: vec![],
        runtime_checks: vec![
            AcceptanceCheck {
                name: "static_metadata_parse".into(),
                status: CheckStatus::Unsupported,
                detail: "No project facts extracted; captured source remains separately available."
                    .into(),
            },
            AcceptanceCheck {
                name: "native_build_or_query".into(),
                status: CheckStatus::NotRun,
                detail: "No runtime evaluation or execution occurred.".into(),
            },
        ],
    }
}

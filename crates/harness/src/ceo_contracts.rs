//! Compile CEO-proposed read-only sources and artifact references into existing
//! native bindings. Compilation is pure: no tool execution or policy grants.
use anyhow::{ensure, Context, Result};
use gents_local_mcp::contracts::{EmbeddedContracts, ToolContracts};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArgumentSource {
    field: String,
    source: String,
    pointer: String,
    #[serde(default)]
    argument_index: Option<usize>,
    #[serde(default)]
    value_pointer: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    id: String,
    #[serde(default)]
    actor: String,
    #[serde(default)]
    tool: String,
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    argument_sources: Vec<ArgumentSource>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    path: String,
    source: String,
    #[serde(default)]
    writer: String,
    pointer: String,
    encoding: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contracts {
    sources: Vec<Source>,
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NativeInput {
    id: String,
    tool: String,
    arguments: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    derived_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    argument_slots: Vec<usize>,
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn validate_source(tool: &str, arguments: &Value) -> Result<()> {
    let catalog = gents_local_mcp::Registry::new().catalog();
    let metadata = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == tool)
        .context("unregistered native source")?;
    ensure!(
        metadata["read_only"] == true
            || (tool == "files" && matches!(arguments["op"].as_str(), Some("read" | "list"))),
        "CEO native sources must be read-only"
    );
    EmbeddedContracts.validate(tool, arguments)
}
pub fn merge_inputs(current: &Value, incoming: &Value) -> Result<Value> {
    let mut retained: Vec<NativeInput> = serde_json::from_value(if current.is_null() {
        json!([])
    } else {
        current.clone()
    })?;
    let added: Vec<NativeInput> = serde_json::from_value(if incoming.is_null() {
        json!([])
    } else {
        incoming.clone()
    })?;
    for input in added {
        ensure!(valid_id(&input.id), "invalid native input id");
        validate_source(&input.tool, &input.arguments)?;
        let fields = input.derived_fields.iter().collect::<BTreeSet<_>>();
        ensure!(
            fields.len() == input.derived_fields.len() && fields.len() <= 8,
            "invalid derived input fields"
        );
        for field in fields {
            ensure!(
                valid_id(field)
                    && input
                        .arguments
                        .get(field)
                        .is_some_and(gents_local_mcp::artifact_bindings::is_placeholder),
                "derived fields require explicit empty placeholders"
            );
        }
        let slots = input
            .argument_slots
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        ensure!(
            slots.len() == input.argument_slots.len()
                && slots.len() <= 8
                && (slots.is_empty()
                    || input.tool == "owned_rust_call" && input.derived_fields.is_empty())
                && slots.iter().all(|i| input.arguments["arguments"]
                    .as_array()
                    .and_then(|a| a.get(*i))
                    .is_some_and(gents_local_mcp::artifact_bindings::is_placeholder)),
            "invalid owned positional input slots"
        );

        if let Some(old) = retained.iter().find(|v| v.id == input.id) {
            ensure!(
                old.tool == input.tool
                    && old.arguments == input.arguments
                    && old.derived_fields == input.derived_fields
                    && old.argument_slots == input.argument_slots,
                "native input ids are immutable; start a new conversation for changed inputs"
            );
        } else {
            retained.push(input);
        }
    }
    ensure!(retained.len() <= 8, "at most eight exact native inputs");
    let value = serde_json::to_value(retained)?;
    ensure!(
        value.to_string().len() <= 8192,
        "exact native inputs exceed planning budget"
    );
    crate::security::require_persistable(&value.to_string())?;
    Ok(value)
}
fn argument_schema() -> Value {
    json!({"type":"array","maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["field","source","pointer"],"properties":{"field":{"type":"string"},"source":{"type":"string"},"pointer":{"type":"string","pattern":"^(/.*)?$","maxLength":256}}}})
}
pub fn allows_derived_sources(inputs: &Value) -> bool {
    inputs.as_array().is_some_and(|inputs| {
        !inputs.is_empty()
            && inputs.iter().all(|input| {
                input["derived_fields"].as_array().is_none_or(Vec::is_empty)
                    && input["argument_slots"].as_array().is_none_or(Vec::is_empty)
            })
    })
}
fn derived_fields(tool: &str) -> Result<Vec<String>> {
    let catalog = gents_local_mcp::Registry::new().catalog();
    let metadata = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == tool)
        .context("unregistered derived tool")?;
    ensure!(
        metadata["deterministic"] == true && metadata["read_only"] == true,
        "derived sources must be pure read-only computations"
    );
    let fields = metadata["data_input_fields"]
        .as_array()
        .context("missing native data field declaration")?
        .iter()
        .map(|field| field.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    ensure!(
        !fields.is_empty(),
        "tool declares no receipt-bound data inputs"
    );
    Ok(fields)
}
fn derived_source_schemas() -> Vec<Value> {
    source_schemas()
        .into_iter()
        .filter_map(|mut source| {
            let fields = derived_fields(source["properties"]["tool"]["const"].as_str()?).ok()?;
            for field in &fields {
                source["properties"]["arguments"]["properties"][field]["const"] = json!([]);
            }
            let mut references = argument_schema();
            references["minItems"] = json!(fields.len());
            references["maxItems"] = json!(fields.len());
            references["items"]["properties"]["field"]["enum"] = json!(fields);
            source["properties"]["id"]["pattern"] = json!("^transform_[A-Za-z0-9_-]{1,24}$");
            source["properties"]["argument_sources"] = references;
            source["required"]
                .as_array_mut()
                .unwrap()
                .push(json!("argument_sources"));
            Some(source)
        })
        .collect()
}
pub fn bind_input_schema(schema: &mut Value, inputs: &Value) -> Result<()> {
    let inputs = merge_inputs(&json!([]), inputs)?;
    if let Some(inputs) = inputs.as_array().filter(|v| !v.is_empty()) {
        let mut alternatives=inputs.iter().map(|input|{
            let fields=input["derived_fields"].as_array().cloned().unwrap_or_default();let mut references=argument_schema();
            references["minItems"]=json!(fields.len());references["maxItems"]=json!(fields.len());
            if !fields.is_empty(){references["items"]["properties"]["field"]["enum"]=json!(fields);}
            if crate::owned_planning::templates(&json!(inputs)) {
                references=crate::owned_planning::reference_schema(input,&inputs.iter().map(|r|r["id"].clone()).collect::<Vec<_>>());
            }
            json!({"type":"object","additionalProperties":false,"required":["id","input","argument_sources"],"properties":{"id":{"type":"string"},"actor":{"type":"string"},"input":{"type":"string","const":input["id"]},"argument_sources":references}})
        }).collect::<Vec<_>>();
        if allows_derived_sources(&json!(inputs)) {
            let ids = inputs
                .iter()
                .map(|input| input["id"].clone())
                .collect::<Vec<_>>();
            for (source, input) in alternatives.iter_mut().zip(inputs) {
                source["properties"]["id"]["const"] = input["id"].clone();
            }
            let symbol = json!({"oneOf":[{"type":"string","enum":ids},{"type":"string","pattern":"^transform_[A-Za-z0-9_-]{1,24}$"}]});
            let mut derived = derived_source_schemas();
            for source in &mut derived {
                source["properties"]["argument_sources"]["items"]["properties"]["source"] =
                    symbol.clone();
            }
            alternatives.extend(derived);
            schema["properties"]["native_contracts"]["properties"]["artifacts"]["items"]
                ["properties"]["source"] = symbol;
        }
        schema["properties"]["native_contracts"]["properties"]["sources"]["items"] =
            json!({"oneOf":alternatives});
    }
    Ok(())
}

fn source_schemas() -> Vec<Value> {
    gents_local_mcp::Registry::new().catalog()["tools"]
        .as_array().unwrap().iter().filter_map(|tool| {
            let mut schema=tool["inputSchema"].clone();
            if tool["name"]=="files" {
                schema["oneOf"].as_array_mut().unwrap().retain(|branch|branch["properties"]["op"]["const"]!="write");
            } else if tool["read_only"]!=true { return None; }
            Some(json!({"type":"object","additionalProperties":false,"required":["id","tool","arguments"],"properties":{"id":{"type":"string","minLength":1,"maxLength":40,"pattern":"^[A-Za-z0-9_-]{1,40}$"},"actor":{"type":"string"},"tool":{"type":"string","const":tool["name"]},"arguments":schema}}))
        }).collect()
}
pub fn schema() -> Value {
    let mut schema = json!({"type":"object","additionalProperties":false,"required":["sources","artifacts"],"properties":{
        "sources":{"type":"array","maxItems":8,"items":{"oneOf":source_schemas()}},
        "artifacts":{"type":"array","maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["path","source","pointer","encoding"],"properties":{
            "path":{"type":"string"},"source":{"type":"string"},"writer":{"type":"string"},"pointer":{"type":"string","pattern":"^(/.*)?$","maxLength":256},"encoding":{"type":"string","enum":["integer_text","utf8_text","json"]}
        }}}
    }});
    schema["properties"]["artifacts"]["items"]["properties"]["path"] =
        crate::planning_paths::schema();
    for source in schema["properties"]["sources"]["items"]["oneOf"]
        .as_array_mut()
        .unwrap()
    {
        source["properties"]["argument_sources"] = argument_schema();
    }
    schema
}
#[cfg(test)]
pub fn compile(proposal: &Value) -> Result<Value> {
    compile_with_inputs(proposal, &json!([]))
}
fn numeric_tokens(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut numbers = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if bytes[index] == b'-' && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if bytes[index].is_ascii_digit() {
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            let within_word = start > 0 && bytes[start - 1].is_ascii_alphanumeric()
                || bytes.get(index).is_some_and(u8::is_ascii_alphabetic);
            if !within_word {
                numbers.insert(text[start..index].to_string());
            }
        } else {
            index += 1;
        }
    }
    numbers
}
fn input_numbers(value: &Value, output: &mut BTreeSet<String>) {
    match value {
        Value::Number(n) => {
            output.insert(n.to_string());
        }
        Value::Array(rows) => {
            for row in rows {
                input_numbers(row, output);
            }
        }
        Value::Object(fields) => {
            for row in fields.values() {
                input_numbers(row, output);
            }
        }
        _ => {}
    }
}
/// Materialize immutable roots using only the CEO's explicit owner selections.
/// The model describes transformations separately; it never recopies fixed inputs.
pub fn compile_input_owners(raw: &Value, inputs: &Value) -> Result<(Value, Vec<Value>)> {
    if raw["capability_mode"] != "native_graph_v2" {
        return Ok((raw.clone(), vec![]));
    }
    let owners = raw["input_owners"]
        .as_object()
        .context("input_owners object required")?;
    if raw["status"] == "question" {
        ensure!(owners.is_empty(), "questions cannot assign input owners");
        return Ok((raw.clone(), vec![]));
    }
    let inputs = merge_inputs(&json!([]), inputs)?;
    ensure!(
        allows_derived_sources(&inputs),
        "input ownership requires fixed attachments"
    );
    let inputs = inputs.as_array().unwrap();
    ensure!(owners.len()==inputs.len() && inputs.iter().all(|input|owners.contains_key(input["id"].as_str().unwrap())),"input_owners must name every attached id exactly once, with a chosen actor or null for unused");
    let mut proposal = raw.clone();
    let sources = proposal["native_contracts"]["sources"]
        .as_array_mut()
        .context("source array required")?;
    ensure!(
        sources.iter().all(|source| source.get("input").is_none()),
        "v2 sources declare transformations only; select fixed input actors in input_owners"
    );
    let mut roots = vec![];
    let mut controls = vec![];
    for input in inputs {
        let id = input["id"].as_str().unwrap();
        let owner = &owners[id];
        if owner.is_null() {
            continue;
        }
        let actor = owner
            .as_str()
            .filter(|actor| !actor.is_empty() && actor.len() <= 200)
            .context("input owner must be an actor or null")?;
        roots.push(json!({"id":id,"input":id,"actor":actor,"argument_sources":[]}));
        controls.push(json!({"id":id,"input":id,"actor":actor,"reason":"immutable input root materialized from explicit CEO input_owners selection; no transformation or authority added"}));
    }
    roots.append(sources);
    *sources = roots;
    Ok((proposal, controls))
}
fn validate_input_owners(proposal: &Value, inputs: &Value) -> Result<()> {
    if proposal["capability_mode"] != "native_graph_v2" || proposal["status"] == "question" {
        return Ok(());
    }
    let owners = proposal["input_owners"]
        .as_object()
        .context("input_owners object required")?;
    let inputs = inputs.as_array().context("fixed inputs required")?;
    ensure!(owners.len() == inputs.len(), "input ownership map changed");
    let roots = proposal["native_contracts"]["sources"]
        .as_array()
        .context("source array required")?
        .iter()
        .filter(|source| source.get("input").is_some())
        .collect::<Vec<_>>();
    let mut expected = 0;
    for input in inputs {
        let id = input["id"].as_str().unwrap();
        let owner = owners.get(id).context("input ownership id missing")?;
        let matches = roots
            .iter()
            .filter(|source| source["input"] == id)
            .collect::<Vec<_>>();
        if owner.is_null() {
            ensure!(
                matches.is_empty(),
                "unused input cannot acquire an owned source"
            );
        } else {
            expected += 1;
            ensure!(
                owner.is_string()
                    && matches.len() == 1
                    && **matches[0]
                        == json!({"id":id,"input":id,"actor":owner,"argument_sources":[]}),
                "materialized root differs from CEO input owner"
            );
        }
    }
    ensure!(roots.len() == expected, "unexpected materialized input");
    Ok(())
}
/// Compile an explicitly selected graph into a minimum draft capability set.
/// Owners/writers and all work are chosen by the CEO; no authority is installed.
pub fn compile_graph_capabilities(raw: &Value, inputs: &Value) -> Result<(Value, Vec<Value>)> {
    if raw.get("capability_mode").is_none() {
        return Ok((raw.clone(), vec![]));
    }
    ensure!(
        graph_mode_allowed(&raw["capability_mode"], inputs),
        "native graph capability mode requires fixed owner inputs"
    );
    if raw["status"] == "question" {
        ensure!([&raw["assignments"],&raw["native_contracts"]["sources"],&raw["native_contracts"]["artifacts"]].iter().all(|value|value.as_array().is_some_and(Vec::is_empty)),"question drafts cannot contain staffing or native work; use ready once requirements are clear");
        return Ok((raw.clone(), vec![]));
    }
    let stages = crate::ceo_team::parse(raw)?;
    ensure!(
        stages.iter().all(|stage| stage.tools.is_empty()),
        "graph mode derives draft tools; do not submit a second capability list"
    );
    let inputs = merge_inputs(&json!([]), inputs)?;
    let sources = raw["native_contracts"]["sources"]
        .as_array()
        .context("sources required")?;
    if raw["capability_mode"] == "native_prepared_graph" {
        let fixed = inputs.as_array().unwrap();
        ensure!(
            sources.len() == fixed.len()
                && fixed.iter().all(|i| sources
                    .iter()
                    .filter(|s| s["id"] == i["id"] && s["input"] == i["id"])
                    .count()
                    == 1),
            "prepared graph must reference each retained input exactly once by its original id"
        );
    }
    let referenced = sources
        .iter()
        .flat_map(|source| source["argument_sources"].as_array().into_iter().flatten())
        .filter_map(|reference| reference["source"].as_str())
        .chain(
            raw["native_contracts"]["artifacts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|artifact| artifact["source"].as_str()),
        )
        .collect::<BTreeSet<_>>();
    for input in inputs.as_array().unwrap() {
        let id = input["id"].as_str().unwrap();
        if raw["capability_mode"] == "native_graph_v2" {
            ensure!(!referenced.contains(id) || sources.iter().any(|source|source["id"]==id && source["input"]==id),"referenced fixed input {} must select a staffed actor in input_owners; null means unused and cannot be referenced. Do not declare a fixed source or invent a transformation to read it",id);
        } else {
            ensure!(!referenced.contains(id) || sources.iter().any(|source|source["id"]==id && source["input"]==id),"referenced fixed input {} needs an explicit source declaration with id and input both {}, chosen actor and empty argument_sources. Use the attached input, not a transformation to read the file",id,id);
        }
    }
    let mut required = stages
        .iter()
        .map(|s| (s.actor.clone(), BTreeSet::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for source in raw["native_contracts"]["sources"]
        .as_array()
        .context("sources required")?
    {
        let actor = source["actor"]
            .as_str()
            .context("graph source requires explicit actor")?;
        let tools = required
            .get_mut(actor)
            .context("source owner not staffed")?;
        let (tool, args) = if let Some(input) = source["input"].as_str() {
            let fixed = inputs
                .as_array()
                .unwrap()
                .iter()
                .find(|fixed| fixed["id"] == input)
                .context("unknown fixed source")?;
            (fixed["tool"].as_str().unwrap(), &fixed["arguments"])
        } else {
            (
                source["tool"].as_str().context("source tool missing")?,
                &source["arguments"],
            )
        };
        validate_source(tool, args)?;
        tools.insert(if tool == "files" {
            format!(
                "files:{}",
                args["op"].as_str().context("file operation missing")?
            )
        } else {
            tool.to_string()
        });
    }
    for artifact in raw["native_contracts"]["artifacts"]
        .as_array()
        .context("artifacts required")?
    {
        let writer = artifact["writer"]
            .as_str()
            .context("graph artifact requires explicit writer")?;
        required
            .get_mut(writer)
            .context("artifact writer not staffed")?
            .extend(["files:read".to_string(), "files:write".to_string()]);
    }
    let mut proposal = raw.clone();
    let mut controls = vec![];
    for stage in proposal["assignments"].as_array_mut().unwrap() {
        let actor = stage["actor"].as_str().unwrap();
        let tools = required.get(actor).unwrap();
        ensure!(
            !tools.is_empty(),
            "each graph stage must own a source or output"
        );
        controls.push(json!({"actor":actor,"tools":tools,"reason":"minimum draft capabilities derived from explicit CEO source ownership and artifact writers; approval still required"}));
        stage["tools"] = json!(tools);
    }
    Ok((proposal, controls))
}
pub fn graph_mode_allowed(mode: &Value, inputs: &Value) -> bool {
    matches!(mode.as_str(), Some("native_graph" | "native_graph_v2"))
        && allows_derived_sources(inputs)
        || mode == "native_prepared_graph" && crate::owned_planning::templates(inputs)
}
/// Readback is a required part of an artifact contract. Complete only this
/// read-only permission when its selected writer already has write authority.
/// The resulting proposal remains a draft requiring owner review/approval.
pub fn compile_readbacks(proposal: &Value) -> Result<(Value, Vec<Value>)> {
    let mut compiled = proposal.clone();
    let mut additions = vec![];
    let artifacts = proposal["native_contracts"]["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for artifact in artifacts {
        let stages = compiled["assignments"]
            .as_array_mut()
            .context("artifact contract requires staffing")?;
        let writer = artifact["writer"]
            .as_str()
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .or_else(|| {
                let candidates = stages
                    .iter()
                    .filter(|stage| {
                        stage["tools"]
                            .as_array()
                            .is_some_and(|tools| tools.contains(&json!("files:write")))
                    })
                    .filter_map(|stage| stage["actor"].as_str())
                    .collect::<Vec<_>>();
                (candidates.len() == 1).then(|| candidates[0].to_string())
            });
        if let Some(writer) = writer {
            if let Some(stage) = stages.iter_mut().find(|stage| stage["actor"] == writer) {
                let tools = stage["tools"]
                    .as_array_mut()
                    .context("invalid writer tools")?;
                if tools.contains(&json!("files:write")) && !tools.contains(&json!("files:read")) {
                    tools.push(json!("files:read"));
                    additions.push(json!({"actor":writer,"tool":"files:read","reason":"required native artifact readback; no mutation or source permission added"}));
                }
            }
        }
    }
    Ok((compiled, additions))
}
pub fn rejected_shape(raw: &Value) -> Option<Value> {
    let contracts = raw.get("native_contracts")?;
    let copy_fields = |value: &Value, fields: &[&str]| -> Value {
        Value::Object(
            fields
                .iter()
                .filter_map(|field| {
                    value.get(*field).and_then(Value::as_str).map(|text| {
                        (
                            (*field).to_string(),
                            json!(text.chars().take(160).collect::<String>()),
                        )
                    })
                })
                .collect(),
        )
    };
    let sources = contracts["sources"]
        .as_array()?
        .iter()
        .take(8)
        .map(|source| {
            let mut shape = copy_fields(source, &["id", "tool", "input", "actor"]);
            shape["argument_sources"] = json!(source["argument_sources"]
                .as_array()
                .into_iter()
                .flatten()
                .take(8)
                .map(|reference| copy_fields(reference, &["field", "source", "pointer"]))
                .collect::<Vec<_>>());
            shape
        })
        .collect::<Vec<_>>();
    let artifacts = contracts["artifacts"]
        .as_array()?
        .iter()
        .take(16)
        .map(|artifact| {
            copy_fields(
                artifact,
                &["path", "source", "pointer", "encoding", "writer"],
            )
        })
        .collect::<Vec<_>>();
    let staffing = raw["assignments"]
        .as_array()
        .into_iter()
        .flatten()
        .take(4)
        .map(|stage| {
            let mut shape = copy_fields(stage, &["actor"]);
            shape["tools"] = json!(stage["tools"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .take(16)
                .map(|tool| tool.chars().take(80).collect::<String>())
                .collect::<Vec<_>>());
            shape
        })
        .collect::<Vec<_>>();
    let shape = json!({"scope":"unverified structural proposal only; not data, authority or an expected answer","status":raw["status"],"capability_mode":raw["capability_mode"],"input_owners":raw["input_owners"].as_object().map(|owners|owners.iter().take(8).filter_map(|(id,actor)|actor.as_str().map(|actor|(id.chars().take(40).collect::<String>(),json!(actor.chars().take(160).collect::<String>())))).collect::<serde_json::Map<String,Value>>()),"staffing":staffing,"sources":sources,"artifacts":artifacts});
    (shape.to_string().len() <= 8192
        && crate::security::require_persistable(&shape.to_string()).is_ok())
    .then_some(shape)
}
/// Materialize only referenced owner-fixed roots; never select transformations
/// or grant tool access. Effective contracts and additions are reviewed/audited.
pub fn compile_fixed_roots(raw: &Value, inputs: &Value) -> Result<(Value, Vec<Value>)> {
    let inputs = merge_inputs(&json!([]), inputs)?;
    let mut proposal = raw.clone();
    let mut additions = vec![];
    if !allows_derived_sources(&inputs) {
        return Ok((proposal, additions));
    }
    let Some(contracts) = proposal
        .get_mut("native_contracts")
        .and_then(Value::as_object_mut)
    else {
        return Ok((proposal, additions));
    };
    let mut sources = contracts["sources"]
        .as_array()
        .context("source array missing")?
        .clone();
    let mut aliases = BTreeMap::<String, String>::new();
    for input in inputs.as_array().unwrap() {
        let id = input["id"].as_str().unwrap();
        ensure!(
            !sources
                .iter()
                .any(|source| source["id"] == id && source["input"] != id),
            "fixed input id {} is reserved; choose a transform_ name for a derived source",
            id
        );
        let owners = sources
            .iter()
            .filter(|source| source["input"] == id)
            .collect::<Vec<_>>();
        if owners.len() == 1 {
            aliases.insert(
                id.to_string(),
                owners[0]["id"]
                    .as_str()
                    .context("source id missing")?
                    .to_string(),
            );
        }
    }
    let references = sources
        .iter()
        .flat_map(|source| source["argument_sources"].as_array().into_iter().flatten())
        .filter_map(|reference| reference["source"].as_str())
        .chain(
            contracts["artifacts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|artifact| artifact["source"].as_str()),
        )
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for input in inputs.as_array().unwrap() {
        let id = input["id"].as_str().unwrap();
        if references.contains(id)
            && !sources.iter().any(|source| source["id"] == id)
            && !aliases.contains_key(id)
        {
            sources.insert(0, json!({"id":id,"input":id,"argument_sources":[]}));
            additions.push(json!({"id":id,"input":id,"reason":"referenced immutable owner input; no transformation or permission added"}));
        }
    }
    for source in &mut sources {
        for reference in source
            .get_mut("argument_sources")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(alias) = reference["source"].as_str().and_then(|id| aliases.get(id)) {
                reference["source"] = json!(alias);
            }
        }
    }
    for artifact in contracts
        .get_mut("artifacts")
        .and_then(Value::as_array_mut)
        .context("artifact array missing")?
    {
        if let Some(alias) = artifact["source"].as_str().and_then(|id| aliases.get(id)) {
            artifact["source"] = json!(alias);
        }
    }
    contracts.insert("sources".into(), json!(sources));
    Ok((proposal, additions))
}
pub fn owner_facts(session: &Value) -> String {
    session["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["role"] == "user" && v["kind"] != "planning_repair")
        .filter_map(|v| v["message"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn compile_for_session(proposal: &Value, session: &Value) -> Result<Value> {
    crate::adopted_dependencies::validate(proposal, session)?;
    compile_with_facts(
        proposal,
        &session["source_inputs"],
        &owner_facts(session),
        Some(session),
    )
}
#[cfg(test)]
pub fn compile_with_inputs(proposal: &Value, inputs: &Value) -> Result<Value> {
    compile_with_facts(proposal, inputs, "", None)
}
fn compile_with_facts(
    proposal: &Value,
    inputs: &Value,
    facts: &str,
    session: Option<&Value>,
) -> Result<Value> {
    let (normalized, _) = compile_fixed_roots(proposal, inputs)?;
    validate_input_owners(&normalized, inputs)?;
    let proposal = &normalized;
    let Some(raw) = proposal.get("native_contracts").filter(|v| !v.is_null()) else {
        return Ok(json!([]));
    };
    let mut contracts: Contracts = serde_json::from_value(raw.clone())?;
    // Report causal producer errors before unused-artifact/staffing symptoms.
    // This only describes the proposal; it cannot repair or choose its graph.
    ensure!(contracts.sources.len()<=8 && contracts.artifacts.len()<=16,"native contract limits exceeded: {} total sources (maximum 8, including fixed input roots), {} artifacts (maximum 16). Reuse one source for its different result projections rather than repeating computations",contracts.sources.len(),contracts.artifacts.len());
    for source in &contracts.sources {
        ensure!(valid_id(&source.id), "invalid native source id");
    }
    crate::planning_graph::validate(
        &contracts
            .sources
            .iter()
            .map(|source| crate::planning_graph::Node {
                id: &source.id,
                inputs: source
                    .argument_sources
                    .iter()
                    .map(|input| (input.field.as_str(), input.source.as_str()))
                    .collect(),
            })
            .collect::<Vec<_>>(),
    )?;
    let inputs = merge_inputs(&json!([]), inputs)?;
    for source in &mut contracts.sources {
        if let Some(id) = source.input.take() {
            ensure!(
                source.tool.is_empty() && source.arguments.is_null(),
                "input references cannot override source arguments"
            );
            let input = inputs
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == id)
                .context("unknown exact native input reference")?;
            let slots = input["argument_slots"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if !slots.is_empty() {
                let declared = slots
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|i| i as usize)
                    .collect::<BTreeSet<_>>();
                let selected = source
                    .argument_sources
                    .iter()
                    .filter_map(|r| r.argument_index)
                    .collect::<BTreeSet<_>>();
                ensure!(declared==selected && selected.len()==source.argument_sources.len()
                    && source.argument_sources.iter().all(|r|r.field=="arguments" && r.pointer=="/output" && r.value_pointer.is_some()),
                    "owned template requires each positional slot exactly once with a typed value pointer");
            } else {
                let declared = input["derived_fields"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>();
                let selected = source
                    .argument_sources
                    .iter()
                    .map(|r| r.field.as_str())
                    .collect::<BTreeSet<_>>();
                ensure!(
                    declared == selected
                        && selected.len() == source.argument_sources.len()
                        && source
                            .argument_sources
                            .iter()
                            .all(|r| r.argument_index.is_none() && r.value_pointer.is_none()),
                    "input requires exactly its declared whole-field dependencies"
                );
            }
            source.tool = input["tool"].as_str().unwrap().to_string();
            source.arguments = input["arguments"].clone();
        } else {
            if !inputs.as_array().unwrap().is_empty() {
                ensure!(
                    allows_derived_sources(&inputs),
                    "attached templates must be referenced, never recopied by the CEO"
                );
                let fields = derived_fields(&source.tool)?;
                let selected = source
                    .argument_sources
                    .iter()
                    .map(|reference| reference.field.as_str())
                    .collect::<BTreeSet<_>>();
                ensure!(
                    selected == fields.iter().map(String::as_str).collect::<BTreeSet<_>>()
                        && selected.len() == source.argument_sources.len(),
                    "derived source requires every declared data input exactly once"
                );
                for field in fields {
                    ensure!(source.arguments.get(&field)==Some(&json!([])),"derived data inputs must be empty arrays; only native receipts may supply rows");
                }
                validate_source(&source.tool, &source.arguments)?;
            }
        }
    }
    let mut receipt_only_paths = BTreeSet::new();
    if proposal["capability_mode"] == "native_prepared_graph" {
        let needed = contracts
            .sources
            .iter()
            .flat_map(|s| s.argument_sources.iter().map(|r| r.source.as_str()))
            .collect::<BTreeSet<_>>();
        let no_publication = contracts.artifacts.is_empty();
        let mut paths = contracts
            .artifacts
            .iter()
            .map(|a| a.path.clone())
            .collect::<BTreeSet<_>>();
        for source in &contracts.sources {
            if (no_publication || needed.contains(source.id.as_str()))
                && !contracts
                    .artifacts
                    .iter()
                    .any(|a| a.source == source.id && a.pointer == "/output")
            {
                let identity = gents_local_mcp::digest(
                    json!({"source":source.id,"pointer":"/output"})
                        .to_string()
                        .as_bytes(),
                );
                let path = (0..=32)
                    .map(|n| format!("mcp-receipt-{identity}-{n}.json"))
                    .find(|p| !paths.contains(p))
                    .context("no unique receipt identity available")?;
                paths.insert(path.clone());
                receipt_only_paths.insert(path.clone());
                contracts.artifacts.push(Artifact {
                    path,
                    source: source.id.clone(),
                    writer: source.actor.clone(),
                    pointer: "/output".into(),
                    encoding: "json".into(),
                });
            }
        }
    }
    let projected = contracts
        .artifacts
        .iter()
        .map(|artifact| artifact.source.as_str())
        .collect::<BTreeSet<_>>();
    let unused = contracts
        .sources
        .iter()
        .filter(|source| !projected.contains(source.id.as_str()))
        .map(|source| source.id.as_str())
        .collect::<Vec<_>>();
    ensure!(unused.is_empty(),"unused native source ids {:?}; remove declarations that contribute no approved artifact projection",unused);
    let staffing = if contracts.sources.is_empty() {
        vec![]
    } else {
        crate::ceo_team::parse(proposal)?
    };
    for source in &mut contracts.sources {
        if source.actor.is_empty() {
            let candidates = staffing
                .iter()
                .filter(|stage| {
                    stage.tools.contains(&source.tool)
                        || source.arguments["op"].as_str().is_some_and(|op| {
                            stage.tools.contains(&format!("{}:{op}", source.tool))
                        })
                })
                .collect::<Vec<_>>();
            ensure!(candidates.len()==1,"source {} tool {} has {} staffing owners; request this registered tool in exactly one suitable draft staffing stage. Draft requests grant no runtime authority",source.id,source.tool,candidates.len());
            source.actor = candidates[0].actor.clone();
        }
    }
    let mut sources = BTreeMap::new();
    for source in &contracts.sources {
        ensure!(valid_id(&source.id), "invalid native source id");
        validate_source(&source.tool, &source.arguments)?;
        ensure!(
            sources.insert(&source.id, source).is_none(),
            "duplicate native source id"
        );
    }
    let mut used = BTreeSet::new();
    let mut bindings = vec![];
    for artifact in &contracts.artifacts {
        let source = sources.get(&artifact.source).with_context(|| {
            format!(
                "artifact {} references unknown source {}; declared source ids: {:?}",
                artifact.path,
                artifact.source,
                sources.keys().collect::<Vec<_>>()
            )
        })?;
        // Report impossible output fields before secondary producer-file
        // requirements. Reuse the native schema check; never compute a value
        // or propose a replacement graph in planning feedback.
        gents_local_mcp::Registry::new()
            .validate_projection(
                &source.tool,
                &source.arguments,
                &artifact.pointer,
                &artifact.encoding,
            )
            .map_err(|error| {
                anyhow::anyhow!("artifact {} source {}: {error}", artifact.path, source.id)
            })?;
        used.insert(&artifact.source);
        let writer = if artifact.writer.is_empty() {
            let source_index = staffing
                .iter()
                .position(|stage| stage.actor == source.actor)
                .context("source actor not staffed")?;
            let candidates = staffing
                .iter()
                .enumerate()
                .filter(|(index, stage)| {
                    *index >= source_index
                        && stage.tools.contains(&"files:read".to_string())
                        && stage.tools.contains(&"files:write".to_string())
                })
                .map(|(_, stage)| stage)
                .collect::<Vec<_>>();
            ensure!(candidates.len()==1,"artifact {} requires exactly one writer with files:read and files:write at or after source {} owned by {}; found {} eligible writers. Order analysis before output and keep one distinct actor per stage. The output draft must request both file operations needed for the requested files; draft requests grant no runtime authority",artifact.path,source.id,source.actor,candidates.len());
            candidates[0].actor.clone()
        } else {
            artifact.writer.clone()
        };
        let mut arguments_from = vec![];
        for input in &source.argument_sources {
            let parent = contracts
                .artifacts
                .iter()
                .find(|a| a.source == input.source && a.pointer == input.pointer)
                .with_context(||format!("source {} field {} requires producer {} projection {}; declare a producer artifact for that exact source/pointer",source.id,input.field,input.source,input.pointer))?;
            let mut reference = json!({"field":input.field,"path":parent.path});
            if let Some(index) = input.argument_index {
                reference["argument_index"] = json!(index);
                reference["pointer"] = json!(input
                    .value_pointer
                    .as_ref()
                    .context("owned value pointer missing")?);
            }
            arguments_from.push(reference);
        }
        let mut binding = json!({"path":artifact.path,"source_actor":source.actor,"writer":writer,"source_tool":source.tool,"arguments":source.arguments,"pointer":artifact.pointer,"encoding":artifact.encoding});
        if receipt_only_paths.contains(&artifact.path) {
            binding["receipt_only"] = json!(true);
        }
        if !arguments_from.is_empty() {
            binding["argument_sources"] = json!(arguments_from);
        }
        bindings.push(binding);
    }
    ensure!(used.len()==sources.len(),"unused native source ids {:?}; remove declarations that contribute no approved artifact projection",sources.keys().filter(|id|!used.contains(*id)).collect::<Vec<_>>());
    if contracts.sources.iter().any(|source| {
        matches!(
            source.tool.as_str(),
            "integer_arithmetic" | "table_aggregate"
        )
    }) {
        let mut allowed = numeric_tokens(facts);
        for source in &contracts.sources {
            input_numbers(&source.arguments, &mut allowed);
            if let Some(rows) = source.arguments["rows"].as_array() {
                allowed.insert(rows.len().to_string());
            }
            if let Some(rows) = source.arguments["calculations"].as_array() {
                allowed.insert(rows.len().to_string());
            }
        }
        allowed.insert(contracts.artifacts.len().to_string());
        allowed.insert(
            proposal["assignments"]
                .as_array()
                .map_or(0, Vec::len)
                .to_string(),
        );
        for artifact in &contracts.artifacts {
            allowed.extend(numeric_tokens(&artifact.path));
        }
        let mut prose = proposal["definition_of_done"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        for stage in proposal["assignments"].as_array().into_iter().flatten() {
            if let Some(task) = stage["task"].as_str() {
                prose.push(task);
            }
            prose.extend(
                stage["acceptance"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            );
        }
        for text in prose {
            ensure!(numeric_tokens(text).is_subset(&allowed),"planning prose introduces a numeric literal absent from owner inputs or contract structure; remove computed expected answers and compare against actual native results");
        }
    }
    let mut ordered = vec![];
    while !bindings.is_empty() {
        let position = bindings
            .iter()
            .position(|b| {
                b["argument_sources"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .all(|reference| {
                        ordered
                            .iter()
                            .any(|prior: &Value| prior["path"] == reference["path"])
                    })
            })
            .with_context(||format!("native input dependency cycle/missing producer: {:?}",bindings.iter().map(|binding|json!({"path":binding["path"],"requires":binding["argument_sources"]})).collect::<Vec<_>>()))?;
        ordered.push(bindings.remove(position));
    }
    let mut intake =
        json!({"ceo_plan":proposal,"ceo_source_inputs":inputs,"artifact_bindings":ordered});
    if proposal["capability_mode"] == "native_prepared_graph" && proposal["status"] == "ready" {
        intake["required_owned_rust_calls"] = crate::owned_inputs::native_requirements(
            session.context("prepared graph requires retained owner metadata")?,
            &intake["artifact_bindings"],
        )?;
    }
    let parsed = gents_local_mcp::artifact_bindings::parse(&intake)?;
    if !parsed.is_empty() {
        crate::ceo_team::validate_bindings(&intake, &parsed)?;
    }
    Ok(intake["artifact_bindings"].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proposal() -> Value {
        json!({"assignments":[{"actor":"Analyst","task":"Calculate","tools":["integer_arithmetic"],"acceptance":["native result"]},{"actor":"Writer","task":"Save and verify","tools":["files:read","files:write"],"acceptance":["exact native bytes"]}],"native_contracts":{
            "sources":[{"id":"calculation","actor":"Analyst","tool":"integer_arithmetic","arguments":{"calculations":[{"id":"input","values":[7,-10],"subtract":2}]}}],
            "artifacts":[{"path":"answer.txt","source":"calculation","writer":"Writer","pointer":"/calculations/0/difference","encoding":"integer_text"},{"path":"ledger.json","source":"calculation","writer":"Writer","pointer":"/calculations","encoding":"json"}]
        }})
    }
    #[test]
    fn self_reference_diagnostic_precedes_unused_fixed_input_symptom() {
        let inputs =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let raw = json!({"assignments":[{"actor":"Analyst","task":"Analyze","tools":["table_read_json","table_aggregate"],"acceptance":["native result"]},{"actor":"Writer","task":"Save","tools":["files:read","files:write"],"acceptance":["readback"]}],"native_contracts":{"sources":[{"id":"transform_bad","actor":"Analyst","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"category","value_field":"units"},"argument_sources":[{"field":"rows","source":"transform_bad","pointer":"/rows"}]}],"artifacts":[{"path":"groups.json","source":"transform_bad","writer":"Writer","pointer":"/groups","encoding":"json"}]}});
        let original = raw.clone();
        let error = compile_with_inputs(&raw, &inputs).unwrap_err().to_string();
        assert!(
            error.contains("source transform_bad field rows references itself"),
            "{error}"
        );
        assert!(!error.contains("unused native source"));
        assert_eq!(raw, original);
    }
    #[test]
    fn invalid_native_projection_is_reported_before_missing_intermediate_file() {
        let inputs =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let raw = json!({"assignments":[{"actor":"Analyst","task":"Analyze","tools":["table_read_json","table_aggregate"],"acceptance":["native result"]},{"actor":"Writer","task":"Save","tools":["files:read","files:write"],"acceptance":["readback"]}],"native_contracts":{"sources":[{"id":"transform_bad","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"category","value_field":"units"},"argument_sources":[{"field":"rows","source":"file-input","pointer":"/rows"}]}],"artifacts":[{"path":"answer.json","source":"transform_bad","pointer":"/rows","encoding":"json"},{"path":"bytes.txt","source":"file-input","pointer":"/source_bytes","encoding":"integer_text"}]}});
        let original = raw.clone();
        let error = compile_with_inputs(&raw, &inputs).unwrap_err();
        let diagnostic = error.to_string();
        assert!(
            diagnostic.contains("artifact answer.json source transform_bad"),
            "{diagnostic}"
        );
        assert!(diagnostic.contains("not declared by native table_aggregate"));
        assert!(!diagnostic.contains("requires producer"));
        assert_eq!(raw, original);
    }
    #[test]
    fn fixed_root_closure_does_not_choose_transforms_or_add_authority() {
        let raw = json!({"assignments":[{"actor":"Analyst","task":"Analyze","tools":["table_read_json","table_aggregate"],"acceptance":["native result"]},{"actor":"Writer","task":"Save","tools":["files:read","files:write"],"acceptance":["readback"]}],"native_contracts":{"sources":[{"id":"transform_summary","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"category","value_field":"units"},"argument_sources":[{"field":"rows","source":"file-input","pointer":"/rows"}]}],"artifacts":[{"path":"rows.json","source":"file-input","pointer":"/rows","encoding":"json"},{"path":"total.txt","source":"transform_summary","pointer":"/total","encoding":"integer_text"}]}});
        let inputs =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let (normalized, added) = compile_fixed_roots(&raw, &inputs).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(normalized["assignments"], raw["assignments"]);
        assert_eq!(
            normalized["native_contracts"]["sources"][1],
            raw["native_contracts"]["sources"][0]
        );
        compile_with_inputs(&raw, &inputs).unwrap();
        assert!(compile_fixed_roots(&normalized, &inputs)
            .unwrap()
            .1
            .is_empty());
        let mut denied = raw.clone();
        denied["assignments"][0]["tools"] = json!(["table_aggregate"]);
        assert!(
            compile_with_inputs(&denied, &inputs).is_err(),
            "root closure must not grant file intake"
        );
        let mut shadow = raw.clone();
        shadow["native_contracts"]["sources"][0]["id"] = json!("file-input");
        assert!(compile_fixed_roots(&shadow, &inputs).is_err());
    }
    #[test]
    fn input_owners_materialize_only_selected_immutable_roots_without_changing_transforms() {
        let inputs = json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}},{"id":"unused","tool":"table_read_json","arguments":{"path":"unused.json"}}]);
        let raw = json!({"status":"ready","question":"","capability_mode":"native_graph_v2","input_owners":{"file-input":"Analyst","unused":null},"definition_of_done":["Actual native readback"],"assignments":[{"actor":"Analyst","tools":[],"task":"Aggregate","acceptance":["native result"]},{"actor":"Writer","tools":[],"task":"Save","acceptance":["readback"]}],"native_contracts":{"sources":[{"id":"transform_sum","actor":"Analyst","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"category","value_field":"units"},"argument_sources":[{"field":"rows","source":"file-input","pointer":"/rows"}]}],"artifacts":[{"path":"rows.json","source":"file-input","writer":"Writer","pointer":"/rows","encoding":"json"},{"path":"total.txt","source":"transform_sum","writer":"Writer","pointer":"/total","encoding":"integer_text"}]}});
        let (owned, controls) = compile_input_owners(&raw, &inputs).unwrap();
        assert_eq!(
            owned["native_contracts"]["sources"][1],
            raw["native_contracts"]["sources"][0]
        );
        assert_eq!(
            owned["native_contracts"]["sources"][0],
            json!({"id":"file-input","input":"file-input","actor":"Analyst","argument_sources":[]})
        );
        assert_eq!(controls.len(), 1);
        assert_eq!(controls[0]["actor"], "Analyst");
        let (compiled, _) = compile_graph_capabilities(&owned, &inputs).unwrap();
        compile_with_inputs(&compiled, &inputs).unwrap();
        let mut changed = compiled.clone();
        changed["input_owners"]["file-input"] = json!("Writer");
        assert!(compile_with_inputs(&changed, &inputs).is_err());
        for failure in ["missing", "extra", "invented", "recopied", "null_used"] {
            let mut bad = raw.clone();
            match failure {
                "missing" => {
                    bad["input_owners"]
                        .as_object_mut()
                        .unwrap()
                        .remove("file-input");
                }
                "extra" => bad["input_owners"]["other"] = json!("Analyst"),
                "invented" => bad["input_owners"]["file-input"] = json!({"private_rows":[42]}),
                "recopied" => {
                    bad["native_contracts"]["sources"][0] =
                        json!({"id":"file-input","input":"file-input","actor":"Analyst"})
                }
                "null_used" => bad["input_owners"]["file-input"] = Value::Null,
                _ => unreachable!(),
            }
            assert!(
                compile_input_owners(&bad, &inputs)
                    .and_then(|(owned, _)| compile_graph_capabilities(&owned, &inputs))
                    .is_err(),
                "accepted {failure}"
            );
        }
    }
    #[test]
    fn source_output_collision_is_rejected_during_planning_compilation() {
        let inputs =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let proposal = json!({"assignments":[{"actor":"Worker","tools":["table_read_json","files:read","files:write"],"task":"Read and save","acceptance":["readback"]}],"native_contracts":{"sources":[{"id":"file-input","input":"file-input","actor":"Worker","argument_sources":[]}],"artifacts":[{"path":"input.json","source":"file-input","writer":"Worker","pointer":"/rows","encoding":"json"}]}});
        assert!(compile_with_inputs(&proposal, &inputs)
            .unwrap_err()
            .to_string()
            .contains("overlaps"));
        let mut separate = proposal.clone();
        separate["native_contracts"]["artifacts"][0]["path"] = json!("output.json");
        compile_with_inputs(&separate, &inputs).unwrap();
    }
    #[test]
    fn graph_capabilities_follow_only_explicit_owners_and_writers() {
        let inputs =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let raw = json!({"status":"ready","question":"","capability_mode":"native_graph","definition_of_done":["Actual native file readback"],"assignments":[{"actor":"Analyst","tools":[],"task":"Read input","acceptance":["actual source"]},{"actor":"Writer","tools":[],"task":"Save rows","acceptance":["readback"]}],"native_contracts":{"sources":[{"id":"file-input","input":"file-input","actor":"Analyst","argument_sources":[]}],"artifacts":[{"path":"rows.json","source":"file-input","writer":"Writer","pointer":"/rows","encoding":"json"}]}});
        let (compiled, controls) = compile_graph_capabilities(&raw, &inputs).unwrap();
        assert_eq!(
            compiled["assignments"][0]["tools"],
            json!(["table_read_json"])
        );
        assert_eq!(
            compiled["assignments"][1]["tools"],
            json!(["files:read", "files:write"])
        );
        assert_eq!(compiled["native_contracts"], raw["native_contracts"]);
        assert_eq!(controls.len(), 2);
        assert!(raw["assignments"][0]["tools"]
            .as_array()
            .unwrap()
            .is_empty());
        compile_with_inputs(&compiled, &inputs).unwrap();
        for failure in [
            "second_list",
            "missing_owner",
            "wrong_writer",
            "duplicate_stage",
            "unused_stage",
        ] {
            let mut bad = raw.clone();
            match failure {
                "second_list" => bad["assignments"][0]["tools"] = json!(["memory_append"]),
                "missing_owner" => {
                    bad["native_contracts"]["sources"][0]
                        .as_object_mut()
                        .unwrap()
                        .remove("actor");
                }
                "wrong_writer" => {
                    bad["native_contracts"]["artifacts"][0]["writer"] = json!("Outsider")
                }
                "duplicate_stage" => bad["assignments"][1]["actor"] = json!("Analyst"),
                "unused_stage" => bad["native_contracts"]["artifacts"] = json!([]),
                _ => unreachable!(),
            }
            assert!(
                compile_graph_capabilities(&bad, &inputs).is_err(),
                "accepted {failure}"
            );
        }
        assert!(compile_graph_capabilities(&raw, &json!([])).is_err());
        let mut omitted = raw.clone();
        omitted["native_contracts"]["sources"] = json!([]);
        assert!(compile_graph_capabilities(&omitted, &inputs)
            .unwrap_err()
            .to_string()
            .contains("explicit source declaration"));
    }
    #[test]
    fn rejected_graph_diagnostics_exclude_arguments_rows_and_task_prose() {
        let raw = json!({"assignments":[{"actor":"Analyst","tools":["table_aggregate"],"task":"PRIVATE_STAFFING_TASK","acceptance":["private computed answer 54321"]}],"definition_of_done":["private computed answer 12345"],"native_contracts":{"sources":[{"id":"source","tool":"table_aggregate","arguments":{"rows":[{"secret":"PRIVATE_ROW_PAYLOAD"}]},"argument_sources":[{"field":"rows","source":"missing","pointer":"/rows"}]}],"artifacts":[{"path":"total.txt","source":"source","pointer":"/total","encoding":"integer_text"}]}});
        let shape = rejected_shape(&raw).unwrap();
        let text = shape.to_string();
        assert!(!text.contains("PRIVATE_ROW_PAYLOAD"));
        assert!(!text.contains("PRIVATE_STAFFING_TASK"));
        assert!(!text.contains("54321"));
        assert_eq!(
            shape["staffing"],
            json!([{"actor":"Analyst","tools":["table_aggregate"]}])
        );
        assert!(!text.contains("12345"));
        assert!(shape["sources"][0].get("arguments").is_none());
        assert_eq!(
            shape["sources"][0]["argument_sources"][0]["source"],
            "missing"
        );
    }
    #[test]
    fn fixed_inputs_allow_only_fully_receipt_bound_pure_derivations() {
        let mut p = proposal();
        p["assignments"][0]["tools"] = json!(["table_read_json", "table_aggregate"]);
        p["native_contracts"] = json!({"sources":[{"id":"file","input":"file-input"},{"id":"summary","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"category","value_field":"units"},"argument_sources":[{"field":"rows","source":"file","pointer":"/rows"}]}],"artifacts":[{"path":"rows.json","source":"file","pointer":"/rows","encoding":"json"},{"path":"total.txt","source":"summary","pointer":"/total","encoding":"integer_text"}]});
        let inputs =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let compiled = compile_with_inputs(&p, &inputs).unwrap();
        assert_eq!(
            compiled[1]["argument_sources"],
            json!([{"field":"rows","path":"rows.json"}])
        );
        assert_eq!(compiled[1]["arguments"]["rows"], json!([]));
        for change in [
            "invented_rows",
            "missing_reference",
            "file_recopy",
            "mutable",
            "arithmetic",
            "cycle",
        ] {
            let mut bad = p.clone();
            match change {
                "invented_rows" => {
                    bad["native_contracts"]["sources"][1]["arguments"]["rows"] =
                        json!([{"category":"invented","units":99}])
                }
                "missing_reference" => {
                    bad["native_contracts"]["sources"][1]["argument_sources"] = json!([])
                }
                "file_recopy" => {
                    bad["native_contracts"]["sources"][1] = json!({"id":"summary","tool":"table_read_json","arguments":{"path":"other.json"}})
                }
                "mutable" => bad["native_contracts"]["sources"][1]["tool"] = json!("memory_append"),
                "arithmetic" => {
                    bad["native_contracts"]["sources"][1] = json!({"id":"summary","tool":"integer_arithmetic","arguments":{"calculations":[{"id":"guessed","values":[1],"subtract":0}]}})
                }
                "cycle" => {
                    bad["native_contracts"]["sources"][1]["argument_sources"][0] =
                        json!({"field":"rows","source":"summary","pointer":"/total"})
                }
                _ => unreachable!(),
            }
            assert!(
                compile_with_inputs(&bad, &inputs).is_err(),
                "accepted {change}"
            );
        }
    }
    #[test]
    fn derived_decoder_choices_follow_native_data_fields_without_overriding_templates() {
        let fixed =
            json!([{"id":"file-input","tool":"table_read_json","arguments":{"path":"input.json"}}]);
        let mut schema = crate::ceo_planning::schema();
        bind_input_schema(&mut schema, &fixed).unwrap();
        let choices = schema["properties"]["native_contracts"]["properties"]["sources"]["items"]
            ["oneOf"]
            .as_array()
            .unwrap();
        let join = choices
            .iter()
            .find(|row| row["properties"]["tool"]["const"] == "table_join")
            .unwrap();
        assert_eq!(join["properties"]["argument_sources"]["minItems"], 2);
        assert_eq!(
            join["properties"]["arguments"]["properties"]["left_rows"]["const"],
            json!([])
        );
        assert!(!choices
            .iter()
            .any(|row| row["properties"]["tool"]["const"] == "integer_arithmetic"));
        let template = json!([{"id":"template","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"category","value_field":"units"},"derived_fields":["rows"]}]);
        bind_input_schema(&mut schema, &template).unwrap();
        let choices = schema["properties"]["native_contracts"]["properties"]["sources"]["items"]
            ["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0]["properties"]["input"]["const"], "template");
    }
    #[test]
    fn readback_control_completes_only_read_permission_and_retains_raw_draft() {
        let mut raw = proposal();
        raw["assignments"][1]["tools"] = json!(["files:write"]);
        let (compiled, added) = compile_readbacks(&raw).unwrap();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0]["tool"], "files:read");
        assert_eq!(raw["assignments"][1]["tools"], json!(["files:write"]));
        compile(&compiled).unwrap();
        raw["assignments"][1]["tools"] = json!(["files:read"]);
        let (compiled, added) = compile_readbacks(&raw).unwrap();
        assert!(added.is_empty());
        assert!(
            compile(&compiled).is_err(),
            "write authority cannot be inferred"
        );
        raw = proposal();
        raw["assignments"][0]["tools"] = json!([]);
        let (compiled, added) = compile_readbacks(&raw).unwrap();
        assert!(added.is_empty());
        assert!(
            compile(&compiled).is_err(),
            "source tools cannot be inferred"
        );
    }
    #[test]
    fn unambiguous_role_bindings_reuse_ceo_staffing_without_granting_new_tools() {
        let mut p = proposal();
        p["native_contracts"]["sources"][0]
            .as_object_mut()
            .unwrap()
            .remove("actor");
        for artifact in p["native_contracts"]["artifacts"].as_array_mut().unwrap() {
            artifact.as_object_mut().unwrap().remove("writer");
        }
        let bindings = compile(&p).unwrap();
        assert_eq!(bindings[0]["source_actor"], "Analyst");
        assert_eq!(bindings[0]["writer"], "Writer");
        p["assignments"][1]["tools"]
            .as_array_mut()
            .unwrap()
            .push(json!("integer_arithmetic"));
        assert!(
            compile(&p).is_err(),
            "multiple owners require an explicit choice"
        );
    }
    #[test]
    fn derived_fields_require_owner_declaration_and_compile_in_dependency_order() {
        let mut p = proposal();
        p["assignments"][0]["tools"] = json!(["table_aggregate"]);
        p["native_contracts"] = json!({"sources":[{"id":"first","actor":"Analyst","input":"raw"},{"id":"second","actor":"Analyst","input":"summary","argument_sources":[{"field":"rows","source":"first","pointer":"/groups"}]}],"artifacts":[{"path":"child.json","source":"second","writer":"Writer","pointer":"/groups","encoding":"json"},{"path":"parent.json","source":"first","writer":"Writer","pointer":"/groups","encoding":"json"}]});
        let inputs = json!([{"id":"raw","tool":"table_aggregate","arguments":{"rows":[{"g":"x","v":1}],"filter":null,"group_by":"g","value_field":"v"}},{"id":"summary","tool":"table_aggregate","arguments":{"rows":[],"filter":null,"group_by":"rows","value_field":"total"},"derived_fields":["rows"]}]);
        let bindings = compile_with_inputs(&p, &inputs).unwrap();
        assert_eq!(bindings[0]["path"], "parent.json");
        assert_eq!(
            bindings[1]["argument_sources"],
            json!([{"field":"rows","path":"parent.json"}])
        );
        assert_eq!(
            bindings[1]["arguments"]["rows"],
            json!([]),
            "compilation cannot invent the upstream result"
        );
        let mut undeclared = inputs.clone();
        undeclared[1]
            .as_object_mut()
            .unwrap()
            .remove("derived_fields");
        assert!(compile_with_inputs(&p, &undeclared).is_err());
        assert!(merge_inputs(&inputs, &undeclared).is_err());
        p["native_contracts"]["sources"][1]["argument_sources"][0]["source"] = json!("second");
        assert!(compile_with_inputs(&p, &inputs).is_err());
    }
    #[test]
    fn arithmetic_plan_cannot_launder_new_numeric_answers_from_prior_model_claims() {
        let mut p = proposal();
        p["assignments"][0]["acceptance"] = json!(["The expected difference is 65"]);
        assert!(compile(&p).is_err());
        let session = json!({"messages":[{"role":"ceo","message":"The answer is 65"},{"role":"user","message":"Use the exact signed input values and compare to native output"}]});
        assert!(compile_for_session(&p, &session).is_err());
        p["assignments"][0]["acceptance"] =
            json!(["The supplied source values [7,-10] and subtract 2 are executed exactly"]);
        compile(&p).unwrap();
        assert!(numeric_tokens("SHA256 receipt").is_empty());
    }
    #[test]
    fn attached_inputs_are_immutable_and_cannot_be_rewritten_by_model_output() {
        let mut p = proposal();
        let original = p["native_contracts"]["sources"][0]["arguments"].clone();
        let inputs = json!([{"id":"owner-input","tool":"integer_arithmetic","arguments":original}]);
        assert!(
            compile_with_inputs(&p, &inputs).is_err(),
            "attached data must be referenced rather than recopied"
        );
        p["native_contracts"]["sources"][0] =
            json!({"id":"calculation","actor":"Analyst","input":"owner-input"});
        let bindings = compile_with_inputs(&p, &inputs).unwrap();
        assert_eq!(bindings[0]["arguments"], original);
        let mut altered = inputs.clone();
        altered[0]["arguments"]["calculations"][0]["values"] = json!([]);
        assert!(merge_inputs(&inputs, &altered).is_err());
        assert_eq!(merge_inputs(&inputs, &inputs).unwrap(), inputs);
        p["native_contracts"]["sources"][0]["arguments"] = json!({"calculations":[]});
        assert!(compile_with_inputs(&p, &inputs).is_err());
        p["native_contracts"]["sources"][0]
            .as_object_mut()
            .unwrap()
            .remove("arguments");
        p["native_contracts"]["sources"][0]["input"] = json!("unknown");
        assert!(compile_with_inputs(&p, &inputs).is_err());
        let mut schema = crate::ceo_planning::schema();
        bind_input_schema(&mut schema, &inputs).unwrap();
        let source =
            &schema["properties"]["native_contracts"]["properties"]["sources"]["items"]["oneOf"][0];
        assert_eq!(source["properties"]["input"]["const"], json!("owner-input"));
        assert!(source["properties"].get("arguments").is_none());
    }
    #[test]
    fn shared_native_source_compiles_without_calculating_or_granting_authority() {
        let p = proposal();
        let bindings = compile(&p).unwrap();
        assert_eq!(bindings.as_array().unwrap().len(), 2);
        assert_eq!(bindings[0]["arguments"], bindings[1]["arguments"]);
        assert_eq!(bindings[0]["source_actor"], "Analyst");
        assert!(bindings[0].get("expected").is_none());
        assert!(bindings[0]["arguments"].get("difference").is_none());
        assert!(p.get("policy").is_none());
        assert_eq!(compile(&json!({})).unwrap(), json!([]));
    }
    #[test]
    fn contracts_reject_nonexistent_fields_wrong_types_and_out_of_range_rows() {
        for (pointer, encoding) in [
            ("/calculations/0/values", "json"),
            ("/calculations/1/difference", "integer_text"),
            ("/calculations/00/difference", "integer_text"),
            ("/calculations/0/id", "integer_text"),
            ("/exact", "integer_text"),
        ] {
            let mut p = proposal();
            p["native_contracts"]["artifacts"][0]["pointer"] = json!(pointer);
            p["native_contracts"]["artifacts"][0]["encoding"] = json!(encoding);
            assert!(compile(&p).is_err(), "accepted {pointer} with {encoding}");
        }
        let mut p = proposal();
        p["native_contracts"]["sources"][0]["arguments"]["calculations"][0]["values"][0] =
            json!(true);
        assert!(compile(&p).is_err());
    }
    #[test]
    fn proposed_bindings_cannot_expand_actor_order_or_tool_ceiling() {
        for change in [
            "unknown-source",
            "future-source",
            "wrong-writer",
            "missing-write",
            "duplicate-path",
            "unused-source",
            "mutation",
        ] {
            let mut p = proposal();
            match change {
                "unknown-source" => {
                    p["native_contracts"]["artifacts"][0]["source"] = json!("invented")
                }
                "future-source" => p["native_contracts"]["sources"][0]["actor"] = json!("Writer"),
                "wrong-writer" => {
                    p["native_contracts"]["artifacts"][0]["writer"] = json!("Outsider")
                }
                "missing-write" => p["assignments"][1]["tools"] = json!(["files:read"]),
                "duplicate-path" => {
                    p["native_contracts"]["artifacts"][1]["path"] = json!("answer.txt")
                }
                "unused-source" => {
                    let mut source = p["native_contracts"]["sources"][0].clone();
                    source["id"] = json!("unused");
                    p["native_contracts"]["sources"]
                        .as_array_mut()
                        .unwrap()
                        .push(source);
                }
                "mutation" => {
                    p["native_contracts"]["sources"][0]["tool"] = json!("files");
                    p["native_contracts"]["sources"][0]["arguments"] =
                        json!({"op":"write","path":"x.txt","content":"no"});
                }
                _ => unreachable!(),
            };
            assert!(compile(&p).is_err(), "accepted {change}");
        }
    }
}

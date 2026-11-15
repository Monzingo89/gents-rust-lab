//! Owner-selected invocation data and retained source custody. Never execution authority.
use anyhow::{ensure, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};
use uuid::Uuid;

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Invocation {
    id: String,
    component_id: String,
    function: String,
    arguments: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    argument_slots: Vec<usize>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    invocation: Invocation,
    origin_company: String,
    source_path: String,
    source_sha256: String,
    api_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_template_sha256: Option<String>,
    bytes: usize,
}
pub struct Prepared {
    manifest: Manifest,
    source: String,
}
fn parse(incoming: &Value) -> Result<Vec<Invocation>> {
    let inputs: Vec<Invocation> = serde_json::from_value(if incoming.is_null() {
        json!([])
    } else {
        incoming.clone()
    })?;
    ensure!(
        inputs.len() <= 8 && incoming.to_string().len() <= 12288,
        "owned invocation input budget exceeded"
    );
    let mut ids = BTreeSet::new();
    for input in &inputs {
        ensure!(
            !input.id.is_empty()
                && input.id.len() <= 40
                && input
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && ids.insert(&input.id),
            "unique lowercase invocation ids required"
        );
        crate::owned_catalog::selection_ids(&json!([input.component_id]))?;
        ensure!(
            !input.function.is_empty() && input.function.len() <= 128,
            "bounded function name required"
        );
        ensure!(
            input.arguments.is_array() && input.arguments.to_string().len() <= 8192,
            "bounded positional arguments required"
        );
        let slots = input
            .argument_slots
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        ensure!(
            slots.len() == input.argument_slots.len()
                && slots.len() <= 8
                && slots.iter().all(|i| input
                    .arguments
                    .as_array()
                    .and_then(|a| a.get(*i))
                    .is_some_and(gents_local_mcp::artifact_bindings::is_placeholder)),
            "invalid, duplicate or nonempty owned argument slots"
        );
    }
    crate::security::require_persistable(&incoming.to_string())?;
    Ok(inputs)
}
fn checked(manifest: Manifest, source: String) -> Result<Prepared> {
    ensure!(
        source.len() <= 8192
            && source.len() == manifest.bytes
            && gents_local_mcp::digest(source.as_bytes()) == manifest.source_sha256,
        "retained component source identity mismatch"
    );
    ensure!(
        manifest.source_path == format!("owned-{}.rs", manifest.invocation.component_id),
        "owned source path mismatch"
    );
    parse(&json!([manifest.invocation]))?;
    let description = gents_local_mcp::component_api::describe(&source)?;
    ensure!(
        description["api_sha256"] == manifest.api_sha256
            && if manifest.invocation.argument_slots.is_empty() {
                manifest.input_template_sha256.is_none()
                    && manifest.input_sha256.as_ref()
                        == Some(&gents_local_mcp::digest(
                            manifest.invocation.arguments.to_string().as_bytes(),
                        ))
            } else {
                manifest.input_sha256.is_none()
                    && manifest.input_template_sha256.as_ref()
                        == Some(&gents_local_mcp::digest(
                            manifest.invocation.arguments.to_string().as_bytes(),
                        ))
            },
        "owned API/input identity mismatch"
    );
    gents_local_mcp::component_api::validate_template(
        &source,
        &manifest.invocation.function,
        &manifest.invocation.arguments,
        &manifest.invocation.argument_slots,
    )?;
    crate::security::require_persistable(&source)?;
    Ok(Prepared { manifest, source })
}
/// Current owner view must agree with the immutable interface selected earlier.
/// Only the host reads source bytes; caller JSON cannot supply a source or actor.
pub async fn prepare(
    root: &Path,
    company: &str,
    owner: &str,
    snapshot: &Value,
    incoming: &Value,
) -> Result<Vec<Prepared>> {
    let inputs = parse(incoming)?;
    if inputs.is_empty() {
        return Ok(vec![]);
    }
    let selected = snapshot["components"]
        .as_array()
        .context("select owned interfaces before adding invocations")?;
    ensure!(
        inputs
            .iter()
            .all(|i| selected.iter().any(|c| c["component_id"] == i.component_id)),
        "invocation must use a selected component"
    );
    let current = crate::owned_catalog::list(root, company, owner).await?;
    let mut prepared = vec![];
    for input in inputs {
        let prior = selected
            .iter()
            .find(|c| c["component_id"] == input.component_id)
            .unwrap();
        let entry = current["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["component_id"] == input.component_id)
            .context("component is no longer available to this owner")?;
        ensure!(
            entry["source_sha256"] == prior["source_sha256"]
                && entry["api_sha256"] == prior["api_sha256"]
                && entry["origin_company"] == prior["origin_company"],
            "selected source/API identity changed; start a new conversation"
        );
        let private = crate::owned_catalog::source(root, &input.component_id).await?;
        ensure!(
            private["component_id"] == input.component_id
                && private["origin_company"] == entry["origin_company"]
                && private["source_sha256"] == entry["source_sha256"],
            "private custody snapshot differs from selected component"
        );
        let source = private["text"]
            .as_str()
            .context("source snapshot missing")?
            .to_owned();
        let manifest = Manifest {
            source_path: format!("owned-{}.rs", input.component_id),
            source_sha256: entry["source_sha256"]
                .as_str()
                .context("source identity missing")?
                .into(),
            api_sha256: entry["api_sha256"]
                .as_str()
                .context("API identity missing")?
                .into(),
            origin_company: entry["origin_company"]
                .as_str()
                .context("source owner missing")?
                .into(),
            input_sha256: input
                .argument_slots
                .is_empty()
                .then(|| gents_local_mcp::digest(input.arguments.to_string().as_bytes())),
            input_template_sha256: (!input.argument_slots.is_empty())
                .then(|| gents_local_mcp::digest(input.arguments.to_string().as_bytes())),
            bytes: source.len(),
            invocation: input,
        };
        prepared.push(checked(manifest, source)?);
    }
    Ok(prepared)
}
fn install(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS ceo_owned_inputs(session TEXT NOT NULL,id TEXT NOT NULL,manifest TEXT NOT NULL,content TEXT NOT NULL,PRIMARY KEY(session,id));")?;
    Ok(())
}
fn stored(db: &Connection, session: Uuid) -> Result<Vec<Prepared>> {
    install(db)?;
    let mut query =
        db.prepare("SELECT CASE WHEN length(CAST(manifest AS BLOB))<=16384 THEN manifest END,CASE WHEN length(CAST(content AS BLOB))<=8192 THEN content END FROM ceo_owned_inputs WHERE session=?1 ORDER BY id LIMIT 9")?;
    let rows = query
        .query_map([session.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(rows.len() <= 8, "retained invocations exceed bound");
    rows.into_iter()
        .map(|(manifest, source)| checked(serde_json::from_str(&manifest)?, source))
        .collect()
}
fn metadata(retained: &[Prepared]) -> Value {
    json!(retained.iter().map(|p| &p.manifest).collect::<Vec<_>>())
}
fn descriptor(p: &Manifest) -> Value {
    let mut input = json!({"id":p.invocation.id,"tool":"owned_rust_call","arguments":{"binding_id":p.invocation.id,"arguments":p.invocation.arguments}});
    if !p.invocation.argument_slots.is_empty() {
        input["argument_slots"] = json!(p.invocation.argument_slots);
    }
    json!([input])
}
/// Atomic with the normal planning turn; rejected turns retain no source bytes.
pub fn retain(
    db: &Connection,
    session: Uuid,
    body: &mut Value,
    prepared: &[Prepared],
    incoming: &Value,
) -> Result<()> {
    let requests = parse(incoming)?;
    ensure!(
        requests.len() == prepared.len()
            && requests
                .iter()
                .zip(prepared)
                .all(|(i, p)| i == &p.manifest.invocation),
        "owned invocation preparation is host-only"
    );
    install(db)?;
    for p in prepared {
        let selected = body["owned_component_catalog"]["components"]
            .as_array()
            .context("selected component metadata missing")?;
        ensure!(
            selected
                .iter()
                .any(|c| c["component_id"] == p.manifest.invocation.component_id
                    && c["source_sha256"] == p.manifest.source_sha256
                    && c["api_sha256"] == p.manifest.api_sha256
                    && c["origin_company"] == p.manifest.origin_company),
            "prepared source differs from selected snapshot"
        );
        let old = stored(db, session)?;
        if let Some(old) = old
            .iter()
            .find(|v| v.manifest.invocation.id == p.manifest.invocation.id)
        {
            ensure!(
                serde_json::to_value(&old.manifest)? == serde_json::to_value(&p.manifest)?
                    && old.source == p.source,
                "owned invocation ids are immutable; start a new conversation"
            );
        } else {
            db.execute(
                "INSERT INTO ceo_owned_inputs VALUES(?1,?2,?3,?4)",
                params![
                    session.to_string(),
                    p.manifest.invocation.id,
                    serde_json::to_string(&p.manifest)?,
                    p.source
                ],
            )?;
        }
    }
    let retained = stored(db, session)?;
    let paths = retained
        .iter()
        .map(|p| p.manifest.source_path.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        paths.len() <= 4 && metadata(&retained).to_string().len() <= 16384,
        "retained owned input budget exceeded"
    );
    for p in &retained {
        body["source_inputs"] =
            crate::ceo_contracts::merge_inputs(&body["source_inputs"], &descriptor(&p.manifest))?;
    }
    if !retained.is_empty() {
        body["owned_invocations"] = metadata(&retained);
    }
    Ok(())
}
fn requirements(retained: &[Manifest], bindings: &Value) -> Result<Value> {
    let mut specs = vec![];
    let bindings = bindings
        .as_array()
        .context("native artifact bindings required for prepared invocations")?;
    for p in retained {
        let matches = bindings
            .iter()
            .filter(|b| {
                b["source_tool"] == "owned_rust_call"
                    && b["arguments"]["binding_id"] == p.invocation.id
            })
            .collect::<Vec<_>>();
        ensure!(
            !matches.is_empty(),
            "prepared invocation needs a native result artifact"
        );
        let actors = matches
            .iter()
            .filter_map(|b| b["source_actor"].as_str())
            .collect::<BTreeSet<_>>();
        ensure!(
            actors.len() == 1
                && matches
                    .iter()
                    .all(|b| b["arguments"] == descriptor(p)[0]["arguments"]),
            "prepared invocation must retain its exact template and one CEO-selected source actor"
        );
        let references = matches[0]["argument_sources"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        ensure!(
            matches.iter().all(|b| b["argument_sources"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                == references),
            "prepared invocation cannot acquire different dependency graphs per output"
        );
        let m = p;
        let mut spec = json!({"id":m.invocation.id,"actor":actors.first().unwrap(),"source_path":m.source_path,
            "source_sha256":m.source_sha256,"api_sha256":m.api_sha256,"function":m.invocation.function,
            "owner_declared_review_package_id":m.invocation.component_id});
        if m.invocation.argument_slots.is_empty() {
            ensure!(
                references.is_empty(),
                "literal prepared invocation cannot acquire dependencies"
            );
            spec["input_sha256"] = json!(m.input_sha256);
        } else {
            let slots = references
                .iter()
                .map(|r| r["argument_index"].as_u64().map(|n| n as usize))
                .collect::<Option<BTreeSet<_>>>()
                .context("every owned dependency requires its argument index")?;
            ensure!(
                slots
                    == m.invocation
                        .argument_slots
                        .iter()
                        .copied()
                        .collect::<BTreeSet<_>>()
                    && references.len() == slots.len()
                    && references
                        .iter()
                        .all(|r| r["field"] == "arguments" && r["pointer"].is_string()),
                "owned dependencies must fill exactly the owner-selected slots"
            );
            spec["input_recipe"] = json!({"arguments":m.invocation.arguments,"sources":references});
        }
        specs.push(spec);
    }
    ensure!(
        bindings
            .iter()
            .all(|b| b["source_tool"] != "owned_rust_call"
                || retained
                    .iter()
                    .any(|p| b["arguments"]["binding_id"] == p.invocation.id)),
        "managed invocation plans cannot mix unprepared owned sources"
    );
    Ok(json!(specs))
}
pub fn native_requirements(session: &Value, bindings: &Value) -> Result<Value> {
    let manifests: Vec<Manifest> = serde_json::from_value(session["owned_invocations"].clone())?;
    ensure!(
        !manifests.is_empty() && manifests.len() <= 8,
        "owned graph requires retained invocation metadata"
    );
    requirements(&manifests, bindings)
}
fn validate_edges(retained: &[Prepared], bindings: &Value) -> Result<()> {
    let specs = requirements(
        &retained
            .iter()
            .map(|p| p.manifest.clone())
            .collect::<Vec<_>>(),
        bindings,
    )?;
    let intake = json!({"required_owned_rust_calls":specs,"artifact_bindings":bindings});
    let bound = gents_local_mcp::artifact_bindings::parse(&intake)?;
    for p in retained
        .iter()
        .filter(|p| !p.manifest.invocation.argument_slots.is_empty())
    {
        let consumer = bound
            .iter()
            .find(|b| {
                b.source_tool == "owned_rust_call"
                    && b.arguments["binding_id"] == p.manifest.invocation.id
            })
            .context("prepared consumer binding missing")?;
        for reference in &consumer.argument_sources {
            let producer = bound
                .iter()
                .find(|b| b.path == reference.path)
                .context("prepared producer path missing")?;
            let source = retained
                .iter()
                .find(|r| producer.arguments["binding_id"] == r.manifest.invocation.id)
                .context("dependent producer is not retained in this conversation")?;
            gents_local_mcp::component_api::connections::describe(
                &source.source,
                &source.manifest.invocation.function,
                reference
                    .pointer
                    .as_deref()
                    .context("prepared projection missing")?,
                &p.source,
                &p.manifest.invocation.function,
                reference
                    .argument_index
                    .context("prepared argument index missing")?,
            )?;
        }
    }
    Ok(())
}
pub fn validate_plan_custody(db: &Connection, id: Uuid, bindings: &Value) -> Result<()> {
    let retained = stored(db, id)?;
    if retained
        .iter()
        .any(|p| !p.manifest.invocation.argument_slots.is_empty())
    {
        validate_edges(&retained, bindings)?;
    }
    Ok(())
}
pub fn validate_plan(session: &Value, bindings: &Value) -> Result<()> {
    if session["owned_invocations"].is_null() {
        return Ok(());
    }
    let manifests: Vec<Manifest> = serde_json::from_value(session["owned_invocations"].clone())?;
    ensure!(
        !manifests.is_empty() && manifests.len() <= 8,
        "invalid prepared invocation count"
    );
    requirements(&manifests, bindings)?;
    Ok(())
}
/// Freeze only existing owner input data against the actual accepted CEO actor.
pub fn freeze(
    db: &Connection,
    root: &Path,
    owner: &str,
    id: Uuid,
    session: &Value,
    proposed_bindings: &Value,
    intake: &mut Value,
) -> Result<()> {
    let retained = stored(db, id)?;
    if retained.is_empty() {
        ensure!(
            intake["ceo_owned_invocations"].is_null(),
            "owned source manifests require retained owner inputs"
        );
        return Ok(());
    }
    ensure!(
        session["owned_invocations"] == metadata(&retained),
        "retained owned metadata mismatch"
    );
    for p in &retained {
        ensure!(
            crate::owned_catalog::owns(root, &p.manifest.origin_company, owner)?,
            "source company no longer belongs to this owner"
        );
    }
    validate_edges(&retained, proposed_bindings)?;
    let specs = requirements(
        &retained
            .iter()
            .map(|p| p.manifest.clone())
            .collect::<Vec<_>>(),
        proposed_bindings,
    )?;
    ensure!(
        requirements(
            &retained
                .iter()
                .map(|p| p.manifest.clone())
                .collect::<Vec<_>>(),
            &intake["artifact_bindings"]
        )? == specs,
        "owner-edited bindings cannot change prepared source inputs or CEO-selected actors"
    );
    for key in ["ceo_owned_invocations", "required_owned_rust_calls"] {
        let expected = if key == "ceo_owned_invocations" {
            metadata(&retained)
        } else {
            specs.clone()
        };
        ensure!(
            intake[key].is_null() || intake[key] == expected,
            "supplied owned binding metadata differs from retained owner inputs"
        );
        intake[key] = expected;
    }
    gents_local_mcp::owned_components::specs(intake)?;
    for (field, source) in [
        ("ceo_owned_argument_data", "owned_argument_data"),
        ("ceo_owned_selection", "accepted_owned_selection"),
    ] {
        let expected = &session[source];
        ensure!(
            intake[field].is_null() || intake[field] == *expected,
            "owned selection provenance differs from retained planning data"
        );
        if !expected.is_null() {
            intake[field] = expected.clone();
        }
    }
    for p in &retained {
        ensure!(
            !intake["artifact_bindings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|b| b["path"] == p.manifest.source_path),
            "owned library sources cannot be output paths"
        );
    }
    Ok(())
}
/// Called only after job approval; never replaces differing workspace bytes.
pub fn stage(
    db: &Connection,
    root: &Path,
    company: &str,
    owner: &str,
    run: Uuid,
    intake: &Value,
) -> Result<Value> {
    if intake["ceo_owned_invocations"].is_null() {
        return Ok(Value::Null);
    }
    let session = Uuid::parse_str(
        intake["ceo_planning_id"]
            .as_str()
            .context("owned sources need a retained planning session")?,
    )?;
    let retained = stored(db, session)?;
    ensure!(
        !retained.is_empty()
            && metadata(&retained) == intake["ceo_owned_invocations"]
            && requirements(
                &retained
                    .iter()
                    .map(|p| p.manifest.clone())
                    .collect::<Vec<_>>(),
                &intake["artifact_bindings"]
            )? == intake["required_owned_rust_calls"],
        "frozen owned sources differ from retained custody"
    );
    validate_edges(&retained, &intake["artifact_bindings"])?;
    let mut seen = BTreeSet::new();
    let mut files = vec![];
    for p in retained {
        ensure!(
            crate::owned_catalog::owns(root, &p.manifest.origin_company, owner)?,
            "source company no longer belongs to this owner"
        );
        if seen.insert(p.manifest.source_path.clone()) {
            files.push((json!({"path":p.manifest.source_path,"bytes":p.manifest.bytes,"sha256":p.manifest.source_sha256}),p.source));
        }
    }
    crate::owner_staging::stage(root, company, run, &files)?;
    Ok(
        json!({"planning_id":session,"files":files.iter().map(|(m,_)|m).collect::<Vec<_>>(),"provider_cost_usd":0,"scope":"Owner-retained source staging only; no candidate execution or tool grants"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (std::path::PathBuf, Connection, Uuid, Value, Prepared) {
        let root = std::env::temp_dir().join(format!("owned-input-test-{}", Uuid::new_v4()));
        for company in ["one", "origin"] {
            crate::company::operation(
                &root,
                company,
                "alice",
                &json!({"op":"register","name":"protocol fixture"}),
            )
            .unwrap();
        }
        let root = root.canonicalize().unwrap();
        let db = crate::company::connect(&root, "one", "alice", false).unwrap();
        // Synthetic protocol fixture, never a qualification or real model source.
        let source = "pub fn must_not_run(x:u64)->u64{loop{}} // PRIVATE_SOURCE_BODY".to_owned();
        let d = gents_local_mcp::component_api::describe(&source).unwrap();
        let invocation = Invocation {
            id: "call-one".into(),
            component_id: "a".repeat(64),
            function: "must_not_run".into(),
            arguments: json!(["3"]),
            argument_slots: vec![],
        };
        let manifest = Manifest {
            origin_company: "origin".into(),
            source_path: format!("owned-{}.rs", invocation.component_id),
            source_sha256: d["source_sha256"].as_str().unwrap().into(),
            api_sha256: d["api_sha256"].as_str().unwrap().into(),
            input_sha256: Some(gents_local_mcp::digest(
                invocation.arguments.to_string().as_bytes(),
            )),
            input_template_sha256: None,
            bytes: source.len(),
            invocation,
        };
        let body = json!({"source_inputs":[],"owned_component_catalog":{"components":[{"component_id":manifest.invocation.component_id,"origin_company":manifest.origin_company,"source_sha256":manifest.source_sha256,"api_sha256":manifest.api_sha256}]}});
        (
            root,
            db,
            Uuid::new_v4(),
            body,
            checked(manifest, source).unwrap(),
        )
    }
    fn bindings() -> Value {
        json!([{"path":"result.json","source_tool":"owned_rust_call","source_actor":"Analyst","arguments":{"binding_id":"call-one","arguments":["3"]},"pointer":"/output","encoding":"json","writer":"Writer","argument_sources":[]}])
    }
    fn keep(db: &Connection, id: Uuid, body: &mut Value, p: Prepared) {
        let incoming = json!([p.manifest.invocation]);
        retain(db, id, body, &[p], &incoming).unwrap();
    }
    #[test]
    fn private_custody_is_not_model_source_and_changed_inputs_roll_back_atomically() {
        let (root, mut db, id, mut body, p) = setup();
        keep(&db, id, &mut body, p);
        assert!(!body.to_string().contains("PRIVATE_SOURCE_BODY"));
        assert_eq!(
            body["source_inputs"][0]["arguments"]["arguments"],
            json!(["3"])
        );
        let mut old = stored(&db, id).unwrap().remove(0);
        let mut fresh = checked(old.manifest.clone(), old.source.clone()).unwrap();
        fresh.manifest.invocation.id = "new-call".into();
        old.manifest.invocation.arguments = json!(["4"]);
        old.manifest.input_sha256 = Some(gents_local_mcp::digest(b"[\"4\"]"));
        let incoming = json!([fresh.manifest.invocation, old.manifest.invocation]);
        let tx = db.transaction().unwrap();
        assert!(retain(&tx, id, &mut body.clone(), &[fresh, old], &incoming).is_err());
        drop(tx);
        assert_eq!(stored(&db, id).unwrap().len(), 1);
        let mut bad = body["owned_invocations"][0].clone();
        bad["invocation"]["arguments"] = json!([3]);
        bad["input_sha256"] = json!(gents_local_mcp::digest(b"[3]"));
        let m: Manifest = serde_json::from_value(bad).unwrap();
        assert!(checked(m, stored(&db, id).unwrap().remove(0).source).is_err());
        for injected in ["actor", "source", "source_path"] {
            let mut raw = body["owned_invocations"][0]["invocation"].clone();
            raw[injected] = json!("forged");
            assert!(parse(&json!([raw])).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn freeze_binds_the_actual_ceo_source_actor_and_never_accepts_client_authority() {
        let (root, db, id, mut body, p) = setup();
        keep(&db, id, &mut body, p);
        let bindings = bindings();
        let mut intake = json!({"artifact_bindings":bindings});
        freeze(&db, &root, "alice", id, &body, &bindings, &mut intake).unwrap();
        assert_eq!(intake["required_owned_rust_calls"][0]["actor"], "Analyst");
        assert_ne!(
            intake["required_owned_rust_calls"][0]["actor"],
            bindings[0]["writer"]
        );
        for edit in ["source_actor", "arguments", "path"] {
            let mut changed = json!({"artifact_bindings":bindings});
            changed["artifact_bindings"][0][edit] = match edit {
                "arguments" => json!({"binding_id":"call-one","arguments":["4"]}),
                "path" => body["owned_invocations"][0]["source_path"].clone(),
                _ => json!("Writer"),
            };
            assert!(
                freeze(&db, &root, "alice", id, &body, &bindings, &mut changed).is_err(),
                "{edit}"
            );
        }
        let mut changed = intake.clone();
        changed["required_owned_rust_calls"][0]["actor"] = json!("Writer");
        assert!(freeze(&db, &root, "alice", id, &body, &bindings, &mut changed).is_err());
        let mut changed = body.clone();
        changed["owned_invocations"][0]["api_sha256"] = json!("b".repeat(64));
        assert!(freeze(&db, &root, "alice", id, &changed, &bindings, &mut intake).is_err());
        let mut unused = json!({"artifact_bindings":[]});
        assert!(freeze(&db, &root, "alice", id, &body, &json!([]), &mut unused).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn approved_admission_stages_exact_bytes_idempotently_and_revocation_conflicts_fail_closed() {
        let (root, db, id, mut body, p) = setup();
        let source = p.source.clone();
        let path = p.manifest.source_path.clone();
        keep(&db, id, &mut body, p);
        let mut intake = json!({"idea":"Synthetic owned input custody","ceo_planning_id":id,"artifact_bindings":bindings()});
        freeze(&db, &root, "alice", id, &body, &bindings(), &mut intake).unwrap();
        let run = Uuid::new_v4();
        db.execute(
            "INSERT INTO jobs VALUES(?1,?2,'fixture',NULL,'draft')",
            params![run.to_string(), intake.to_string()],
        )
        .unwrap();
        let file = root.join(format!("data/companies/one/runs/{run}/workspace/{path}"));
        assert!(
            crate::company::admit(&root, "one", "alice", run, "Synthetic owned input custody")
                .is_err()
        );
        assert!(!file.exists());
        db.execute("UPDATE jobs SET state='approved'", []).unwrap();
        crate::company::admit(&root, "one", "alice", run, "Synthetic owned input custody").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), source);
        stage(&db, &root, "one", "alice", run, &intake).unwrap();
        std::fs::write(&file, "conflict").unwrap();
        assert!(stage(&db, &root, "one", "alice", run, &intake).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "conflict");
        let origin = crate::company::connect(&root, "origin", "alice", false).unwrap();
        origin
            .execute("UPDATE company SET owner='bob'", [])
            .unwrap();
        let another = Uuid::new_v4();
        assert!(stage(&db, &root, "one", "alice", another, &intake).is_err());
        assert!(!root
            .join(format!("data/companies/one/runs/{another}/workspace"))
            .exists());
        assert!(freeze(&db, &root, "alice", id, &body, &bindings(), &mut intake).is_err());
        let audit: String = db
            .query_row(
                "SELECT body FROM audit WHERE kind='owner_component_sources_staged'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!audit.contains("PRIVATE_SOURCE_BODY"));
        db.execute("UPDATE ceo_owned_inputs SET content='corrupt'", [])
            .unwrap();
        assert!(stored(&db, id).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn owner_templates_keep_private_bytes_and_freeze_only_ceo_selected_typed_edges() {
        let (root, db, id, mut body, first) = setup();
        let source =
            "pub fn consume(x:u64,tag:u64)->u64{let _=tag;loop{}} // PRIVATE_CONSUMER_BODY"
                .to_owned();
        let d = gents_local_mcp::component_api::describe(&source).unwrap();
        let invocation = Invocation {
            id: "call-two".into(),
            component_id: "b".repeat(64),
            function: "consume".into(),
            arguments: json!([null, "4"]),
            argument_slots: vec![0],
        };
        let second = checked(
            Manifest {
                source_path: format!("owned-{}.rs", invocation.component_id),
                source_sha256: d["source_sha256"].as_str().unwrap().into(),
                api_sha256: d["api_sha256"].as_str().unwrap().into(),
                origin_company: "origin".into(),
                input_sha256: None,
                input_template_sha256: Some(gents_local_mcp::digest(
                    invocation.arguments.to_string().as_bytes(),
                )),
                bytes: source.len(),
                invocation,
            },
            source,
        )
        .unwrap();
        body["owned_component_catalog"]["components"].as_array_mut().unwrap().push(json!({"component_id":second.manifest.invocation.component_id,"origin_company":"origin","source_sha256":second.manifest.source_sha256,"api_sha256":second.manifest.api_sha256}));
        let incoming = json!([first.manifest.invocation, second.manifest.invocation]);
        retain(&db, id, &mut body, &[first, second], &incoming).unwrap();
        assert!(!body.to_string().contains("PRIVATE_SOURCE_BODY"));
        assert!(!body.to_string().contains("PRIVATE_CONSUMER_BODY"));
        assert_eq!(body["source_inputs"][1]["argument_slots"], json!([0]));
        assert!(body["owned_invocations"][1].get("input_sha256").is_none());
        let raw = json!({"status":"ready","question":"","capability_mode":"native_prepared_graph","definition_of_done":["Native outputs saved and read back"],"assignments":[{"actor":"Analyst","tools":[],"task":"Compute the prepared chain","acceptance":["Native results available"]},{"actor":"Writer","tools":[],"task":"Save both native outputs","acceptance":["Exact readbacks"]}],"native_contracts":{"sources":[{"id":"call-one","input":"call-one","actor":"Analyst","argument_sources":[]},{"id":"call-two","input":"call-two","actor":"Analyst","argument_sources":[{"field":"arguments","source":"call-one","pointer":"/output","argument_index":0,"value_pointer":""}]}],"artifacts":[{"path":"first.json","source":"call-one","writer":"Writer","pointer":"/output","encoding":"json"},{"path":"second.json","source":"call-two","writer":"Writer","pointer":"/output","encoding":"json"}]}});
        let (proposal, _) =
            crate::ceo_contracts::compile_graph_capabilities(&raw, &body["source_inputs"]).unwrap();
        let bindings = crate::ceo_contracts::compile_for_session(&proposal, &body).unwrap();
        validate_plan_custody(&db, id, &bindings).unwrap();
        let mut intake = json!({"artifact_bindings":bindings,"ceo_planning_id":id});
        freeze(&db, &root, "alice", id, &body, &bindings, &mut intake).unwrap();
        assert_eq!(intake["required_owned_rust_calls"][1]["actor"], "Analyst");
        assert!(intake["required_owned_rust_calls"][1]
            .get("input_sha256")
            .is_none());
        assert_eq!(
            intake["required_owned_rust_calls"][1]["input_recipe"]["arguments"],
            json!([null, "4"])
        );
        assert_eq!(
            intake["required_owned_rust_calls"][1]["input_recipe"]["sources"][0]["path"],
            "first.json"
        );
        assert_eq!(
            intake["required_owned_rust_calls"][1]["input_recipe"]["sources"][0]["pointer"],
            ""
        );
        // A model/owner edit cannot replace fixed literal data or select an invalid typed projection.
        let mut wrong = bindings.clone();
        wrong[1]["arguments"]["arguments"][1] = json!("5");
        assert!(validate_plan(&body, &wrong).is_err());
        let mut wrong = bindings.clone();
        wrong[1]["argument_sources"][0]["pointer"] = json!("/some");
        assert!(validate_plan_custody(&db, id, &wrong).is_err());
        let mut wrong = intake.clone();
        wrong["required_owned_rust_calls"][1]["actor"] = json!("Writer");
        assert!(freeze(&db, &root, "alice", id, &body, &bindings, &mut wrong).is_err());

        let mut final_only = raw.clone();
        final_only["native_contracts"]["artifacts"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        final_only["planning_constraints"] = json!({"separate_source_writers":true});
        let (compiled, _) =
            crate::ceo_contracts::compile_graph_capabilities(&final_only, &body["source_inputs"])
                .unwrap();
        let connected = crate::ceo_contracts::compile_for_session(&compiled, &body).unwrap();
        assert_eq!(connected.as_array().unwrap().len(), 2);
        assert_eq!(connected[0]["receipt_only"], true);
        assert_eq!(connected[0]["writer"], "Analyst");
        assert_eq!(connected[1]["path"], "second.json");
        let mut connected_intake = json!({"artifact_bindings":connected,"ceo_plan":compiled});
        freeze(
            &db,
            &root,
            "alice",
            id,
            &body,
            &connected,
            &mut connected_intake,
        )
        .unwrap();
        assert_eq!(
            crate::artifact_requirements::named_paths(&connected_intake, "Writer", true).unwrap(),
            Some(vec!["second.json".into()])
        );
        let mut forged = final_only.clone();
        forged["native_contracts"]["artifacts"][0]["receipt_only"] = json!(true);
        assert!(
            crate::ceo_contracts::compile_for_session(&forged, &body).is_err(),
            "a model cannot waive a requested file's publication gate"
        );
        let mut native_only = raw.clone();
        native_only["native_contracts"]["artifacts"] = json!([]);
        native_only["assignments"].as_array_mut().unwrap().pop();
        let (compiled, _) =
            crate::ceo_contracts::compile_graph_capabilities(&native_only, &body["source_inputs"])
                .unwrap();
        assert_eq!(
            compiled["assignments"][0]["tools"],
            json!(["owned_rust_call"])
        );
        let receipts = crate::ceo_contracts::compile_for_session(&compiled, &body).unwrap();
        assert!(receipts
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["receipt_only"] == true));

        let run = Uuid::new_v4();
        stage(&db, &root, "one", "alice", run, &intake).unwrap();
        for p in stored(&db, id).unwrap() {
            assert_eq!(
                std::fs::read_to_string(root.join(format!(
                    "data/companies/one/runs/{run}/workspace/{}",
                    p.manifest.source_path
                )))
                .unwrap(),
                p.source
            );
        }
        assert_eq!(
            db.query_row::<i64, _, _>(
                "SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
            0
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_templates_validate_all_other_literals_and_never_invent_an_input_hash() {
        let (root, _, _, _, p) = setup();
        let mut m = p.manifest.clone();
        m.invocation.argument_slots = vec![0];
        m.invocation.arguments = json!([null]);
        m.input_template_sha256 = Some(gents_local_mcp::digest(b"[null]"));
        m.input_sha256 = None;
        assert!(checked(m.clone(), p.source.clone()).is_ok());
        let mut both = m.clone();
        both.input_sha256 = Some(gents_local_mcp::digest(b"[null]"));
        assert!(checked(both, p.source.clone()).is_err());
        for slots in [vec![0, 0], vec![1]] {
            let mut bad = m.clone();
            bad.invocation.argument_slots = slots;
            assert!(checked(bad, p.source.clone()).is_err());
        }
        let raw = json!({"id":"slot","component_id":"a".repeat(64),"function":"f","arguments":["7"],"argument_slots":[0]});
        assert!(parse(&json!([raw])).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

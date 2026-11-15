//! Versioned source references. A valid citation establishes availability and
//! integrity, never the truth of a model's interpretation of that source.
use super::{program, Contract, Message, Turn};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub fn references(contract: &Contract, turn: &Turn, history: &[Message]) -> Result<Vec<String>> {
    let mut refs = vec![format!(
        "input:{}",
        crate::company_factory::digest(&contract.input)?
    )];
    for ordinal in &turn.sources {
        ensure!(*ordinal < turn.ordinal, "citation references a future turn");
        let message = history.get(*ordinal).context("citation source missing")?;
        refs.push(format!(
            "message:{ordinal}:{}",
            crate::company_factory::digest(message)?
        ));
    }
    Ok(refs)
}

pub fn validate(
    contract: &Contract,
    turn: &Turn,
    content: &Value,
    history: &[Message],
) -> Result<()> {
    program::validate_content(turn, content)?;
    if contract.version == 2 && turn.purpose != program::Purpose::Route {
        let allowed = references(contract, turn, history)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let evidence = content["evidence"]
            .as_array()
            .context("evidence references missing")?;
        let mut seen = BTreeSet::new();
        for reference in evidence {
            let id = reference.as_str().context("citation ID must be text")?;
            ensure!(
                allowed.contains(id) && seen.insert(id),
                "citation is unavailable, altered or duplicated"
            );
        }
    }
    Ok(())
}

pub fn schema(packet: &Value) -> Result<Value> {
    ensure!(
        packet["contract_version"] == 2,
        "typed citations need version 2"
    );
    let refs = packet["evidence_references"]
        .as_array()
        .context("citation language missing")?;
    ensure!(
        !refs.is_empty() && refs.len() <= 21,
        "invalid citation budget"
    );
    let mut ids = BTreeSet::new();
    for id in refs {
        let id = id.as_str().context("citation ID must be text")?;
        ensure!(
            id.len() <= 100 && ids.insert(id),
            "invalid citation identity"
        );
    }
    let mut schema = program::response_schema(false);
    schema["properties"]["evidence"]["items"] = json!({"type":"string","enum":refs});
    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::company_factory::contracts::Coordination;

    #[test]
    fn version_two_rejects_fabricated_citations_and_preserves_legacy_history() {
        let mut contract = super::super::tests::contract(Coordination::Ordered);
        let turn = program::next(&contract, &[]).unwrap().unwrap();
        let invented = json!({"deliverable":"Candidate", "evidence":["Worker contribution that never happened"],"unknowns":[]});
        assert!(validate(&contract, &turn, &invented, &[]).is_ok());
        contract.version = 2;
        assert!(validate(&contract, &turn, &invented, &[]).is_err());
        let refs = references(&contract, &turn, &[]).unwrap();
        assert_eq!(refs.len(), 1);
        let valid = json!({"deliverable":"Candidate","evidence":refs,"unknowns":[]});
        validate(&contract, &turn, &valid, &[]).unwrap();
        let message = Message {
            turn,
            content: valid,
            lineage: BTreeSet::new(),
        };
        let next = program::next(&contract, std::slice::from_ref(&message))
            .unwrap()
            .unwrap();
        let refs = references(&contract, &next, std::slice::from_ref(&message)).unwrap();
        assert_eq!(refs.len(), 2);
        let mut reply = json!({"deliverable":"Candidate","evidence":[refs[1]],"unknowns":[]});
        validate(&contract, &next, &reply, std::slice::from_ref(&message)).unwrap();
        reply["evidence"] = json!([format!("message:9:{}", "a".repeat(64))]);
        assert!(validate(&contract, &next, &reply, std::slice::from_ref(&message)).is_err());
        reply["evidence"] = json!([refs[0], refs[0]]);
        assert!(validate(&contract, &next, &reply, std::slice::from_ref(&message)).is_err());
        let schema = schema(&json!({"contract_version":2,"evidence_references":refs})).unwrap();
        assert_eq!(
            schema["properties"]["evidence"]["items"]["enum"],
            json!(refs)
        );
    }
}

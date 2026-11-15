//! Structural diagnostics only. No tool choice, values, effects or authority.
use anyhow::{ensure, Result};
use std::collections::{BTreeMap, BTreeSet};

pub struct Node<'a> {
    pub id: &'a str,
    pub inputs: Vec<(&'a str, &'a str)>, // argument field, producer id
}

pub fn validate(nodes: &[Node<'_>]) -> Result<()> {
    ensure!(nodes.len() <= 8, "native source graph exceeds eight nodes");
    let mut pending = BTreeMap::new();
    for node in nodes {
        ensure!(
            pending.insert(node.id, node).is_none(),
            "duplicate native source id {}",
            node.id
        );
    }
    for node in nodes {
        for (field, producer) in &node.inputs {
            ensure!(pending.contains_key(producer), "source {} field {} references undeclared producer {}; declare dependencies using existing exact source ids", node.id, field, producer);
            ensure!(*producer != node.id, "source {} field {} references itself; a transformation requires an earlier producer result, never its own output", node.id, field);
        }
    }
    let mut resolved = BTreeSet::new();
    while !pending.is_empty() {
        let ready = pending
            .values()
            .filter(|node| node.inputs.iter().all(|(_, id)| resolved.contains(id)))
            .map(|node| node.id)
            .collect::<Vec<_>>();
        ensure!(
            !ready.is_empty(),
            "native source dependency cycle among {:?}; producer references must form an acyclic graph",
            pending.keys().collect::<Vec<_>>()
        );
        for id in ready {
            pending.remove(id);
            resolved.insert(id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node<'a>(id: &'a str, inputs: &[&'a str]) -> Node<'a> {
        Node {
            id,
            inputs: inputs.iter().map(|id| ("rows", *id)).collect(),
        }
    }
    #[test]
    fn shuffled_fan_in_accepts_structure_without_computing_or_reordering_it() {
        let nodes = [
            node("merge", &["left", "right"]),
            node("right", &[]),
            node("left", &[]),
        ];
        validate(&nodes).unwrap();
        assert_eq!(nodes[0].id, "merge");
    }
    #[test]
    fn missing_duplicate_self_and_indirect_cycle_have_distinct_diagnostics() {
        for (nodes, message) in [
            (vec![node("a", &["missing"])], "undeclared producer missing"),
            (
                vec![node("a", &[]), node("a", &[])],
                "duplicate native source id a",
            ),
            (vec![node("a", &["a"])], "references itself"),
            (
                vec![node("root", &[]), node("a", &["b"]), node("b", &["a"])],
                "dependency cycle",
            ),
        ] {
            assert!(validate(&nodes).unwrap_err().to_string().contains(message));
        }
        let nodes = (0..9).map(|_| node("a", &[])).collect::<Vec<_>>();
        assert!(validate(&nodes)
            .unwrap_err()
            .to_string()
            .contains("exceeds eight"));
    }
}

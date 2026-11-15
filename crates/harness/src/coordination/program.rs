//! Bounded communication programs. Models choose delegated work, never actors,
//! permissions, budgets, execution effects or completion authority.
use crate::company_factory::{contracts::Coordination, digest};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub actor: String,
    pub assignment: String,
    pub model_sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub version: u32,
    pub coordination: Coordination,
    pub objective: String,
    pub definition_of_done: Vec<String>,
    pub input: Value,
    pub roles: Vec<Role>,
    pub max_delegations: usize,
    pub peer_rounds: usize,
}
fn text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}
pub fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl Contract {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.version, 1 | 2) && crate::company_factory::objective(&self.objective),
            "invalid coordination contract"
        );
        ensure!(
            !self.roles.is_empty() && self.roles.len() <= 4,
            "coordination requires 1..4 roles"
        );
        ensure!(
            !self.definition_of_done.is_empty()
                && self.definition_of_done.len() <= 8
                && self.definition_of_done.iter().all(|v| text(v, 1000)),
            "explicit bounded definition of done required"
        );
        ensure!(
            (1..=4).contains(&self.max_delegations) && (1..=2).contains(&self.peer_rounds),
            "invalid coordination budget"
        );
        let mut actors = BTreeSet::new();
        for role in &self.roles {
            ensure!(
                text(&role.actor, 200)
                    && text(&role.assignment, 2000)
                    && hash(&role.model_sha256)
                    && actors.insert(&role.actor),
                "invalid or duplicate coordination role"
            );
        }
        ensure!(
            self.coordination == Coordination::Ordered || self.roles.len() >= 2,
            "delegation and peer review require distinct actors"
        );
        let encoded = serde_json::to_vec(self)?;
        ensure!(encoded.len() <= 16_000, "coordination input exceeds budget");
        crate::security::require_persistable(&String::from_utf8(encoded)?)?;
        Ok(())
    }
    pub fn sha256(&self) -> Result<String> {
        self.validate()?;
        digest(self)
    }
    pub fn role(&self, actor: &str) -> Result<&Role> {
        self.roles
            .iter()
            .find(|r| r.actor == actor)
            .context("coordination actor not in frozen roster")
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Contribute,
    Route,
    Delegate,
    Critique,
    Revise,
    Synthesize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    pub ordinal: usize,
    pub actor: String,
    pub purpose: Purpose,
    pub assignment: String,
    /// Exact prior message ordinals. The host computes transitive lineage.
    pub sources: BTreeSet<usize>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub turn: Turn,
    pub content: Value,
    pub lineage: BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Route {
    action: String,
    recipient: String,
    assignment: String,
}

/// Returns the next actual turn, or None after the bounded program finishes.
/// Every retained turn is replay-validated against previous actual messages.
pub fn next(contract: &Contract, history: &[Message]) -> Result<Option<Turn>> {
    contract.validate()?;
    ensure!(history.len() <= 20, "coordination history exceeds budget");
    for index in 0..history.len() {
        let expected =
            advance(contract, &history[..index])?.context("message after program completion")?;
        ensure!(
            history[index].turn == expected,
            "retained turn differs from frozen coordination program"
        );
        super::evidence::validate(
            contract,
            &expected,
            &history[index].content,
            &history[..index],
        )?;
        ensure!(
            history[index].lineage == lineage(&expected, &history[..index])?,
            "retained message lineage changed"
        );
    }
    advance(contract, history)
}

fn advance(contract: &Contract, history: &[Message]) -> Result<Option<Turn>> {
    let ordinal = history.len();
    let roles = &contract.roles;
    let make = |actor: &str, purpose, assignment: String, sources: BTreeSet<usize>| {
        Some(Turn {
            ordinal,
            actor: actor.into(),
            purpose,
            assignment,
            sources,
        })
    };
    match contract.coordination {
        Coordination::Ordered => Ok(roles.get(ordinal).and_then(|r| {
            make(
                &r.actor,
                Purpose::Contribute,
                r.assignment.clone(),
                (0..ordinal).collect(),
            )
        })),
        Coordination::AdaptiveDelegation => {
            let hub = &roles[0];
            let delegates = history
                .iter()
                .filter(|m| m.turn.purpose == Purpose::Delegate)
                .count();
            let Some(last) = history.last() else {
                return Ok(make(
                    &hub.actor,
                    Purpose::Route,
                    hub.assignment.clone(),
                    BTreeSet::new(),
                ));
            };
            match last.turn.purpose {
                Purpose::Route => {
                    let route: Route = serde_json::from_value(last.content.clone())?;
                    if route.action == "synthesize" {
                        ensure!(delegates > 0, "hub cannot skip all delegated work");
                        return Ok(make(
                            &hub.actor,
                            Purpose::Synthesize,
                            hub.assignment.clone(),
                            (0..ordinal).collect(),
                        ));
                    }
                    ensure!(
                        delegates < contract.max_delegations,
                        "hub exceeded frozen delegation budget"
                    );
                    ensure!(
                        route.recipient != hub.actor,
                        "hub may delegate only to a distinct frozen worker"
                    );
                    contract.role(&route.recipient)?;
                    Ok(make(
                        &route.recipient,
                        Purpose::Delegate,
                        route.assignment,
                        BTreeSet::from([ordinal - 1]),
                    ))
                }
                Purpose::Delegate => Ok(make(
                    &hub.actor,
                    if delegates == contract.max_delegations {
                        Purpose::Synthesize
                    } else {
                        Purpose::Route
                    },
                    hub.assignment.clone(),
                    (0..ordinal).collect(),
                )),
                Purpose::Synthesize => Ok(None),
                _ => anyhow::bail!("invalid hub program state"),
            }
        }
        Coordination::PeerReview => {
            let author = &roles[0];
            if ordinal == 0 {
                return Ok(make(
                    &author.actor,
                    Purpose::Contribute,
                    author.assignment.clone(),
                    BTreeSet::new(),
                ));
            }
            let width = roles.len();
            let round = (ordinal - 1) / width;
            if round >= contract.peer_rounds {
                return Ok(None);
            }
            let slot = (ordinal - 1) % width;
            if slot + 1 < width {
                let peer = &roles[slot + 1];
                Ok(make(
                    &peer.actor,
                    Purpose::Critique,
                    peer.assignment.clone(),
                    (0..ordinal).collect(),
                ))
            } else {
                Ok(make(
                    &author.actor,
                    Purpose::Revise,
                    author.assignment.clone(),
                    (0..ordinal).collect(),
                ))
            }
        }
    }
}

pub fn lineage(turn: &Turn, history: &[Message]) -> Result<BTreeSet<String>> {
    let mut lineage = BTreeSet::new();
    for ordinal in &turn.sources {
        ensure!(
            *ordinal < turn.ordinal,
            "message may not read future sources"
        );
        let source = history
            .get(*ordinal)
            .context("missing coordination source")?;
        lineage.insert(source.turn.actor.clone());
        lineage.extend(source.lineage.iter().cloned());
    }
    Ok(lineage)
}
pub fn validate_content(turn: &Turn, content: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(content)?.len() <= 10_000,
        "coordination reply exceeds budget"
    );
    crate::security::require_persistable(&content.to_string())?;
    if turn.purpose == Purpose::Route {
        let route: Route = serde_json::from_value(content.clone())?;
        ensure!(
            matches!(route.action.as_str(), "delegate" | "synthesize"),
            "unknown route action"
        );
        ensure!(
            (route.action == "delegate"
                && text(&route.recipient, 200)
                && text(&route.assignment, 2000))
                || (route.action == "synthesize"
                    && route.recipient.is_empty()
                    && route.assignment.is_empty()),
            "invalid route fields"
        );
    } else {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Contribution {
            deliverable: String,
            evidence: Vec<String>,
            unknowns: Vec<String>,
        }
        let value: Contribution = serde_json::from_value(content.clone())?;
        ensure!(
            !value.deliverable.trim().is_empty()
                && value.deliverable.len() <= 6000
                && value.evidence.len() <= 8
                && value.unknowns.len() <= 8
                && value
                    .evidence
                    .iter()
                    .chain(&value.unknowns)
                    .all(|s| text(s, 500)),
            "invalid bounded contribution"
        );
    }
    Ok(())
}
pub fn response_schema(route: bool) -> Value {
    if route {
        let mut delegate = json!({"type":"object","additionalProperties":false,"required":["action","recipient","assignment"],"properties":{"action":{"type":"string","const":"delegate"},"recipient":{"type":"string","minLength":1,"maxLength":200},"assignment":{"type":"string","minLength":1,"maxLength":2000}}});
        let mut synthesize = delegate.clone();
        synthesize["properties"] = json!({"action":{"type":"string","const":"synthesize"},"recipient":{"type":"string","const":""},"assignment":{"type":"string","const":""}});
        delegate["properties"]["action"] = json!({"type":"string","const":"delegate"});
        json!({"oneOf":[delegate,synthesize]})
    } else {
        json!({"type":"object","additionalProperties":false,"required":["deliverable","evidence","unknowns"],"properties":{"deliverable":{"type":"string","minLength":1,"maxLength":6000},"evidence":{"type":"array","maxItems":8,"items":{"type":"string","maxLength":500}},"unknowns":{"type":"array","maxItems":8,"items":{"type":"string","maxLength":500}}}})
    }
}

/// Derive the complete routing language from the frozen runtime roster and
/// actual turn counter. No task answer or next recipient is chosen by the host.
pub fn bound_route_schema(packet: &Value) -> Result<Value> {
    let roles = packet["roles"]
        .as_array()
        .context("routing roster missing")?;
    ensure!((2..=4).contains(&roles.len()), "invalid routing roster");
    let roster = roles
        .iter()
        .map(|role| {
            role["actor"]
                .as_str()
                .filter(|v| text(v, 200))
                .map(str::to_owned)
                .context("routing actor missing")
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        roster.iter().collect::<BTreeSet<_>>().len() == roster.len(),
        "duplicate routing actor"
    );
    let count = packet["delegated_turns"]
        .as_u64()
        .context("routing counter missing")?;
    let maximum = packet["max_delegations"]
        .as_u64()
        .filter(|v| (1..=4).contains(v))
        .context("routing budget missing")?;
    ensure!(count < maximum, "routing turn exceeds frozen budget");
    let mut schema = response_schema(true);
    schema["oneOf"][0]["properties"]["recipient"] = json!({"type":"string","enum":roster[1..]});
    if count == 0 {
        schema["oneOf"].as_array_mut().unwrap().truncate(1);
    }
    Ok(schema)
}

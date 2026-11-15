//! Deterministic access policy. Model decisions may attenuate an owner ceiling,
//! never create authority. The authenticated runtime supplies every actor ID.
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use tokio::sync::watch;

type EpochMap = BTreeMap<(String, uuid::Uuid), watch::Sender<u64>>;
fn epochs() -> &'static Mutex<EpochMap> {
    static EPOCHS: OnceLock<Mutex<EpochMap>> = OnceLock::new();
    EPOCHS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Event-driven permission cancellation, not a model/time polling loop.
pub fn subscribe(company: &str, run: uuid::Uuid, revision: u64) -> Result<watch::Receiver<u64>> {
    let mut all = epochs()
        .lock()
        .map_err(|_| anyhow::anyhow!("policy notification state unavailable"))?;
    all.retain(|_, sender| sender.receiver_count() > 0);
    let sender = all
        .entry((company.into(), run))
        .or_insert_with(|| watch::channel(revision).0);
    sender.send_if_modified(|current| {
        if revision > *current {
            *current = revision;
            true
        } else {
            false
        }
    });
    Ok(sender.subscribe())
}
pub fn publish(company: &str, run: uuid::Uuid, revision: u64) -> Result<()> {
    let all = epochs()
        .lock()
        .map_err(|_| anyhow::anyhow!("policy notification state unavailable"))?;
    if let Some(sender) = all.get(&(company.into(), run)) {
        sender.send_if_modified(|current| {
            if revision > *current {
                *current = revision;
                true
            } else {
                false
            }
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub tools: BTreeSet<String>,
    pub skills: BTreeSet<String>,
    pub memory_from: BTreeSet<String>,
    /// Owner-selected deterministic execution of exact frozen native calls.
    /// Empty preserves ordinary model review and historical policy encodings.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub contract_execution_phases: BTreeSet<String>,
}

impl Access {
    fn within(&self, ceiling: &Self) -> bool {
        self.tools.is_subset(&ceiling.tools)
            && self.skills.is_subset(&ceiling.skills)
            && self.memory_from.is_subset(&ceiling.memory_from)
            && self
                .contract_execution_phases
                .is_subset(&ceiling.contract_execution_phases)
    }

    pub fn permits(&self, action: &str, arguments: &Value) -> bool {
        // A broad action grant includes its operations; an operation-specific
        // grant never includes another operation or a missing operation.
        self.tools.contains(action)
            || arguments["op"]
                .as_str()
                .is_some_and(|op| self.tools.contains(&format!("{action}:{op}")))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub revision: u64,
    pub master: String,
    pub ceilings: BTreeMap<String, Access>,
    pub grants: BTreeMap<String, Access>,
}

impl Policy {
    pub fn validate(&self) -> Result<()> {
        if self.revision == 0
            || self.revision > i64::MAX as u64
            || self.ceilings.is_empty()
            || self.ceilings.len() > 64
            || !self.ceilings.contains_key(&self.master)
        {
            bail!("invalid governance revision, master or agent count");
        }
        let tools: BTreeMap<String, Value> =
            serde_json::from_str(include_str!("../../config/company-tools.json"))?;
        let skills: Value = serde_json::from_str(include_str!(
            "../../docs/companies/we-need-getns-llc/shared-skills/catalog.json"
        ))?;
        for (agent, access) in &self.ceilings {
            if access
                .contract_execution_phases
                .iter()
                .any(|phase| phase != "worker")
            {
                bail!("approved contract execution currently supports worker phase only");
            }
            if agent.trim().is_empty()
                || agent.len() > 200
                || agent.chars().any(char::is_control)
                || access.tools.len() > 100
                || access.skills.len() > 100
                || access.memory_from.len() > 65
            {
                bail!("invalid agent access ceiling");
            }
            for tool in &access.tools {
                let action = tool.split(':').next().unwrap_or_default();
                if !tools.contains_key(action)
                    || tool.len() > 120
                    || tool
                        .bytes()
                        .any(|b| !(b.is_ascii_lowercase() || b == b'_' || b == b':'))
                    || tool.matches(':').count() > 1
                    || tool.ends_with(':')
                {
                    bail!("unknown or invalid capability in ceiling");
                }
            }
            for skill in &access.skills {
                if !skills["skills"]
                    .as_array()
                    .context("skill catalog missing")?
                    .iter()
                    .any(|entry| {
                        entry["id"].as_str() == Some(skill)
                            && matches!(
                                entry["status"].as_str(),
                                Some("approved-demo" | "approved-local")
                            )
                    })
                {
                    bail!("unknown approved skill in ceiling");
                }
            }
            if access
                .memory_from
                .iter()
                .any(|sender| sender != "company" && !self.ceilings.contains_key(sender))
            {
                bail!("unknown memory sender in ceiling");
            }
        }
        for (agent, grant) in &self.grants {
            if !self
                .ceilings
                .get(agent)
                .is_some_and(|ceiling| grant.within(ceiling))
            {
                bail!("grant exceeds owner ceiling");
            }
        }
        Ok(())
    }

    pub fn authorize(&self, actor: &str, action: &str, args: &Value) -> Result<()> {
        if !self
            .grants
            .get(actor)
            .is_some_and(|grant| grant.permits(action, args))
        {
            bail!(
                "governance denied capability for runtime actor at policy revision {}",
                self.revision
            );
        }
        if action == "skill_read"
            && !args["id"]
                .as_str()
                .is_some_and(|id| self.grants[actor].skills.contains(id))
        {
            bail!("governance denied unassigned skill");
        }
        Ok(())
    }

    pub fn delegate(
        &mut self,
        actor: &str,
        recipient: &str,
        mut grant: Access,
        revision: u64,
    ) -> Result<()> {
        self.authorize(actor, "access_delegate", &Value::Null)?;
        let owner_mode = self
            .grants
            .get(recipient)
            .map(|g| g.contract_execution_phases.clone())
            .unwrap_or_default();
        if !grant.contract_execution_phases.is_empty()
            && grant.contract_execution_phases != owner_mode
        {
            bail!("delegation cannot select an owner authorization mode");
        }
        // Delegation changes capability subsets, never the owner's constraint
        // mode. Missing fields cannot silently disable an active owner mode.
        grant.contract_execution_phases = owner_mode;
        if actor != self.master
            || revision != self.revision
            || !self
                .ceilings
                .get(recipient)
                .is_some_and(|ceiling| grant.within(ceiling))
        {
            bail!(
                "delegation requires master identity, current revision and owner-approved ceiling"
            );
        }
        self.revision = self
            .revision
            .checked_add(1)
            .context("policy revision exhausted")?;
        self.grants.insert(recipient.to_owned(), grant);
        self.validate()
    }

    pub fn filter(&self, actor: &str, skills: bool, values: &mut Vec<Value>) {
        values.retain(|value| {
            self.grants.get(actor).is_some_and(|grant| {
                let id = value["id"].as_str().unwrap_or_default();
                if skills {
                    grant.skills.contains(id)
                } else {
                    grant
                        .tools
                        .iter()
                        .any(|tool| tool.split(':').next() == Some(id))
                }
            })
        });
        if !skills {
            for value in values {
                value["granted_operations"] = serde_json::json!(self
                    .grants
                    .get(actor)
                    .map(|g| g
                        .tools
                        .iter()
                        .filter(|t| t.split(':').next() == value["id"].as_str())
                        .collect::<Vec<_>>())
                    .unwrap_or_default());
            }
        }
    }
}

pub fn load(db: &Connection, run: &str) -> Result<Option<Policy>> {
    let body: Option<String> = db
        .query_row(
            "SELECT manifest FROM run_governance WHERE run_id=?1",
            [run],
            |row| row.get(0),
        )
        .optional()?;
    body.map(|body| {
        let policy: Policy = serde_json::from_str(&body)?;
        policy.validate()?;
        Ok(policy)
    })
    .transpose()
}

pub fn save(db: &Connection, run: &str, policy: &Policy, expected: u64) -> Result<()> {
    policy.validate()?;
    if expected == 0 {
        if policy.revision != 1 {
            bail!("initial policy revision must be one");
        }
        db.execute(
            "INSERT INTO run_governance VALUES (?1,?2,?3)",
            params![run, 1, serde_json::to_string(policy)?],
        )?;
    } else {
        if expected >= i64::MAX as u64 || policy.revision != expected + 1 {
            bail!("policy revision must increment once");
        }
        if db.execute(
            "UPDATE run_governance SET revision=?2,manifest=?3 WHERE run_id=?1 AND revision=?4",
            params![
                run,
                policy.revision as i64,
                serde_json::to_string(policy)?,
                expected as i64
            ],
        )? != 1
        {
            bail!("stale governance update rejected");
        }
    }
    Ok(())
}

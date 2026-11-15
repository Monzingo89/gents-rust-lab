//! Canonical append-only records with current-policy, run and sender checks.
//! Search is literal keyword retrieval, not a claim of vector/semantic RAG.
use crate::{
    contracts::{EmbeddedContracts, ToolContracts},
    digest, now_ms, Tool, ToolContext,
};
use anyhow::{bail, Context, Result};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
pub struct Append;
pub struct Search;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Write {
    kind: String,
    body: String,
    #[serde(default)]
    source_refs: Vec<Reference>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    id: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    query: String,
    #[serde(default = "limit")]
    limit: usize,
}
fn limit() -> usize {
    4
}
fn readable(context: &ToolContext<'_>, sender: &str) -> bool {
    sender == context.scope.actor
        || context
            .policy
            .grants
            .get(&context.scope.actor)
            .is_some_and(|g| g.memory_from.contains(sender))
}
impl Tool for Append {
    fn source(&self) -> &'static str {
        file!()
    }
    fn name(&self) -> &'static str {
        "memory_append"
    }
    fn description(&self) -> &'static str {
        "Append a bounded private canonical fact/advisory/experiment record. Sender, run, policy epoch and phase come from the host, not tool arguments."
    }
    fn read_only(&self) -> bool {
        false
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        let args: Write = serde_json::from_value(args)?;
        if !matches!(args.kind.as_str(), "fact" | "advisory" | "experiment")
            || args.body.trim().is_empty()
            || args.body.len() > 4096
            || args.source_refs.len() > 8
        {
            bail!("invalid bounded memory record");
        }
        crate::safety::require_persistable(&args.body)?;
        for reference in &args.source_refs {
            let (sender,hash,body):(String,String,String)=context.db.query_row("SELECT agent,sha256,body FROM mcp_memory WHERE id=?1 AND run_id=?2 AND policy_revision=?3",params![reference.id,context.scope.run.to_string(),context.policy.revision as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            if !readable(context, &sender)
                || hash != reference.sha256
                || digest(body.as_bytes()) != hash
            {
                bail!("source reference is not currently authorized");
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let hash = digest(args.body.as_bytes());
        let refs = args
            .source_refs
            .iter()
            .map(|r| json!({"id":r.id,"sha256":r.sha256}))
            .collect::<Vec<_>>();
        context.db.execute(
            "INSERT INTO mcp_memory VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                context.scope.run.to_string(),
                context.scope.actor,
                serde_json::to_string(&context.scope.phase)?,
                args.kind,
                args.body,
                hash,
                json!(refs).to_string(),
                context.policy.revision as i64,
                now_ms() as i64
            ],
        )?;
        Ok(
            json!({"id":id,"sha256":hash,"sender":context.scope.actor,"policy_revision":context.policy.revision as i64,"source_refs":refs}),
        )
    }
}
impl Tool for Search {
    fn source(&self) -> &'static str {
        file!()
    }
    fn recovery_arguments(&self) -> Option<Value> {
        Some(json!({"query":"","limit":4}))
    }
    fn name(&self) -> &'static str {
        "memory_search"
    }
    fn description(&self) -> &'static str {
        "Retrieve a bounded literal keyword packet from currently authorized senders in this run and policy epoch. Records remain canonical; previews are marked truncated."
    }
    fn read_only(&self) -> bool {
        true
    }
    fn schema(&self) -> Value {
        EmbeddedContracts.input_schema(self.name()).unwrap()
    }
    fn execute(&self, context: &ToolContext<'_>, args: Value) -> Result<Value> {
        EmbeddedContracts.validate(self.name(), &args)?;
        let args: Query = serde_json::from_value(args)?;
        if args.query.len() > 256 || !(1..=8).contains(&args.limit) {
            bail!("invalid bounded memory query");
        }
        let grant = context
            .policy
            .grants
            .get(&context.scope.actor)
            .context("missing grant")?;
        let senders = std::iter::once(context.scope.actor.clone())
            .chain(grant.memory_from.iter().cloned())
            .collect::<Vec<_>>();
        let query = format!(
            "%{}%",
            args.query
                .replace('!', "!!")
                .replace('%', "!%")
                .replace('_', "!_")
        );
        let mut statement=context.db.prepare("SELECT id,agent,kind,body,sha256,source_refs,created FROM mcp_memory WHERE run_id=?1 AND policy_revision=?2 AND agent IN (SELECT value FROM json_each(?3)) AND body LIKE ?4 ESCAPE '!' ORDER BY created DESC,id DESC LIMIT ?5")?;
        let rows = statement.query_map(
            params![
                context.scope.run.to_string(),
                context.policy.revision as i64,
                json!(senders).to_string(),
                query,
                args.limit as i64
            ],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                ))
            },
        )?;
        let mut result = vec![];
        let mut budget = 4096usize;
        for row in rows {
            let (id, sender, kind, body, hash, refs, created) = row?;
            if digest(body.as_bytes()) != hash {
                bail!("canonical memory integrity failure");
            }
            let preview = body.chars().take(1024.min(budget / 4)).collect::<String>();
            budget = budget.saturating_sub(preview.len());
            if preview.is_empty() {
                break;
            }
            result.push(json!({"id":id,"sender":sender,"kind":kind,"body":preview,"truncated":preview!=body,"sha256":hash,"source_refs":serde_json::from_str::<Value>(&refs)?,"created_at_ms":created,"tier":if now_ms().saturating_sub(created as u64)<120000{"hot"}else{"warm"}}));
        }
        Ok(
            json!({"records":result,"policy_revision":context.policy.revision as i64,"retrieval":"literal keyword, authorized before limiting","private_storage":"local SQLite"}),
        )
    }
}

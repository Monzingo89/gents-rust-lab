//! Durable, permission-checked DAG checkpoints for read-only local computation.
//! A completed node is advisory output, not independent task acceptance.
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub actor: String,
    /// A weight hash is not publisher/base-model admission evidence.
    pub model_sha256: String,
    pub depends_on: BTreeSet<String>,
    pub max_attempts: u32,
    pub lease_ms: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub version: u32,
    pub contract_sha256: String,
    /// Only this effect class can be retried automatically after a crash.
    pub execution: String,
    pub nodes: Vec<Node>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Graph {
    fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.execution != "read_only_local_computation"
            || !hash(&self.contract_sha256)
            || self.nodes.is_empty()
            || self.nodes.len() > 64
        {
            bail!("unsupported workflow contract, effect class or node count");
        }
        let mut ids = BTreeSet::new();
        for node in &self.nodes {
            crate::company::identifier(&node.id)?;
            if !ids.insert(&node.id)
                || node.actor.trim().is_empty()
                || node.actor.len() > 200
                || node.actor.chars().any(char::is_control)
                || !hash(&node.model_sha256)
                || !(1..=3).contains(&node.max_attempts)
                || !(1000..=1_800_000).contains(&node.lease_ms)
            {
                bail!("invalid or duplicate workflow node");
            }
        }
        let mut done = BTreeSet::new();
        loop {
            let before = done.len();
            for node in &self.nodes {
                if node.depends_on.iter().any(|id| !ids.contains(id)) {
                    bail!("workflow dependency does not exist");
                }
                if node.depends_on.iter().all(|id| done.contains(id)) {
                    done.insert(node.id.clone());
                }
            }
            if done.len() == self.nodes.len() {
                return Ok(());
            }
            if before == done.len() {
                bail!("workflow dependencies contain a cycle");
            }
        }
    }
    fn node(&self, id: &str) -> Result<&Node> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .context("workflow node not found")
    }
    fn authorize(&self, node: &Node, policy: &crate::governance::Policy) -> Result<()> {
        let grant = policy
            .grants
            .get(&node.actor)
            .context("workflow actor has no current grant")?;
        let mut queue = node.depends_on.iter().cloned().collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some(id) = queue.pop() {
            if !visited.insert(id.clone()) {
                continue;
            }
            let source = self.node(&id)?;
            // Transitive graph lineage is checked too, not just the immediate
            // sender. This does not taint arbitrary legacy context outside DAGs.
            if source.actor != node.actor && !grant.memory_from.contains(&source.actor) {
                bail!("workflow dependency memory is not currently authorized");
            }
            queue.extend(source.depends_on.iter().cloned());
        }
        Ok(())
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub data: Value,
    /// Original local receipts, never synthetic replacement replies on resume.
    pub calls: Vec<crate::provider::Reply>,
}
impl Output {
    fn encode(&self) -> Result<String> {
        crate::security::require_persistable(&self.data.to_string())?;
        if self.calls.is_empty() || self.calls.len() > 64 {
            bail!("workflow needs bounded real call receipts");
        }
        for call in &self.calls {
            crate::security::require_persistable(&call.text)?;
            if call.local_execution.is_none() || call.usage.cost_usd != Some(0.0) {
                bail!("workflow accepts only actual zero-provider-charge local receipts");
            }
        }
        // Trusted numeric runtime metadata is not user-supplied PAN input.
        let encoded = serde_json::to_string(self)?;
        if encoded.len() > 262_144 {
            bail!("workflow output exceeds checkpoint budget");
        }
        Ok(encoded)
    }
}
#[derive(Debug)]
pub struct Lease {
    stage: String,
    actor: String,
    token: String,
    attempt: u32,
    policy_revision: u64,
    dependency_sha256: String,
}
impl Lease {
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}
pub enum Claim {
    Cached(Output),
    Acquired(Lease),
}

/// OS-owned fencing for the trusted controller, not a time-based node lease.
/// Never delete the lock file: replacing its inode could admit two owners.
#[derive(Clone)]
pub struct ControllerLease {
    _file: Arc<std::fs::File>,
    scope: PathBuf,
    run: String,
    epoch: String,
}
impl ControllerLease {
    pub fn acquire(root: &Path, company: &str, owner: &str, run: Uuid) -> Result<Self> {
        // Resolve authenticated tenant ownership before touching its artifacts.
        crate::company::connect(root, company, owner, false)?;
        let scope = crate::company::directory(root, company)?.canonicalize()?;
        let directory = crate::company::scoped(root, company, &["runs", &run.to_string()])?;
        std::fs::create_dir_all(&directory)?;
        let path = directory.join("controller.lock");
        if path.is_symlink() {
            bail!("workflow controller lock symlink rejected");
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        if !file.metadata()?.is_file() {
            bail!("workflow controller lock must be a regular file");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if file.metadata()?.nlink() != 1 {
                bail!("workflow controller lock hardlink rejected");
            }
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        file.try_lock()
            .context("another controller owns this company workflow")?;
        Ok(Self {
            _file: Arc::new(file),
            scope,
            run: run.to_string(),
            epoch: Uuid::new_v4().to_string(),
        })
    }
}

/// Runtime-supplied actors and model hashes are never taken from model JSON.
/// `now` is monotonic elapsed milliseconds, scoped to the fenced controller.
/// Persisted deadlines are never interpreted across ownership epochs; UTC
/// audit timestamps are recorded independently inside `event`.
pub trait WorkflowRepository {
    fn install(&mut self, graph: &Graph) -> Result<String>;
    fn claim(&mut self, stage: &str, actor: &str, model_sha256: &str, now: u64) -> Result<Claim>;
    fn inputs(&mut self, lease: &Lease, now: u64) -> Result<Value>;
    fn finish(&mut self, lease: &Lease, output: &Output, now: u64) -> Result<()>;
    fn fail(&mut self, lease: &Lease, retryable: bool, now: u64) -> Result<()>;
    fn snapshot(&self) -> Result<Value>;
}
pub struct SqliteWorkflows {
    db: Connection,
    run: String,
    scope: PathBuf,
    root: PathBuf,
    company: String,
    owner: String,
    controller: Option<ControllerLease>,
}
impl SqliteWorkflows {
    pub fn contract_sha256(&self) -> Result<String> {
        Ok(admitted(&self.db, &self.run)?.0)
    }
    pub fn open(root: &Path, company: &str, owner: &str, run: Uuid) -> Result<Self> {
        let db = crate::company::connect(root, company, owner, false)?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS workflow_graphs(run_id TEXT PRIMARY KEY,manifest TEXT NOT NULL,sha256 TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS workflow_nodes(run_id TEXT NOT NULL,id TEXT NOT NULL,state TEXT NOT NULL,attempt INTEGER NOT NULL,token TEXT,deadline INTEGER,policy_revision INTEGER,output TEXT,output_sha256 TEXT,input_sha256 TEXT,PRIMARY KEY(run_id,id));
            CREATE TABLE IF NOT EXISTS workflow_events(seq INTEGER PRIMARY KEY,run_id TEXT NOT NULL,stage TEXT NOT NULL,attempt INTEGER NOT NULL,kind TEXT NOT NULL,detail TEXT NOT NULL,created INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS workflow_controllers(run_id TEXT PRIMARY KEY,epoch TEXT NOT NULL);")?;
        Ok(Self {
            db,
            run: run.to_string(),
            scope: crate::company::directory(root, company)?.canonicalize()?,
            root: root.into(),
            company: company.into(),
            owner: owner.into(),
            controller: None,
        })
    }

    pub fn bind_controller(&mut self, guard: &ControllerLease) -> Result<()> {
        anyhow::ensure!(
            guard.scope == self.scope && guard.run == self.run,
            "workflow controller belongs to a different tenant or run"
        );
        self.controller = Some(guard.clone());
        Ok(())
    }
    fn hold_controller(&mut self) -> Result<()> {
        if self.controller.is_none() {
            self.controller = Some(ControllerLease::acquire(
                &self.root,
                &self.company,
                &self.owner,
                self.run.parse()?,
            )?);
        }
        Ok(())
    }
    fn ensure_controller(&mut self, now: u64) -> Result<()> {
        self.hold_controller()?;
        let guard = self.controller.clone().context("controller missing")?;
        let registered: Option<String> = self
            .db
            .query_row(
                "SELECT epoch FROM workflow_controllers WHERE run_id=?1",
                [&self.run],
                |r| r.get(0),
            )
            .optional()?;
        if registered.as_deref() != Some(&guard.epoch) {
            self.recover_orphaned(&guard, now)?;
        }
        Ok(())
    }

    /// Only an exclusive controller may invalidate unexpired orphan tokens.
    /// Graph admission excludes ambiguous/external effects before any reset.
    pub fn recover_orphaned(&mut self, guard: &ControllerLease, now: u64) -> Result<usize> {
        if guard.scope != self.scope || guard.run != self.run {
            bail!("workflow controller belongs to a different tenant or run");
        }
        self.bind_controller(guard)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        running(&tx, &self.run)?;
        let (contract, _) = admitted(&tx, &self.run)?;
        let graph = graph(&tx, &self.run)?;
        if graph.contract_sha256 != contract {
            bail!("workflow recovery contract mismatch");
        }
        let registered: Option<String> = tx
            .query_row(
                "SELECT epoch FROM workflow_controllers WHERE run_id=?1",
                [&self.run],
                |r| r.get(0),
            )
            .optional()?;
        if registered.as_deref() == Some(&guard.epoch) {
            tx.commit()?;
            return Ok(0);
        }
        let rows = {
            let mut statement = tx.prepare(
                "SELECT id,attempt FROM workflow_nodes WHERE run_id=?1 AND state='running'",
            )?;
            let rows = statement
                .query_map([&self.run], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (stage, attempt) in &rows {
            let node = graph.node(stage)?;
            let next = if *attempt < node.max_attempts {
                "pending"
            } else {
                "failed"
            };
            tx.execute("UPDATE workflow_nodes SET state=?3,token=NULL,deadline=NULL WHERE run_id=?1 AND id=?2 AND state='running'", params![self.run,stage,next])?;
            event(
                &tx,
                &self.run,
                stage,
                *attempt,
                "controller_recovered",
                json!({"next_state":next,"effect":"read_only_local_computation","old_token_invalidated":true}),
                now,
            )?;
        }
        tx.execute("INSERT INTO workflow_controllers VALUES(?1,?2) ON CONFLICT(run_id) DO UPDATE SET epoch=excluded.epoch",params![self.run,guard.epoch])?;
        event(
            &tx,
            &self.run,
            "controller",
            0,
            "controller_epoch_registered",
            json!({"epoch":guard.epoch,"prior_controller":registered.is_some(),"orphaned_nodes":rows.len()}),
            now,
        )?;
        tx.commit()?;
        Ok(rows.len())
    }
}
fn admitted(db: &Connection, run: &str) -> Result<(String, crate::governance::Policy)> {
    let (intake, expected, approval, state): (String, String, Option<String>, String) = db
        .query_row(
            "SELECT intake,digest,approval,state FROM jobs WHERE run_id=?1",
            [run],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    if digest(intake.as_bytes()) != expected
        || approval.is_none()
        || !matches!(state.as_str(), "approved" | "running")
    {
        bail!("workflow requires the unchanged approved live task");
    }
    let policy = crate::governance::load(db, run)?.context("workflow requires live governance")?;
    Ok((expected, policy))
}
fn graph(db: &Connection, run: &str) -> Result<Graph> {
    let (body, sha): (String, String) = db.query_row(
        "SELECT manifest,sha256 FROM workflow_graphs WHERE run_id=?1",
        [run],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if digest(body.as_bytes()) != sha {
        bail!("workflow graph integrity check failed");
    }
    let graph: Graph = serde_json::from_str(&body)?;
    graph.validate()?;
    Ok(graph)
}
fn output(db: &Connection, run: &str, stage: &str) -> Result<Output> {
    let (body,sha):(String,String)=db.query_row("SELECT output,output_sha256 FROM workflow_nodes WHERE run_id=?1 AND id=?2 AND state='completed'",params![run,stage],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if digest(body.as_bytes()) != sha {
        bail!("workflow output integrity check failed");
    }
    Ok(serde_json::from_str(&body)?)
}
fn dependencies(db: &Connection, run: &str, node: &Node, contract: &str) -> Result<String> {
    let mut bindings = Vec::new();
    for id in &node.depends_on {
        output(db, run, id)?;
        let sha: String = db.query_row(
            "SELECT output_sha256 FROM workflow_nodes WHERE run_id=?1 AND id=?2",
            params![run, id],
            |r| r.get(0),
        )?;
        bindings.push((id, sha));
    }
    Ok(digest(
        serde_json::to_string(&json!({"contract":contract,"stage":node.id,"sources":bindings}))?
            .as_bytes(),
    ))
}
fn running(db: &Connection, run: &str) -> Result<()> {
    let state: String = db.query_row("SELECT state FROM jobs WHERE run_id=?1", [run], |r| {
        r.get(0)
    })?;
    if state != "running" {
        bail!("workflow execution requires a running approved task");
    }
    Ok(())
}
fn event(
    db: &Connection,
    run: &str,
    stage: &str,
    attempt: u32,
    kind: &str,
    mut detail: Value,
    now: u64,
) -> Result<()> {
    if let Some(fields) = detail.as_object_mut() {
        fields.insert("lease_clock".into(), json!("controller_monotonic"));
        fields.insert("lease_uptime_ms".into(), json!(now));
    }
    db.execute("INSERT INTO workflow_events(run_id,stage,attempt,kind,detail,created) VALUES(?1,?2,?3,?4,?5,?6)",params![run,stage,attempt,kind,detail.to_string(),crate::now_ms() as i64])?;
    Ok(())
}
fn check_lease(
    db: &Connection,
    run: &str,
    lease: &Lease,
    now: u64,
) -> Result<(Graph, crate::governance::Policy)> {
    running(db, run)?;
    let (contract, policy) = admitted(db, run)?;
    let graph = graph(db, run)?;
    if graph.contract_sha256 != contract || policy.revision != lease.policy_revision {
        bail!("workflow task or policy changed during lease");
    }
    let node = graph.node(&lease.stage)?;
    if node.actor != lease.actor {
        bail!("workflow lease actor mismatch");
    }
    graph.authorize(node, &policy)?;
    if dependencies(db, run, node, &contract)? != lease.dependency_sha256 {
        bail!("workflow dependency checkpoint changed during lease");
    }
    let active:bool=db.query_row("SELECT COUNT(*)=1 FROM workflow_nodes WHERE run_id=?1 AND id=?2 AND state='running' AND token=?3 AND attempt=?4 AND deadline>?5 AND policy_revision=?6",
        params![run,lease.stage,lease.token,lease.attempt,now as i64,lease.policy_revision as i64],|r|r.get(0))?;
    if !active {
        bail!("workflow lease is expired, replaced or already committed");
    }
    Ok((graph, policy))
}
impl WorkflowRepository for SqliteWorkflows {
    fn install(&mut self, requested: &Graph) -> Result<String> {
        requested.validate()?;
        self.hold_controller()?;
        let body = serde_json::to_string(requested)?;
        crate::security::require_persistable(&body)?;
        if body.len() > 64_000 {
            bail!("workflow graph exceeds budget");
        }
        let sha = digest(body.as_bytes());
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (contract, policy) = admitted(&tx, &self.run)?;
        if requested.contract_sha256 != contract
            || requested
                .nodes
                .iter()
                .any(|n| !policy.ceilings.contains_key(&n.actor))
        {
            bail!("workflow does not match approved contract or owner actor ceiling");
        }
        let existing: Option<String> = tx
            .query_row(
                "SELECT sha256 FROM workflow_graphs WHERE run_id=?1",
                [&self.run],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            graph(&tx, &self.run)?;
            if existing != sha {
                bail!("installed workflow is immutable; use a new approved run");
            }
        } else {
            tx.execute(
                "INSERT INTO workflow_graphs VALUES(?1,?2,?3)",
                params![self.run, body, sha],
            )?;
            for node in &requested.nodes {
                tx.execute(
                    "INSERT INTO workflow_nodes VALUES(?1,?2,'pending',0,NULL,NULL,NULL,NULL,NULL,NULL)",
                    params![self.run, node.id],
                )?;
            }
            event(
                &tx,
                &self.run,
                "graph",
                0,
                "workflow_installed",
                json!({"sha256":sha,"nodes":requested.nodes.len(),"task_accepted":false}),
                crate::lease_clock::now_ms(),
            )?;
        }
        tx.commit()?;
        Ok(sha)
    }
    fn claim(&mut self, stage: &str, actor: &str, model_sha256: &str, now: u64) -> Result<Claim> {
        self.ensure_controller(now)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        running(&tx, &self.run)?;
        let (contract, policy) = admitted(&tx, &self.run)?;
        let graph = graph(&tx, &self.run)?;
        let node = graph.node(stage)?;
        if graph.contract_sha256 != contract
            || node.actor != actor
            || node.model_sha256 != model_sha256
        {
            bail!("workflow dispatch identity or contract mismatch");
        }
        graph.authorize(node, &policy)?;
        let dependency_sha256 = dependencies(&tx, &self.run, node, &contract)?;
        let (mut state,attempt,deadline,revision,inputs):(String,u32,Option<i64>,Option<i64>,Option<String>)=tx.query_row(
            "SELECT state,attempt,deadline,policy_revision,input_sha256 FROM workflow_nodes WHERE run_id=?1 AND id=?2",params![self.run,stage],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        if state == "completed" {
            if inputs.as_deref() != Some(&dependency_sha256) {
                bail!("cached workflow input binding changed");
            }
            let cached = output(&tx, &self.run, stage)?;
            event(
                &tx,
                &self.run,
                stage,
                attempt,
                "checkpoint_reused",
                json!({"new_model_calls":0,"policy_revision":policy.revision,"input_sha256":dependency_sha256}),
                now,
            )?;
            tx.commit()?;
            return Ok(Claim::Cached(cached));
        }
        if state == "running"
            && (deadline.is_some_and(|d| d <= now as i64)
                || revision != Some(policy.revision as i64))
        {
            state = if attempt < node.max_attempts {
                "pending"
            } else {
                "failed"
            }
            .into();
            tx.execute("UPDATE workflow_nodes SET state=?3,token=NULL,deadline=NULL WHERE run_id=?1 AND id=?2",params![self.run,stage,state])?;
            event(
                &tx,
                &self.run,
                stage,
                attempt,
                "lease_recovered",
                json!({"next_state":state,"effect":"read_only_local_computation","prior_policy_revision":revision,"policy_revision":policy.revision}),
                now,
            )?;
        }
        if state != "pending" || attempt >= node.max_attempts {
            tx.commit()?;
            bail!("workflow stage is busy, failed or retry budget exhausted");
        }
        let token = Uuid::new_v4().to_string();
        let deadline = now
            .checked_add(node.lease_ms)
            .filter(|d| *d <= i64::MAX as u64)
            .context("workflow lease time overflow")?;
        let attempt = attempt + 1;
        tx.execute("UPDATE workflow_nodes SET state='running',attempt=?3,token=?4,deadline=?5,policy_revision=?6,input_sha256=?7 WHERE run_id=?1 AND id=?2",params![self.run,stage,attempt,token,deadline as i64,policy.revision as i64,dependency_sha256])?;
        event(
            &tx,
            &self.run,
            stage,
            attempt,
            "stage_claimed",
            json!({"actor":actor,"policy_revision":policy.revision,"model_sha256":model_sha256,"lease_id":token,"input_sha256":dependency_sha256}),
            now,
        )?;
        tx.commit()?;
        Ok(Claim::Acquired(Lease {
            stage: stage.into(),
            actor: actor.into(),
            token,
            attempt,
            policy_revision: policy.revision,
            dependency_sha256,
        }))
    }
    fn inputs(&mut self, lease: &Lease, now: u64) -> Result<Value> {
        self.ensure_controller(now)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (graph, _) = check_lease(&tx, &self.run, lease, now)?;
        let node = graph.node(&lease.stage)?;
        let mut rows = Vec::new();
        for id in &node.depends_on {
            let source = output(&tx, &self.run, id)?;
            let source_sha: String = tx.query_row(
                "SELECT output_sha256 FROM workflow_nodes WHERE run_id=?1 AND id=?2",
                params![self.run, id],
                |r| r.get(0),
            )?;
            rows.push(json!({"stage":id,"actor":graph.node(id)?.actor,"data":source.data,"output_sha256":source_sha,"trust":"model_advisory_not_task_acceptance"}));
        }
        tx.commit()?;
        Ok(json!(rows))
    }
    fn finish(&mut self, lease: &Lease, output: &Output, now: u64) -> Result<()> {
        self.ensure_controller(now)?;
        let body = output.encode()?;
        let sha = digest(body.as_bytes());
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_lease(&tx, &self.run, lease, now)?;
        tx.execute("UPDATE workflow_nodes SET state='completed',token=NULL,deadline=NULL,output=?3,output_sha256=?4 WHERE run_id=?1 AND id=?2",params![self.run,lease.stage,body,sha])?;
        event(
            &tx,
            &self.run,
            &lease.stage,
            lease.attempt,
            "stage_checkpointed",
            json!({"output_sha256":sha,"policy_revision":lease.policy_revision,"lease_id":lease.token,"trust":"model_advisory_not_task_acceptance","calls":output.calls.len()}),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    fn fail(&mut self, lease: &Lease, retryable: bool, now: u64) -> Result<()> {
        self.ensure_controller(now)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let graph = graph(&tx, &self.run)?;
        let state = if retryable && lease.attempt < graph.node(&lease.stage)?.max_attempts {
            "pending"
        } else {
            "failed"
        };
        let changed=tx.execute("UPDATE workflow_nodes SET state=?5,token=NULL,deadline=NULL WHERE run_id=?1 AND id=?2 AND state='running' AND token=?3 AND attempt=?4",params![self.run,lease.stage,lease.token,lease.attempt,state])?;
        if changed != 1 {
            bail!("stale failure cannot change another lease");
        }
        event(
            &tx,
            &self.run,
            &lease.stage,
            lease.attempt,
            "stage_failed",
            json!({"next_state":state,"lease_id":lease.token,"diagnostic":"see original correlated model-call events"}),
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    fn snapshot(&self) -> Result<Value> {
        let graph = graph(&self.db, &self.run)?;
        let epoch: Option<String> = self
            .db
            .query_row(
                "SELECT epoch FROM workflow_controllers WHERE run_id=?1",
                [&self.run],
                |r| r.get(0),
            )
            .optional()?;
        let mut stmt=self.db.prepare("SELECT id,state,attempt,policy_revision,output_sha256 FROM workflow_nodes WHERE run_id=?1 ORDER BY id")?;
        let nodes=stmt.query_map([&self.run],|r|Ok(json!({"id":r.get::<_,String>(0)?,"state":r.get::<_,String>(1)?,"attempt":r.get::<_,u32>(2)?,"policy_revision":r.get::<_,Option<i64>>(3)?,"output_sha256":r.get::<_,Option<String>>(4)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let events = self.db.query_row(
            "SELECT COUNT(*) FROM workflow_events WHERE run_id=?1",
            [&self.run],
            |r| r.get::<_, i64>(0),
        )?;
        for node in &nodes {
            if node["state"] == "completed" {
                output(
                    &self.db,
                    &self.run,
                    node["id"].as_str().context("checkpoint identity missing")?,
                )?;
            }
        }
        Ok(
            json!({"version":1,"run_id":self.run,"lease_clock":"controller_monotonic","controller_epoch":epoch,"graph_sha256":digest(serde_json::to_string(&graph)?.as_bytes()),"nodes":nodes,"events":events,"task_acceptance":"requires independent verifier"}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: std::path::PathBuf,
        run: Uuid,
        graph: Graph,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("gents-dag-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            let run = Uuid::new_v4();
            let op = |v: Value| crate::company::operation(&root, "one", "alice", &v).unwrap();
            op(json!({"op":"register","name":"Unit fixture"}));
            let state = op(
                json!({"op":"intake","run_id":run,"intake":{"idea":"Synthetic DAG unit fixture","acceptance":"Independent verification required","exclusions":"External effects","timeline":"Test","company_signer":"A","provider_signer":"B","budget_usd":0}}),
            );
            let contract = state["jobs"][0]["digest"].as_str().unwrap().to_string();
            op(json!({"op":"approve","run_id":run,"digest":contract}));
            let master = json!({"tools":["access_delegate"],"skills":[],"memory_from":[]});
            let a = json!({"tools":[],"skills":[],"memory_from":[]});
            let b = json!({"tools":[],"skills":[],"memory_from":["A"]});
            let c = json!({"tools":[],"skills":[],"memory_from":["A","B"]});
            let grants = json!({"Master":master,"A":a,"B":b,"C":c});
            op(
                json!({"op":"governance_install","run_id":run,"expected_revision":0,"policy":{"revision":1,"master":"Master","ceilings":grants,"grants":grants}}),
            );
            crate::company::admit(&root, "one", "alice", run, "Synthetic DAG unit fixture")
                .unwrap();
            let nodes = [
                ("a", "A", None),
                ("b", "B", Some("a")),
                ("c", "C", Some("b")),
            ]
            .into_iter()
            .map(|(id, actor, dep)| Node {
                id: id.into(),
                actor: actor.into(),
                model_sha256: "a".repeat(64),
                depends_on: dep.into_iter().map(String::from).collect(),
                max_attempts: 3,
                lease_ms: 2000,
            })
            .collect();
            Self {
                root,
                run,
                graph: Graph {
                    version: 1,
                    contract_sha256: contract,
                    execution: "read_only_local_computation".into(),
                    nodes,
                },
            }
        }
        fn store(&self) -> SqliteWorkflows {
            SqliteWorkflows::open(&self.root, "one", "alice", self.run).unwrap()
        }
        fn ready(&self) -> SqliteWorkflows {
            let mut store = self.store();
            store.install(&self.graph).unwrap();
            store
        }
        fn claim(&self, store: &mut SqliteWorkflows, id: &str, now: u64) -> Lease {
            match store
                .claim(
                    id,
                    &self.graph.node(id).unwrap().actor,
                    &"a".repeat(64),
                    now,
                )
                .unwrap()
            {
                Claim::Acquired(lease) => lease,
                Claim::Cached(_) => panic!("expected a new lease"),
            }
        }
        fn grant_c(&self, sources: Value, revision: u64) {
            crate::company::delegate_access(&self.root,"one","alice",self.run,"Master",
                &json!({"recipient":"C","expected_revision":revision,"grant":{"tools":[],"skills":[],"memory_from":sources}})).unwrap();
        }
    }
    fn fixture_output() -> Output {
        Output {
            data: json!({"fixture":"advisory unit result, not a real inference receipt"}),
            calls: vec![crate::provider::Reply {
                text: "{\"fixture\":true}".into(),
                usage: crate::provider::Usage {
                    cost_usd: Some(0.0),
                    source: "SIMULATED unit fixture, not capability evidence".into(),
                    ..Default::default()
                },
                latency_ms: 1,
                response_id: None,
                local_execution: Some(crate::provider::LocalExecution {
                    finish_reason: None,
                    task_input_sha256: None,
                    backend_pid: 1,
                    port: 1,
                    dispatched_at_ms: 1,
                    received_at_ms: 2,
                    resident_kib: None,
                    launch_policy: None,
                    observed_context_tokens: None,
                    generation_budget: None,
                    separated_reasoning_observed: None,
                    prompt_preflight: None,
                }),
            }],
        }
    }
    #[test]
    fn wall_epoch_values_cannot_steal_a_live_owner_or_expire_its_elapsed_lease() {
        let f = Fixture::new();
        let mut live = f.ready();
        let lease = f.claim(&mut live, "a", 100);
        let mut other = f.store();
        assert!(other
            .claim("a", "A", &"a".repeat(64), crate::now_ms() + 86_400_000)
            .is_err());
        live.finish(&lease, &fixture_output(), 101).unwrap();
        let created: i64 = live
            .db
            .query_row(
                "SELECT created FROM workflow_events WHERE kind='stage_checkpointed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            created > 101,
            "audit UTC and elapsed lease clocks must stay separate"
        );
        assert_eq!(live.snapshot().unwrap()["nodes"][0]["state"], "completed");
    }

    #[test]
    fn exclusive_controller_recovers_orphans_without_waiting_for_expiry() {
        let f = Fixture::new();
        let mut store = f.ready();
        let first = f.claim(&mut store, "a", 100);
        store.finish(&first, &fixture_output(), 101).unwrap();
        let stale = f.claim(&mut store, "b", 102);
        let old_owner = store.controller.clone().unwrap();
        assert_eq!(store.recover_orphaned(&old_owner, 103).unwrap(), 0);
        assert!(ControllerLease::acquire(&f.root, "one", "alice", f.run).is_err());
        assert!(ControllerLease::acquire(&f.root, "one", "mallory", f.run).is_err());
        drop(old_owner);
        drop(store);
        let owner = ControllerLease::acquire(&f.root, "one", "alice", f.run).unwrap();
        let mut store = f.store();
        assert_eq!(store.recover_orphaned(&owner, 103).unwrap(), 1);
        assert!(store.finish(&stale, &fixture_output(), 104).is_err());
        assert!(store.fail(&stale, true, 104).is_err());
        let next = f.claim(&mut store, "b", 105);
        assert_eq!(next.attempt, 2);
        store.finish(&next, &fixture_output(), 106).unwrap();
        assert!(matches!(
            store.claim("a", "A", &"a".repeat(64), 107).unwrap(),
            Claim::Cached(_)
        ));
        assert_eq!(store.recover_orphaned(&owner, 108).unwrap(), 0);
        drop(owner);
        assert!(ControllerLease::acquire(&f.root, "one", "alice", f.run).is_err());
        drop(store);
        let replacement = ControllerLease::acquire(&f.root, "one", "alice", f.run).unwrap();
        let mut store = f.store();
        assert_eq!(store.recover_orphaned(&replacement, 109).unwrap(), 0);
    }

    #[test]
    fn orphan_recovery_is_run_scoped_bounded_and_preserves_operator_stop() {
        let f = Fixture::new();
        let other = Fixture::new();
        let mut store = f.ready();
        let wrong = ControllerLease::acquire(&other.root, "one", "alice", other.run).unwrap();
        assert!(store.recover_orphaned(&wrong, 100).is_err());
        for attempt in 1..=3 {
            let lease = f.claim(&mut store, "a", 100 + attempt);
            assert_eq!(lease.attempt, attempt as u32);
            drop(store);
            let owner = ControllerLease::acquire(&f.root, "one", "alice", f.run).unwrap();
            store = f.store();
            assert_eq!(store.recover_orphaned(&owner, 110 + attempt).unwrap(), 1);
        }
        assert_eq!(store.snapshot().unwrap()["nodes"][0]["state"], "failed");
        assert!(store.claim("a", "A", &"a".repeat(64), 200).is_err());
        crate::company::outcome(&f.root, "one", "alice", f.run, "cancelled").unwrap();
        let owner = store.controller.clone().unwrap();
        assert!(store.recover_orphaned(&owner, 201).is_err());
        assert_eq!(
            crate::company::interrupt_running(&f.root, "one", "alice", f.run).unwrap(),
            "cancelled"
        );
        assert!(crate::company::resume_interrupted(
            &f.root,
            "one",
            "alice",
            f.run,
            "Synthetic DAG unit fixture"
        )
        .is_err());
    }

    #[test]
    fn interrupted_resume_requires_original_approval_and_does_not_resume_other_states() {
        let f = Fixture::new();
        assert!(crate::company::resume_interrupted(
            &f.root,
            "one",
            "alice",
            f.run,
            "Synthetic DAG unit fixture"
        )
        .is_err());
        assert_eq!(
            crate::company::interrupt_running(&f.root, "one", "alice", f.run).unwrap(),
            "interrupted"
        );
        assert!(crate::company::resume_interrupted(
            &f.root,
            "one",
            "alice",
            f.run,
            "Modified task"
        )
        .is_err());
        assert!(crate::company::resume_interrupted(
            &f.root,
            "one",
            "alice",
            f.run,
            "Synthetic DAG unit fixture"
        )
        .is_ok());
        for state in ["completed", "failed", "cancelled"] {
            crate::company::outcome(&f.root, "one", "alice", f.run, state).unwrap();
            assert_eq!(
                crate::company::interrupt_running(&f.root, "one", "alice", f.run).unwrap(),
                state
            );
            assert!(crate::company::resume_interrupted(
                &f.root,
                "one",
                "alice",
                f.run,
                "Synthetic DAG unit fixture"
            )
            .is_err());
        }
    }

    #[test]
    fn graph_rejects_cycles_unknown_nodes_duplicates_and_external_effects() {
        let f = Fixture::new();
        for mutation in 0..4 {
            let mut graph = f.graph.clone();
            match mutation {
                0 => {
                    graph.nodes[0].depends_on.insert("c".into());
                }
                1 => {
                    graph.nodes[0].depends_on.insert("missing".into());
                }
                2 => graph.nodes.push(graph.nodes[0].clone()),
                _ => graph.execution = "money_movement".into(),
            }
            assert!(graph.validate().is_err());
        }
        let mut store = f.ready();
        let mut changed = f.graph.clone();
        changed.nodes[0].max_attempts = 2;
        assert!(store.install(&changed).is_err());
        assert!(store.install(&f.graph).is_ok());
        assert!(store.claim("a", "B", &"a".repeat(64), 100).is_err());
        assert!(store.claim("a", "A", &"b".repeat(64), 100).is_err());
        assert!(store.claim("b", "B", &"a".repeat(64), 100).is_err());
    }
    #[test]
    fn reopen_reuses_completed_checkpoints_and_never_claims_a_busy_stage_twice() {
        let f = Fixture::new();
        let mut first = f.ready();
        let lease = f.claim(&mut first, "a", 100);
        let mut second = f.store();
        assert!(second.claim("a", "A", &"a".repeat(64), 101).is_err());
        first.finish(&lease, &fixture_output(), 102).unwrap();
        assert!(first.finish(&lease, &fixture_output(), 103).is_err());
        drop(first);
        drop(second);
        let mut reopened = f.store();
        let Claim::Cached(saved) = reopened.claim("a", "A", &"a".repeat(64), 104).unwrap() else {
            panic!("checkpoint lost")
        };
        assert_eq!(saved.data, fixture_output().data);
        assert_eq!(saved.calls[0].text, fixture_output().calls[0].text);
        let b = f.claim(&mut reopened, "b", 105);
        assert_eq!(reopened.inputs(&b, 106).unwrap()[0]["data"], saved.data);
        assert_eq!(reopened.snapshot().unwrap()["nodes"][0]["attempt"], 1);
    }
    #[test]
    fn expired_lease_recovery_is_bounded_and_old_tokens_cannot_commit_or_fail() {
        let f = Fixture::new();
        let mut store = f.ready();
        let stale = f.claim(&mut store, "a", 100);
        drop(store);
        let mut store = f.store();
        let second = f.claim(&mut store, "a", 2100);
        assert_eq!(second.attempt, 2);
        assert!(store.finish(&stale, &fixture_output(), 2101).is_err());
        assert!(store.fail(&stale, true, 2101).is_err());
        store.fail(&second, true, 2102).unwrap();
        let third = f.claim(&mut store, "a", 2103);
        assert_eq!(third.attempt, 3);
        store.fail(&third, true, 2104).unwrap();
        assert!(store.claim("a", "A", &"a".repeat(64), 2105).is_err());
        assert_eq!(store.snapshot().unwrap()["nodes"][0]["state"], "failed");
    }
    #[test]
    fn transitive_memory_access_and_live_revocation_cover_reads_commits_and_cache_hits() {
        let f = Fixture::new();
        let mut store = f.ready();
        for id in ["a", "b"] {
            let lease = f.claim(&mut store, id, 100);
            store.finish(&lease, &fixture_output(), 101).unwrap();
        }
        f.grant_c(json!(["B"]), 1);
        assert!(store.claim("c", "C", &"a".repeat(64), 102).is_err());
        f.grant_c(json!(["A", "B"]), 2);
        let stale = f.claim(&mut store, "c", 103);
        assert_eq!(
            store.inputs(&stale, 104).unwrap().as_array().unwrap().len(),
            1
        );
        f.grant_c(json!(["B"]), 3);
        assert!(store.inputs(&stale, 105).is_err());
        assert!(store.finish(&stale, &fixture_output(), 105).is_err());
        f.grant_c(json!(["A", "B"]), 4);
        let current = f.claim(&mut store, "c", 106);
        assert_eq!(current.attempt, 2);
        store.finish(&current, &fixture_output(), 107).unwrap();
        f.grant_c(json!(["B"]), 5);
        assert!(store.claim("c", "C", &"a".repeat(64), 108).is_err());
    }
    #[test]
    fn altered_inputs_graph_outputs_and_frozen_task_fail_closed() {
        let f = Fixture::new();
        let mut store = f.ready();
        let a = f.claim(&mut store, "a", 100);
        store.finish(&a, &fixture_output(), 101).unwrap();
        let b = f.claim(&mut store, "b", 102);
        store
            .db
            .execute("UPDATE workflow_nodes SET output='{}' WHERE id='a'", [])
            .unwrap();
        assert!(store.inputs(&b, 103).is_err());
        let mut altered = fixture_output();
        altered.data = json!({"changed":true});
        let body = altered.encode().unwrap();
        store
            .db
            .execute(
                "UPDATE workflow_nodes SET output=?1,output_sha256=?2 WHERE id='a'",
                params![body, digest(body.as_bytes())],
            )
            .unwrap();
        assert!(store.finish(&b, &fixture_output(), 104).is_err());
        store
            .db
            .execute("UPDATE workflow_graphs SET manifest='{}'", [])
            .unwrap();
        assert!(store.snapshot().is_err());
        let f2 = Fixture::new();
        let mut second = f2.ready();
        second
            .db
            .execute("UPDATE jobs SET intake='{}'", [])
            .unwrap();
        assert!(second.claim("a", "A", &"a".repeat(64), 100).is_err());
    }
    #[test]
    fn owner_tenant_run_and_sensitive_output_boundaries_are_preserved() {
        let f = Fixture::new();
        let mut store = f.ready();
        assert!(SqliteWorkflows::open(&f.root, "one", "mallory", f.run).is_err());
        assert!(SqliteWorkflows::open(&f.root, "two", "alice", f.run).is_err());
        let mut unrelated = SqliteWorkflows::open(&f.root, "one", "alice", Uuid::new_v4()).unwrap();
        assert!(unrelated.claim("a", "A", &"a".repeat(64), 100).is_err());
        let lease = f.claim(&mut store, "a", 100);
        let mut bad = fixture_output();
        bad.data = json!({"card":"4111111111111111"});
        assert!(store.finish(&lease, &bad, 101).is_err());
        bad = fixture_output();
        bad.calls[0].local_execution = None;
        assert!(store.finish(&lease, &bad, 101).is_err());
        store.finish(&lease, &fixture_output(), 102).unwrap();
    }
}

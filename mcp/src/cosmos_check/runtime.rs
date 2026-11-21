//! Read-only REST execution against an explicitly configured local emulator.
//! No auth, provisioning, redirects, proxies, automatic retries or cloud path.
use super::{Case, Contract, PartitionKey, Query, QueryFile};
use anyhow::{ensure, Context, Result};
use reqwest::{blocking::Client, header::HeaderMap, Method};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Constructed by trusted host code, never deserialized from model/tool JSON.
/// The instance label binds configuration; unauthenticated HTTP cannot attest
/// an image digest or distinguish another process that replaces the listener.
#[derive(Clone, Debug, Serialize)]
pub struct HostEmulator {
    port: u16,
    instance: String,
}
impl HostEmulator {
    pub fn new(port: u16, instance: String) -> Result<Self> {
        ensure!(
            port != 0 && super::component(&instance),
            "Explicit local emulator port/instance required"
        );
        Ok(Self { port, instance })
    }
}
pub struct Snapshot {
    contract: Contract,
    queries: QueryFile,
    host: HostEmulator,
    identity: String,
    query_sha256: String,
}
impl Snapshot {
    pub fn prepare(contract: Contract, host: HostEmulator, text: &str) -> Result<Self> {
        let queries = QueryFile::parse(text, &contract)?;
        let query_sha256 = crate::digest(text.as_bytes());
        let implementation = crate::digest(
            concat!(
                include_str!("../cosmos_check.rs"),
                include_str!("runtime.rs"),
                include_str!("../strict_json.rs"),
                include_str!("../safety.rs")
            )
            .as_bytes(),
        );
        let identity = crate::digest(&serde_json::to_vec(
            &json!({"version":1,"contract":contract,"host":host,"query_sha256":query_sha256,"implementation":implementation}),
        )?);
        Ok(Self {
            contract,
            queries,
            host,
            identity,
            query_sha256,
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Check {
    pub name: String,
    pub status: String,
    pub detail: String,
}
fn check(name: &str, pass: bool, detail: &str) -> Check {
    Check {
        name: name.into(),
        status: if pass { "passed" } else { "failed" }.into(),
        detail: detail.into(),
    }
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Observation {
    pub status: u16,
    pub response_sha256: String,
    pub response_bytes: usize,
    pub elapsed_micros: u64,
    /// An emulator-returned header. This is never a measured production RU.
    pub reported_request_charge: Option<String>,
    pub request_charge_scope: String,
    pub query_metrics: Option<String>,
    pub query_metrics_scope: String,
    pub session_token_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct PlanResult {
    pub observation: Observation,
    pub state: String,
    pub plan: Option<Value>,
    pub scope: String,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct CaseResult {
    pub id: String,
    pub query_sha256: String,
    pub partition: Option<Vec<Value>>,
    pub expected_rows_sha256: String,
    pub rows: Vec<Value>,
    pub rows_sha256: String,
    pub complete: bool,
    pub page_size_hint_honored: bool,
    pub pages: Vec<Observation>,
    pub plan: PlanResult,
    pub checks: Vec<Check>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Evaluation {
    pub schema_version: u32,
    pub status: String,
    pub verification_identity: String,
    pub input_sha256: String,
    pub observed_at_unix_seconds: u64,
    pub collection_before: Observation,
    pub collection_after: Observation,
    pub observed_partition_key: Option<PartitionKey>,
    pub observed_indexing_policy: Option<Value>,
    pub cases: Vec<CaseResult>,
    pub checks: Vec<Check>,
    pub actual_http_requests: u32,
    pub elapsed_micros: u64,
    pub real_ru: Option<f64>,
    pub real_ru_state: String,
    pub model_calls: u32,
    pub provider_cost_usd: u32,
    pub qualified: bool,
    pub promoted: bool,
    pub authority_granted: bool,
    pub scope: String,
}
struct Reply {
    observation: Observation,
    json: Value,
    continuation: Option<String>,
}
struct Io<'a> {
    client: Client,
    snapshot: &'a Snapshot,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    calls: u32,
}
impl Io<'_> {
    fn request(&mut self, query: Option<(&Query, &Case, bool, Option<&str>)>) -> Result<Reply> {
        ensure!(
            !self.cancelled.load(Ordering::Acquire),
            "Cosmos check cancelled"
        );
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .context("Cosmos deadline exhausted")?;
        let contract = &self.snapshot.contract;
        let base = format!(
            "http://127.0.0.1:{}/dbs/{}/colls/{}",
            self.snapshot.host.port, contract.database, contract.container
        );
        let mut request = self
            .client
            .request(
                if query.is_some() {
                    Method::POST
                } else {
                    Method::GET
                },
                if query.is_some() {
                    format!("{base}/docs")
                } else {
                    base
                },
            )
            .timeout(remaining.min(Duration::from_millis(contract.request_timeout_ms)))
            .header("x-ms-version", "2018-12-31")
            .header("accept", "application/json");
        if let Some((query, case, plan, continuation)) = query {
            request = request
                .header("content-type", "application/query+json")
                .header("x-ms-documentdb-isquery", "True")
                .header("x-ms-documentdb-populatequerymetrics", "True")
                .header("x-ms-max-item-count", case.page_size_hint.to_string())
                .json(&json!({"query":query.sql,"parameters":query.parameters}));
            request = if let Some(partition) = &case.partition {
                request.header(
                    "x-ms-documentdb-partitionkey",
                    serde_json::to_string(partition)?,
                )
            } else {
                request.header("x-ms-documentdb-query-enablecrosspartition", "True")
            };
            if plan {
                request = request.header("x-ms-cosmos-is-query-plan-request", "True")
                .header("x-ms-cosmos-query-version", "1.4")
                .header("x-ms-cosmos-supported-query-features", "Aggregate,CompositeAggregate,Distinct,GroupBy,MultipleOrderBy,OffsetAndLimit,OrderBy,Top");
            }
            if let Some(token) = continuation {
                request = request.header("x-ms-continuation", token);
            }
        }
        self.calls += 1;
        let start = Instant::now();
        let response = request.send().map_err(|_| anyhow::anyhow!("Cosmos transport failed within fixed local destination/deadline; no automatic retry"))?;
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let content_type = header(&headers, "content-type")?;
        ensure!(
            content_type.as_deref().is_some_and(|v| v
                .split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))),
            "Cosmos response must be JSON; redirects and arbitrary payloads are rejected"
        );
        let mut bytes = vec![];
        response
            .take(32769)
            .read_to_end(&mut bytes)
            .map_err(|_| anyhow::anyhow!("Cosmos response read failed"))?;
        ensure!(bytes.len() <= 32768, "Cosmos response byte bound exceeded");
        let text = std::str::from_utf8(&bytes).context("Cosmos response must be UTF-8")?;
        // Gate raw bytes AND decoded strings before digesting or returning any
        // observation. Escaped private data cannot bypass the raw-text check.
        crate::safety::require_persistable(text)?;
        let value = crate::strict_json::parse(text).map_err(|_| {
            anyhow::anyhow!("Cosmos response JSON is invalid or contains duplicate keys")
        })?;
        crate::safety::require_persistable(&serde_json::to_string(&value)?)?;
        let continuation = header(&headers, "x-ms-continuation")?.filter(|s| !s.is_empty());
        let charge = header(&headers, "x-ms-request-charge")?;
        if let Some(value) = &charge {
            ensure!(
                value
                    .parse::<f64>()
                    .is_ok_and(|v| v.is_finite() && v >= 0.0),
                "Invalid reported Cosmos request charge"
            );
        }
        let metrics = header(&headers, "x-ms-documentdb-query-metrics")?;
        let observation = Observation { status, response_sha256: crate::digest(text.as_bytes()), response_bytes: bytes.len(), elapsed_micros: start.elapsed().as_micros() as u64,
            reported_request_charge: charge, request_charge_scope:"Emulator header only; RU accounting is unsupported, so this value is not real RU or a production cost estimate.".into(),
            query_metrics: metrics, query_metrics_scope:"Raw emulator-returned metrics; placeholders and timing semantics are not production-validated.".into(),
            session_token_sha256: header(&headers,"x-ms-session-token")?.map(|s| crate::digest(s.as_bytes())) };
        ensure!(
            !self.cancelled.load(Ordering::Acquire) && Instant::now() < self.deadline,
            "Cosmos check cancelled or deadline exhausted"
        );
        Ok(Reply {
            observation,
            json: value,
            continuation,
        })
    }
}
fn header(headers: &HeaderMap, name: &str) -> Result<Option<String>> {
    let values = headers.get_all(name).iter().collect::<Vec<_>>();
    ensure!(values.len() <= 1, "Duplicate Cosmos response metadata");
    values
        .first()
        .map(|value| {
            let text = value.to_str().context("Invalid Cosmos response header")?;
            ensure!(
                text.len() <= 8192 && text.is_ascii() && !text.chars().any(char::is_control),
                "Cosmos header bound exceeded"
            );
            crate::safety::require_persistable(text)?;
            Ok(text.to_owned())
        })
        .transpose()
}
fn rows_digest(rows: &[Value], ordered: bool) -> Result<String> {
    let mut encoded = rows
        .iter()
        .map(serde_json::to_string)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !ordered {
        encoded.sort();
    }
    Ok(crate::digest(&serde_json::to_vec(&encoded)?))
}
fn run_case(io: &mut Io<'_>, case: &Case, query: &Query) -> Result<CaseResult> {
    let plan = io.request(Some((query, case, true, None)))?;
    let recognized = plan.observation.status == 200
        && plan
            .json
            .get("partitionedQueryExecutionInfoVersion")
            .and_then(Value::as_u64)
            .is_some()
        && plan.json.get("queryInfo").is_some_and(Value::is_object)
        && plan.json.get("queryRanges").is_some_and(Value::is_array);
    let plan = PlanResult { observation:plan.observation, state:if recognized {"emulator_reported"} else {"unavailable"}.into(), plan:recognized.then_some(plan.json), scope:"Server-returned logical query-plan response only; no production execution-plan accuracy, physical index usage or efficiency claim.".into() };
    let mut pages = vec![];
    let mut rows = vec![];
    let mut next = None;
    let mut seen = BTreeSet::new();
    let mut complete = false;
    let mut page_size_hint_honored = true;
    let mut malformed = false;
    for _ in 0..case.max_pages {
        let reply = io.request(Some((query, case, false, next.as_deref())))?;
        let values = reply.json.get("Documents").and_then(Value::as_array);
        if reply.observation.status != 200
            || values.is_none()
            || values.is_some_and(|v| {
                reply.json.get("_count").and_then(Value::as_u64) != Some(v.len() as u64)
            })
        {
            pages.push(reply.observation);
            malformed = true;
            break;
        }
        let values = values.unwrap();
        page_size_hint_honored &= values.len() <= case.page_size_hint as usize;
        ensure!(
            rows.len() + values.len() <= case.max_rows as usize,
            "Cosmos row bound exceeded; no partial result establishes a pass"
        );
        rows.extend(values.iter().cloned());
        ensure!(
            serde_json::to_vec(&rows)?.len() <= 32768,
            "Cosmos accumulated row byte bound exceeded"
        );
        pages.push(reply.observation);
        next = reply.continuation;
        if let Some(token) = &next {
            if !seen.insert(crate::digest(token.as_bytes())) {
                break;
            }
        } else {
            complete = true;
            break;
        }
    }
    let actual = rows_digest(&rows, case.ordered)?;
    let expected = rows_digest(&case.expected_rows, case.ordered)?;
    let mut checks = vec![check("executed_query_rows", !malformed && complete && actual == expected, "Exact host-frozen row oracle over all bounded pages; duplicate rows retain multiplicity. Missing/repeated continuation or partial results cannot pass.")];
    if io.snapshot.contract.require_query_plan {
        checks.push(check("query_plan_response", recognized, "A recognized plan was returned by this emulator; response presence does not validate plan accuracy."));
    }
    if io.snapshot.contract.require_query_metrics {
        checks.push(check("query_metrics_response", pages.iter().all(|p| p.query_metrics.as_ref().is_some_and(|v| !v.is_empty())), "Every actual query page must return a query-metrics header; emulator scope remains explicit."));
    }
    Ok(CaseResult {
        id: case.id.clone(),
        query_sha256: crate::digest(&serde_json::to_vec(query)?),
        partition: case.partition.clone(),
        expected_rows_sha256: expected,
        rows,
        rows_sha256: actual,
        complete,
        page_size_hint_honored,
        pages,
        plan,
        checks,
    })
}
/// In-process host API only. The caller must establish current authorization
/// before dispatch. Cancellation is checked around every bounded HTTP request.
/// No result from a mutable database is eligible for source-only cache reuse.
pub fn evaluate(snapshot: &Snapshot, cancelled: &AtomicBool) -> Result<Evaluation> {
    let start = Instant::now();
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(0)
        .build()?;
    let mut io = Io {
        client,
        snapshot,
        deadline: start + Duration::from_millis(snapshot.contract.total_timeout_ms),
        cancelled,
        calls: 0,
    };
    let before = io.request(None)?;
    let observed_partition_key = before
        .json
        .get("partitionKey")
        .cloned()
        .and_then(|v| serde_json::from_value::<PartitionKey>(v).ok());
    let mut checks = vec![check("container_partition", before.observation.status == 200 && before.json["id"] == snapshot.contract.container
        && observed_partition_key.as_ref() == Some(&snapshot.contract.partition_key), "Observed NoSQL collection ID and Hash v2 partition specification equal the frozen contract.")];
    let mut cases = vec![];
    if checks[0].status == "passed" {
        for case in &snapshot.contract.cases {
            let query = snapshot
                .queries
                .queries
                .iter()
                .find(|q| q.id == case.id)
                .unwrap();
            cases.push(run_case(&mut io, case, query)?);
        }
    }
    let after = io.request(None)?;
    checks.push(check("container_metadata_stable", before.observation.status == 200 && after.observation.status == 200 && before.json == after.json, "Collection metadata is unchanged across finite checks; this is not an atomic snapshot of document data."));
    checks.push(Check { name:"real_ru".into(), status:"unavailable".into(), detail:"vNext emulator RU accounting is unsupported. Returned request-charge headers are preserved as emulator observations only; no production RU measurement or cost saving is inferred.".into() });
    let failed = checks
        .iter()
        .chain(cases.iter().flat_map(|c| &c.checks))
        .any(|c| c.status == "failed");
    let status = if failed {
        "check_failed"
    } else if snapshot.contract.require_real_ru {
        "unsupported"
    } else {
        "query_checks_passed"
    };
    let result = Evaluation { schema_version:1,status:status.into(),verification_identity:snapshot.identity.clone(),input_sha256:snapshot.query_sha256.clone(),observed_at_unix_seconds:crate::now_ms()/1000,
        collection_before:before.observation,collection_after:after.observation,observed_partition_key,observed_indexing_policy:before.json.get("indexingPolicy").cloned(),cases,checks,actual_http_requests:io.calls,
        elapsed_micros:start.elapsed().as_micros() as u64,real_ru:None,real_ru_state:"unavailable_emulator".into(),model_calls:0,provider_cost_usd:0,qualified:false,promoted:false,authority_granted:false,
        scope:"Point-in-time local emulator observations over explicit host-frozen partition/row oracles. No credentials, provisioning, mutations, cloud, model, general correctness, expert qualification or promotion. Listener/image identity is trusted host configuration, not HTTP attestation. Document reads are not one atomic snapshot; mutable database results must be re-executed for later validation.".into() };
    let encoded = serde_json::to_string(&result)?;
    ensure!(encoded.len() <= 65536, "Cosmos result exceeds output bound");
    crate::safety::require_persistable(&encoded)?;
    Ok(result)
}

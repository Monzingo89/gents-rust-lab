//! Mock transports prove bounds/oracles, not Cosmos correctness or expertise.
use gents_local_mcp::cosmos_check::{
    self,
    runtime::{self, HostEmulator, Snapshot},
    Contract,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

fn contract() -> Contract {
    serde_json::from_value(json!({"schema_version":1,"id":"items","actor":"Worker","query_path":"cosmos/queries.json","api":"nosql_gateway","database":"GentsContract","container":"Items","partition_key":{"paths":["/tenant"],"kind":"Hash","version":2},"request_timeout_ms":1000,"total_timeout_ms":10000,"require_query_plan":true,"require_query_metrics":true,"require_real_ru":false,"cases":[{"id":"alpha","partition":["alpha"],"expected_rows":[{"id":"a1","quantity":2},{"id":"a2","quantity":5}],"ordered":true,"page_size_hint":1,"max_pages":4,"max_rows":8}]})).unwrap()
}
fn query(sql: &str) -> String {
    json!({"schema_version":1,"queries":[{"id":"alpha","sql":sql,"parameters":[{"name":"@tenant","value":"alpha"}]}]}).to_string()
}
fn query_text() -> String {
    query("SELECT c.id,c.quantity FROM c WHERE c.tenant=@tenant AND c.quantity>=0 ORDER BY c.id")
}
fn collection() -> Value {
    json!({"id":"Items","partitionKey":{"paths":["/tenant"],"kind":"Hash","version":2},"indexingPolicy":{"automatic":true,"indexingMode":"consistent"},"_etag":"version-one"})
}
fn plan() -> Value {
    json!({"partitionedQueryExecutionInfoVersion":2,"queryInfo":{},"queryRanges":[{"min":"","max":"FF"}]})
}
#[derive(Clone)]
struct Response {
    status: u16,
    body: String,
    headers: Vec<(String, String)>,
}
fn response(value: Value) -> Response {
    Response {
        status: 200,
        body: value.to_string(),
        headers: vec![
            ("x-ms-request-charge".into(), "1".into()),
            (
                "x-ms-documentdb-query-metrics".into(),
                "totalExecutionTimeInMs=0.15;outputDocumentCount=1".into(),
            ),
        ],
    }
}
fn rows(rows: Value) -> Response {
    response(json!({"_count":rows.as_array().unwrap().len(),"Documents":rows}))
}
#[test]
fn fixture_request_reader_waits_for_delayed_bytes_on_an_accepted_nonblocking_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut accepted, _) = listener.accept().unwrap();
    accepted.set_nonblocking(true).unwrap();
    let reader = thread::spawn(move || read_request(&mut accepted));
    thread::sleep(Duration::from_millis(20));
    let request = "POST /fixture HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";
    // A broken reader may close first; joining must report its panic rather
    // than obscure it with the client's broken-pipe failure.
    let _ = client.write_all(request.as_bytes());
    assert_eq!(reader.join().unwrap(), request);
}
fn read_request(stream: &mut TcpStream) -> String {
    // Nonblocking acceptance bounds the fixture's accept loop. Socket flag
    // inheritance differs by OS; request reads use the existing finite timeout.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut bytes = vec![];
    let mut chunk = [0; 4096];
    loop {
        let n = stream.read(&mut chunk).unwrap();
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..index]);
            let length = header
                .lines()
                .find_map(|s| {
                    s.to_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if bytes.len() >= index + 4 + length {
                break;
            }
        }
    }
    String::from_utf8(bytes).unwrap()
}
type Requests = Arc<Mutex<Vec<String>>>;
fn server(replies: Vec<Response>) -> (HostEmulator, Requests, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = HostEmulator::new(
        listener.local_addr().unwrap().port(),
        "mock-local-service".into(),
    )
    .unwrap();
    let requests = Arc::new(Mutex::new(vec![]));
    let records = requests.clone();
    let worker = thread::spawn(move || {
        for reply in replies {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => return,
                }
            };
            records.lock().unwrap().push(read_request(&mut stream));
            let headers = reply
                .headers
                .iter()
                .map(|(k, v)| format!("{k}: {v}\r\n"))
                .collect::<String>();
            let data=format!("HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",reply.status,reply.body.len(),headers,reply.body);
            let _ = stream.write_all(data.as_bytes());
        }
    });
    (host, requests, worker)
}
fn evaluate(
    c: Contract,
    replies: Vec<Response>,
) -> anyhow::Result<(runtime::Evaluation, Vec<String>)> {
    let (host, requests, worker) = server(replies);
    let result = runtime::evaluate(
        &Snapshot::prepare(c, host, &query_text())?,
        &AtomicBool::new(false),
    );
    worker.join().unwrap();
    let recorded = requests.lock().unwrap().clone();
    Ok((result?, recorded))
}
fn happy() -> Vec<Response> {
    let mut first = rows(json!([{"id":"a1","quantity":2}]));
    first.headers.push((
        "x-ms-continuation".into(),
        "private-pagination-token".into(),
    ));
    vec![
        response(collection()),
        response(plan()),
        first,
        rows(json!([{"id":"a2","quantity":5}])),
        response(collection()),
    ]
}
#[test]
fn actual_paging_and_frozen_partition_oracle_preserve_unavailable_ru() {
    let (result, requests) = evaluate(contract(), happy()).unwrap();
    assert_eq!(result.status, "query_checks_passed");
    assert_eq!(result.actual_http_requests, 5);
    assert_eq!(result.cases[0].pages.len(), 2);
    assert!(result.cases[0].complete);
    assert!(result.real_ru.is_none());
    assert_eq!(result.real_ru_state, "unavailable_emulator");
    assert_eq!(
        result.cases[0].pages[0].reported_request_charge.as_deref(),
        Some("1")
    );
    assert!(!result.qualified && !result.promoted && !result.authority_granted);
    assert_eq!(result.model_calls, 0);
    assert!(requests[0].starts_with("GET /dbs/GentsContract/colls/Items "));
    assert!(requests[1].contains("x-ms-cosmos-is-query-plan-request: True"));
    assert!(requests[2].contains("x-ms-documentdb-partitionkey: [\"alpha\"]"));
    assert!(!requests[2].contains("enablecrosspartition"));
    assert!(requests[3].contains("x-ms-continuation: private-pagination-token"));
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("private-pagination-token"));
    let mut c = contract();
    c.require_real_ru = true;
    assert_eq!(evaluate(c, happy()).unwrap().0.status, "unsupported");
}
#[test]
fn wrong_rows_duplicates_order_and_partial_pages_cannot_pass() {
    for values in [
        json!([{"id":"a1","quantity":2}]),
        json!([{"id":"a2","quantity":5},{"id":"a1","quantity":2}]),
        json!([{"id":"a1","quantity":2},{"id":"a1","quantity":2}]),
    ] {
        let replies = vec![
            response(collection()),
            response(plan()),
            rows(values),
            response(collection()),
        ];
        assert_eq!(
            evaluate(contract(), replies).unwrap().0.status,
            "check_failed"
        );
    }
    let mut c = contract();
    c.cases[0].max_pages = 1;
    let mut replies = happy();
    replies.remove(3);
    let result = evaluate(c, replies).unwrap().0;
    assert_eq!(result.status, "check_failed");
    assert!(!result.cases[0].complete);
    let mut replies = happy();
    replies[3].headers.push((
        "x-ms-continuation".into(),
        "private-pagination-token".into(),
    ));
    assert!(!evaluate(contract(), replies).unwrap().0.cases[0].complete);
}
#[test]
fn cross_partition_is_explicit_and_server_page_hint_is_not_a_hard_bound() {
    let mut c = contract();
    c.cases[0].partition = None;
    c.cases[0].ordered = false;
    let replies = vec![
        response(collection()),
        response(plan()),
        rows(json!([{"id":"a2","quantity":5},{"id":"a1","quantity":2}])),
        response(collection()),
    ];
    let (result, requests) = evaluate(c, replies).unwrap();
    assert_eq!(result.status, "query_checks_passed");
    assert!(!result.cases[0].page_size_hint_honored);
    assert!(requests[2].contains("x-ms-documentdb-query-enablecrosspartition: True"));
    assert!(!requests[2].contains("x-ms-documentdb-partitionkey:"));
    let mut c = contract();
    c.cases[0].max_rows = 2;
    let mut replies = happy();
    replies[2] = rows(json!([1, 2, 3]));
    replies.truncate(3);
    assert!(evaluate(c, replies)
        .unwrap_err()
        .to_string()
        .contains("row bound"));
}
#[test]
fn container_mismatch_and_change_are_failures_without_claiming_data_snapshot() {
    let mut wrong = collection();
    wrong["partitionKey"]["paths"] = json!(["/other"]);
    let (result, requests) =
        evaluate(contract(), vec![response(wrong.clone()), response(wrong)]).unwrap();
    assert_eq!(result.status, "check_failed");
    assert!(result.cases.is_empty());
    assert_eq!(requests.len(), 2);
    let mut replies = happy();
    let mut after = collection();
    after["_etag"] = json!("changed");
    replies[4] = response(after);
    assert_eq!(
        evaluate(contract(), replies).unwrap().0.status,
        "check_failed"
    );
}
#[test]
fn no_plan_or_metrics_response_is_never_upgraded_to_a_measurement() {
    let mut replies = happy();
    replies[1] = rows(json!([]));
    replies[3].headers.clear();
    let result = evaluate(contract(), replies).unwrap().0;
    assert_eq!(result.status, "check_failed");
    assert!(result.cases[0].plan.plan.is_none());
    assert_eq!(result.cases[0].plan.state, "unavailable");
}
#[test]
fn raw_or_escaped_private_rows_are_rejected_before_any_report() {
    for body in [
        r#"{"Documents":["jane@example.com"],"_count":1}"#,
        r#"{"Documents":["jane\u0040example.com"],"_count":1}"#,
    ] {
        let mut private = rows(json!([]));
        private.body = body.into();
        let error = evaluate(
            contract(),
            vec![response(collection()), response(plan()), private],
        )
        .unwrap_err();
        assert!(error.to_string().contains("sensitive data gate"));
        assert!(!error.to_string().contains("jane"));
    }
}
#[test]
fn malformed_duplicates_oversize_redirects_and_http_errors_cannot_pass() {
    for body in [
        r#"{"Documents":[],"Documents":[],"_count":0}"#.to_owned(),
        " ".repeat(32769),
        r#"{"Documents":[],"_count":1}"#.into(),
    ] {
        let mut bad = rows(json!([]));
        bad.body = body;
        let mut replies = vec![response(collection()), response(plan()), bad];
        if replies[2].body.len() < 32769 && !replies[2].body.contains("Documents\":[],\"Documents")
        {
            replies.push(response(collection()));
        }
        assert!(evaluate(contract(), replies).map_or(true, |(r, _)| r.status == "check_failed"));
    }
    let mut bad = response(collection());
    bad.status = 302;
    bad.headers
        .push(("location".into(), "http://127.0.0.1:1/private".into()));
    let (result, requests) = evaluate(contract(), vec![bad, response(collection())]).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(result.status, "check_failed");
    let mut replies = happy();
    replies[2].status = 429;
    replies.truncate(3);
    replies.push(response(collection()));
    let (result, requests) = evaluate(contract(), replies).unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(result.status, "check_failed");
}
#[test]
fn schemas_sources_and_cancellation_cannot_create_authority_or_destinations() {
    for field in ["endpoint", "command", "credential", "headers", "provision"] {
        let mut value = serde_json::to_value(contract()).unwrap();
        value[field] = json!("untrusted");
        assert!(serde_json::from_value::<Contract>(value).is_err());
    }
    let mut omitted = serde_json::to_value(contract()).unwrap();
    omitted["cases"][0]
        .as_object_mut()
        .unwrap()
        .remove("partition");
    assert!(serde_json::from_value::<Contract>(omitted).is_err());
    for sql in [
        "DELETE FROM c",
        "SELECT * FROM c; SELECT * FROM c",
        "SELECT udf.sideEffect(c) FROM c",
        "SELECT udf .sideEffect(c) FROM c",
        "SELECT * FROM c -- comment",
    ] {
        assert!(cosmos_check::QueryFile::parse(&query(sql), &contract()).is_err());
    }
    for path in ["../queries.json", "/queries.json", "secret/.queries.json"] {
        let mut c = contract();
        c.query_path = path.into();
        assert!(c.validate().is_err());
    }
    let escaped = r#"{"schema_version":1,"queries":[{"id":"alpha","sql":"SELECT * FROM c","parameters":[{"name":"@private","value":"jane\u0040example.com"}]}]}"#;
    assert!(cosmos_check::QueryFile::parse(escaped, &contract()).is_err());
    let dup = r#"{"schema_version":1,"queries":[{"id":"alpha","sql":"SELECT * FROM c","parameters":[{"name":"@p","value":{"a":1,"\u0061":2}}]}]}"#;
    assert!(cosmos_check::QueryFile::parse(dup, &contract()).is_err());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let host = HostEmulator::new(
        listener.local_addr().unwrap().port(),
        "cancellation-probe".into(),
    )
    .unwrap();
    let a = Snapshot::prepare(contract(), host.clone(), &query_text()).unwrap();
    let b = Snapshot::prepare(contract(), host, &format!("{} ", query_text())).unwrap();
    assert_ne!(a.identity(), b.identity());
    assert!(runtime::evaluate(&a, &AtomicBool::new(true)).is_err());
    assert!(listener.accept().is_err());
}

#[test]
fn slow_response_is_bounded_and_cancellation_during_io_discards_result() {
    for cancel in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = HostEmulator::new(listener.local_addr().unwrap().port(), "slow-service".into())
            .unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            if cancel {
                flag.store(true, std::sync::atomic::Ordering::Release);
            }
            thread::sleep(Duration::from_millis(200));
            let body = collection().to_string();
            let _ = write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
        });
        let mut c = contract();
        if !cancel {
            c.request_timeout_ms = 100;
        }
        let start = Instant::now();
        let error = runtime::evaluate(
            &Snapshot::prepare(c, host, &query_text()).unwrap(),
            &cancelled,
        )
        .unwrap_err();
        assert!(start.elapsed() < Duration::from_millis(800));
        assert!(error.to_string().contains(if cancel {
            "cancelled"
        } else {
            "transport failed"
        }));
        worker.join().unwrap();
    }
}

#[test]
#[ignore = "explicit local pinned emulator and synthetic fixture required; no install, provisioning or cloud"]
fn actual_local_nosql_partition_rows_plan_metrics_and_negative_oracles() {
    let port = std::env::var("GENTS_COSMOS_EMULATOR_PORT")
        .expect("explicit emulator port")
        .parse()
        .unwrap();
    let database =
        std::env::var("GENTS_COSMOS_TEST_DATABASE").expect("explicit synthetic database");
    let host = HostEmulator::new(port, "pinned-emulator-live-fixture".into()).unwrap();
    let mut c = contract();
    c.database = database;
    let snapshot = Snapshot::prepare(c.clone(), host.clone(), &query_text()).unwrap();
    let pass = runtime::evaluate(&snapshot, &AtomicBool::new(false)).unwrap();
    assert_eq!(pass.status, "query_checks_passed");
    assert!(pass.cases[0].plan.plan.is_some());
    assert!(pass.cases[0]
        .pages
        .iter()
        .all(|p| p.query_metrics.is_some()));
    let partition_sql = query("SELECT c.id,c.quantity FROM c WHERE c.quantity>=0 ORDER BY c.id");
    let alpha = runtime::evaluate(
        &Snapshot::prepare(c.clone(), host.clone(), &partition_sql).unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(alpha.status, "query_checks_passed");
    let mut beta_contract = c.clone();
    beta_contract.cases[0].partition = Some(vec![json!("beta")]);
    beta_contract.cases[0].expected_rows = vec![json!({"id":"b1","quantity":7})];
    let beta = runtime::evaluate(
        &Snapshot::prepare(beta_contract.clone(), host.clone(), &partition_sql).unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(beta.status, "query_checks_passed");
    let mut cross_contract = c.clone();
    cross_contract.cases[0].partition = None;
    cross_contract.cases[0]
        .expected_rows
        .push(json!({"id":"b1","quantity":7}));
    let cross = runtime::evaluate(
        &Snapshot::prepare(cross_contract.clone(), host.clone(), &partition_sql).unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(cross.status, "query_checks_passed");
    let wrong = runtime::evaluate(
        &Snapshot::prepare(
            c.clone(),
            host.clone(),
            &query(
                "SELECT c.id,c.quantity+1 AS quantity FROM c WHERE c.tenant=@tenant AND c.quantity>=0 ORDER BY c.id",
            ),
        )
        .unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(wrong.status, "check_failed");
    let mut mismatch = c.clone();
    mismatch.partition_key.paths = vec!["/other".into()];
    let mismatch = runtime::evaluate(
        &Snapshot::prepare(mismatch, host.clone(), &query_text()).unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(mismatch.status, "check_failed");
    assert!(mismatch.cases.is_empty());
    let mut real = c.clone();
    real.require_real_ru = true;
    let ru = runtime::evaluate(
        &Snapshot::prepare(real, host.clone(), &query_text()).unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(ru.status, "unsupported");
    assert!(ru.real_ru.is_none());
    let private = runtime::evaluate(
        &Snapshot::prepare(
            c.clone(),
            host,
            &query("SELECT VALUE c.contact FROM c WHERE c.id='private' AND c.tenant=@tenant"),
        )
        .unwrap(),
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert!(private.to_string().contains("sensitive data gate"));
    let barrier = std::path::PathBuf::from(
        std::env::var_os("GENTS_COSMOS_MUTATION_BARRIER").expect("host fixture mutation barrier"),
    );
    std::fs::write(barrier.with_extension("ready"), b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !barrier.with_extension("done").exists() {
        assert!(
            Instant::now() < deadline,
            "host fixture did not acknowledge its private data mutation"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let mutated = runtime::evaluate(&snapshot, &AtomicBool::new(false)).unwrap();
    assert_eq!(mutated.status, "check_failed");
    assert_eq!(mutated.verification_identity, pass.verification_identity);
    assert_ne!(mutated.cases[0].rows_sha256, pass.cases[0].rows_sha256);
    assert!(mutated.actual_http_requests >= 4);
    if let Some(path) = std::env::var_os("GENTS_COSMOS_REPORT") {
        std::fs::write(path,serde_json::to_vec_pretty(&json!({"scope":"actual local emulator in-process execution; no MCP/task receipt, production RU or model expertise","frozen_contract":c,"query_file":query_text(),"partition_query_file":partition_sql,"beta_contract":beta_contract,"cross_contract":cross_contract,"host_binding":{"port":port,"instance":"pinned-emulator-live-fixture"},"pass":pass,"partition_header_only":alpha,"other_partition":beta,"cross_partition":cross,"wrong_query":wrong,"partition_mismatch":mismatch,"real_ru_required":ru,"live_data_changed_with_same_source_identity":mutated,"raw_private_rows_rejected_before_report":true})).unwrap()).unwrap();
    }
}

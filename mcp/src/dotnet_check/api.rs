//! Frozen host HTTP oracles. Expected data is never copied into the candidate
//! process or its scratch files. Only an inherited loopback listener is used.
use anyhow::{ensure, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub schema_version: u32,
    pub listen_mode: String,
    pub startup_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub runtime_timeout_ms: u64,
    pub cases: Vec<Case>,
}
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub method: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_json: Option<Value>,
    pub expected_status: u16,
    pub expected_json: Value,
}
impl Contract {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && self.listen_mode == "inherited_loopback_socket",
            "API contract requires version 1 and an inherited loopback socket; no caller endpoint"
        );
        ensure!(
            !self.cases.is_empty()
                && self.cases.len() <= 8
                && (100..=5000).contains(&self.startup_timeout_ms)
                && (100..=2000).contains(&self.request_timeout_ms)
                && self.runtime_timeout_ms <= 15000
                && self.runtime_timeout_ms
                    >= self.startup_timeout_ms
                        + self.request_timeout_ms * (self.cases.len() as u64 - 1)
                        + 100,
            "invalid bounded API request/deadline contract"
        );
        let mut ids = BTreeSet::new();
        let mut bytes = 0;
        for case in &self.cases {
            ensure!(
                super::component(&case.id)
                    && ids.insert(&case.id)
                    && matches!(case.method.as_str(), "GET" | "POST" | "PUT" | "DELETE")
                    && case.path.starts_with('/')
                    && !case.path.starts_with("//")
                    && case.path.len() <= 512
                    && case.path.is_ascii()
                    && !case
                        .path
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '#' | '\\' | ' '))
                    && (100..=599).contains(&case.expected_status),
                "invalid API case identity, method or relative request target"
            );
            if case.method == "GET" {
                ensure!(
                    case.request_json.is_none(),
                    "GET API checks do not send request bodies"
                );
            }
            let data = serde_json::to_string(case)?;
            bytes += data.len();
            ensure!(
                data.len() <= 8192 && bytes <= 32768,
                "API expected/request data bound exceeded"
            );
            crate::safety::require_persistable(&data)?;
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaseResult {
    pub id: String,
    pub status: String,
    pub expected_status: u16,
    pub actual_status: Option<u16>,
    pub expected_json_sha256: String,
    pub actual_json_sha256: Option<String>,
    pub response_bytes: usize,
    pub elapsed_ms: u64,
    pub detail: String,
}
#[derive(Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Host observation time. Exact millisecond timing remains in the canonical
    /// audit; epoch seconds avoid ambiguous card-shaped numeric diagnostics.
    pub observed_at_unix_seconds: u64,
    pub contract_sha256: String,
    pub passed: bool,
    pub http_requests: u32,
    pub cases: Vec<CaseResult>,
    pub runtime: super::sandbox::PhaseResult,
    pub scope: String,
}
#[cfg(any(target_os = "macos", test))]
pub(super) fn pending(contract: &Contract, detail: &str) -> Vec<CaseResult> {
    contract
        .cases
        .iter()
        .map(|c| CaseResult {
            id: c.id.clone(),
            status: "not_run".into(),
            expected_status: c.expected_status,
            actual_status: None,
            expected_json_sha256: crate::digest(
                serde_json::to_string(&c.expected_json).unwrap().as_bytes(),
            ),
            actual_json_sha256: None,
            response_bytes: 0,
            elapsed_ms: 0,
            detail: detail.into(),
        })
        .collect()
}
#[cfg(any(target_os = "macos", test))]
pub(super) fn requests(contract: &Contract, port: u16) -> (Vec<CaseResult>, u32) {
    use std::io::Read;
    let mut results = pending(
        contract,
        "Earlier request or runtime condition prevented execution",
    );
    let mut attempts = 0;
    let client = match reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(0)
        .build()
    {
        Ok(client) => client,
        Err(_) => return (results, attempts),
    };
    for (index, (case, result)) in contract.cases.iter().zip(&mut results).enumerate() {
        let start = std::time::Instant::now();
        attempts += 1;
        let attempt = (|| -> Result<()> {
            let url = format!("http://127.0.0.1:{port}{}", case.path);
            let mut request = client
                .request(reqwest::Method::from_bytes(case.method.as_bytes())?, url)
                .timeout(std::time::Duration::from_millis(if index == 0 {
                    contract.startup_timeout_ms
                } else {
                    contract.request_timeout_ms
                }));
            if let Some(body) = &case.request_json {
                request = request.json(body);
            }
            let response = request.send()?;
            result.actual_status = Some(response.status().as_u16());
            let json_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .is_some_and(|v| {
                    v.to_str().is_ok_and(|v| {
                        v.split(';')
                            .next()
                            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
                    })
                });
            let mut bytes = vec![];
            response.take(8193).read_to_end(&mut bytes)?;
            result.response_bytes = bytes.len();
            ensure!(bytes.len() <= 8192, "response byte limit exceeded");
            let text = std::str::from_utf8(&bytes)?;
            crate::safety::require_persistable(text)?;
            let actual: Value = crate::strict_json::parse(text)?;
            result.actual_json_sha256 =
                Some(crate::digest(serde_json::to_string(&actual)?.as_bytes()));
            ensure!(
                json_type
                    && result.actual_status == Some(case.expected_status)
                    && actual == case.expected_json,
                "status, media type or JSON differs from frozen contract"
            );
            Ok(())
        })();
        result.elapsed_ms = start.elapsed().as_millis() as u64;
        result.status = if attempt.is_ok() { "passed" } else { "failed" }.into();
        // Never persist network error strings or candidate response payloads.
        result.detail=if attempt.is_ok(){"Host observed the exact frozen status and JSON response."}else{"Host request did not establish the exact frozen status/JSON within transport, privacy and response bounds; no response payload logged."}.into();
        if attempt.is_err() {
            break;
        }
    }
    (results, attempts)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn valid() -> Contract {
        Contract {
            schema_version: 1,
            listen_mode: "inherited_loopback_socket".into(),
            startup_timeout_ms: 1000,
            request_timeout_ms: 500,
            runtime_timeout_ms: 5000,
            cases: vec![Case {
                id: "health".into(),
                method: "GET".into(),
                path: "/health".into(),
                request_json: None,
                expected_status: 200,
                expected_json: serde_json::json!({"ok":true}),
            }],
        }
    }
    #[test]
    fn api_contract_cannot_select_an_endpoint_or_unbounded_work() {
        valid().validate().unwrap();
        for path in [
            "http://example.com/",
            "//example.com/",
            "/path#fragment",
            "/bad\\target",
            "/bad\r\nheader",
        ] {
            let mut c = valid();
            c.cases[0].path = path.into();
            assert!(c.validate().is_err());
        }
        let mut c = valid();
        c.cases[0].method = "CONNECT".into();
        assert!(c.validate().is_err());
        let mut c = valid();
        c.listen_mode = "bind-any".into();
        assert!(c.validate().is_err());
        let mut c = valid();
        c.cases = vec![c.cases[0].clone(); 9];
        assert!(c.validate().is_err());
        let mut c = valid();
        c.runtime_timeout_ms = 15001;
        assert!(c.validate().is_err());
    }

    fn serve(response: String) -> (u16, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            let _ = stream.write_all(response.as_bytes());
        });
        (port, server)
    }
    #[test]
    fn http_oracle_rejects_duplicate_keys_wrong_media_and_oversized_bodies() {
        for (media, body) in [
            ("application/json", r#"{"ok":false,"ok":true}"#.to_owned()),
            ("text/plain", r#"{"ok":true}"#.to_owned()),
            (
                "application/json",
                r#"{"card":"4111 1111 1111 1111"}"#.to_owned(),
            ),
            (
                "application/json",
                format!("{{\"padding\":\"{}\"}}", "x".repeat(8193)),
            ),
        ] {
            let response = format!("HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: {media}\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            let (port, server) = serve(response);
            let (cases, attempts) = requests(&valid(), port);
            server.join().unwrap();
            assert_eq!(attempts, 1);
            assert_eq!(cases[0].status, "failed");
            assert!(cases[0].response_bytes <= 8193);
            assert!(!cases[0].detail.contains(&body));
            if body.contains("4111") {
                assert!(cases[0].actual_json_sha256.is_none());
            }
        }
    }
    #[test]
    fn http_oracle_does_not_follow_redirects_or_retry_mutations() {
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let response = format!("HTTP/1.1 307 Temporary Redirect\r\nConnection: close\r\nLocation: http://127.0.0.1:{}/private\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{{\"ok\":true}}",target.local_addr().unwrap().port());
        let (port, server) = serve(response);
        let mut contract = valid();
        contract.cases[0].method = "POST".into();
        contract.cases[0].request_json = Some(serde_json::json!({"input":7}));
        contract.cases.push(contract.cases[0].clone());
        contract.cases[1].id = "later".into();
        let (cases, attempts) = requests(&contract, port);
        server.join().unwrap();
        assert_eq!(attempts, 1);
        assert_eq!(cases[0].actual_status, Some(307));
        assert_eq!(cases[0].status, "failed");
        assert_eq!(cases[1].status, "not_run");
        assert!(target.accept().is_err());
    }
    #[test]
    fn http_oracle_has_a_finite_first_response_deadline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // A retained listener accepts a connection into its backlog but never
        // answers it, exercising the complete request deadline without retries.
        let mut contract = valid();
        contract.startup_timeout_ms = 100;
        let start = std::time::Instant::now();
        let (cases, attempts) = requests(&contract, port);
        assert_eq!(attempts, 1);
        assert_eq!(cases[0].status, "failed");
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
}

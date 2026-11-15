mod support;
use gents_local_mcp::{default_authority, governance, protocol, transports::http, Registry};
use serde_json::{json, Value};
use support::Fixture;

async fn reply(response: reqwest::Response) -> Value {
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let body = response.text().await.unwrap();
    serde_json::from_str(body.lines().find_map(|v| v.strip_prefix("data: ")).unwrap()).unwrap()
}

#[tokio::test]
async fn sse_preserves_governance_and_rejects_network_boundary_attacks() {
    let f = Fixture::new();
    let listener = http::listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let endpoint = format!("http://{}/mcp", listener.local_addr().unwrap());
    let token = uuid::Uuid::new_v4().to_string();
    let server = tokio::spawn(http::serve(
        listener,
        default_authority(f.root.clone()).unwrap(),
        f.scope.clone(),
        token.clone(),
    ));
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let request = |value: Value| {
        client
            .post(&endpoint)
            .bearer_auth(&token)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", protocol::VERSION)
            .json(&value)
    };
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":protocol::VERSION}});
    assert_eq!(
        client
            .post(&endpoint)
            .json(&init)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        request(init.clone())
            .header("Origin", "https://attacker.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        request(init.clone())
            .header("Host", "attacker.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        request(init.clone())
            .header("Forwarded", "host=localhost")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        request(init.clone())
            .header("mcp-protocol-version", "invalid")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let response = request(init).send().await.unwrap();
    let session = response.headers()["mcp-session-id"]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(
        reply(response).await["result"]["protocolVersion"],
        protocol::VERSION
    );
    let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list"});
    assert_eq!(request(list.clone()).send().await.unwrap().status(), 404);
    assert!(reply(
        request(list.clone())
            .header("mcp-session-id", &session)
            .send()
            .await
            .unwrap()
    )
    .await
    .get("error")
    .is_some());
    assert_eq!(
        request(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .header("mcp-session-id", &session)
            .send()
            .await
            .unwrap()
            .status(),
        202
    );
    assert_eq!(
        reply(
            request(list)
                .header("mcp-session-id", &session)
                .send()
                .await
                .unwrap()
        )
        .await["result"],
        Registry::new().list(&f.authority, &f.scope).unwrap()
    );
    let call = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"integer_arithmetic","arguments":{"calculations":[{"id":"one","values":[6,7],"subtract":2}]}}});
    let result = reply(
        request(call.clone())
            .header("mcp-session-id", &session)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        result["result"]["structuredContent"]["value"]["calculations"][0]["difference"],
        11
    );
    assert_eq!(
        result["result"]["structuredContent"]["receipt"]["actor"],
        "Worker"
    );
    assert_eq!(
        result["result"]["structuredContent"]["receipt"]["provider_cost_usd"],
        0
    );
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    let revision = policy.revision;
    policy.revision += 1;
    policy.grants.get_mut("Worker").unwrap().tools.clear();
    governance::save(&db, &f.scope.run.to_string(), &policy, revision).unwrap();
    assert_eq!(
        reply(
            request(call)
                .header("mcp-session-id", &session)
                .send()
                .await
                .unwrap()
        )
        .await["result"]["isError"],
        true
    );
    let forged = json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"integer_arithmetic","arguments":{},"actor":"Reader"}});
    assert_eq!(
        reply(
            request(forged)
                .header("mcp-session-id", &session)
                .send()
                .await
                .unwrap()
        )
        .await["error"]["code"],
        -32602
    );
    assert_eq!(
        request(json!({}))
            .header("mcp-session-id", &session)
            .body("x".repeat(65537))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    assert_eq!(
        client
            .get(&endpoint)
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        405
    );
    assert_eq!(client.get(&endpoint).send().await.unwrap().status(), 401);
    assert!(http::listen("0.0.0.0:0".parse().unwrap()).await.is_err());
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn managed_http_child_uses_same_receipts_and_cancellation() {
    // This test process has no other callers of invoke or transport environment.
    let mut f = Fixture::new();
    f.scope.expected_revision = Some(1);
    let bin = std::path::Path::new(env!("CARGO_BIN_EXE_gents-local-mcp"));
    let folder = f.root.join("target/debug");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(bin, folder.join("gents-local-mcp")).unwrap();
    std::env::set_var("GENTS_MCP_TRANSPORT", "sse");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let result = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "integer_arithmetic",
        &json!({"calculations":[{"id":"managed","values":[4,5],"subtract":1}]}),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["result"]["calculations"][0]["difference"], 8);
    assert_eq!(result["mcp_receipt"]["actor"], f.scope.actor);
    let db = f.db();
    let mut policy = governance::load(&db, &f.scope.run.to_string())
        .unwrap()
        .unwrap();
    policy.revision = 2;
    governance::save(&db, &f.scope.run.to_string(), &policy, 1).unwrap();
    let stale = gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "integer_arithmetic",
        &json!({"calculations":[{"id":"stale","values":[1],"subtract":0}]}),
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(stale["ok"], false);
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(gents_local_mcp::client::invoke(
        &f.root,
        &f.scope,
        "integer_arithmetic",
        &json!({}),
        &cancel
    )
    .await
    .is_err());
    std::env::remove_var("GENTS_MCP_TRANSPORT");
}

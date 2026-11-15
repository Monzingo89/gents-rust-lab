//! One process-bound session over loopback Streamable HTTP with SSE responses.
//! This is not the deprecated 2024 two-endpoint HTTP+SSE transport.
use crate::{authority::SqliteAuthority, protocol, Registry, Scope};
use anyhow::{ensure, Result};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde_json::Value;
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;

struct Host {
    registry: Registry,
    authority: SqliteAuthority,
    scope: Scope,
    token: String,
    endpoint: String,
    session_id: String,
    lifecycle: Mutex<(bool, bool)>,
}
fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}
fn boundary(host: &Host, headers: &HeaderMap) -> Result<(), StatusCode> {
    for name in [
        "host",
        "authorization",
        "mcp-protocol-version",
        "mcp-session-id",
    ] {
        if headers.get_all(name).iter().count() > 1 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if header(headers, "host") != Some(host.endpoint.as_str())
        || headers.contains_key("origin")
        || headers.contains_key("x-forwarded-host")
        || headers.contains_key("forwarded")
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if header(headers, "authorization") != Some(format!("Bearer {}", host.token).as_str()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if headers.contains_key("mcp-protocol-version")
        && header(headers, "mcp-protocol-version") != Some(protocol::VERSION)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}
async fn unsupported(State(host): State<Arc<Host>>, headers: HeaderMap) -> Response {
    match boundary(&host, &headers) {
        Err(code) => code.into_response(),
        Ok(()) => (StatusCode::METHOD_NOT_ALLOWED, [("allow", "POST")]).into_response(),
    }
}
async fn message(State(host): State<Arc<Host>>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(code) = boundary(&host, &headers) {
        return code.into_response();
    }
    let accept = header(&headers, "accept").unwrap_or("");
    let accepts = |kind| accept.split(',').any(|v| v.trim() == kind);
    if !accepts("application/json") || !accepts("text/event-stream") {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    }
    if header(&headers, "content-type").and_then(|v| v.split(';').next())
        != Some("application/json")
    {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let request: Value = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let Ok(mut lifecycle) = host.lifecycle.lock() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    if lifecycle.0 && header(&headers, "mcp-session-id") != Some(host.session_id.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Each process is launched for exactly one trusted run/actor/phase. HTTP
    // clients cannot supply identity, open extra sessions or install grants.
    let mut session = protocol::Session::new(&host.registry, &host.authority, &host.scope);
    session.initialized = lifecycle.0;
    session.ready = lifecycle.1;
    let response = session.handle(request);
    *lifecycle = (session.initialized, session.ready);
    match response {
        None => StatusCode::ACCEPTED.into_response(),
        Some(mut response) => {
            if response.to_string().len() > 65535 {
                response = protocol::error(
                    response["id"].clone(),
                    -32603,
                    "Tool output exceeds bounded frame",
                );
            }
            (
                [
                    ("content-type", "text/event-stream"),
                    ("cache-control", "no-store"),
                    ("mcp-session-id", host.session_id.as_str()),
                ],
                format!("event: message\ndata: {response}\n\n"),
            )
                .into_response()
        }
    }
}
/// Bind before advertising; only exact IPv4 loopback is supported in this adapter.
pub async fn listen(bind: SocketAddr) -> Result<TcpListener> {
    ensure!(
        bind.ip() == std::net::Ipv4Addr::LOCALHOST,
        "MCP HTTP requires 127.0.0.1"
    );
    Ok(TcpListener::bind(bind).await?)
}
pub async fn serve(
    listener: TcpListener,
    authority: SqliteAuthority,
    scope: Scope,
    token: String,
) -> Result<()> {
    ensure!(
        listener.local_addr()?.ip() == std::net::Ipv4Addr::LOCALHOST,
        "MCP listener must be loopback"
    );
    ensure!(
        token.len() >= 32
            && token.len() <= 200
            && token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "MCP bearer must be a private random token of 32..200 characters"
    );
    scope.validate()?;
    let endpoint = listener.local_addr()?.to_string();
    let host = Arc::new(Host {
        registry: Registry::new(),
        authority,
        scope,
        token,
        endpoint,
        session_id: uuid::Uuid::new_v4().to_string(),
        lifecycle: Mutex::new((false, false)),
    });
    let router = Router::new()
        .route("/mcp", post(message).get(unsupported).delete(unsupported))
        .layer(DefaultBodyLimit::max(65536))
        .with_state(host);
    axum::serve(listener, router).await?;
    Ok(())
}

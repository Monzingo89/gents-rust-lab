//! Browser/authority checks for trusted-host local authentication. This is not
//! authentication of hostile OS clients or a replacement for Entra identity.
use anyhow::{Context, Result};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{collections::BTreeSet, net::SocketAddr};

#[derive(Clone)]
pub struct LocalIngress {
    authorities: BTreeSet<String>,
    origins: BTreeSet<String>,
}

fn origin(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return None;
    }
    let serialized = match url.scheme() {
        "http" | "https" => url.origin().ascii_serialization(),
        "tauri" if url.host_str() == Some("localhost") && url.port().is_none() => {
            "tauri://localhost".into()
        }
        _ => return None,
    };
    // Origin headers are origins, not URLs with paths or suffixes. Require
    // canonical configuration rather than silently broadening an allowlist.
    (serialized == value).then_some(serialized)
}

impl LocalIngress {
    pub fn new(bind: SocketAddr, configured: &[HeaderValue]) -> Result<Self> {
        anyhow::ensure!(
            bind.ip().is_loopback(),
            "local ingress requires loopback binding"
        );
        let mut authorities = BTreeSet::new();
        for host in [
            "localhost".to_string(),
            "127.0.0.1".into(),
            "[::1]".into(),
            if bind.is_ipv6() {
                format!("[{}]", bind.ip())
            } else {
                bind.ip().to_string()
            },
        ] {
            authorities.insert(format!("{host}:{}", bind.port()));
            if bind.port() == 80 {
                authorities.insert(host);
            }
        }
        let mut origins = authorities
            .iter()
            .map(|a| format!("http://{a}"))
            .collect::<BTreeSet<_>>();
        for configured in configured {
            origins.insert(origin(configured.to_str()?).context("CORS_ORIGINS requires canonical explicit origins, not wildcards, credentials or paths")?);
        }
        Ok(Self {
            authorities,
            origins,
        })
    }

    pub fn allows(&self, headers: &HeaderMap, uri: &Uri) -> bool {
        let mut hosts = headers.get_all("host").iter();
        let host = hosts.next().and_then(|v| v.to_str().ok());
        if hosts.next().is_some() {
            return false;
        }
        let absolute = uri.authority().map(|a| a.as_str());
        if host
            .zip(absolute)
            .is_some_and(|(h, a)| !h.eq_ignore_ascii_case(a))
        {
            return false;
        }
        let Some(authority) = host.or(absolute) else {
            return false;
        };
        if !self.authorities.contains(&authority.to_ascii_lowercase()) {
            return false;
        }
        let mut supplied = headers.get_all("origin").iter();
        if let Some(value) = supplied.next() {
            if supplied.next().is_some() {
                return false;
            }
            return value
                .to_str()
                .ok()
                .and_then(origin)
                .is_some_and(|o| self.origins.contains(&o));
        }
        if !uri.path().starts_with("/api/") {
            return true;
        }
        // Legacy browser fallback. Do not honor X-Forwarded-* as authority.
        if let Some(referer) = headers.get("referer") {
            let Some(url) = referer
                .to_str()
                .ok()
                .and_then(|v| reqwest::Url::parse(v).ok())
            else {
                return false;
            };
            if !url.username().is_empty()
                || url.password().is_some()
                || !self.origins.contains(&url.origin().ascii_serialization())
            {
                return false;
            }
        }
        let mut sites = headers.get_all("sec-fetch-site").iter();
        if let Some(site) = sites.next() {
            if sites.next().is_some() {
                return false;
            }
            return matches!(site.to_str(), Ok("same-origin" | "none"))
                || (site == "same-site" && headers.contains_key("referer"));
        }
        // CLI and native local integrations do not send browser headers. Their
        // OS identity remains trusted; this boundary does not authenticate them.
        true
    }
}

pub async fn enforce(
    State(policy): State<Option<LocalIngress>>,
    request: Request,
    next: Next,
) -> Response {
    if policy
        .as_ref()
        .is_some_and(|p| !p.allows(request.headers(), request.uri()))
    {
        return (
            StatusCode::FORBIDDEN,
            "Local request authority/origin denied",
        )
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> LocalIngress {
        LocalIngress::new(
            "127.0.0.1:8788".parse().unwrap(),
            &[
                HeaderValue::from_static("http://localhost:1420"),
                HeaderValue::from_static("tauri://localhost"),
            ],
        )
        .unwrap()
    }
    #[test]
    fn local_authority_origin_and_metadata_fail_closed() {
        let p = policy();
        let api: Uri = "/api/probe".parse().unwrap();
        let mut h = HeaderMap::new();
        h.insert("host", HeaderValue::from_static("127.0.0.1:8788"));
        assert!(p.allows(&h, &api));
        for value in [
            "http://127.0.0.1:8788",
            "http://localhost:1420",
            "tauri://localhost",
        ] {
            h.insert("origin", value.parse().unwrap());
            assert!(p.allows(&h, &api));
        }
        for value in [
            "null",
            "https://untrusted.invalid",
            "http://localhost:1420.untrusted.invalid",
            "http://localhost:1420/",
            "http://user@localhost:1420",
            "http://localhost:1420?ignored",
        ] {
            h.insert("origin", value.parse().unwrap());
            assert!(!p.allows(&h, &api));
        }
        h.remove("origin");
        h.insert("host", HeaderValue::from_static("untrusted.invalid:8788"));
        h.insert(
            "x-forwarded-host",
            HeaderValue::from_static("127.0.0.1:8788"),
        );
        assert!(!p.allows(&h, &api));
        h.insert("host", HeaderValue::from_static("127.0.0.1:8788"));
        assert!(!p.allows(
            &h,
            &"http://untrusted.invalid:8788/api/probe".parse().unwrap()
        ));
        for site in ["cross-site", "same-site", "unknown"] {
            h.insert("sec-fetch-site", site.parse().unwrap());
            assert!(!p.allows(&h, &api));
        }
        h.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        assert!(p.allows(&h, &api));
        h.insert(
            "referer",
            HeaderValue::from_static("https://untrusted.invalid/page"),
        );
        assert!(!p.allows(&h, &api));
        h.insert(
            "referer",
            HeaderValue::from_static("http://localhost:1420/page?q=1"),
        );
        assert!(p.allows(&h, &api));
        h.append("host", HeaderValue::from_static("localhost:8788"));
        assert!(!p.allows(&h, &api));
        assert!(!p.allows(&HeaderMap::new(), &api));
        for bad in ["*", "null", "https://untrusted.invalid/path"] {
            assert!(
                LocalIngress::new("127.0.0.1:8788".parse().unwrap(), &[bad.parse().unwrap()])
                    .is_err()
            );
        }
        assert!(LocalIngress::new("0.0.0.0:8788".parse().unwrap(), &[]).is_err());
    }
    #[tokio::test]
    async fn rejected_requests_never_reach_handler_and_allowed_preflight_works() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bind = listener.local_addr().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let handler_count = count.clone();
        let approved = HeaderValue::from_static("http://localhost:1420");
        let app = axum::Router::new()
            .route(
                "/api/probe",
                axum::routing::any(move || {
                    let c = handler_count.clone();
                    async move {
                        c.fetch_add(1, Ordering::SeqCst);
                        "ok"
                    }
                }),
            )
            .layer(
                tower_http::cors::CorsLayer::new()
                    .allow_origin([approved.clone()])
                    .allow_methods([axum::http::Method::POST]),
            )
            .layer(axum::middleware::from_fn_with_state(
                Some(LocalIngress::new(bind, &[approved]).unwrap()),
                enforce,
            ));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("http://{bind}/api/probe");
        assert_eq!(
            client.get(&url).send().await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .post(&url)
                .header("origin", "http://localhost:1420")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        for (key, value) in [
            ("host", "untrusted.invalid"),
            ("origin", "https://untrusted.invalid"),
            ("origin", "null"),
            ("sec-fetch-site", "cross-site"),
            ("referer", "https://untrusted.invalid/page"),
        ] {
            assert_eq!(
                client
                    .post(&url)
                    .header(key, value)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            client
                .request(reqwest::Method::OPTIONS, &url)
                .header("origin", "http://localhost:1420")
                .header("access-control-request-method", "POST")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
        server.abort();
    }
}

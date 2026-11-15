use anyhow::{bail, Context, Result};
use axum::http::HeaderMap;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

pub struct Auth {
    pub tenant: Option<String>,
    pub audience: Option<String>,
    pub native_client: Option<String>,
    pub local: bool,
    keys: Mutex<(Instant, Instant, HashMap<String, DecodingKey>)>,
}
#[derive(Clone, Deserialize)]
struct Claims {
    tid: String,
    oid: String,
    scp: Option<String>,
    azp: Option<String>,
}
impl Auth {
    pub fn new(local: bool) -> Result<Self> {
        let a = Self {
            local,
            tenant: std::env::var("TENANT_ID").ok(),
            audience: std::env::var("API_CLIENT_ID").ok(),
            native_client: std::env::var("NATIVE_CLIENT_ID").ok(),
            keys: Mutex::new((
                Instant::now() - Duration::from_secs(7200),
                Instant::now() - Duration::from_secs(7200),
                HashMap::new(),
            )),
        };
        if !local {
            for v in [&a.tenant, &a.audience, &a.native_client] {
                let value = v.as_deref().context("Entra configuration missing")?;
                uuid::Uuid::parse_str(value).context("Entra IDs must be UUIDs")?;
            }
        }
        Ok(a)
    }
    pub async fn owner(&self, headers: &HeaderMap, client: &reqwest::Client) -> Result<String> {
        if self.local {
            return Ok("local-developer".into());
        }
        let token = headers
            .get("authorization")
            .context("missing bearer token")?
            .to_str()?
            .strip_prefix("Bearer ")
            .context("missing bearer scheme")?;
        if token.len() > 16384 {
            bail!("access token too large");
        }
        let header = decode_header(token)?;
        if header.alg != Algorithm::RS256 {
            bail!("algorithm rejected");
        }
        let kid = header.kid.context("missing kid")?;
        let tenant = self.tenant.as_deref().unwrap();
        let mut keys = self.keys.lock().await;
        if keys.0.elapsed() > Duration::from_secs(3600) || !keys.2.contains_key(&kid) {
            if keys.1.elapsed() < Duration::from_secs(30) {
                bail!("signing-key refresh throttled");
            }
            keys.1 = Instant::now();
            let v: Value = client
                .get(format!(
                    "https://login.microsoftonline.com/{tenant}/discovery/v2.0/keys"
                ))
                .timeout(Duration::from_secs(15))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let mut updated = HashMap::new();
            for k in v["keys"].as_array().context("invalid JWKS")? {
                if let (Some(id), Some(n), Some(e)) =
                    (k["kid"].as_str(), k["n"].as_str(), k["e"].as_str())
                {
                    updated.insert(id.to_string(), DecodingKey::from_rsa_components(n, e)?);
                }
            }
            *keys = (Instant::now(), Instant::now(), updated);
        }
        let key = keys.2.get(&kid).context("unknown signing key")?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.audience.as_deref().unwrap()]);
        validation.set_issuer(&[format!("https://login.microsoftonline.com/{tenant}/v2.0")]);
        validation.validate_nbf = true;
        let claims = decode::<Claims>(token, key, &validation)?.claims;
        if claims.tid != tenant || claims.azp.as_deref() != self.native_client.as_deref() {
            bail!("tenant or client rejected");
        }
        if !claims
            .scp
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .any(|s| s == "Harness.Run")
        {
            bail!("scope required");
        }
        let oid = uuid::Uuid::parse_str(&claims.oid).context("invalid object ID")?;
        Ok(format!("{}:{}", claims.tid, oid))
    }
}

#[cfg(test)]
mod claim_validation_tests {
    use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
    use serde_json::{json, Value};

    #[test]
    fn malformed_optional_time_claims_cannot_bypass_validation() {
        let secret = b"only a synthetic unit test signing fixture";
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_nbf = true;
        validation.validate_aud = false;
        validation.required_spec_claims.clear();
        let check = |claims: Value| {
            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(secret),
            )
            .unwrap();
            decode::<Value>(&token, &DecodingKey::from_secret(secret), &validation)
        };
        assert!(check(json!({"exp":4_000_000_000_u64,"nbf":0})).is_ok());
        assert!(check(json!({"exp":4_000_000_000_u64,"nbf":4_000_000_000_u64})).is_err());
        for malformed in [json!("future"), json!(true), json!([]), json!({})] {
            assert!(check(json!({"exp":4_000_000_000_u64,"nbf":malformed})).is_err());
            assert!(check(json!({"exp":malformed,"nbf":0})).is_err());
        }
    }
}

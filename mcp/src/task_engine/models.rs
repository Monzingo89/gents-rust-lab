//! Discovery is metadata, never model admission, loading or proof of locality.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Supported,
    Unsupported,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    pub provider: String,
    pub endpoint: Option<String>,
    pub path: Option<String>,
    pub revision: Option<String>,
    pub digest: Option<String>,
    pub quantization: Option<String>,
    pub disk_bytes: Option<u64>,
    pub context_tokens: Option<u64>,
    pub capabilities: BTreeMap<String, Support>,
    pub thinking_controls: Vec<Value>,
    pub downloaded: bool,
    pub serveable: Support,
    pub loaded: Support,
    pub reachability: String,
    pub locality: String,
    pub aliases: Vec<String>,
    pub provenance: String,
    pub evaluated_profiles: BTreeSet<String>,
}
pub fn files(root: &Path) -> Result<Vec<ModelEntry>> {
    let dir = root.join("models");
    if !dir.exists() {
        return Ok(vec![]);
    }
    ensure!(!dir.is_symlink(), "model root symlink rejected");
    let mut result = vec![];
    let mut pending = vec![(dir, 0)];
    let mut visited = 0;
    while let Some((folder, depth)) = pending.pop() {
        for entry in std::fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            let info = std::fs::symlink_metadata(&path)?;
            visited += 1;
            ensure!(visited <= 20000, "bounded inventory exceeded");
            if info.is_symlink() {
                continue;
            }
            if info.is_dir() {
                if depth < 5 && entry.file_name() != ".cache" {
                    pending.push((path, depth + 1));
                }
                continue;
            }
            if !info.is_file()
                || !matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("gguf" | "safetensors" | "onnx")
                )
            {
                continue;
            }
            let relative = path.strip_prefix(root)?.to_string_lossy().to_string();
            let provider = match path.extension().and_then(|s| s.to_str()) {
                Some("gguf") => "local",
                Some("onnx") => "onnx",
                _ => "mlx_or_transformers",
            };
            result.push(ModelEntry{id:relative.clone(),provider:provider.into(),endpoint:None,path:Some(relative),revision:None,digest:None,quantization:None,disk_bytes:Some(info.len()),context_tokens:None,
                capabilities:BTreeMap::from([("generation".into(),Support::Unknown)]),thinking_controls:vec![],downloaded:info.len()>0,serveable:Support::Unknown,loaded:Support::Unknown,reachability:"not_probed_file_inventory".into(),locality:"local_artifact_inference_not_started".into(),aliases:vec![],provenance:"bounded file metadata only; shards/media/adapters are artifacts, not additional base models".into(),evaluated_profiles:BTreeSet::new()});
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}
/// Only trusted, explicitly configured endpoints are probed. Redirects disabled;
/// neither a loopback URL nor API compatibility proves local execution.
pub async fn ollama(endpoint: &str, locality_attested: bool) -> Result<Vec<ModelEntry>> {
    let url = reqwest::Url::parse(endpoint)?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "invalid configured discovery endpoint"
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let tags = bounded_json(
        client
            .get(format!("{}/api/tags", endpoint.trim_end_matches('/')))
            .send()
            .await?,
    )
    .await?;
    let ps = match client
        .get(format!("{}/api/ps", endpoint.trim_end_matches('/')))
        .send()
        .await
    {
        Ok(r) => bounded_json(r).await.ok(),
        Err(_) => None,
    };
    let mut entries = vec![];
    let models = tags["models"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("provider model catalog missing"))?;
    ensure!(models.len() <= 512, "provider catalog bound exceeded");
    for model in models {
        let Some(id) = model["name"].as_str().filter(|s| s.len() <= 512) else {
            continue;
        };
        let show = match client
            .post(format!("{}/api/show", endpoint.trim_end_matches('/')))
            .json(&json!({"model":id,"verbose":false}))
            .send()
            .await
        {
            Ok(r) => bounded_json(r).await.ok(),
            Err(_) => None,
        };
        let cloud = show
            .as_ref()
            .is_some_and(|s| s.get("remote_host").is_some() || s.get("remote_model").is_some())
            || id.ends_with("-cloud");
        let capabilities = show.as_ref().and_then(|s| s["capabilities"].as_array());
        let mut caps = BTreeMap::new();
        for (name, wire) in [
            ("generation", "completion"),
            ("embedding", "embedding"),
            ("tools", "tools"),
            ("vision", "vision"),
            ("thinking", "thinking"),
        ] {
            caps.insert(
                name.into(),
                match capabilities {
                    Some(c) if c.iter().any(|v| v == wire) => Support::Supported,
                    Some(_) => Support::Unsupported,
                    None => Support::Unknown,
                },
            );
        }
        let context = show
            .as_ref()
            .and_then(|s| s["model_info"].as_object())
            .and_then(|o| o.iter().find(|(k, _)| k.ends_with(".context_length")))
            .and_then(|(_, v)| v.as_u64());
        entries.push(ModelEntry {
            id: id.into(),
            provider: "ollama".into(),
            endpoint: Some(endpoint.into()),
            path: None,
            revision: model["modified_at"].as_str().map(str::to_owned),
            digest: model["digest"].as_str().map(str::to_owned),
            quantization: model["details"]["quantization_level"]
                .as_str()
                .map(str::to_owned),
            disk_bytes: model["size"].as_u64(),
            context_tokens: context,
            capabilities: caps,
            thinking_controls: show
                .as_ref()
                .and_then(|s| s["thinking"]["values"].as_array())
                .cloned()
                .unwrap_or_default(),
            downloaded: !cloud,
            serveable: if show.is_some() {
                Support::Supported
            } else {
                Support::Unknown
            },
            loaded: match &ps {
                Some(ps)
                    if ps["models"]
                        .as_array()
                        .is_some_and(|models| models.iter().any(|m| m["name"] == id)) =>
                {
                    Support::Supported
                }
                Some(_) => Support::Unsupported,
                None => Support::Unknown,
            },
            reachability: "reachable".into(),
            locality: if cloud {
                "cloud_proxy"
            } else if locality_attested {
                "operator_attested_local"
            } else {
                "unknown_even_on_loopback"
            }
            .into(),
            aliases: vec![],
            provenance: "Ollama tags/show/ps metadata; no generation or loading".into(),
            evaluated_profiles: BTreeSet::new(),
        });
    }
    Ok(entries)
}
async fn bounded_json(mut response: reqwest::Response) -> Result<Value> {
    ensure!(
        response.status().is_success(),
        "discovery endpoint rejected request"
    );
    let mut bytes = vec![];
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 2_000_000,
            "discovery response bound exceeded"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}
/// No profile triggers residency. Evaluated quality is an admission requirement;
/// size alone is never enough. Rank by measured full-path latency when present.
pub fn select<'a>(
    entries: &'a [ModelEntry],
    profile: &str,
    local_required: bool,
    measured_ms: &BTreeMap<String, u64>,
) -> Result<&'a ModelEntry> {
    entries
        .iter()
        .filter(|m| {
            m.evaluated_profiles.contains(profile)
                && m.capabilities.get("generation") == Some(&Support::Supported)
                && m.serveable == Support::Supported
                && (!local_required
                    || matches!(
                        m.locality.as_str(),
                        "owned_local_runtime" | "operator_attested_local"
                    ))
        })
        .min_by_key(|m| {
            (
                measured_ms.get(&m.id).copied().unwrap_or(u64::MAX),
                m.disk_bytes.unwrap_or(u64::MAX),
            )
        })
        .ok_or_else(|| anyhow::anyhow!("no evaluated feasible model; discovery is not admission"))
}
pub fn default_roots(repo: &Path) -> Vec<PathBuf> {
    vec![repo.join("models")]
}
pub fn configured(root: &Path) -> Result<Vec<ModelEntry>> {
    let mut entries = files(root)?;
    let dir = root.join("config");
    if !dir.is_dir() {
        return Ok(entries);
    }
    fn visit(
        value: &Value,
        out: &mut Vec<(String, String, Option<String>, Option<String>)>,
        depth: usize,
    ) {
        if depth > 12 {
            return;
        }
        if let (Some(provider), Some(id)) = (value["provider"].as_str(), value["model"].as_str()) {
            if id.len() <= 512 {
                let endpoint = value["url"]
                    .as_str()
                    .and_then(|s| reqwest::Url::parse(s).ok())
                    .map(|mut u| {
                        u.set_query(None);
                        u.set_fragment(None);
                        let _ = u.set_username("");
                        let _ = u.set_password(None);
                        u.to_string()
                    });
                out.push((
                    provider.into(),
                    id.into(),
                    value["path"].as_str().map(str::to_owned),
                    endpoint,
                ));
            }
        }
        match value {
            Value::Array(a) => {
                for v in a {
                    visit(v, out, depth + 1)
                }
            }
            Value::Object(o) => {
                for v in o.values() {
                    visit(v, out, depth + 1)
                }
            }
            _ => {}
        }
    }
    let mut configured = vec![];
    let mut count = 0;
    for file in std::fs::read_dir(dir)? {
        let path = file?.path();
        let info = std::fs::symlink_metadata(&path)?;
        if info.is_symlink()
            || !info.is_file()
            || info.len() > 1_000_000
            || path.extension().and_then(|s| s.to_str()) != Some("json")
        {
            continue;
        }
        count += 1;
        ensure!(count <= 256, "config discovery bound exceeded");
        if let Ok(value) = serde_json::from_slice::<Value>(&std::fs::read(&path)?) {
            visit(&value, &mut configured, 0);
        }
    }
    let mut seen = BTreeSet::new();
    for (provider, id, path, endpoint) in configured {
        if !seen.insert((provider.clone(), id.clone(), path.clone(), endpoint.clone())) {
            continue;
        }
        if let Some(existing) = entries
            .iter_mut()
            .find(|m| m.path == path && path.is_some())
        {
            existing.aliases.push(format!("{provider}/{id}"));
            continue;
        }
        entries.push(ModelEntry{id,provider:provider.clone(),endpoint,path,revision:None,digest:None,quantization:None,disk_bytes:None,context_tokens:None,capabilities:BTreeMap::new(),thinking_controls:vec![],downloaded:false,serveable:Support::Unknown,loaded:Support::Unknown,reachability:if matches!(provider.as_str(),"local"|"mlx"){"configured_artifact_missing_or_not_in_inventory"}else{"configured_host_not_probed"}.into(),locality:if matches!(provider.as_str(),"local"|"mlx"){"owned_local_runtime"}else if provider=="mock"{"simulated_fixture"}else{"remote_or_unknown"}.into(),aliases:vec![],provenance:"existing config provider/model identity; URL credentials/query omitted; no host contacted".into(),evaluated_profiles:BTreeSet::new()});
    }
    for entry in &mut entries {
        entry.aliases.sort();
        entry.aliases.dedup();
    }
    Ok(entries)
}

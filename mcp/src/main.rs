//! Identity is supplied by the trusted launching host, never by tool JSON.
use anyhow::{bail, Context, Result};
use gents_local_mcp::{default_authority, protocol, Phase, Registry, Scope};
use std::{collections::BTreeMap, path::PathBuf};
async fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--implementation-catalog"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&Registry::new().catalog())?
        );
        return Ok(());
    }
    if args.len() < 12 || args.len() % 2 != 0 {
        bail!("usage: gents-local-mcp --root REPOSITORY --company COMPANY --owner OWNER --run UUID --actor ACTOR --phase planner|worker");
    }
    let mut flags = BTreeMap::new();
    for pair in args.as_chunks::<2>().0 {
        if ![
            "--transport",
            "--bind",
            "--root",
            "--company",
            "--owner",
            "--run",
            "--actor",
            "--phase",
            "--policy-revision",
        ]
        .contains(&pair[0].as_str())
            || flags.insert(pair[0].as_str(), pair[1].clone()).is_some()
        {
            bail!("invalid or duplicate launch flag");
        }
    }
    let get = |key| flags.get(key).cloned().context("missing launch scope");
    let phase = match get("--phase")?.as_str() {
        "planner" => Phase::Planner,
        "worker" => Phase::Worker,
        _ => bail!("invalid phase"),
    };
    let authority = default_authority(PathBuf::from(get("--root")?))?;
    let scope = Scope {
        company: get("--company")?,
        owner: get("--owner")?,
        run: get("--run")?.parse()?,
        actor: get("--actor")?,
        phase,
        expected_revision: flags
            .get("--policy-revision")
            .map(|v| v.parse())
            .transpose()?,
    };
    scope.validate()?;
    match flags
        .get("--transport")
        .map(String::as_str)
        .unwrap_or("stdio")
    {
        "stdio" => {
            if flags.contains_key("--bind") {
                bail!("--bind requires --transport sse");
            }
        }
        "sse" => {
            let listener = gents_local_mcp::transports::http::listen(
                flags
                    .get("--bind")
                    .map(String::as_str)
                    .unwrap_or("127.0.0.1:0")
                    .parse()?,
            )
            .await?;
            let token =
                std::env::var("GENTS_MCP_TOKEN").context("SSE requires private GENTS_MCP_TOKEN")?;
            eprintln!(
                "{}",
                serde_json::json!({"mcp_endpoint":format!("http://{}/mcp",listener.local_addr()?)})
            );
            return gents_local_mcp::transports::http::serve(listener, authority, scope, token)
                .await;
        }
        _ => bail!("transport must be stdio or sse"),
    }
    let registry = Registry::new();
    let mut session = protocol::Session::new(&registry, &authority, &scope);
    protocol::serve(
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut session,
    )
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Local MCP stopped safely: {error}");
        std::process::exit(1);
    }
}

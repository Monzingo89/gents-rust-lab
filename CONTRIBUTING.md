# Contribute to Metivolve / Gents

This is an MIT-licensed local agent-company research project. Start with the
[README](README.md), [architecture](docs/00-start-here.md) and
[MCP cookbook](docs/121-owned-mcp-cookbook.md). Model weights are not distributed;
operators supply models and review their licenses and hardware requirements.

Open an issue describing the problem and a reproducible example, or send a small
pull request with the resulting behavior and relevant checks. Never include real
customer records, medical/financial documents, credentials, model weights or local
`data/` contents. Use synthetic fixtures with clear provenance.

```sh
npm ci
bash scripts/check.sh
```

The check script runs the Rust workspace, deterministic local tests and website
build. It does not require model weights, paid providers or cloud credentials.
Native GPU/model experiments are separate, explicitly invoked development work.
Keep original weights immutable, freeze before/after criteria, retain failures and
reject regressions. A training loss decrease is not a task-quality result.

For website-only changes run `npm run test:site`. Test production metadata with
`npm run site:build:production && python3 scripts/verify_introduction.py`.
The hosted site explains the repository; it does not run the local company.

Keep MCP tool implementations independent of transports, enforce actor permissions
in trusted code, bound inputs and effects, and add meaningful native tests. Update
the tool cookbook when schemas or capabilities change. Runtime and security claims
must say what was actually verified and on which platform.

Report a suspected vulnerability using the private route in [SECURITY.md](SECURITY.md).
Do not put exploit data or secrets in a public issue. Contributions retain their
existing notices; see [third-party licensing](THIRD_PARTY_NOTICES.md).

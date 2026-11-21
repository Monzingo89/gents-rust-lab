# Repository agent entry

Canonical repository skills live in `.agents/skills/<name>/SKILL.md`:

- `repo-simplify`: reversible consolidation with preserved behavior.
- `harness-routing`: cache-first task routing and host admission.
- `context-governance`: task authority and source-bound handoffs.
- `building-interactions`: runtime-backed building UI and accounting displays.
- `local-evaluate`: bounded local checks and honest measurement.

Read the relevant skill when doing that work. These development skills do not install company runtime grants; runtime-approved skills remain a separate governed catalog.

Start with current Git state and [the architecture baseline](docs/162-local-execution-baseline.md). Preserve user changes. Use actual local entrypoints, no implicit paid runtime fallback, and distinguish generated/mocked output from verified task completion. Keep original evaluators and governance intact.

Preserve the [verified monorepo boundaries](docs/174-monorepo-architecture.md) during subsequent refactoring. Keep the harness/orchestrator, MCP, runtime adapters, configuration and building UI in this Git repository. The root Cargo workspace owns `crates/harness` and `mcp`; `src-tauri` remains an intentionally excluded desktop package with its own lockfile and reviewed dependency patches. Frontend sources, third-party dependencies and downloaded models do not become Cargo members. Keep model weights, secrets, caches and build outputs ignored. Report affected paths and history before deleting nested Git metadata, moving an external repository or changing these workspace boundaries.

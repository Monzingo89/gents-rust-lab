# First-party Rust and Python lint cleanup

Inspection: November 21, 2026, starting HEAD `4a72d483ea8c1ea428e6cd042c315966f1766f05`, branch `codex/feature/public-research-view`. The checkout already contained extensive Python edits. This cleanup preserves those edits and the [monorepo boundaries](174-monorepo-architecture.md).

## Configuration and scope

- Root Cargo workspace: `gents-harness` and `gents-local-mcp`. The intentionally excluded `src-tauri` desktop package was checked separately. None declares optional package features; all-target/all-feature checks cover the supported local macOS build graph. Linux/iOS runtime behavior is not established by those checks.
- No repository or ancestor Ruff/Mypy configuration was found. Ruff 0.17.6 reports that it uses its defaults. No rule selection, ignore list, `noqa`, type-ignore or compiler-warning suppression was added.
- First-party Python inventory: 166 Git-tracked/unignored files under `scripts`, `mcp/media`, the owner evaluator and source evidence. Third-party retained GTK backports, downloaded models, virtual environments, caches, generated run workspaces and frontend dependencies are outside mutation scope.
- Existing environments are split: `.venv` uses Python 3.15 and supplies Ruff; `.venv-inference` and `.venv-media` use Python 3.12.13 with different optional runtime dependencies. Tests and type checking use the appropriate installed environment; no model weights or model-runtime dependencies were downloaded.
- The bundled editor Mypy 1.15.0 and newer editor Markdown stubs were incompatible. A temporary development-only environment supplies Mypy 2.4.1, `types-Markdown==3.11.3.20261108` and `types-reportlab==5.1.0.20261104`. Project runtime environments remain unchanged. Official stub contents were not edited. Fixture module aliases point to the actual first-party candidate/test sources, matching their sandbox import names.

## Applied cleanup

Ran Clippy's fix mode and rustfmt for the root workspace and desktop package, excluding vendor formatting. Both were already diagnostic-free; no first-party Rust implementation change was required. Ran Ruff safe fixes and formatting over mutable first-party Python sources, then reviewed the remaining changes individually.

The Python fixes include explicit `check=False` where subprocess exit status is deliberately inspected, loop-variable binding for callbacks, equivalent dictionary/set/pairwise operations, combined context managers, import-spec checks, and dictionary/list guards in negative integration fixtures. All 166 Python files pass Ruff's formatting check. Fifty-four first-party scripts with shebangs are now executable; their CLI bodies and shebangs remain present.

Concrete failures repaired:

- Restore the `port` export from `task_engine_live.py`, used by the intake, MLX and workspace live probes. An explicit public export list keeps that compatibility visible to the linter and type checker. Existing import-contract tests exercise the probes without inference.
- Restore the `maximum_path` and `compare_timing` imports in the optional audio experiment/interpolation helpers.
- Carry the native planning revision explicitly between turns instead of referencing `planned` before its assignment.
- Give heterogeneous report values their actual scalar union type, distinguish preparation/intake/policy fixture copies, validate nested fixture containers, and annotate decoded JSON history without removing its existing source/contract assertions.
- Validate optional regex matches, executables, sitemap text and module loaders before use. Supported successful inputs retain the same behavior.

## Visible unresolved diagnostics

Ruff was reduced from 155 to **37 diagnostics**; it still exits nonzero. These findings remain checked and are not suppressed:

| Count | Scope | Reason left visible |
| ---: | --- | --- |
| 16 | Frozen source evidence and accepted observer/host fixtures | The findings are loop closures, type-exception recommendations, a nested context manager and a string predicate. These files are bound into recorded acceptance hashes. This slice does not rewrite frozen evaluator/candidate bytes or manufacture replacement historical hashes. |
| 12 | Broad exception handlers (`BLE001`) | These are runtime/privacy/error-containment boundaries, including the best-effort SQLite destructor. Narrowing them or logging arbitrary exceptions can change containment or disclose private diagnostics. |
| 7 | Type-exception recommendations (`TRY004`) in mutable validators | Existing schema/checkpoint validators reject malformed values through a `ValueError` contract. Changing that public exception behavior requires caller/negative-case review. |
| 1 | SQLite destructor (`S110`) | Deliberately best-effort cleanup currently suppresses destructor failures; this overlaps one broad-handler finding. A logging policy change is outside behavior-preserving formatting. |
| 1 | Python sandbox (`PLW1509`) | Its `preexec_fn` installs child resource limits. Removing it would weaken isolation. Replacing it safely needs a separately tested child-startup design. |

Mypy checks all 166 files using their real runtime dependencies and official available stubs. The 136 main files and eight standalone fixture/evidence files pass. Two diagnostics remain among the 22 media files: installed `onnxruntime` and `mflux.models.flux2.variants` have no stubs/`py.typed` metadata (`import-untyped`). No fabricated stubs or missing-import suppression was added. These checks use normal Mypy coverage; they do not establish type safety throughout every unannotated function body or prove model/backend competence.

The starting tree already disagreed with recorded SHA-256 values for public research, observer sources, observer acceptance/company tests and Python host checks. These mismatches were verified before mutations. The nine protected source files remain byte-identical to their starting state. The introduction site's evidence-gated build therefore needs independent reconciliation of those pre-existing edits; acceptance hashes and the builder were left intact.

The [repo-simplify skill](../.agents/skills/repo-simplify/SKILL.md) says, "Never alter fixed evaluators to make a candidate pass." It does not explicitly forbid formatting. Preserving the hash-bound files here is an evidence-preservation decision under the user's functionality/unrelated-change constraints, rather than permission to replace recorded acceptance with a fresh hash after formatting.

## Validation

- Root and desktop rustfmt plus strict offline locked all-target/all-feature Clippy: passed, including fix mode.
- Python tests: **276 passed** across the installed environments: 272 in media and four document-renderer tests in inference. The initial single-environment failures exposed missing optional packages and the live-probe export regression; no tests were disabled to resolve them.
- `cargo test --workspace --all-features --offline --locked`: **712 passed**, 34 opt-in/platform-dependent tests ignored.
- The standard eleven-check offline suite passed against rebuilt current binaries. Nine additional bounded smokes passed: coordination, factory, owned discovery, review protocol, proposal feedback, inference budgets, both artifact-gate modes and audio inspection. The [private integration report](../data/lint-cleanup/03f9162f-d9cf-41ed-949a-0746ff04e47b/report.json) retains command results and log hashes. No failure was hidden by changing a fixture expectation.
- Office frontend `npm run build`: passed using the already-installed Vite 7.3.7. A separately appearing `package.json` version edit was left untouched; this build does not qualify those newly requested dependency versions.
- Ruff formatting, Python syntax, preservation checks and `git diff --check` are checked separately from the intentionally nonzero lint/type results.

No model generation, model download, paid runtime fallback, service deployment or production-company restart was requested or performed. CLI/static/unit/fixture results do not prove real-model quality, Linux/iOS behavior or desktop GUI operation.

The [retained lint evidence](evidence/lint-cleanup-2026-11-21.json) lists all 37 Ruff findings with paths/lines, the two external typing diagnostics, checked environments, 43 Python content edits from this cleanup, 54 executable-bit repairs and preservation checks. Root/desktop Cargo manifests, lockfiles, Rust implementation sources, third-party vendor sources and the nine protected evidence sources match their starting hashes.

An independent content edit to `scripts/acquire_audio_training_checkpoint.py` also appeared, replacing the Certifi CA store with the platform default. That edit was left untouched, separate from this cleanup's executable-bit repair on the file. The preceding cleanup test results do not qualify that late trust-store change. The independent `package.json` version edits were likewise preserved.

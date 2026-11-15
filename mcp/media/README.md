# Local media engines

The native MCP tools are `image_generate` (PNG) and `audio_generate` (English speech WAV). The existing `audio_inspect` remains a separate read-only measurement tool. Users describe the deliverable; an independent local output judge classifies text/image/audio/video combinations during the architecture review. There is no media mode selector. Classification, engine availability, the frozen tool plan and owner authorization are separate checks.

The tool adapters live in `mcp/src/functions/media_generate.rs`. `mcp/src/media.rs` owns the backend interface, platform adapter and binary publication boundary. `worker.py` is embedded in the native executable, receives only a bounded host-built packet and uses fixed installed engines. A different platform/engine can implement the backend without changing transport, contracts, permissions or user prompts.

## Explicit setup

On the currently supported Apple Silicon/macOS host:

```sh
uv venv --python 3.12 .venv-media
uv pip install --python .venv-media/bin/python -r mcp/media/requirements.lock
HF_HUB_DISABLE_IMPLICIT_TOKEN=1 .venv-media/bin/python scripts/setup-local-media.py
cargo build --workspace --locked
```

Setup uses internet access to download public models at the exact revisions and file hashes in `config/media.local.json`. Models and the environment stay ignored by Git. No token/key is needed. Setup is separate from tool execution; tools never download, install or use a paid fallback. The manifest is compiled into the native executable; changing its pins requires a rebuild and a new runtime bundle. The current isolated adapter fails closed on other operating systems until a corresponding sandbox adapter is implemented.

## Engines and limits

- Image: MFLUX 0.24.0 (MIT), FLUX.2 Klein 4B 4-bit weights (Apache-2.0), four steps, an explicit seed and 256/384/512 pixel dimensions. Approximately 4.62 GB of reviewed local model files. No reference-image editing, transparent-image generation, training or video is claimed by this adapter.
- Speech: Piper 1.8.0 (GPL-3.0 engine) in a separate process, generic English LJ Speech voice; its model card identifies the training dataset as public domain. Approximately 114 MB of local voice files. Up to 600 characters. No cloning, music or transcription.
- Output: <=8 MiB, new confined `.png`/`.wav` paths only. Publication uses descriptor-relative no-replace linking; existing files and frozen inputs are protected. Results bind artifact/request/model-manifest hashes and $0 provider fees. A valid file is not semantic acceptance.
- Execution: worker phase plus current actor grant and approved active job. Inference helpers use the shared harness exclusive lease, plus a nonblocking cross-process media lease. Free-memory floors are observed (8 GiB image, 512 MiB speech). The MLX working-set setting is a guideline, not a hard GPU-memory quota.
- Isolation: a fixed Python environment, cleared credentials, offline library flags, default-deny macOS filesystem/network/process sandbox, bounded stdin/stdout, 150-second CPU and 160-second wall deadlines, 180-second MCP caller deadline. A stdlib-only watchdog acknowledges startup before the render request is sent, and kills the owned process group on parent-pipe EOF or its deadline. Parent cancellation cannot leave the model running after the MCP host exits. No user-defined executable or module is accepted.
- The Apple Metal compiler needs its specific OS cache subtree and scoped sandbox extensions. These are limited to `com.apple.metalfe`, following the mechanism documented in Chromium's [sandbox policy](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/sandbox/policy/mac/common.sb); arbitrary cache/home access is not granted.
- Failure diagnostics are bounded to 16 KiB in owner-only files under ignored `data/media-diagnostics`. Raw diagnostics are not sent to a model or MCP client. Inspect a retained path/hash after uncertain effects; never blindly replay a write.

## Checks and evidence

```sh
cargo test -p gents-local-mcp --test media_generate --locked
cargo test -p gents-local-mcp --test media_generate actual_offline_audio_and_image_generators_publish_hash_bound_files --locked -- --ignored
cargo test -p gents-local-mcp media_sandbox_denies_private_files_network_and_shell --lib --locked -- --ignored
python3 scripts/media-routing-native-probe.py --run-local
```

The opt-in checks use already-installed local engines/models, no paid inference and no network during generation. The native tool test has an explicitly approved fixture job and exact actor grants; retained artifacts/receipts are under `data/mcp-basement-verification`. It is not evidence that a real user task was completed. The six-case output-routing matrix is a development regression set, not a held-out benchmark or general accuracy guarantee. It tests an installed small local model against the source-bound output-judge instruction; production architecture review uses its owner-listed judge model.

The current verification also passed a real three-call assessment with the configured distinct CEO/judge weights (CEO architecture opinion, minimal output-intent judgment, architecture review), canonical readback and factory-reference binding. Its source-bound seeded text fixture created no task job. The retained report is `data/architecture-native-probes/6f842d6a-ece5-492a-aa40-7a636fed8b6e/report.json`. The configured larger judge is slower than the small-model development probe; neither timing is a user-task completion forecast.

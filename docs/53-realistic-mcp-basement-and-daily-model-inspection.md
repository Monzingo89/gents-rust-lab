# Realistic MCP basement and daily offline model inspection

B1 now uses a local photorealistic backdrop matching the other eleven floors. The asset is `web/assets/rooms/floor-0.png`; its full prompt is saved in `floor-0-prompt.txt` and the twelve-room manifest. The built-in image generation tool produced the decorative asset during repository development. It is not a model/tool service used by company tasks. No API-key or CLI fallback was used.

The image shows enclosed server racks, routed cabling, ventilation, walnut work surfaces and brass lighting, with blank screens and no baked-in status values, people or health indicators. The native fallback remains available and its decorative rack indicators are neutral. Existing code separately renders the live catalog, actor state and observability. The same photograph loader/cache and exterior preview path serve B1 as the other rooms; no separate renderer or UI framework was introduced.

Chrome was refreshed after the asset build. B1's realistic cutaway was visually inspected, then the idle office was updated and refreshed again. Its catalog shows ten functions over local SSE, including `rust_verify`, whose source/phase/effects and input/result schema disclosures are visible. The office remains at `http://127.0.0.1:8788/?autoconnect=1`; no task was launched through the browser. Twenty existing layout/asset tests and the Vite production build passed. The generated asset is 1586 × 992 and is bundled locally.

## Offline inspection boundaries

`model_security_inspection.py` performs read-only artifact inspection. `model_security_scan.py` owns daily scheduling, single-scan locking and private durable reports. `model-security-service.mjs` is the launcher lifecycle adapter; the normal `run-harness.mjs` starts it and stops it with the company. The scanner receives a small environment whitelist, excluding HOME, company credentials and provider tokens. No model code, framework, network request or inference is executed.

The filesystem adapter uses descriptor-relative reads, no-follow directory/file opens, regular singly linked files and before/after file identity checks. It rejects symlink roots and reports linked/special files and symlink subdirectories. The scan streams actual SHA-256 weight hashes; it compares reviewed roster pins without learning unknown files as trusted baselines. It also checks bounded GGUF/Safetensors headers and tensor offsets, bounded model configuration, remote-code configuration flags and executable/unsafe serialization extensions. Parser diagnostics and model content are not copied into reports. This is a bounded inspection, not full serialization validation or a proof against malicious model behavior.

The office is currently running through this launcher with the scanner process attached; its initial check returned `NOT_DUE` against the retained full scan. The scanner checks at launcher start and hourly while the company remains running, with full scans due every 24 hours after completion. It catches up after downtime at the next launch; it cannot run while the computer is off. A process-held lock prevents concurrent scans; only a completed scan updates `latest.json`. Timestamped immutable scan files and the latest snapshot are private files under `data/model-security/`. Scanning does not grant model admission, delete files or promote weights. The current secure filesystem adapter supports POSIX systems; a Windows adapter remains required and unsupported operation fails closed.

Manual commands:

```sh
python3 scripts/model_security_scan.py          # scan if due
python3 scripts/model_security_scan.py --force  # explicitly rescan
python3 scripts/model_security_scan.py --watch  # remain running and check hourly
```

The first real scan, `bca1905916484a5bbaa89c11ef9e46af`, hashed eighteen weight files (97.8 GiB) in about fifty seconds. Ten matched reviewed pins; eight were unreviewed. There were no reviewed hash mismatches. Its status is intentionally `REVIEW_REQUIRED`, not a security or admission PASS. A subsequent invocation returned `NOT_DUE` without rehashing. No paid provider charges or local-model tokens were used by the scanner.

Seven focused Python tests cover pin mismatches/unknown weights, hostile links/files/configuration, invalid tensor ranges, private durable daily reports, single-scan locking and output-root symlink rejection. The Node service/environment tests verify credential exclusion, shutdown behavior and preservation of an explicitly selected local model port. The checks are included in `scripts/check.sh`.

## Limits and next work

Hash equality does not establish trustworthy training data, absence of backdoors, known-CVE coverage, model license suitability or regulatory compliance. An offline scan cannot discover newly published advisories. The scan is observable evidence and a review queue; admission must independently enforce selected-model provenance and freshness. Privileged same-user tampering of the repository/roster/reports is outside this local integrity mechanism.

Hardware-aware acquisition, safe automatic model selection, portable isolation, training/promotion qualification and industry/modality benchmark coverage remain active goal work. Existing synthetic reconciliation and one recovered Rust fixture do not establish banking, healthtech, audio, video or extreme-task competence. Future comparisons must separate fresh uninterrupted runs, repaired/resumed runs, failed planning, local tokens, elapsed time and frontier development cost. No universal capability or vulnerability-free claim is supported.

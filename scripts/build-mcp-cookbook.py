"""Render project-owned implementation metadata, never agent grants or tool calls."""

import argparse
import hashlib
import json
from datetime import date
from pathlib import Path

RECIPES = {
    "python_check": (
        "Retest copied Python source after a repair",
        "The owner freezes explicit source paths, a top-level unittest suite/hash, actor and installed interpreter hash in python_check_contracts. Supply contract_id; the host binds the current source identity before independent review. Each invocation executes fresh copied sources. frozen_inputs pins immutable dependencies; scratch_bytes defaults to zero and may admit bounded isolated temporary I/O. No shell, network, run-workspace writes, package installation or downloads. Inspect reported_fail diagnostics and repair actual source before checking again. Python can spoof a zero exit: reported_pass is not independent acceptance, source promotion or company completion. Currently macOS only with owner-configured GENTS_PYTHON.",
    ),
    "image_generate": (
        "Create a small image locally",
        "Freeze an approved confined output_path and grant image_generate to the assigned worker. Supply prompt, dimensions, seed and the bounded step count. The installed pinned FLUX model runs locally with no network or downloads. Inspect the created PNG hash and dimensions; a valid PNG does not establish that the image matches the prompt. Existing destinations and frozen inputs cannot be overwritten.",
    ),
    "audio_generate": (
        "Speak a short script locally",
        "Freeze an approved confined output_path and grant audio_generate to the assigned worker. Supply the exact text and supported local voice. The installed pinned Piper model creates a WAV without network or downloads. Inspect the actual receipt and WAV properties; the generic voice is not cloned, and speech quality still needs listening. Existing destinations and frozen inputs cannot be overwritten.",
    ),
    "audio_inspect": (
        "Check the structure of a small WAV",
        "Freeze its confined path and complete file hash, then inspect PCM16 frame/rate/channel statistics. Inspect the native receipt and exact source hash. Do not infer speech, content, health, clipping or transcription from these statistics.",
    ),
    "files": (
        "Read, write, list or rename a task file",
        "Use explicit operation grants. Read bounded UTF-8 pages, or use read metadata_only=true without pagination to fingerprint bounded text/binary without exposing its body. Worker-only rename requires files:rename plus files:read, the exact source expected_sha256, an absent destination and existing parents. It preserves regular-file bytes/mode up to 16 MiB; no overwrite, links, directory moves or copy/delete fallback. Frozen inputs cannot be moved or replaced. Lost/error receipts require BOTH paths/hash and canonical audit reconciliation; the harness retains unconfirmed rename intents and stops.",
    ),
    "host_resources": (
        "Check local capacity before choosing a model",
        "Read the host estimate and any observed Linux process limit. Keep unobserved GPU memory unknown, preserve a context/runtime allowance and fail capacity admission when required process limits are unknown. This does not download or select a model.",
    ),
    "integer_arithmetic": (
        "Calculate exact integer rows",
        "Supply bounded calculations with stable IDs, integer values and an explicit subtract field. Use the actual returned row/field through approved bindings; do not calculate an expected answer during CEO planning.",
    ),
    "memory_append": (
        "Keep an attributable advisory or experiment",
        "Supply the bounded body and kind, plus source references where available. The host supplies sender/run/phase/epoch. A record called fact is not automatically verified truth, a permission, or completed-task promotion.",
    ),
    "memory_search": (
        "Retrieve currently allowed memory",
        "Supply a literal keyword query and bounded limit. The native policy filters senders/run/epoch before limiting. Check truncated previews and source identities; retrieval cannot expand communication grants.",
    ),
    "owned_rust_call": (
        "Use an admitted pure function",
        "The owner freezes actor, source/API hashes, function and binding first. Supply binding_id and positional arguments with the documented wire types. Approved dependencies use canonical producer receipts; do not paste invented upstream answers. Inspect unchanged source and independent task QA after native success.",
    ),
    "rust_verify": (
        "Check a quarantined pure Rust candidate",
        "The owner freezes the separate acceptance executable and hash for the assigned worker. Supply only the confined source_path. Review actual compiler/test diagnostics, current source hash and full readback. Failure remains failure; the tool does not promote source or certify the task.",
    ),
    "skill_list": (
        "Discover assigned instructions",
        "Use an empty object. The result lists only reviewed skills assigned to this actor and its process-bound planner/worker phase. The implementation catalog is not a substitute for these assignments.",
    ),
    "skill_read": (
        "Read one assigned instruction set",
        "Use an ID returned by the authorized skill list. Instructions can guide work but never install policy, authorize a tool, broaden data scope or override the frozen task.",
    ),
    "table_aggregate": (
        "Group exact integer data",
        "Supply flat scalar rows, group_by, value_field and an explicit filter or null. Preserve duplicate rows and zero-total groups. Feed actual /groups or /total through approved source bindings; unsupported floats/coercion/expressions fail rather than being guessed.",
    ),
    "table_join": (
        "Combine two bounded flat tables",
        "Choose inner or left based on the confirmed treatment of unmatched records. Supply exact typed keys and explicit output columns. Preserve duplicate matches and native output ordering. Use actual /rows downstream; missing fields and output overflow must stop the task.",
    ),
    "table_read_json": (
        "Import a complete bounded JSON table",
        "Freeze the confined input path and, when required, complete source hash. Check returned source identity/size and preserve row order, duplicates and scalar types. A truncated page, duplicate decoded key or unsupported nested/float value imports nothing.",
    ),
}
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("catalog", type=Path)
parser.add_argument("--snapshot-date", required=True, type=date.fromisoformat)
parser.add_argument(
    "--output",
    type=Path,
    default=Path(__file__).resolve().parents[1] / "docs/121-owned-mcp-cookbook.md",
)
args = parser.parse_args()
catalog = json.loads(args.catalog.read_text())
assert catalog["server"] == "gents-local-mcp"
tools = catalog["tools"]
assert len(tools) == len(RECIPES) and {t["name"] for t in tools} == set(RECIPES), (
    "Review recipes before adding/removing native tools"
)
summary = "\n".join(
    f"| [{t['name']}](#{t['name'].replace('_', '-')}) | {RECIPES[t['name']][0]} | {', '.join(t['phases'])} |"
    for t in tools
)
sections = []
for tool in tools:
    title, recipe = RECIPES[tool["name"]]
    descriptor_hash = hashlib.sha256(
        json.dumps(
            tool, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode()
    ).hexdigest()
    details = {
        k: tool[k]
        for k in [
            "name",
            "description",
            "source",
            "phases",
            "read_only",
            "deterministic",
            "external_effects",
            "runtime_requirements",
            "data_input_fields",
            "file_input_fields",
            "provider_cost_usd",
            "inputSchema",
            "resultSchema",
        ]
    }
    sections.append(
        f"## {tool['name'].replace('_', ' ')}\n\n**Recipe: {title}.** {recipe}\n\nImplementation: [{tool['source']}](../{tool['source']}). Descriptor snapshot SHA-256: `{descriptor_hash}`.\n\n<details>\n<summary>Exact parameters, result shape, limits and implementation metadata</summary>\n\n```json\n{json.dumps(details, indent=2, ensure_ascii=False)}\n```\n\n</details>\n"
    )
header = """# Owned native MCP cookbook

This covers all {tool_count} tools in the local native implementation catalog captured on 2026-11-13. It is a source-derived recipe/reference, not a list of an agent's grants, proof that every task works, a security attestation or a migration claim for legacy Python tools. Tool calls target $0 provider fees; models, hardware and electricity are separate.

## Before calling any tool

Confirm the task/definition of done, review the exact tool/source/API identity, freeze the ordinary intake, approve its digest and install current actor/phase permissions. The trusted host binds company, owner, run, actor and phase to the process. Model JSON cannot replace those values or acquire authority.

Discover implementation metadata in the B1 MCP room or `GET /api/mcp/catalog`. An initialized MCP `tools/list` response is instead filtered to the current actor, phase and policy. Do not launch an ad hoc privileged server or copy a bearer token into a prompt/log.

For the existing local launcher, choose the transport without changing domain tool code:

```sh
GENTS_MCP_TRANSPORT=stdio npm run company
# Or choose the loopback Streamable HTTP/SSE adapter:
GENTS_MCP_TRANSPORT=sse npm run company
```

Keep the terminal running. The launcher refuses an incompatible already-running mode. Current HTTP/SSE is one private child/session and one `/mcp` endpoint; it is not a public server or the deprecated paired SSE/message transport. See [transport boundaries](39-mcp-basement-and-local-ceo.md). A mode switch never broadens permissions or migrates legacy Python tools.

Native receipts record source/action/result identities and current policy. A successful call is not final task acceptance. Rejected/uncertain effects need reconciliation; never replay a write merely because a response was lost. Use owner-frozen artifact/dependency contracts for multi-tool work, not prose or manually substituted upstream answers.

## Choose the smallest sufficient capability

| Native name | Recipe | Implementation phases |
| --- | --- | --- |
"""
footer = """
## What is not included

`finish` is a host submission controller, not a native tool in this catalog. It cannot self-approve a task. Local PNG generation and generic text-to-speech are explicit tools above. PDF extraction, image conversion, audio transcription and video creation are not implied by these tools or model filenames; those expansions remain unimplemented. Media generation currently needs the installed macOS isolation adapter, pinned local models and dependencies; see [local media setup](../mcp/media/README.md). The output judge infers requested media from the prompt without a media-mode switch, but cannot grant tools or hide a missing engine. Model-download/sprite-generation tools and the governed builder/progress API remain follow-on work.

Generated-code verification/invocation currently requires the installed macOS isolation adapter; unsupported platforms fail closed. Native confined file support and resource observations have their own platform limits. Models and open-source components retain their own licenses/provenance obligations.

The exact schemas below are implementation declarations, not complete semantic preconditions. Frozen task contracts, current native policy, source confinement and independent acceptance remain authoritative. Refresh this document from a reviewed current catalog when implementations change; the generator refuses an unreviewed tool-name set. The current catalog snapshot is not a release attestation.
"""
header = header.replace("{tool_count}", str(len(tools))).replace(
    "captured on 2026-11-13", f"captured on {args.snapshot_date}"
)
args.output.write_text(header + summary + "\n\n" + "\n".join(sections) + footer)
print(f"Wrote {len(tools)} native recipes and exact schemas to {args.output}")

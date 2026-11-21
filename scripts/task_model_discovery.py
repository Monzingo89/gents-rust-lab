#!/usr/bin/env python3
"""Bounded metadata discovery across existing local providers; never loads weights."""

import argparse
import importlib.util
import json
import shutil
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(
        name, ROOT / "scripts" / (name + ".py")
    )
    if spec is None or spec.loader is None:
        raise ImportError(f"Could not load module spec for {name}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def metadata_json(path, limit=1_000_000):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        return None
    try:
        return json.loads(path.read_text())
    except (ValueError, OSError, UnicodeError):
        return None


def safe_endpoint(value):
    parsed = urllib.parse.urlsplit(value)
    if parsed.scheme not in ("http", "https") or parsed.username or parsed.password:
        return None
    return urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, parsed.path, "", ""))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def get_json(endpoint, route):
    request = urllib.request.Request(
        endpoint.rstrip("/") + route, headers={"Accept": "application/json"}
    )
    with urllib.request.build_opener(NoRedirect).open(request, timeout=2) as response:
        body = response.read(2_000_001)
        if len(body) > 2_000_000:
            raise ValueError("catalog response bound exceeded")
        return json.loads(body)


def discover(root=ROOT, probe=False, endpoints=()):
    inventory = load("model_inventory").inventory(root=root)
    media = load("model_improvement_inventory").inventory(root=root)
    records = []
    indexed = {row["path"]: row for row in inventory["artifacts"]}
    aliases = {}
    endpoints = set(endpoints)

    def visit(value, origin, depth=0):
        if depth > 12:
            return
        if isinstance(value, dict):
            if isinstance(value.get("provider"), str) and isinstance(
                value.get("model"), str
            ):
                alias = {
                    key: value[key]
                    for key in ("provider", "model", "path")
                    if key in value
                }
                alias["configured_in"] = origin
                if isinstance(value.get("url"), str):
                    alias["endpoint"] = safe_endpoint(value["url"])
                    if alias["endpoint"] and value["provider"] in (
                        "ollama",
                        "localai",
                        "llamacpp",
                    ):
                        endpoints.add(alias["endpoint"])
                aliases.setdefault(
                    value.get(
                        "path", "provider:" + value["provider"] + "/" + value["model"]
                    ),
                    [],
                ).append(alias)
            for v in value.values():
                visit(v, origin, depth + 1)
        elif isinstance(value, list):
            for v in value:
                visit(v, origin, depth + 1)

    for path in sorted((root / "config").glob("*.json")):
        value = metadata_json(path)
        if value is not None:
            visit(value, str(path.relative_to(root)))
    for model in media["models"]:
        for artifact in model["artifacts"]:
            path = artifact["path"]
            meta = indexed.get(path, {})
            records.append(
                {
                    "id": path,
                    "provider": "owned_local_artifact",
                    "downloaded": artifact["bytes"] > 0,
                    "disk_bytes": artifact["bytes"],
                    "format": artifact["format"],
                    "modality": model["modality"],
                    "revision_claim": meta.get("cached_commit_claim"),
                    "digest_claim": meta.get("cached_etag_claim"),
                    "digest_measured": None,
                    "quantization_claim": meta.get("metadata", {}).get(
                        "general.file_type"
                    ),
                    "context_tokens_claim": next(
                        (
                            v
                            for k, v in meta.get("metadata", {}).items()
                            if k.endswith(".context_length")
                        ),
                        None,
                    ),
                    "name_claim": meta.get("name_claim"),
                    "architecture": meta.get("architecture"),
                    "aliases": aliases.pop(path, []),
                    "serveable": "unknown_until_runtime_probe",
                    "loaded": "not_observed",
                    "locality": "local_artifact",
                    "evaluation": "not_admitted_by_discovery",
                }
            )
    manifests = Path.home() / ".ollama/models/manifests"
    blobs = manifests.parent / "blobs"
    ollama = []
    if manifests.is_dir():
        for n, path in enumerate(sorted(manifests.rglob("*"))):
            if n > 10000:
                raise ValueError("Ollama inventory bound exceeded")
            value = metadata_json(path)
            if not isinstance(value, dict):
                continue
            weights = [
                l
                for l in value.get("layers", [])
                if l.get("mediaType", "").endswith(".model")
            ]
            ollama.append(
                {
                    "id": str(path.relative_to(manifests)),
                    "provider": "ollama",
                    "layers": [
                        {
                            "digest": l["digest"],
                            "disk_bytes": l["size"],
                            "downloaded": (
                                blobs / l["digest"].replace(":", "-")
                            ).is_file(),
                        }
                        for l in weights
                    ],
                    "serveable": "unknown_daemon_not_probed",
                    "loaded": "unknown",
                    "aliases_may_share_weight_digest": True,
                }
            )
    roots = []
    hf = Path.home() / ".cache/huggingface/hub"
    snapshots = []
    if hf.is_dir():
        for path in sorted(hf.glob("models--*/snapshots/*")):
            if len(snapshots) >= 512:
                break
            # HF snapshots intentionally link blobs. Read bounded config only;
            # never follow a link outside the selected provider cache.
            config = path / "config.json"
            try:
                resolved = config.resolve(strict=True)
                if resolved.is_relative_to(hf.resolve()):
                    value = metadata_json(resolved)
                    if value is not None:
                        snapshots.append(
                            {
                                "provider_id": path.parent.parent.name,
                                "revision": path.name,
                                "model_type": value.get("model_type"),
                                "locality": "local_cached_snapshot",
                                "serveable": "unknown_compatibility_and_completeness",
                                "loaded": "not_observed",
                            }
                        )
            except OSError:
                pass
    for path in [
        hf,
        Path.home() / ".lmstudio/models",
        Path.home() / ".ollama/models",
        root / "models",
    ]:
        roots.append({"path": str(path), "exists": path.exists()})
    probes = []
    if probe:
        endpoints.add("http://127.0.0.1:11434")
        for endpoint in sorted(endpoints):
            endpoint = safe_endpoint(endpoint)
            if endpoint is None:
                continue
            urllib.parse.urlsplit(endpoint)
            # Defaults never contact arbitrary hosts. Operator CLI endpoints
            # explicitly opt a private multi-host provider into metadata probes.
            try:
                version = get_json(endpoint, "/api/version")
                tags = get_json(endpoint, "/api/tags")
                ps = get_json(endpoint, "/api/ps")
                probes.append(
                    {
                        "endpoint": endpoint,
                        "reachable": True,
                        "version": version.get("version"),
                        "model_ids": [m.get("name") for m in tags.get("models", [])],
                        "loaded_ids": [m.get("name") for m in ps.get("models", [])],
                        "locality": "unknown_even_on_loopback",
                    }
                )
            except (OSError, ValueError, urllib.error.HTTPError) as error:
                probes.append(
                    {
                        "endpoint": endpoint,
                        "reachable": False,
                        "error_type": type(error).__name__,
                        "files_missing": False,
                        "inference_attempted": False,
                    }
                )
    runtimes = {}
    for command, args in [("ollama", ["--version"]), ("llama-server", ["--version"])]:
        exe = shutil.which(command)
        if exe:
            run = subprocess.run(
                [exe, *args], capture_output=True, text=True, timeout=10, check=False
            )
            runtimes[command] = {
                "path": exe,
                "version_output": (run.stdout + run.stderr).strip()[:500],
            }
    return {
        "schema_version": 1,
        "observed_at_ms": int(time.time() * 1000),
        "weights_loaded": False,
        "weights_hashed": False,
        "downloaded_artifacts": records,
        "top_level_metadata_groups": inventory["candidate_group_count"],
        "top_level_weight_artifacts": len(inventory["artifacts"]),
        "ollama_manifests": ollama,
        "hf_snapshots": snapshots,
        "configured_without_matching_artifact": [
            item for group in aliases.values() for item in group
        ],
        "provider_roots": roots,
        "endpoint_probes": probes,
        "runtimes": runtimes,
        "parse_failures": inventory["failures"],
        "counting_rule": "Artifacts, shards, adapters, aliases and conversions are not independent base models or admitted workers.",
        "provider_cost_usd": 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--probe",
        action="store_true",
        help="Probe configured metadata endpoints without model loading",
    )
    parser.add_argument(
        "--endpoint",
        action="append",
        default=[],
        help="Trusted configured Ollama-compatible metadata endpoint",
    )
    parser.add_argument("--output", default="data/model-registry/discovery.json")
    args = parser.parse_args()
    report = discover(probe=args.probe, endpoints=args.endpoint)
    output = Path(args.output).resolve()
    if not output.is_relative_to((ROOT / "data").resolve()):
        raise ValueError("inventory output must remain in ignored data/")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(
        json.dumps(
            {
                "output": str(output),
                "top_level_weight_artifacts": report["top_level_weight_artifacts"],
                "top_level_metadata_groups": report["top_level_metadata_groups"],
                "all_artifacts": len(report["downloaded_artifacts"]),
                "ollama_manifests": len(report["ollama_manifests"]),
                "hf_snapshots": len(report["hf_snapshots"]),
                "endpoint_probes": report["endpoint_probes"],
                "provider_cost_usd": 0,
            }
        )
    )


if __name__ == "__main__":
    main()

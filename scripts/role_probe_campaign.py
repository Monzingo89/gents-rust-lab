#!/usr/bin/env python3
"""Run each provenance-checked local model serially on its role readiness cases."""

import argparse
import hashlib
import json
import os
import signal
import subprocess
import time
import uuid
from pathlib import Path

from model_provenance import ROOT, ROSTER, local_path, private_json, validate_roster


def summarize(models):
    protocol = json.loads((ROOT / "config/enterprise-role-probes.json").read_text())
    candidates = sorted(
        (ROOT / "data/model-registry/role-campaigns").glob("*/report.json"),
        key=lambda path: path.stat().st_mtime_ns,
        reverse=True,
    )
    rows = []
    for model in models:
        contract = next(r for r in protocol["roles"] if r["id"] == model["id"])
        selected = None
        for path in candidates:
            manifest_path = path.parent / "manifest.json"
            if not manifest_path.is_file():
                continue
            local_path(ROOT, str(manifest_path.relative_to(ROOT)), "data")
            manifest = json.loads(manifest_path.read_text())
            old_model = next(
                (m for m in manifest["roster"]["models"] if m["id"] == model["id"]),
                None,
            )
            old_contract = next(
                (r for r in manifest["protocol"]["roles"] if r["id"] == model["id"]),
                None,
            )
            if (
                not old_model
                or old_contract != contract
                or any(
                    old_model[k] != model[k]
                    for k in (
                        "publisher",
                        "source_repo",
                        "source_revision",
                        "path",
                        "sha256",
                        "context_tokens",
                    )
                )
            ):
                continue
            local_path(ROOT, str(path.relative_to(ROOT)), "data")
            matching = next(
                (
                    r
                    for r in json.loads(path.read_text())["models"]
                    if r["id"] == model["id"]
                ),
                None,
            )
            if matching is not None:
                selected = matching
                break  # Latest matching failure wins too; never cherry-pick an old pass.
        row = {
            "id": model["id"],
            "publisher": model["publisher"],
            "weight_sha256": model["sha256"],
            "ready": False,
            "enterprise_qualified": False,
        }
        if selected and selected.get("passed") and selected.get("evidence"):
            path = Path(selected["evidence"])
            local_path(ROOT, str(path.relative_to(ROOT)), "data")
            receipt = json.loads(path.read_text())
            contract_digest = hashlib.sha256(
                json.dumps(
                    contract, sort_keys=True, separators=(",", ":"), ensure_ascii=False
                ).encode()
            ).hexdigest()
            if receipt.get("role_contract_sha256") != contract_digest:
                row["reason"] = (
                    "readiness receipt missing current canonical role contract"
                )
                rows.append(row)
                continue  # Do not resurrect an older pass behind a stale latest receipt.
            assert (
                receipt["id"] == model["id"]
                and receipt["weight_sha256"] == model["sha256"]
            )
            assert (
                receipt["model"]["provider"] == "local"
                and receipt["model"]["path"] == model["path"]
            )
            assert len(receipt["calls"]) == len(contract["cases"]) == 2
            for case, call in zip(contract["cases"], receipt["calls"]):
                source = json.dumps(
                    case["input"],
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=False,
                ).encode()
                assert call["case_id"] == case["input"]["case_id"]
                assert call["source_sha256"] == hashlib.sha256(source).hexdigest()
                assert json.loads(call["reply"]["text"]) == case["expected"]
                assert (
                    call["reply"]["usage"]["cost_usd"] == 0
                    and call["reply"]["local_execution"]
                )
            row.update(
                ready=True,
                evidence=str(path),
                evidence_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                cases=2,
                provider_cost_usd=0,
            )
        rows.append(row)
    report = {
        "version": 1,
        "models": rows,
        "ready_models": sum(r["ready"] for r in rows),
        "distinct_publishers": len({r["publisher"] for r in rows if r["ready"]}),
        "enterprise_qualified_models": 0,
        "scope": "exploratory two-case readiness checks across recorded runs; not blind evaluation, full workflow or compliance certification",
    }
    destination = local_path(ROOT, "data/model-registry/readiness-index.json", "data")
    private_json(destination, report)
    print(
        json.dumps({"index": str(destination), "ready_models": report["ready_models"]}),
        flush=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", action="append", help="Select named roster IDs")
    parser.add_argument(
        "--summarize",
        action="store_true",
        help="Verify and index the latest matching receipts without inference",
    )
    args = parser.parse_args()
    models = validate_roster(json.loads(ROSTER.read_text()))
    if args.model and set(args.model) - {m["id"] for m in models}:
        raise ValueError("unknown roster ID")
    if args.summarize:
        summarize(models)
        return
    folder = local_path(
        ROOT, f"data/model-registry/role-campaigns/{uuid.uuid4().hex}", "data"
    )
    folder.mkdir(mode=0o700, parents=True)
    private_json(
        folder / "manifest.json",
        {
            "roster": json.loads(ROSTER.read_text()),
            "protocol": json.loads(
                (ROOT / "config/enterprise-role-probes.json").read_text()
            ),
            "selected_ids": args.model,
            "scope": "readiness only; not full workflow acceptance",
        },
    )
    env = {
        k: v
        for k, v in os.environ.items()
        if k in {"PATH", "HOME", "USER", "LANG", "TMPDIR", "SHELL"}
    }
    env.update(HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", CARGO_NET_OFFLINE="true")
    rows = []
    print(json.dumps({"campaign": str(folder)}), flush=True)
    for model in models:
        if args.model and model["id"] not in args.model:
            continue
        log_path = folder / (model["id"] + ".log")
        print(json.dumps({"id": model["id"], "status": "RUNNING"}), flush=True)
        started = time.monotonic()
        with log_path.open("xb") as log:
            os.chmod(log_path, 0o600)
            process = subprocess.Popen(
                [
                    "cargo",
                    "test",
                    "-p",
                    "gents-harness",
                    "--locked",
                    "native_role_readiness",
                    "--",
                    "--ignored",
                    "--nocapture",
                ],
                cwd=ROOT,
                env={**env, "GENTS_ROLE_PROBE": model["id"]},
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                code = process.wait(timeout=600)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    code = process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    code = process.wait()
        evidence = None
        for line in log_path.read_text().splitlines():
            if line.startswith("{"):
                item = json.loads(line)
                if item.get("id") == model["id"] and "evidence" in item:
                    evidence = Path(item["evidence"])
        row = {
            "id": model["id"],
            "passed": False,
            "exit_code": code,
            "elapsed_seconds": round(time.monotonic() - started, 2),
        }
        if evidence:
            local_path(ROOT, str(evidence.relative_to(ROOT)), "data")
            report = json.loads(evidence.read_text())
            assert (
                report["id"] == model["id"]
                and report["weight_sha256"] == model["sha256"]
            )
            row.update(
                passed=code == 0 and report["passed"],
                passed_cases=report["passed_cases"],
                total_cases=report["total_cases"],
                evidence=str(evidence),
            )
        rows.append(row)
        private_json(
            folder / "report.json",
            {
                "version": 1,
                "models": rows,
                "ready_models": sum(r["passed"] for r in rows),
                "enterprise_qualified_models": 0,
                "scope": "role-specific compatibility only; hard workflow and broader qualification remain pending",
            },
        )
        print(json.dumps(row), flush=True)


if __name__ == "__main__":
    main()

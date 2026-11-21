#!/usr/bin/env python3
"""Opt-in real local-model tool loop. No web, mail, training or downloads."""

import hashlib
import json
import os
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


with tempfile.TemporaryDirectory(prefix="gents-local-loop-") as temporary:
    folder = Path(temporary)
    model = {
        "provider": "local",
        "model": "Qwen3-8B",
        "path": "models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf",
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Engineer Gent",
                "assignment": "Complete the simple file fixture using real tools",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 768,
        "max_run_seconds": 600,
    }
    path = folder / "config.json"
    path.write_text(json.dumps(config))
    port, model_port = free_port(), free_port()
    base = f"http://127.0.0.1:{port}"
    process = subprocess.Popen(
        [str(ROOT / "target/debug/gents-harness")],
        cwd=ROOT,
        env=dict(
            os.environ,
            AUTH_MODE="local",
            BIND=f"127.0.0.1:{port}",
            CONFIG=str(path),
            RUN_STORE=str(folder / "runs.sqlite3"),
            LOCAL_MODEL_PORT=str(model_port),
        ),
        stdout=subprocess.DEVNULL,
    )
    company, run = "local-smoke-" + uuid.uuid4().hex[:10], str(uuid.uuid4())
    directory = ROOT / f"data/companies/{company}/runs/{run}"
    started = time.monotonic()
    report: dict[str, str | int | float] = {
        "stage": "easy",
        "status": "FAIL",
        "real_model": model["model"],
        "run_id": run,
        "artifacts": str(directory),
    }

    def request(path, data=None):
        req = urllib.request.Request(
            base + path,
            data=None if data is None else json.dumps(data).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=10) as response:
            return json.load(response)

    def op(kind, **data):
        return request(f"/api/companies/{company}", {"op": kind, **data})

    try:
        for _ in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                time.sleep(0.1)
        op("register", name="Real local-model smoke fixture")
        task = "Create result.txt containing exactly READY. Read result.txt back to verify the contents, then finish citing that file. Use only files and finish tools; no shell, web, email, downloads or training. No question is needed."
        criteria = [
            "result.txt contains exactly READY and was read back with the files tool"
        ]
        intake = {
            "idea": task,
            "acceptance": "\n".join(criteria),
            "exclusions": "Other tools",
            "timeline": "Smoke test",
            "company_signer": "Local test",
            "provider_signer": "Local test",
            "budget_usd": 0,
            "deliverable_type": "agent",
        }
        state = op("intake", run_id=run, intake=intake)
        op("approve", run_id=run, digest=state["jobs"][0]["digest"])
        access = {
            "tools": ["files:write", "files:read", "finish"],
            "skills": [],
            "memory_from": [],
        }
        op(
            "governance_install",
            run_id=run,
            expected_revision=0,
            policy={
                "revision": 1,
                "master": "Engineer Gent",
                "ceilings": {"Engineer Gent": access},
                "grants": {"Engineer Gent": access},
            },
        )
        request(
            "/api/runs",
            {
                "company_id": company,
                "run_id": run,
                "task": task,
                "consent": False,
                "definition_of_done": criteria,
            },
        )
        print(
            json.dumps(
                {
                    "status": "STARTED",
                    "pid": process.pid,
                    "run_id": run,
                    "api": base,
                    "artifacts": str(directory),
                }
            ),
            flush=True,
        )
        deadline = time.monotonic() + 600
        while time.monotonic() < deadline:
            saved = request(f"/api/runs/{run}")
            attention = request(f"/api/runs/{run}/attention")
            if attention.get("attention") and not attention["attention"].get("answer"):
                raise AssertionError(
                    "Real loop unexpectedly needs attention: "
                    + attention["attention"]["question"]
                )
            if saved["status"] != "running":
                break
            time.sleep(0.5)
        assert saved["status"] == "completed", f"Real loop ended {saved['status']}"
        assert (directory / "workspace/result.txt").read_text() == "READY"
        journals = [json.loads(p.read_text()) for p in directory.glob("tool-*.json")]
        assert any(
            j["request"]["action"] == "files"
            and j["request"]["arguments"]["op"] == "read"
            and j["result"]["result"]["ok"] is True
            and j["result"]["result"]["result"]["text"] == "READY"
            for j in journals
        )
        assert all(j["admin"]["decision"] == "allow" for j in journals)
        assert all(j["request"]["action"] == "files" for j in journals)
        calls = [e for e in saved["events"] if e.get("reply")]
        report.update(
            status="PASS",
            model_calls=len(calls),
            provider_cost_usd=sum(e["reply"]["usage"]["cost_usd"] for e in calls),
            artifact_sha256=hashlib.sha256(
                (directory / "workspace/result.txt").read_bytes()
            ).hexdigest(),
            verified="admin admission, governed file write/read, actual READY readback, independent QA acceptance",
        )
        print(json.dumps(report), flush=True)
    except Exception as error:
        report["failure"] = type(error).__name__ + ": " + str(error)[:500]
        raise
    finally:
        process.terminate()
        process.wait(timeout=20)
        if directory.exists():
            report["elapsed_seconds"] = round(time.monotonic() - started, 2)
            (directory / "easy-stage-result.json").write_text(
                json.dumps(report, indent=2) + "\n"
            )
            if report["status"] == "PASS":
                (directory / "easy-stage-paper.md").write_text(
                    "# One local agent using one governed file tool\n\n"
                    "## Method\n\nOne Qwen3-8B model supplied the agent, admission and independent QA roles through the existing Rust harness. "
                    "The frozen task required creating result.txt with exactly READY and reading it back. "
                    "A run-scoped deterministic policy admitted only file write/read and completion. No web, mail, training or downloads were permitted.\n\n"
                    f"## Results\n\nThe local run completed with {report['model_calls']} recorded model replies in {report['elapsed_seconds']} seconds. "
                    f"Provider charges were USD {report['provider_cost_usd']}; electricity and hardware costs are excluded. "
                    f"The independently checked artifact SHA-256 is {report['artifact_sha256']}. "
                    "The successful tool journal contains the actual READY readback; easy-stage-result.json records this trial.\n\n"
                    "## Limitations\n\nThis single deterministic task does not establish general coding quality, frontier-model equivalence, "
                    "parallel inference, ten-model orchestration or regulatory compliance. The same base model filling QA and agent roles "
                    "can make correlated mistakes, so the external byte-for-byte assertion remains essential.\n"
                )

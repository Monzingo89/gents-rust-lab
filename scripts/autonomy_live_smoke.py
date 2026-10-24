#!/usr/bin/env python3
"""Opt-in real local-model tool loop. No web, mail, training or downloads."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]

def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]

with tempfile.TemporaryDirectory(prefix="gents-local-loop-") as temporary:
    folder = Path(temporary)
    model = {"provider": "local", "model": "Qwen3-8B", "path": "models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf"}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Engineer Gent", "assignment": "Complete the simple file fixture using real tools", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512, "max_run_seconds": 180}
    path = folder / "config.json"
    path.write_text(json.dumps(config))
    port, model_port = free_port(), free_port()
    base = f"http://127.0.0.1:{port}"
    process = subprocess.Popen([str(ROOT / "target/debug/gents-harness")], cwd=ROOT,
        env=dict(os.environ, AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(path), RUN_STORE=str(folder / "runs.sqlite3"), LOCAL_MODEL_PORT=str(model_port)),
        stdout=subprocess.DEVNULL)
    company, run = "local-smoke-" + uuid.uuid4().hex[:10], str(uuid.uuid4())
    def request(path, data=None):
        req = urllib.request.Request(base + path, data=None if data is None else json.dumps(data).encode(), headers={"Content-Type": "application/json"})
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
                time.sleep(.1)
        op("register", name="Real local-model smoke fixture")
        task = "Create result.txt containing exactly READY. Read result.txt back to verify the contents, then finish citing that file. Use only files and finish tools; no shell, web, email, downloads or training. No question is needed."
        criteria = ["result.txt contains exactly READY and was read back with the files tool"]
        intake = {"idea": task, "acceptance": "\n".join(criteria), "exclusions": "Other tools", "timeline": "Smoke test",
                  "company_signer": "Local test", "provider_signer": "Local test", "budget_usd": 0, "deliverable_type": "agent"}
        state = op("intake", run_id=run, intake=intake)
        op("approve", run_id=run, digest=state["jobs"][0]["digest"])
        request("/api/runs", {"company_id": company, "run_id": run, "task": task, "consent": False, "definition_of_done": criteria})
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            saved = request(f"/api/runs/{run}")
            attention = request(f"/api/runs/{run}/attention")
            if attention.get("attention") and not attention["attention"].get("answer"):
                raise AssertionError("Real loop unexpectedly needs attention: " + attention["attention"]["question"])
            if saved["status"] != "running":
                break
            time.sleep(.5)
        assert saved["status"] == "completed", f"Real loop ended {saved['status']}"
        directory = ROOT / f"data/companies/{company}/runs/{run}"
        assert (directory / "workspace/result.txt").read_text() == "READY"
        journals = [json.loads(p.read_text()) for p in directory.glob("tool-*.json")]
        assert any(j["request"]["action"] == "files" and j["request"]["arguments"]["op"] == "read" for j in journals)
        assert all(j["admin"]["decision"] == "allow" for j in journals)
        calls = [e for e in saved["events"] if e.get("reply")]
        print(json.dumps({"status": "PASS", "real_model": "Qwen3-8B", "model_calls": len(calls), "provider_cost_usd": sum(e["reply"]["usage"]["cost_usd"] for e in calls),
                          "verified": "admin admission, independently reviewed file write/read, QA acceptance", "artifacts": str(directory)}))
    finally:
        process.terminate()
        process.wait(timeout=20)

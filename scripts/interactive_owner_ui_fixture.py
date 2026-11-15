"""Disposable browser fixture: seeded SIMULATED planning/QA, real local API/MCP.

Run manually, inspect printed URL/session, then create the printed stop file.
Never counts as company qualification or real-model planning evidence.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid

import local_sqlite

REPO = Path(__file__).resolve().parents[1]
OUTPUT = REPO / "data/mcp-basement-verification/interactive-owner-ui-fixture.json"
STOP = REPO / "data/mcp-basement-verification/interactive-owner-ui-fixture.stop"
assert not STOP.exists(), "Remove only the previous disposable fixture stop marker before starting"

with tempfile.TemporaryDirectory(prefix="gents-owner-ui-") as temporary:
    root = Path(temporary)
    shutil.copytree(REPO / "web", root / "web")
    binary_dir = root / "target/debug"
    binary_dir.mkdir(parents=True)
    for name in ["gents-local-mcp", "gents-sqlite-bridge"]:
        shutil.copyfile(REPO / "target/debug" / name, binary_dir / name)
        (binary_dir / name).chmod(0o500)
    model = {"provider": "mock", "model": "mock-review-protocol"}
    worker = {"provider": "local", "model": "unused UI worker", "path": "models/absent-fixture.gguf", "context_tokens": 8192}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": actor, "assignment": "SIMULATED UI regression", "planner": model, "worker": worker} for actor in ["Analyst", "Writer"]],
              "max_steps": 1, "max_tokens": 1024, "max_run_seconds": 120}
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    env = {key: value for key, value in os.environ.items() if key in {"PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "WINDIR"}}
    env.update(AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(root / "config.json"), RUN_STORE=str(root / "runs.sqlite3"),
               GENTS_LOCAL_ONLY="1", GENTS_DISABLE_SCHEDULER="1", GENTS_OFFICE_ACTIVITY="0", HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1")
    base = f"http://127.0.0.1:{port}"

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=10) as response:
            return json.load(response)

    process = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env, stdout=subprocess.DEVNULL)
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None
                if attempt == 99:
                    raise AssertionError("UI fixture startup timed out")
                time.sleep(.05)
        company, session = "we-need-gents-lc", str(uuid.uuid4())
        request(f"/api/companies/{company}", {"op": "register", "name": "SIMULATED browser regression only"})
        arguments = {"calculations": [{"id": "measurement", "values": [7, -10], "subtract": 2}]}
        task = "SIMULATED model fixture for browser regression only. Save the actual native calculation array for values 7,-10 subtract 2 to result.json and read it back."
        criteria = ["result.json equals the actual native calculation array and has a full readback"]
        source = {"id": "calculation", "actor": "Analyst", "tool": "integer_arithmetic", "arguments": arguments}
        artifact = {"path": "result.json", "source": "calculation", "writer": "Writer", "pointer": "/calculations", "encoding": "json"}
        proposal = {"status": "ready", "question": "", "definition_of_done": criteria,
                    "assignments": [{"actor": "Analyst", "task": "Compute the native fixture input", "tools": ["integer_arithmetic"], "acceptance": ["Native result exists"]},
                                    {"actor": "Writer", "task": "Write the exact array and read it back", "tools": ["files:read", "files:write"], "acceptance": criteria}],
                    "native_contracts": {"sources": [source], "artifacts": [artifact]}}
        binding = {"path": "result.json", "source_actor": "Analyst", "writer": "Writer", "source_tool": "integer_arithmetic", "arguments": arguments, "pointer": "/calculations", "encoding": "json"}
        planning = {"id": session, "revision": 1, "status": "ready", "source_inputs": [], "source_files": None,
                    "messages": [{"role": "user", "kind": "requirements", "message": task}], "proposal": proposal, "proposed_artifact_bindings": [binding]}
        with local_sqlite.connect(root / f"data/companies/{company}/brain.sqlite3") as db:
            db.execute("CREATE TABLE IF NOT EXISTS ceo_planning(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL)")
            db.execute("INSERT INTO ceo_planning VALUES(?,1,'ready',?)", (session, json.dumps(planning)))
        receipt = {"scope": "Disposable SIMULATED UI planning/QA only; real HTTP and native operations, not model competence", "origin": base, "url": base + "/?autoconnect=1", "company": company, "session": session, "pid": process.pid, "stop_file": str(STOP)}
        OUTPUT.write_text(json.dumps(receipt, indent=2) + "\n")
        print(json.dumps(receipt), flush=True)
        deadline = time.monotonic() + 1200
        while not STOP.exists():
            assert process.poll() is None and time.monotonic() < deadline, "UI fixture stopped or exceeded its manual test bound"
            time.sleep(.2)
        runs = request("/api/runs")["runs"]
        receipt["run_states"] = [{"run": r.get("id", r.get("run_id")), "status": r["status"]} for r in runs]
        assert len(runs) == 1 and runs[0]["status"] == "completed", "UI must complete exactly one private test run"
        run = receipt["run_states"][0]["run"]
        calls = [json.loads(p.read_text()) for p in (root / f"data/companies/{company}/runs/{run}").glob("tool-*.json")]
        assert len(calls) == 3 and all(c["result"]["result"]["ok"] and c["result"]["result"]["mcp_receipt"]["contract_enforced"] for c in calls)
        receipt.update(status="PASS", native_calls=3, model_planning_simulated=True, qa_simulated=True)
        OUTPUT.write_text(json.dumps(receipt, indent=2) + "\n")
        print("PASS: browser-selected exact authorization reached real native enforcement with one source/write/read; simulated planning and QA only", flush=True)
    finally:
        process.terminate()
        assert process.wait(timeout=15) == 0

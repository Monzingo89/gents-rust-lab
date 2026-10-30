#!/usr/bin/env python3
"""Real HTTP, durable questions, restart, and sandbox tools; mock model decisions."""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]

with tempfile.TemporaryDirectory(prefix="gents-autonomy-smoke-") as temporary:
    root = Path(temporary)
    (root / "scripts").mkdir()
    shutil.copyfile(ROOT / "scripts/company_tools.py", root / "scripts/company_tools.py")
    model = {"provider": "mock", "model": "autonomy-fixture"}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Engineer Gent", "assignment": "fixture", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512, "max_run_seconds": 120}
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    env = dict(os.environ, AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(root / "config.json"),
               RUN_STORE=str(root / "runs.sqlite3"), GENTS_SUPERVISED="1")
    env.pop("RUST_OPENAI_API_KEY", None)  # This smoke must never incur a cloud call.
    process = None
    def request(path, data=None):
        req = urllib.request.Request(base + path, data=None if data is None else json.dumps(data).encode(), headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)
    def op(kind, **data):
        return request("/api/companies/one", {"op": kind, **data})
    def start():
        proc = subprocess.Popen([str(ROOT / "target/debug/gents-harness")], cwd=root, env=env, stdout=subprocess.DEVNULL)
        for _ in range(120):
            try:
                request("/api/config")
                return proc
            except OSError:
                time.sleep(.1)
        raise AssertionError("harness startup failed")
    def wait_for(action, predicate, seconds=40):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            value = action()
            if predicate(value):
                return value
            time.sleep(.15)
        raise AssertionError(f"timed out waiting for state: {value}")
    def intake(task):
        run = str(uuid.uuid4())
        frozen = {"idea": task, "acceptance": "Write a real deliverable and verify it", "exclusions": "paid services",
                  "timeline": "test", "company_signer": "Fixture", "provider_signer": "Fixture", "budget_usd": 0, "deliverable_type": "agent"}
        state = op("intake", run_id=run, intake=frozen)
        job = next(j for j in state["jobs"] if j["run_id"] == run)
        op("approve", run_id=run, digest=job["digest"])
        request("/api/runs", {"company_id": "one", "run_id": run, "task": task, "consent": False,
                              "definition_of_done": frozen["acceptance"].splitlines()})
        return run
    try:
        process = start()
        op("register", name="One")
        run = intake("autonomy-smoke")
        question = wait_for(lambda: request(f"/api/runs/{run}/attention"), lambda r: r.get("attention"))["attention"]
        request("/api/company/restart", {})
        assert process.wait(timeout=15) == 75
        process = start()
        recovered = request(f"/api/runs/{run}/attention")["attention"]
        assert recovered["id"] == question["id"], "question changed across restart"
        process.terminate()
        assert process.wait(timeout=15) == 0
        process = start()
        recovered = request(f"/api/runs/{run}/attention")["attention"]
        assert recovered["id"] == question["id"], "program shutdown lost the resumable question"
        request(f"/api/runs/{run}/cancel", {})
        stopped = wait_for(lambda: request(f"/api/runs/{run}"), lambda r: r["status"] != "running")
        assert stopped["status"] == "cancelled", stopped["status"]
        resumed = request(f"/api/runs/{run}/resume", {})
        assert resumed["resumed"] and resumed["id"] == run
        assert request(f"/api/runs/{run}/resume", {})["already_running"], "resume launched duplicate execution"
        assert request(f"/api/runs/{run}/attention")["attention"]["id"] == question["id"]
        try:
            request(f"/api/runs/{run}/attention", {"question_id": "wrong", "answer": "stale"})
            raise AssertionError("stale answer admitted")
        except urllib.error.HTTPError as error:
            assert error.code == 400
        request(f"/api/runs/{run}/attention", {"question_id": question["id"], "answer": "Smoke Project"})
        result = wait_for(lambda: request(f"/api/runs/{run}"), lambda r: r["status"] != "running")
        assert result["status"] == "completed", result
        try:
            request(f"/api/runs/{run}/resume", {})
            raise AssertionError("completed run resumed")
        except urllib.error.HTTPError as error:
            assert error.code == 409
        work = root / f"data/companies/one/runs/{run}"
        assert (work / "workspace/deliverable.txt").read_text().startswith("Actual file written")
        journals = [json.loads(p.read_text()) for p in sorted(work.glob("tool-*.json"))]
        shell = next(j for j in journals if j["request"]["action"] == "shell")
        assert shell["result"]["result"]["result"]["exit_code"] == 0, shell
        assert shell["result"]["result"]["result"]["output"] == "verified", shell
        assert all(j["admin"]["decision"] == "allow" for j in journals)
        loop_run = intake("autonomy-loop-smoke")
        result = wait_for(lambda: request(f"/api/runs/{loop_run}"), lambda r: r["status"] != "running")
        assert result["status"] == "completed", result
        assert any(e["kind"] == "loop_recovery" for e in result["events"])
        journals = [json.loads(p.read_text()) for p in sorted((root / f"data/companies/one/runs/{loop_run}").glob("tool-*.json"))]
        assert sum(j["request"]["action"] == "files" and j["request"]["arguments"]["op"] == "list" for j in journals) == 2
        assert any(j["request"]["action"] == "shell" for j in journals)
        ambiguous = intake("autonomy-ambiguous-smoke")
        wait_for(lambda: request(f"/api/runs/{ambiguous}/attention"), lambda r: r.get("attention"))
        checkpoint = root / f"data/companies/one/runs/{ambiguous}/autonomy.json"
        fixture = json.loads(checkpoint.read_text())
        fixture["attention"] = None
        fixture["pending"] = {"id": str(uuid.uuid4()), "step": 0, "action": "email_send", "arguments": {"to": "fixture@example.com"}, "phase": "started"}
        checkpoint.write_text(json.dumps(fixture))
        request("/api/company/restart", {})
        assert process.wait(timeout=15) == 75
        process = start()
        attention = wait_for(lambda: request(f"/api/runs/{ambiguous}/attention"), lambda r: r.get("attention"))["attention"]
        assert "interrupted" in attention["question"] and "email_send" in attention["question"]
        request(f"/api/runs/{ambiguous}/attention", {"question_id": attention["id"], "answer": "The fixture send never occurred; do not replay it."})
        result = wait_for(lambda: request(f"/api/runs/{ambiguous}"), lambda r: r["status"] != "running")
        assert result["status"] == "completed", result
        assert not any(e["kind"] == "tool_started" and "email_send" in e["message"] for e in result["events"])
        denied = intake("deny-admin-smoke")
        result = wait_for(lambda: request(f"/api/runs/{denied}"), lambda r: r["status"] != "running")
        assert result["status"] == "blocked", result
        assert not list((root / f"data/companies/one/runs/{denied}").glob("tool-*.json"))
        stalled = intake("autonomy-stall-smoke")
        question = wait_for(lambda: request(f"/api/runs/{stalled}/attention"), lambda r: r.get("attention"), seconds=75)["attention"]
        assert "12 recovery attempts" in question["question"]
        observed = request(f"/api/runs/{stalled}")
        assert any(e["kind"] == "codex_phone_ringing" for e in observed["events"])
        assert any(e["kind"] == "codex_phone_unanswered" for e in observed["events"])
        request(f"/api/runs/{stalled}/cancel", {})
        wait_for(lambda: request(f"/api/runs/{stalled}"), lambda r: r["status"] != "running")
        tool_failure = intake("autonomy-tool-failure-smoke")
        observed = wait_for(lambda: request(f"/api/runs/{tool_failure}"), lambda r: any(e["kind"] == "loop_recovery" and "recorded_tool_failure" in e["message"] for e in r["events"]))
        assert any(e["kind"] == "tool_result" and not json.loads(e["message"])["result"]["ok"] for e in observed["events"])
        request(f"/api/runs/{tool_failure}/cancel", {})
    finally:
        if process and process.poll() is None:
            process.terminate()
            process.wait(timeout=15)

print("PASS: repeated-operation rejection and automatic replanning, admin veto before tools, real HTTP intake, persistent questions, restart recovery, no replay of uncertain email intent, stale-answer rejection, actual sandbox file and shell output; model decisions are simulated")

"""Real HTTP/durable recovery, simulated QA rejection; not model competence."""
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

REPO = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="gents-qa-recovery-") as temporary:
    root = Path(temporary)
    (root / "scripts").mkdir()
    (root / "config").mkdir()
    for name in ["company_tools.py", "public_network.py"]:
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    shutil.copyfile(REPO / "config/public-network-policy.json", root / "config/public-network-policy.json")
    model = {"provider": "mock", "model": "mock-qa-revise"}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Engineer Gent", "assignment": "fixture", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512, "max_run_seconds": 120}
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    env = dict(os.environ, AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(root / "config.json"),
               RUN_STORE=str(root / "runs.sqlite3"), GENTS_DISABLE_SCHEDULER="1", GENTS_LOCAL_ONLY="1")
    env.pop("RUST_OPENAI_API_KEY", None)
    process = None

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def wait_for(action, predicate):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                value = action()
                if predicate(value): return value
            except OSError:
                pass
            time.sleep(.1)
        raise AssertionError("QA recovery fixture timed out")

    def start():
        child = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env,
                                 stdout=subprocess.DEVNULL)
        try:
            wait_for(lambda: request("/api/config"), bool)
            return child
        except Exception:
            child.terminate(); child.wait(timeout=15)
            raise

    def stop():
        process.terminate()
        assert process.wait(timeout=15) == 0

    run = str(uuid.uuid4())
    path = f"/api/runs/{run}"
    try:
        process = start()
        request("/api/companies/one", {"op": "register", "name": "QA recovery fixture"})
        intake = {"idea": "qa-recovery-private-fixture", "acceptance": "Independently review the synthetic text",
                  "exclusions": "paid services", "timeline": "fixture", "company_signer": "Fixture",
                  "provider_signer": "Fixture", "budget_usd": 0, "deliverable_type": "agent"}
        state = request("/api/companies/one", {"op": "intake", "run_id": run, "intake": intake})
        digest = next(j["digest"] for j in state["jobs"] if j["run_id"] == run)
        request("/api/companies/one", {"op": "approve", "run_id": run, "digest": digest})
        request("/api/runs", {"company_id": "one", "run_id": run, "task": intake["idea"],
                              "definition_of_done": [intake["acceptance"]], "consent": False})
        wait_for(lambda: request(path + "/attention"), lambda x: x.get("attention"))
        request(path + "/cancel", {})
        wait_for(lambda: request(path), lambda x: x["status"] == "cancelled")
        stop()
        work = root / f"data/companies/one/runs/{run}"
        checkpoint = work / "autonomy.json"
        state = json.loads(checkpoint.read_text())
        assert state["admitted"] and state["pending"] is None
        # Accelerate ONLY this disposable fixture to the existing budget edge.
        state.update(attention=None, review_intent=None, recovery_attempts=0, repairs_since_progress=11,
                     plan_queue=[{"action": "finish", "arguments": {"deliverable": "SIMULATED fixture text", "evidence": "No external execution claimed"}}])
        checkpoint.write_text(json.dumps(state))
        process = start()
        request(path + "/resume", {})
        attention = wait_for(lambda: request(path + "/attention"), lambda x: x.get("attention"))["attention"]
        assert "12 recovery attempts" in attention["question"]
        saved = request(path)
        assert saved["status"] != "completed"
        assert any(e["kind"] == "completion_review" and json.loads(e["message"])["verdict"] == "revise" for e in saved["events"])
        assert any(e["kind"] == "loop_recovery" and json.loads(e["message"])["attempt"] == 1 for e in saved["events"])
        assert not list(work.glob("tool-*.json")), "review rejection must not execute a tool"
        state = json.loads(checkpoint.read_text())
        assert state["repairs_since_progress"] == 12
        assert any("qa_revision_required" in item for item in state["history"])
        stop()
        process = start()
        restored = request(path + "/attention")["attention"]
        assert restored["id"] == attention["id"]
        assert not list(work.glob("tool-*.json"))
        assert request(path)["events"][:len(saved["events"])] == saved["events"]
        print("PASS: final QA rejection consumes bounded recovery, never accepts or executes a tool, and retains its question/history across restart; simulated model verdicts")
    finally:
        if process is not None and process.poll() is None:
            process.terminate(); process.wait(timeout=15)

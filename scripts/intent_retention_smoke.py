"""Real HTTP/restart/file I/O; simulated authorization failure, never model competence."""
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
with tempfile.TemporaryDirectory(prefix="gents-intent-retention-") as temporary:
    root = Path(temporary)
    (root / "scripts").mkdir()
    (root / "config").mkdir()
    for name in ["company_tools.py", "public_network.py", "public_research.py"]:
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    shutil.copyfile(REPO / "config/public-network-policy.json", root / "config/public-network-policy.json")
    model = {"provider": "mock", "model": "intent-fixture"}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Engineer Gent", "assignment": "fixture", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512, "max_run_seconds": 120}
    config_path = root / "config.json"
    config_path.write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    env = dict(os.environ, AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(config_path),
               RUN_STORE=str(root / "runs.sqlite3"), GENTS_SUPERVISED="1", GENTS_DISABLE_SCHEDULER="1")
    env.pop("RUST_OPENAI_API_KEY", None)
    process = None

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def wait_for(action, predicate):
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            value = action()
            if predicate(value):
                return value
            time.sleep(.1)
        raise AssertionError(f"fixture timed out: {value}")

    def start():
        child = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env,
                                 stdout=subprocess.DEVNULL)
        try:
            wait_for(lambda: request("/api/config") if child.poll() is None else {}, bool)
        except (OSError, AssertionError):
            # Startup connection errors are expected before the listener binds.
            for _ in range(100):
                if child.poll() is not None:
                    raise AssertionError("fixture harness exited")
                try:
                    request("/api/config")
                    return child
                except OSError:
                    time.sleep(.05)
            child.terminate()
            child.wait(timeout=15)
            raise AssertionError("fixture harness did not bind")
        return child

    def stop():
        process.terminate()
        assert process.wait(timeout=15) == 0

    run = str(uuid.uuid4())
    path = f"/api/runs/{run}"
    try:
        process = start()
        request("/api/companies/one", {"op": "register", "name": "Intent fixture"})
        intake = {"idea": "intent-retention-fixture", "acceptance": "Inspect fixture I/O", "exclusions": "paid services",
                  "timeline": "fixture", "company_signer": "Fixture", "provider_signer": "Fixture",
                  "budget_usd": 0, "deliverable_type": "agent"}
        state = request("/api/companies/one", {"op": "intake", "run_id": run, "intake": intake})
        job = next(j for j in state["jobs"] if j["run_id"] == run)
        request("/api/companies/one", {"op": "approve", "run_id": run, "digest": job["digest"]})
        request("/api/runs", {"company_id": "one", "run_id": run, "task": intake["idea"],
                              "definition_of_done": [intake["acceptance"]], "consent": False})
        wait_for(lambda: request(path + "/attention"), lambda x: x["attention"] is not None)
        request(path + "/cancel", {})
        wait_for(lambda: request(path), lambda x: x["status"] == "cancelled")
        stop()
        work = root / f"data/companies/one/runs/{run}"
        checkpoint = work / "autonomy.json"
        state = json.loads(checkpoint.read_text())
        assert state["admitted"] and state["pending"] is None
        write = {"action": "files", "arguments": {"op": "write", "path": "retained.txt", "content": "infrastructure fixture"}}
        read = {"action": "files", "arguments": {"op": "read", "path": "retained.txt"}}
        # Seed only this disposable controller fixture, never a benchmark run.
        state.update(attention=None, review_intent=None, plan_queue=[write, read])
        checkpoint.write_text(json.dumps(state))
        config["admin_judge"] = {"provider": "mock", "model": "mock-invalid-authorization"}
        config_path.write_text(json.dumps(config))
        process = start()
        request(path + "/resume", {})
        failed = wait_for(lambda: request(path), lambda x: any(e["kind"] == "queued_intent_retained" for e in x["events"]))
        assert not (work / "workspace/retained.txt").exists()
        state = json.loads(checkpoint.read_text())
        assert state["review_intent"] == write and state["plan_queue"] == [read] and state["pending"] is None
        request(path + "/cancel", {})
        wait_for(lambda: request(path), lambda x: x["status"] == "cancelled")
        stop()
        config["admin_judge"] = model
        config_path.write_text(json.dumps(config))
        process = start()
        request(path + "/resume", {})
        def results(snapshot):
            return [json.loads(e["message"]) for e in snapshot["events"] if e["kind"] == "tool_result"]
        resumed = wait_for(lambda: request(path), lambda x: len(results(x)) >= 2)
        calls = results(resumed)
        assert [(c["action"], c["arguments"].get("op")) for c in calls[:2]] == [("files", "write"), ("files", "read")]
        assert calls[0]["arguments"] == write["arguments"] and calls[1]["arguments"] == read["arguments"]
        assert all(c["result"]["ok"] for c in calls[:2])
        assert sum(c["action"] == "files" and c["arguments"].get("op") == "write" for c in calls) == 1
        assert (work / "workspace/retained.txt").read_text() == "infrastructure fixture"
        boundary = max(e["seq"] for e in failed["events"])
        first_effect = next(e["seq"] for e in resumed["events"] if e["kind"] == "tool_started")
        assert any(e["kind"] == "admin_decision" and boundary < e["seq"] < first_effect for e in resumed["events"])
        assert not any(e["role"] == "worker" and e["kind"] == "call_started" and boundary < e["seq"] < first_effect for e in resumed["events"])
        request(path + "/cancel", {})
    finally:
        if process and process.poll() is None:
            stop()
print("PASS: failed authorization retained exact unexecuted intent across real HTTP restart; fresh review, ordered file write/read, one write, no author replanning; model decisions simulated")

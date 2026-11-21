"""Real HTTP worker dispatch of rejected-source feedback; simulated models only."""

import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="gents-proposal-feedback-") as temporary:
    root = Path(temporary)
    (root / "scripts").mkdir()
    (root / "config").mkdir()
    for name in ["company_tools.py", "public_network.py", "public_research.py"]:
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    shutil.copyfile(
        REPO / "config/public-network-policy.json",
        root / "config/public-network-policy.json",
    )
    model = {"provider": "mock", "model": "proposal-feedback-bootstrap"}
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Engineer Gent",
                "assignment": "fixture",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 512,
        "max_run_seconds": 120,
    }
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    env = dict(
        os.environ,
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{port}",
        CONFIG=str(root / "config.json"),
        RUN_STORE=str(root / "runs.sqlite3"),
        GENTS_DISABLE_SCHEDULER="1",
        GENTS_LOCAL_ONLY="1",
    )
    env.pop("RUST_OPENAI_API_KEY", None)
    process = None

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def wait_for(action, predicate):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                value = action()
                if predicate(value):
                    return value
            except OSError:
                pass
            time.sleep(0.1)
        raise AssertionError("Proposal feedback fixture timed out")

    def start():
        child = subprocess.Popen(
            [str(REPO / "target/debug/gents-harness")],
            cwd=root,
            env=env,
            stdout=subprocess.DEVNULL,
        )
        try:
            wait_for(lambda: request("/api/config"), bool)
            return child
        except Exception:
            child.terminate()
            child.wait(timeout=15)
            raise

    def stop():
        process.terminate()
        assert process.wait(timeout=15) == 0

    run = str(uuid.uuid4())
    path = f"/api/runs/{run}"
    try:
        process = start()
        request("/api/companies/one", {"op": "register", "name": "QA recovery fixture"})
        intake = {
            "idea": "proposal-feedback-private-fixture",
            "acceptance": "Retain parser feedback without executing a proposal",
            "required_rust_checks": [
                {
                    "actor": "Engineer Gent",
                    "source_path": "source.rs",
                    "tests_path": "acceptance.rs",
                    "tests_sha256": "0" * 64,
                }
            ],
            "exclusions": "paid services",
            "timeline": "fixture",
            "company_signer": "Fixture",
            "provider_signer": "Fixture",
            "budget_usd": 0,
            "deliverable_type": "agent",
        }
        state = request(
            "/api/companies/one", {"op": "intake", "run_id": run, "intake": intake}
        )
        digest = next(j["digest"] for j in state["jobs"] if j["run_id"] == run)
        request(
            "/api/companies/one", {"op": "approve", "run_id": run, "digest": digest}
        )
        request(
            "/api/runs",
            {
                "company_id": "one",
                "run_id": run,
                "task": intake["idea"],
                "definition_of_done": [intake["acceptance"]],
                "consent": False,
            },
        )
        wait_for(lambda: request(path + "/attention"), lambda x: x.get("attention"))
        request(path + "/cancel", {})
        wait_for(lambda: request(path), lambda x: x["status"] == "cancelled")
        stop()
        work = root / f"data/companies/one/runs/{run}"
        checkpoint = work / "autonomy.json"
        state = json.loads(checkpoint.read_text())
        assert state["admitted"] and state["pending"] is None
        # Seed only this disposable test's prior denial, not a benchmark.
        denied = {
            "actor": "Engineer Gent",
            "ceo_stage": 0,
            "action": "files",
            "arguments": {
                "op": "write",
                "path": "source.rs",
                "content": "pub fn f(){todo!()}",
            },
        }
        state.update(
            attention=None,
            review_intent=None,
            worker=None,
            plan_queue=[],
            recovery_attempts=1,
        )
        state["history"].append(
            {"denied": denied, "reason": "SIMULATED independent rejection"}
        )
        checkpoint.write_text(json.dumps(state))
        special = {"provider": "mock", "model": "mock-proposal-feedback"}
        config["admin_judge"] = special
        configured_agents = config["agents"]
        assert isinstance(configured_agents, list)
        configured_agents[0].update(planner=special, worker=special)
        (root / "config.json").write_text(json.dumps(config))
        process = start()
        request(path + "/resume", {})
        attention = wait_for(
            lambda: request(path + "/attention"), lambda x: x.get("attention")
        )["attention"]
        assert (
            attention["question"]
            == "Which synthetic recovery marker should the fixture record?"
        )
        saved = request(path)
        assert saved["status"] != "completed"
        assert not (work / "workspace/source.rs").exists()
        assert not list(work.glob("tool-*.json"))
        assert any(
            e["kind"] == "call_completed" and e["role"] == "worker"
            for e in saved["events"]
        )
        assert any(
            e["kind"] == "admin_decision"
            and json.loads(e["message"])
            .get("proposal", {})
            .get("request", {})
            .get("action")
            == "ask_user"
            for e in saved["events"]
        )
        print(
            "PASS: current parser feedback for retained rejected source reached the actual worker dispatch; independent authorization retained; no source write/tool execution; simulated models"
        )
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=15)

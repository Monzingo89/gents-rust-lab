"""Private HTTP coordination admission/terminal-failure check; no installed models.

CEO plans are explicitly seeded synthetic fixtures. This is infrastructure
verification, not native model task qualification or independent acceptance.
"""
import copy
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

import local_sqlite

REPO = Path(__file__).resolve().parents[1]


def port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


with tempfile.TemporaryDirectory(prefix="gents-coordination-http-") as temporary:
    root = Path(temporary).resolve()
    model = {"provider": "local", "model": "not-installed-fixture", "path": "models/not-installed.gguf"}
    actors = ["Hub", "Worker", "Critic"]
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": actor, "assignment": "Synthetic protocol fixture", "planner": model, "worker": model} for actor in actors],
              "max_steps": 1, "max_tokens": 256, "max_run_seconds": 30}
    (root / "config.json").write_text(json.dumps(config))
    http_port, model_port = port(), port()
    while model_port == http_port:
        model_port = port()
    env = dict(os.environ, AUTH_MODE="local", BIND=f"127.0.0.1:{http_port}", CONFIG=str(root / "config.json"),
               RUN_STORE=str(root / "runs.sqlite3"), LOCAL_MODEL_PORT=str(model_port), GENTS_LOCAL_ONLY="1",
               GENTS_SQLITE_BRIDGE=str(REPO / "target/debug/gents-sqlite-bridge"),
               GENTS_DISABLE_SCHEDULER="1", GENTS_OFFICE_ACTIVITY="0", HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1")
    base = f"http://127.0.0.1:{http_port}"

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=15) as response:
            return json.load(response)

    def rejected(path, body):
        try:
            request(path, body)
        except urllib.error.HTTPError as error:
            assert error.code == 400, error.code
            error.close()
        else:
            raise AssertionError("Expected explicit rejection")

    process = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None
                if attempt == 99:
                    raise AssertionError("Private coordination startup timed out")
                time.sleep(.05)
        path = "/api/companies/fixture"
        request(path, {"op": "register", "name": "Explicit synthetic coordination fixture"})
        database = root / "data/companies/fixture/brain.sqlite3"
        for pattern in ("ordered", "adaptive_delegation", "peer_review"):
            sid, run = str(uuid.uuid4()), str(uuid.uuid4())
            criteria = ["Return the requested synthetic text candidate"]
            roles = [{"actor": actor, "assignment": "Contribute synthetic text", "model_sha256": "0" * 64} for actor in actors]
            contract = {"version": 1, "coordination": pattern, "objective": "Inspect supplied synthetic text", "definition_of_done": criteria,
                        "input": {"text": "Synthetic fixture"}, "roles": roles, "max_delegations": 2, "peer_rounds": 1}
            proposal = {"status": "ready", "question": "", "definition_of_done": criteria,
                        "assignments": [{"actor": role["actor"], "task": role["assignment"], "tools": [], "acceptance": ["Bounded contribution"]} for role in roles]}
            with local_sqlite.connect(database) as db:
                db.execute("CREATE TABLE IF NOT EXISTS ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)")
                db.execute("INSERT INTO ceo_planning VALUES(?,2,'ready',?)", (sid, json.dumps({"revision": 2, "status": "ready", "proposal": proposal, "fixture": "SIMULATED CEO plan"})))
            intake = {"idea": contract["objective"], "acceptance": "\n".join(criteria), "exclusions": "Network and paid services",
                      "timeline": "Private fixture", "company_signer": "Fixture owner", "provider_signer": "Fixture provider", "budget_usd": 0,
                      "deliverable_type": "text", "ceo_planning_id": sid, "ceo_planning_revision": 2, "coordination_contract": contract}
            specification = {"version": 1, "objective": contract["objective"], "inputs": ["Supplied synthetic text"],
                             "outputs": ["Text candidate"], "definition_of_done": criteria, "exclusions": ["Network and paid services"],
                             "coordination": pattern, "data_classification": "synthetic", "control_profiles": ["local_baseline"],
                             "required_capabilities": []}
            preparation = {"expected_revision": 2, "specification": specification, "input": contract["input"],
                           "model_sha256": {actor: "0" * 64 for actor in actors}, "max_delegations": 2, "peer_rounds": 1}
            with local_sqlite.connect(database) as db:
                before = db.execute("SELECT COUNT(*) FROM jobs").fetchone()[0]
            prepared = request(path + "/planning/" + sid + "/prepare", preparation)
            assert prepared["candidate_only"] and not prepared["execution_authorized"]
            assert prepared["blueprint"]["architecture"]["coordination"] == pattern
            assert prepared["blueprint"]["architecture"]["version"] == 2
            with local_sqlite.connect(database) as db:
                assert db.execute("SELECT COUNT(*) FROM jobs").fetchone()[0] == before
            invalid = copy.deepcopy(preparation)
            invalid["specification"]["required_capabilities"] = ["files:read"]
            rejected(path + "/planning/" + sid + "/prepare", invalid)
            invalid = copy.deepcopy(preparation)
            invalid["expected_revision"] = 1
            rejected(path + "/planning/" + sid + "/prepare", invalid)
            intake.update(prepared["intake_patch"])
            contract = intake["coordination_contract"]
            assert contract["version"] == 2
            invalid = copy.deepcopy(intake)
            invalid["factory_binding"]["program_sha256"] = "1" * 64
            rejected(path, {"op": "intake", "run_id": str(uuid.uuid4()), "intake": invalid})
            invalid = copy.deepcopy(intake)
            invalid["coordination_contract"]["roles"][0]["assignment"] = "Changed scope"
            rejected(path, {"op": "intake", "run_id": str(uuid.uuid4()), "intake": invalid})
            invalid = copy.deepcopy(intake)
            invalid["required_mcp_calls"] = [{"actor": "Hub", "tool": "files"}]
            rejected(path, {"op": "intake", "run_id": str(uuid.uuid4()), "intake": invalid})
            state = request(path, {"op": "intake", "run_id": run, "intake": intake})
            job = next(job for job in state["jobs"] if job["run_id"] == run)
            controls = job["intake"]["ceo_host_controls"]
            assert controls["actions"] == [] and "transitive_lineage" in controls
            access = {actor: {"tools": [], "skills": [], "memory_from": controls["handoff_from"][actor]} for actor in actors}
            policy = {"revision": 1, "master": actors[0], "ceilings": access, "grants": access}
            invalid = copy.deepcopy(policy)
            invalid["ceilings"]["Hub"]["tools"] = ["files:read"]
            rejected(path, {"op": "governance_install", "run_id": run, "expected_revision": 0, "policy": invalid})
            request(path, {"op": "approve", "run_id": run, "digest": job["digest"]})
            request(path, {"op": "governance_install", "run_id": run, "expected_revision": 0, "policy": policy})
            admitted = request("/api/runs", {"company_id": "fixture", "run_id": run, "task": contract["objective"], "definition_of_done": criteria, "consent": False})
            for attempt in range(150):
                result = request("/api/runs/" + admitted["id"])
                if result["status"] != "running":
                    break
                time.sleep(.05)
            assert result["status"] == "failed", result["status"]
            kinds = [event["kind"] for event in result["events"]]
            assert kinds.count("call_started") == 1 and kinds.count("call_failed") == 1, kinds
            assert "retry_scheduled" not in kinds and "coordination_candidate_submitted" not in kinds
            with local_sqlite.connect(database) as db:
                assert db.execute("SELECT COUNT(*) FROM coordination_turns WHERE run_id=? AND state='pending'", (run,)).fetchone()[0] == 1
                assert db.execute("SELECT COUNT(*) FROM context WHERE run_id=?", (run,)).fetchone()[0] == 0
        with socket.socket() as connection:
            assert connection.connect_ex(("127.0.0.1", model_port)) != 0
    finally:
        process.terminate()
        process.wait(timeout=10)

print("PASS: private HTTP owner-bound coordination admission for three patterns; scope/tool ceilings enforced; missing model fails terminally without automatic replay; seeded plans, no model/task qualification")

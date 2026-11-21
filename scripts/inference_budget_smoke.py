"""Actual HTTP/journal budget stops with simulated decisions; no model competence claim."""

import json
import os
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="gents-budget-stop-") as temporary:
    root = Path(temporary).resolve()
    (root / "target/debug").mkdir(parents=True)
    shutil.copyfile(
        REPO / "target/debug/gents-local-mcp", root / "target/debug/gents-local-mcp"
    )
    (root / "target/debug/gents-local-mcp").chmod(0o700)
    (root / "scripts").mkdir()
    (root / "config").mkdir()
    for name in ["company_tools.py", "public_network.py", "public_research.py"]:
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    shutil.copyfile(
        REPO / "config/public-network-policy.json",
        root / "config/public-network-policy.json",
    )
    model = {"provider": "mock", "model": "budget-lifecycle-fixture"}
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

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def wait_for(action, predicate):
        deadline = time.monotonic() + 35
        while time.monotonic() < deadline:
            try:
                value = action()
                if predicate(value):
                    return value
            except OSError:
                pass
            time.sleep(0.1)
        raise AssertionError(f"Budget fixture timed out: {value}")

    def start():
        child = subprocess.Popen(
            [str(REPO / "target/debug/gents-harness")],
            cwd=root,
            env=env,
            stdout=subprocess.DEVNULL,
        )
        try:
            wait_for(lambda: request("/api/config"), bool)
        except BaseException:
            child.terminate()
            child.wait(timeout=15)
            raise
        return child

    def stop():
        process.terminate()
        assert process.wait(timeout=15) == 0

    def company(op, **fields):
        return request("/api/companies/one", {"op": op, **fields})

    def reservation_count(run):
        with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
            return db.execute(
                "SELECT COUNT(*) FROM inference_attempt_reservations WHERE run_id=?",
                (run,),
            ).fetchone()[0]

    process = None
    try:
        process = start()
        company("register", name="Budget lifecycle fixture")
        cases = []
        for limit in [1, 2, 3]:
            run = str(uuid.uuid4())
            path = "/api/runs/" + run
            task = "autonomy-tool-failure-smoke budget fixture " + str(limit)
            intake = {
                "idea": task,
                "acceptance": "Record the original bounded attempt without claiming completion",
                "exclusions": "paid services",
                "timeline": "fixture",
                "company_signer": "Fixture",
                "provider_signer": "Fixture",
                "budget_usd": 0,
                "deliverable_type": "agent",
                "max_model_calls": limit,
            }
            state = company("intake", run_id=run, intake=intake)
            job = next(j for j in state["jobs"] if j["run_id"] == run)
            company("approve", run_id=run, digest=job["digest"])
            access = {"tools": ["files:read"], "skills": [], "memory_from": []}
            policy = {
                "revision": 1,
                "master": "Engineer Gent",
                "ceilings": {"Engineer Gent": access},
                "grants": {"Engineer Gent": access},
            }
            company(
                "governance_install", run_id=run, expected_revision=0, policy=policy
            )
            request(
                "/api/runs",
                {
                    "company_id": "one",
                    "run_id": run,
                    "task": task,
                    "definition_of_done": [intake["acceptance"]],
                    "consent": False,
                },
            )
            saved = wait_for(
                lambda path=path: request(path),
                lambda x: x["status"] == "budget_exhausted",
            )
            events = saved["events"]
            denied = [e for e in events if e["kind"] == "inference_budget_denied"]
            assert len(denied) == 1
            assert sum(e["kind"] == "call_started" for e in events) == limit
            assert reservation_count(run) == limit
            assert not any(
                e["kind"]
                in [
                    "resource_recovery",
                    "retry_scheduled",
                    "completed",
                    "restart_resumed",
                ]
                for e in events
            )
            assert not any(
                e["kind"] in ["call_started", "tool_started"]
                for e in events
                if e["seq"] > denied[0]["seq"]
            )
            assert sum(e["kind"] == "tool_started" for e in events) == (
                1 if limit == 3 else 0
            )
            if limit == 3:
                with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
                    assert (
                        db.execute(
                            "SELECT COUNT(*) FROM audit WHERE run_id=? AND kind='mcp_tool_rejected'",
                            (run,),
                        ).fetchone()[0]
                        == 1
                    )
            job = next(j for j in company("status")["jobs"] if j["run_id"] == run)
            assert job["state"] == "budget_exhausted"
            work = root / f"data/companies/one/runs/{run}"
            checkpoint = json.loads((work / "autonomy.json").read_text())
            if limit == 2:
                assert checkpoint["review_intent"]["action"] == "files"
                intent = checkpoint["review_intent"]
                arguments = intent.get("arguments") or json.loads(
                    intent["arguments_json"]
                )
                assert arguments == {"op": "read", "path": "missing-fixture.txt"}
            cases.append((run, path, saved, (work / "autonomy.json").read_bytes()))
        stop()
        process = None
        process = start()
        for run, path, saved, checkpoint in cases:
            current = request(path)
            assert current["status"] == "budget_exhausted"
            assert len(current["events"]) == len(saved["events"])
            assert reservation_count(run) == sum(
                e["kind"] == "call_started" for e in saved["events"]
            )
            try:
                request(path + "/resume", {})
                raise AssertionError("Exhausted allowance was renewed")
            except urllib.error.HTTPError as error:
                assert error.code == 409
            assert (
                root / f"data/companies/one/runs/{run}/autonomy.json"
            ).read_bytes() == checkpoint
        stop()
        process = None
        # Crash-window fixture: stop reason persisted, final terminal write missing.
        run, path, saved, checkpoint = cases[1]
        denial = next(
            e["seq"] for e in saved["events"] if e["kind"] == "inference_budget_denied"
        )
        with sqlite3.connect(root / "runs.sqlite3") as db:
            payload = json.loads(
                db.execute("SELECT payload FROM runs WHERE id=?", (run,)).fetchone()[0]
            )
            assert payload["inference_budget_stop"] == {"used": 2, "maximum": 2}
            payload["status"] = "running"
            db.execute(
                "UPDATE runs SET payload=?,revision=? WHERE id=?",
                (json.dumps(payload), denial + 1, run),
            )
            db.execute("DELETE FROM run_events WHERE run_id=? AND seq>?", (run, denial))
        with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
            db.execute("UPDATE jobs SET state='running' WHERE run_id=?", (run,))
        file_checkpoint = root / f"data/companies/one/runs/{run}/checkpoint.json"
        partial = json.loads(file_checkpoint.read_text())
        partial["status"] = "running"
        partial["events"] = partial["events"][: denial + 1]
        file_checkpoint.write_text(json.dumps(partial))
        process = start()
        current = request(path)
        assert current["status"] == "budget_exhausted"
        assert not any(e["kind"] == "restart_resumed" for e in current["events"])
        assert reservation_count(run) == 2
        assert (
            root / f"data/companies/one/runs/{run}/autonomy.json"
        ).read_bytes() == checkpoint
        canonical = next(j for j in company("status")["jobs"] if j["run_id"] == run)[
            "state"
        ]
        assert canonical == "budget_exhausted", (
            canonical,
            current["status"],
            current["events"][-1],
        )
        stop()
        process = None
        # Failed authorization preparation may retry within the allowance only.
        config["admin_judge"] = {
            "provider": "mock",
            "model": "mock-invalid-authorization",
        }
        (root / "config.json").write_text(json.dumps(config))
        process = start()
        run = str(uuid.uuid4())
        path = "/api/runs/" + run
        task = "Failed authorization fixture"
        intake["idea"] = task
        intake["max_model_calls"] = 2
        state = company("intake", run_id=run, intake=intake)
        job = next(j for j in state["jobs"] if j["run_id"] == run)
        company("approve", run_id=run, digest=job["digest"])
        company("governance_install", run_id=run, expected_revision=0, policy=policy)
        request(
            "/api/runs",
            {
                "company_id": "one",
                "run_id": run,
                "task": task,
                "definition_of_done": [intake["acceptance"]],
                "consent": False,
            },
        )
        current = wait_for(
            lambda: request(path), lambda x: x["status"] == "budget_exhausted"
        )
        events = current["events"]
        denied = next(
            e["seq"] for e in events if e["kind"] == "inference_budget_denied"
        )
        assert reservation_count(run) == 2
        assert sum(e["kind"] == "call_started" for e in events) == 2
        assert not any(
            e["kind"] in ["retry_scheduled", "call_started", "tool_started"]
            for e in events
            if e["seq"] > denied
        )
        trace = request(path + "/observability")
        assert any(e["kind"] == "budget_exhausted" for e in trace["issues"])
        print(
            "PASS: actual HTTP terminal budget stops in worker/admin/post-tool phases; no retry or post-stop dispatch; queued intent preserved; restart and manual resume cannot renew allowance; crash-window reason reconciles without replay. Model decisions simulated; no inference performed."
        )
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=15)

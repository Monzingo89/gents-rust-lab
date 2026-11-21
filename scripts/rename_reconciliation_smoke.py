"""Actual harness/native rename with injected audit failure; model decisions simulated."""

import hashlib
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
with tempfile.TemporaryDirectory(prefix="gents-rename-reconcile-") as temporary:
    root = Path(temporary).resolve()
    (root / "scripts").mkdir()
    (root / "config").mkdir()
    (root / "target/debug").mkdir(parents=True)
    shutil.copyfile(
        REPO / "target/debug/gents-local-mcp", root / "target/debug/gents-local-mcp"
    )
    (root / "target/debug/gents-local-mcp").chmod(0o700)
    for name in ["company_tools.py", "public_network.py", "public_research.py"]:
        shutil.copyfile(REPO / "scripts" / name, root / "scripts" / name)
    shutil.copyfile(
        REPO / "config/public-network-policy.json",
        root / "config/public-network-policy.json",
    )
    model = {"provider": "mock", "model": "rename-lifecycle-fixture"}
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Engineer Gent",
                "assignment": "Synthetic lifecycle fixture",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 512,
        "max_run_seconds": 120,
    }
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    env = dict(
        os.environ,
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{port}",
        CONFIG=str(root / "config.json"),
        RUN_STORE=str(root / "runs.sqlite3"),
        GENTS_LOCAL_ONLY="1",
        GENTS_DISABLE_SCHEDULER="1",
    )

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def wait(action, predicate):
        deadline = time.monotonic() + 40
        last = None
        while time.monotonic() < deadline:
            try:
                last = action()
                if predicate(last):
                    return last
            except OSError:
                pass
            time.sleep(0.1)
        raise AssertionError(f"Rename fixture timed out: {last}")

    def start():
        child = subprocess.Popen(
            [str(REPO / "target/debug/gents-harness")],
            cwd=root,
            env=env,
            stdout=subprocess.DEVNULL,
        )
        try:
            wait(lambda: request("/api/config"), bool)
        except BaseException:
            child.terminate()
            child.wait(timeout=15)
            raise
        return child

    def stop():
        process.terminate()
        assert process.wait(timeout=15) == 0

    def op(operation, **fields):
        return request("/api/companies/one", {"op": operation, **fields})

    process = None
    try:
        process = start()
        op("register", name="Synthetic rename lifecycle fixture")
        run = str(uuid.uuid4())
        path = "/api/runs/" + run
        intake = {
            "idea": "Rename a private synthetic marker and retain actual evidence",
            "acceptance": "Preserve bytes and reconcile an unconfirmed move without automatic replay",
            "exclusions": "paid APIs and real model claims",
            "timeline": "fixture",
            "company_signer": "Fixture",
            "provider_signer": "Fixture",
            "budget_usd": 0,
            "deliverable_type": "agent",
            "max_model_calls": 16,
        }
        state = op("intake", run_id=run, intake=intake)
        job = next(j for j in state["jobs"] if j["run_id"] == run)
        op("approve", run_id=run, digest=job["digest"])
        access = {
            "tools": ["ask_user", "files:read", "files:rename"],
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
                "company_id": "one",
                "run_id": run,
                "task": intake["idea"],
                "definition_of_done": [intake["acceptance"]],
                "consent": False,
            },
        )
        wait(lambda: request(path + "/attention"), lambda x: x.get("attention"))
        request(path + "/cancel", {})
        saved = wait(lambda: request(path), lambda x: x["status"] == "cancelled")
        stop()
        process = None
        folder = root / f"data/companies/one/runs/{run}"
        workspace = folder / "workspace"
        workspace.mkdir(exist_ok=True)
        body = b"opaque\0\xffartifact"
        (workspace / "draft.bin").write_bytes(body)
        args = {
            "op": "rename",
            "path": "draft.bin",
            "destination": "final.bin",
            "expected_sha256": hashlib.sha256(body).hexdigest(),
        }
        # Only this disposable fixture's next decision is seeded. Real approval,
        # runtime identity, policy, native adapter, receipt failure and restart apply.
        checkpoint = folder / "autonomy.json"
        state = json.loads(checkpoint.read_text())
        assert state["pending"] is None
        state.update(
            attention=None,
            review_intent=None,
            plan_queue=[
                {
                    "action": "files",
                    "arguments": args,
                    "summary": "SIMULATED next action; actual native effect",
                }
            ],
            recovery_attempts=0,
            repairs_since_progress=0,
        )
        checkpoint.write_text(json.dumps(state))
        database = root / "data/companies/one/brain.sqlite3"
        with sqlite3.connect(database) as db:
            db.execute(
                "CREATE TRIGGER lose_rename_receipt BEFORE INSERT ON audit WHEN NEW.kind='mcp_tool_call' BEGIN SELECT RAISE(ABORT,'injected rename receipt loss'); END"
            )
        process = start()
        boundary = len(request(path)["events"])
        request(path + "/resume", {})
        saved = wait(
            lambda: request(path), lambda x: x["status"] == "needs_reconciliation"
        )
        events = saved["events"][boundary:]
        assert sum(e["kind"] == "tool_started" for e in events) == 1
        assert not any(
            e["kind"] in ["retry_scheduled", "completed", "attention_required"]
            for e in events
        )
        assert any(e["kind"] == "rename_needs_reconciliation" for e in events)
        retained = json.loads(checkpoint.read_text())
        assert retained["pending"]["arguments"] == args
        assert retained["plan_queue"] == []
        assert not (workspace / "draft.bin").exists(), [
            e["message"] for e in events if e["kind"] == "tool_result"
        ]
        assert (workspace / "final.bin").read_bytes() == body
        with sqlite3.connect(database) as db:
            assert (
                db.execute(
                    "SELECT COUNT(*) FROM audit WHERE run_id=? AND kind='mcp_tool_effect_uncertain'",
                    (run,),
                ).fetchone()[0]
                == 1
            )
        assert (
            next(j for j in op("status")["jobs"] if j["run_id"] == run)["state"]
            == "needs_reconciliation"
        )
        stop()
        process = None
        before = checkpoint.read_bytes()
        process = start()
        recovered = request(path)
        assert recovered["status"] == "needs_reconciliation"
        assert len(recovered["events"]) == len(saved["events"])
        assert checkpoint.read_bytes() == before
        try:
            request(path + "/resume", {})
            raise AssertionError("Unconfirmed rename automatically resumed")
        except urllib.error.HTTPError as error:
            assert error.code == 409
        stop()
        process = None
        # Crash-window simulation: native effect happened, but the terminal harness
        # checkpoint was not saved. The retained rename intent must not become advice
        # that clears it or a model/tool replay on startup.
        last_start = next(
            e["seq"] for e in reversed(saved["events"]) if e["kind"] == "tool_started"
        )
        with sqlite3.connect(root / "runs.sqlite3") as db:
            payload = json.loads(
                db.execute("SELECT payload FROM runs WHERE id=?", (run,)).fetchone()[0]
            )
            payload["status"] = "interrupted"
            db.execute(
                "UPDATE runs SET payload=?,revision=? WHERE id=?",
                (json.dumps(payload), last_start + 1, run),
            )
            db.execute(
                "DELETE FROM run_events WHERE run_id=? AND seq>?", (run, last_start)
            )
        snapshot = folder / "checkpoint.json"
        partial = json.loads(snapshot.read_text())
        partial["status"] = "interrupted"
        partial["events"] = partial["events"][: last_start + 1]
        snapshot.write_text(json.dumps(partial))
        with sqlite3.connect(database) as db:
            db.execute("UPDATE jobs SET state='interrupted' WHERE run_id=?", (run,))
        process = start()
        recovered = wait(
            lambda: request(path), lambda x: x["status"] == "needs_reconciliation"
        )
        tail = recovered["events"][last_start + 1 :]
        assert not any(
            e["kind"] in ["call_started", "tool_started", "attention_required"]
            for e in tail
        )
        assert json.loads(checkpoint.read_text())["pending"]["arguments"] == args
        assert (workspace / "final.bin").read_bytes() == body and not (
            workspace / "draft.bin"
        ).exists()
        print(
            "PASS: actual binary rename survives injected receipt failure; harness retains both paths/hash in pending intent and stops for reconciliation; no model/tool retry or task acceptance; restart and resume cannot replay. Decisions and initial next-action queue are simulated."
        )
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=15)

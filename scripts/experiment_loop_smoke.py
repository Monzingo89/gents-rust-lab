"""Actual loopback admission, journals and restart; model decisions are simulated."""

import json
import os
import socket
import sqlite3
import subprocess
import tempfile
import time
from pathlib import Path

from experiment_loop import Controller, LoopbackAPI

REPO = Path(__file__).resolve().parents[1]


def main():
    with tempfile.TemporaryDirectory(prefix="gents-experiment-loop-") as temporary:
        root = Path(temporary).resolve()
        signal = root / "data/experiment-inputs/signal.json"
        signal.parent.mkdir(parents=True)
        signal.write_text("baseline")
        model = {"provider": "mock", "model": "experiment-lifecycle-fixture"}
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
        api = LoopbackAPI(f"http://127.0.0.1:{port}")
        calls = []
        lose_next_dispatch = False

        def recorded(path, body=None):
            nonlocal lose_next_dispatch
            calls.append((path, body))
            result = api(path, body)
            if path == "/api/runs" and lose_next_dispatch:
                lose_next_dispatch = False
                raise OSError(
                    "fixture: actual dispatch succeeded but response was lost"
                )
            return result

        env = {
            k: v
            for k, v in os.environ.items()
            if k in {"PATH", "HOME", "USER", "LANG", "TMPDIR", "SHELL"}
        }
        env.update(
            AUTH_MODE="local",
            BIND=f"127.0.0.1:{port}",
            CONFIG=str(root / "config.json"),
            RUN_STORE=str(root / "runs.sqlite3"),
            GENTS_DISABLE_SCHEDULER="1",
            GENTS_LOCAL_ONLY="1",
        )
        child = subprocess.Popen(
            [str(REPO / "target/debug/gents-harness")],
            cwd=root,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            deadline = time.monotonic() + 20
            while True:
                try:
                    api("/api/config")
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise
                    time.sleep(0.1)
            api(
                "/api/companies/one",
                {"op": "register", "name": "Experiment lifecycle fixture"},
            )
            trial = {
                "id": "first",
                "actor": "Engineer Gent",
                "task": "autonomy-tool-failure-smoke bounded experiment",
                "criteria": [
                    "Retain the original bounded attempt without claiming acceptance"
                ],
                "tools": ["files:read"],
                "max_model_calls": 1,
            }
            plan = {
                "version": 1,
                "company": "one",
                "evidence_paths": ["data/experiment-inputs/signal.json"],
                "max_model_calls": 2,
                "experiments": [trial, {**trial, "id": "second"}],
            }
            controller = Controller(root, "smoke", recorded)
            frozen = controller.freeze(plan)
            assert frozen["initial_dispatch"] is False
            calls.clear()
            for _ in range(20):
                assert controller.poll()["status"] == "unchanged"
            assert not calls, "unchanged evidence must cause zero HTTP requests"

            def await_result():
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    report = controller.poll()
                    if report["status"] == "result":
                        return report
                    assert report["status"] == "observing", report
                    time.sleep(0.1)
                raise AssertionError("Original bounded run did not reach a result")

            signal.write_text("new owner evidence")
            first = controller.poll()
            assert first["status"] == "dispatched"
            result = await_result()
            assert result["result"]["runtime_status"] == "budget_exhausted"
            assert result["result"]["model_calls"] == 1
            assert result["result"]["provider_cost_usd"] == 0
            assert result["result"]["independently_accepted"] is False
            signal.write_text("baseline")
            assert controller.poll()["status"] == "unchanged"
            signal.write_text("new owner evidence")
            assert controller.poll()["status"] == "unchanged"
            signal.write_text("second owner evidence")
            lose_next_dispatch = True
            uncertain = controller.poll()
            assert uncertain["status"] == "needs_reconciliation"
            assert uncertain["run_id"] != first["run_id"]
            # Reopen both durable bookkeeping and the original API identity.
            controller = Controller(root, "smoke", recorded)
            reconciled = await_result()
            assert reconciled["run_id"] == uncertain["run_id"]
            assert controller.poll()["status"] == "exhausted"
            status = controller.status()
            assert status["reserved_model_calls"] == 2
            assert len(status["trials"]) == 2
            assert sum(path == "/api/runs" for path, _ in calls) == 2
            with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
                assert (
                    db.execute(
                        "SELECT COUNT(*) FROM inference_attempt_reservations"
                    ).fetchone()[0]
                    == 2
                )
                jobs = db.execute("SELECT run_id,state,intake FROM jobs").fetchall()
                assert len(jobs) == 2 and all(j[1] == "budget_exhausted" for j in jobs)
                assert all(json.loads(j[2])["max_model_calls"] == 1 for j in jobs)
            print(
                "PASS: actual HTTP/company journals; unchanged evidence made zero HTTP requests; two finite one-call runs; original run reconciled after a lost dispatch response; exactly two durable reservations; no replay, acceptance or promotion. Model decisions simulated; no inference performed."
            )
        finally:
            child.terminate()
            if child.wait(timeout=15) != 0:
                raise RuntimeError("fixture harness did not stop cleanly")


if __name__ == "__main__":
    main()

"""Actual HTTP/native custody and isolated acceptance; source intents simulated."""
import copy
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from unittest.mock import patch

import experiment_evaluation as evaluation
from experiment_loop import Controller, LoopbackAPI, canonical
import local_sqlite as sqlite3

REPO = Path(__file__).resolve().parents[1]


def main(continuations=False, research=False):
    if os.uname().sysname != "Darwin":
        print("SKIP: actual isolated Rust evaluation requires supported macOS adapter")
        return
    with tempfile.TemporaryDirectory(prefix="gents-experiment-evaluation-") as temporary:
        root = Path(temporary).resolve()
        binary_dir = root / "target/debug"
        binary_dir.mkdir(parents=True)
        for name in ("gents-local-mcp", "gents-rust-component", "gents-rust-acceptance-check"):
            shutil.copyfile(REPO / "target/debug" / name, binary_dir / name)
            (binary_dir / name).chmod(0o500)
        signal = root / "data/experiment-inputs/signal.json"
        signal.parent.mkdir(parents=True)
        signal.write_text("baseline")
        tests = root / "data/experiment-inputs/acceptance.rs"
        tests.write_text("#[test]fn empty(){std::assert_eq!(candidate::remaining(0,7),7);}\n#[test]fn spent(){std::assert_eq!(candidate::remaining(8,3),0);}")
        model = {"provider": "mock", "model": "source-intent-fixture-not-inference"}
        config = {"company_mode": True, "company_tools": True, "admin_judge": model,
                  "agents": [{"id": "Engineer Gent", "assignment": "fixture", "planner": model, "worker": model}],
                  "max_steps": 1, "max_tokens": 512, "max_run_seconds": 120}
        if research:
            config["agents"].append({"id": "Researcher Gent", "assignment": "fixture", "planner": model, "worker": model})
        (root / "config.json").write_text(json.dumps(config))
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        api = LoopbackAPI(f"http://127.0.0.1:{port}")
        passed_source = "pub fn remaining(used:usize,limit:usize)->usize{limit.saturating_sub(used)}"
        sources = [passed_source, "pub fn remaining(_:usize,_:usize)->usize{0}", passed_source]
        if continuations:
            sources = [sources[1], passed_source]
        if research:
            note = {"hypothesis": "Saturating subtraction handles exhausted limits.",
                    "procedure": ["Compare used with limit.", "Return the saturating difference."],
                    "prediction": "Both original owner tests will pass."}
            sources = [passed_source, sources[1], json.dumps(note), passed_source]
        seeded = []
        mutations = []

        def request(path, body=None):
            if body is not None:
                mutations.append((path, copy.deepcopy(body)))
            if path == "/api/runs":
                # Seed only the source intent, never write/read receipts or an
                # acceptance result. Actual typed stages must execute those.
                run = body["run_id"]
                source = sources[len(seeded)]
                state = {"stage": "write_ready", "attempt": 1, "source": source, "source_sha256": evaluation.sha(source.encode()),
                         "summary": "SIMULATED source intent for protocol coverage", "offset": 0, "write_receipt": None,
                         "read_receipts": [], "reconciled_write": False, "feedback": []}
                encoded = canonical(state)
                with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
                    contract = db.execute("SELECT digest FROM jobs WHERE run_id=?", (run,)).fetchone()[0]
                    db.execute("CREATE TABLE IF NOT EXISTS candidate_workflows(run_id TEXT PRIMARY KEY,contract TEXT NOT NULL,revision INTEGER NOT NULL,state TEXT NOT NULL,state_sha256 TEXT NOT NULL)")
                    db.execute("INSERT INTO candidate_workflows VALUES(?,?,1,?,?)", (run, contract, encoded, evaluation.sha(encoded.encode())))
                seeded.append(run)
            result = api(path, body)
            if research and path == "/api/runs" and len(seeded) == 3:
                raise OSError("fixture: research dispatch response lost after native admission")
            return result

        env = {k: v for k, v in os.environ.items() if k in {"PATH", "HOME", "USER", "LANG", "TMPDIR", "SHELL"}}
        env.update(AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(root / "config.json"),
                   RUN_STORE=str(root / "runs.sqlite3"), GENTS_DISABLE_SCHEDULER="1", GENTS_LOCAL_ONLY="1")
        child = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root,
                                 env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            deadline = time.monotonic() + 20
            while True:
                try:
                    api("/api/config")
                    break
                except OSError:
                    if time.monotonic() > deadline:
                        raise
                    time.sleep(.1)
            api("/api/companies/one", {"op": "register", "name": "Independent evaluation protocol fixture"})
            trial = {"id": "passing", "actor": "Engineer Gent", "task": "Submit the exact simulated source intent for private protocol verification",
                     "criteria": ["Submit a quarantined candidate with exact native readback; independent host review is required before execution or promotion."],
                     "tools": ["files:read", "files:write"], "max_model_calls": 1,
                     "candidate_workflow": {"version": 1, "actor": "Engineer Gent", "path": "candidate.rs", "max_bytes": 8192, "max_attempts": 1},
                     "evaluation": {"kind": "rust_acceptance", "tests_path": "data/experiment-inputs/acceptance.rs", "tests_sha256": evaluation.sha(tests.read_bytes())}}
            plan = {"version": 1, "company": "one", "evidence_paths": ["data/experiment-inputs/signal.json"],
                    "max_model_calls": 3, "experiments": [trial, {**copy.deepcopy(trial), "id": "failing"}, {**copy.deepcopy(trial), "id": "uncertain"}]}
            if continuations:
                for item, identifier in zip(plan["experiments"], ("baseline", "repair", "unused")):
                    item["id"] = identifier
                plan["experiments"][1]["after"] = {"experiment": "baseline", "outcome": "rejected"}
                plan["experiments"][2]["after"] = {"experiment": "repair", "outcome": "rejected"}
            controller = Controller(root, "smoke", request)
            controller.freeze(plan)
            assert controller.poll()["status"] == "unchanged"

            def await_result():
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    report = controller.poll()
                    if report["status"] in ("result", "needs_reconciliation"):
                        return report
                    assert report["status"] == "observing", report
                    time.sleep(.1)
                raise AssertionError("Private independent evaluation fixture timed out")

            if continuations:
                signal.write_text("one actual evidence change")
                original_signal = signal.read_bytes()
                assert controller.poll()["status"] == "dispatched"
                rejected = await_result()
                assert rejected["result"]["independently_accepted"] is False, rejected
                followup = controller.poll()
                assert followup["status"] == "dispatched", followup
                accepted = await_result()
                assert accepted["result"]["independently_accepted"] is True, accepted
                assert controller.poll()["status"] == "skipped"
                assert controller.poll()["status"] == "exhausted"
                assert signal.read_bytes() == original_signal
                with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
                    intake = json.loads(db.execute("SELECT intake FROM jobs WHERE run_id=?", (followup["run_id"],)).fetchone()[0])
                    assert intake["candidate_feedback"]["prior_run"] == rejected["run_id"]
                    assert intake["candidate_feedback"]["source"] == sources[0]
                    assert intake["candidate_feedback"]["outcome"] == "rejected"
                    assert db.execute("SELECT COUNT(*) FROM jobs").fetchone()[0] == 2
                    assert db.execute("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'").fetchone()[0] == 4
                status = controller.status()
                assert status["reserved_model_calls"] == 2
                assert status["independently_accepted_trials"] == 1
                assert len(status["skipped_trials"]) == 1
                print("PASS: actual HTTP/native writes/readbacks and isolated Rust failure->frozen conditional follow-up->PASS; exact predecessor source/diagnostic in approved intake; unchanged input after initial signal; unused repair skipped; no promotion. Source intents simulated; zero model inference.")
                return
            results = []
            for index in range(2):
                signal.write_text("owner evidence " + str(index))
                assert controller.poll()["status"] == "dispatched"
                result = await_result()
                assert result["status"] == "result", result
                independent = result["result"]
                assert independent["runtime_status"] == "candidate_submitted"
                assert independent["independently_accepted"] is (index == 0), independent
                assert independent["evaluation"]["native_result"]["stage"] == "acceptance"
                assert independent["promotion_authorized"] is False
                assert independent["model_calls"] == 0
                snapshot = api("/api/runs/" + result["run_id"])
                assert not any(event["kind"] == "call_started" for event in snapshot["events"])
                assert sum(event["kind"] == "tool_started" for event in snapshot["events"]) == 2
                results.append(result)
            assert controller.status()["independently_accepted_trials"] == 1
            if research:
                from research_agenda import Agenda
                envelope = {"version": 1, "company": "one", "researcher": "Researcher Gent", "author": "Engineer Gent",
                    "case": {"task": trial["task"], "source_path": "candidate.rs", "evaluation": trial["evaluation"]},
                    "failed_reference": {"company": "one", "run_id": results[1]["run_id"], "actor": "Engineer Gent"},
                    "teacher_reference": {"company": "one", "run_id": results[0]["run_id"], "actor": "Engineer Gent"},
                    "max_cycles": 2, "max_model_calls": 4}
                agenda = Agenda(root, "native-fixture", request)
                assert agenda.freeze(envelope)["status"] == "ready"
                before = len(mutations)
                for _ in range(5):
                    assert agenda.poll()["status"] == "ready"
                assert len(mutations) == before
                agenda.start()
                deadline = time.monotonic() + 30
                statuses = []
                while time.monotonic() < deadline:
                    # Reconstruct on every poll: both source and call claims must survive.
                    agenda = Agenda(root, "native-fixture", request)
                    report = agenda.poll()
                    statuses.append(report["status"])
                    if report["status"] == "accepted":
                        break
                    assert report["status"] in ("active", "reconciling"), report
                    time.sleep(.1)
                assert report["status"] == "accepted", report
                assert "reconciling" in statuses
                assert report["reserved_model_calls"] == 2
                before = len(mutations)
                journal = (agenda.storage.folder / "ledger.sqlite3").read_bytes()
                for _ in range(20):
                    assert agenda.poll() == report
                assert (agenda.storage.folder / "ledger.sqlite3").read_bytes() == journal
                assert len(mutations) == before
                assert sum(path == "/api/runs" for path, _ in mutations) == 4
                with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
                    assert db.execute("SELECT COUNT(*) FROM jobs").fetchone()[0] == 4
                    assert db.execute("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'").fetchone()[0] == 8
                for run in seeded:
                    snapshot = api("/api/runs/" + run)
                    assert not any(e["kind"] == "call_started" for e in snapshot["events"])
                print("PASS: real HTTP/native researcher-note -> fixed author -> isolated acceptance; lost research dispatch response recovered by original UUID across restarts; two-call cycle reservation; 20 unchanged terminal polls with no API mutations or ledger rewrite. Source intents simulated; zero inference, no promotion.")
                return
            signal.write_text("owner evidence uncertain")
            assert controller.poll()["status"] == "dispatched"
            actual = evaluation.run_native
            completed = []
            def lost(runtime, request):
                result = actual(runtime, request)
                assert result["passed"] is True
                completed.append(result)
                raise OSError("fixture: result lost after actual isolated acceptance")
            with patch.object(evaluation, "run_native", side_effect=lost):
                assert await_result()["status"] == "needs_reconciliation"
                controller = Controller(root, "smoke", request)
                assert controller.poll()["status"] == "needs_reconciliation"
            assert len(completed) == 1
            assert len(seeded) == 3
            assert controller.status()["independently_accepted_trials"] == 1
            with sqlite3.connect(root / "data/companies/one/brain.sqlite3") as db:
                assert db.execute("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'").fetchone()[0] == 6
            print("PASS: actual HTTP, native source writes/full readbacks and isolated Rust acceptance; distinct passing/failing outcomes; lost actual evaluator result held without replay after controller restart. Source intents simulated; zero model calls, no code adoption or promotion.")
        finally:
            child.terminate()
            if child.wait(timeout=15) != 0:
                raise RuntimeError("private evaluation harness did not stop cleanly")


if __name__ == "__main__":
    if any(arg not in ("--continuation", "--research") for arg in sys.argv[1:]):
        raise SystemExit("unknown private fixture option")
    main(continuations="--continuation" in sys.argv, research="--research" in sys.argv)

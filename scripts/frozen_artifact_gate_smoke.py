"""Real HTTP, native MCP and handoffs; private seeded plan/mock judgments only.

Regression for prose template names blocking a fully verified frozen contract.
This fixture is never company-competence or local-model evidence.
"""

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

import local_sqlite

REPO = Path(__file__).resolve().parents[1]
assert all(option == "--approved-workflow" for option in sys.argv[1:]), (
    "Unknown private fixture option"
)
APPROVED_WORKFLOW = "--approved-workflow" in sys.argv[1:]
with tempfile.TemporaryDirectory(prefix="gents-frozen-artifact-gate-") as temporary:
    root = Path(temporary)
    binary_dir = root / "target/debug"
    binary_dir.mkdir(parents=True)
    for name in ["gents-local-mcp", "gents-sqlite-bridge"]:
        shutil.copyfile(REPO / "target/debug" / name, binary_dir / name)
        (binary_dir / name).chmod(0o500)
    model = {"provider": "mock", "model": "mock-review-protocol"}
    actors = ["Analyst", "Writer"]
    # Required native worker declarations are deliberately unloadable; the host
    # contract must execute without a worker generation. QA/admin remain mock.
    unloaded_worker = {
        "provider": "local",
        "model": "unloaded-private-fixture",
        "path": "models/absent-fixture.gguf",
        "context_tokens": 8192,
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": actor,
                "assignment": "Private regression only",
                "planner": model,
                "worker": unloaded_worker,
            }
            for actor in actors
        ],
        "max_steps": 1,
        "max_tokens": 1024,
        "max_run_seconds": 120,
    }
    config_path = root / "config.json"
    config_path.write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    env = {
        key: value
        for key, value in os.environ.items()
        if key
        in {"PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "WINDIR"}
    }
    env.update(
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{port}",
        CONFIG=str(config_path),
        RUN_STORE=str(root / "runs.sqlite3"),
        GENTS_DISABLE_SCHEDULER="1",
        GENTS_OFFICE_ACTIVITY="0",
        GENTS_LOCAL_ONLY="1",
    )
    base = f"http://127.0.0.1:{port}"

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req, timeout=5) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise AssertionError(
                f"Private fixture {path}: {error.code}: {error.read(4096).decode()}"
            ) from error

    process = subprocess.Popen(
        [str(REPO / "target/debug/gents-harness")],
        cwd=root,
        env=env,
        stdout=subprocess.DEVNULL,
    )
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None, "Private regression harness exited"
                if attempt == 99:
                    raise AssertionError("Private regression startup timed out")
                time.sleep(0.1)
        company, run, session = (
            "frozen-output-test",
            str(uuid.uuid4()),
            str(uuid.uuid4()),
        )
        op = lambda kind, **fields: request(
            f"/api/companies/{company}", {"op": kind, **fields}
        )
        op("register", name="Private frozen output regression")
        task = "Deliver STEM_results.json for each input label. Replace STEM with SampleA and SampleB. Use the native integer input measurement values 7,-10 and subtract 2; preserve its exact calculation array in both files."
        criteria = [
            "Both named outputs equal the actual native calculation array and have full readbacks"
        ]
        assignments = [
            {
                "actor": "Analyst",
                "task": "Calculate the specified native input",
                "tools": ["integer_arithmetic"],
                "acceptance": ["Actual native result is available"],
            },
            {
                "actor": "Writer",
                "task": "Copy both requested output arrays and read them back",
                "tools": ["files:read", "files:write"],
                "acceptance": criteria,
            },
        ]
        source = {
            "id": "calculation",
            "actor": "Analyst",
            "tool": "integer_arithmetic",
            "arguments": {
                "calculations": [
                    {"id": "measurement", "values": [7, -10], "subtract": 2}
                ]
            },
        }
        names = ["SampleA_results.json", "SampleB_results.json"]
        proposal = {
            "status": "ready",
            "question": "",
            "definition_of_done": criteria,
            "assignments": assignments,
            "native_contracts": {
                "sources": [source],
                "artifacts": [
                    {
                        "path": name,
                        "source": "calculation",
                        "writer": "Writer",
                        "pointer": "/calculations",
                        "encoding": "json",
                    }
                    for name in names
                ],
            },
        }
        # Deliberately seed ONLY this disposable test's retained proposal. The
        # production endpoint requires real local planning; it is not bypassed
        # for issued company work or counted as a real CEO conversation here.
        planning = {
            "id": session,
            "revision": 1,
            "status": "ready",
            "source_inputs": [],
            "source_files": None,
            "messages": [{"role": "user", "kind": "requirements", "message": task}],
            "proposal": proposal,
        }
        with local_sqlite.connect(
            root / f"data/companies/{company}/brain.sqlite3"
        ) as db:
            db.execute(
                "CREATE TABLE IF NOT EXISTS ceo_planning(id TEXT PRIMARY KEY,revision INTEGER NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL)"
            )
            db.execute(
                "INSERT INTO ceo_planning VALUES(?,1,'ready',?)",
                (session, json.dumps(planning)),
            )
        intake = {
            "idea": task,
            "acceptance": "\n".join(criteria),
            "exclusions": "paid services, external execution",
            "timeline": "private test",
            "company_signer": "Fixture",
            "provider_signer": "Fixture",
            "budget_usd": 0,
            "deliverable_type": "agent",
            "ceo_planning_id": session,
            "ceo_planning_revision": 1,
        }
        state = op("intake", run_id=run, intake=intake)
        frozen_job = next(job for job in state["jobs"] if job["run_id"] == run)
        digest = frozen_job["digest"]
        op("approve", run_id=run, digest=digest)
        grants = {}
        for actor, stage in zip(actors, assignments):
            tools = stage["tools"]
            assert isinstance(tools, list)
            grants[actor] = {
                "tools": tools + ["finish"],
                "skills": [],
                "memory_from": [] if actor == "Analyst" else ["Analyst"],
            }
        if APPROVED_WORKFLOW:
            for grant in grants.values():
                grant["contract_execution_phases"] = ["worker"]
        op(
            "governance_install",
            run_id=run,
            expected_revision=0,
            policy={
                "revision": 1,
                "master": "Analyst",
                "ceilings": grants,
                "grants": grants,
            },
        )
        request(
            "/api/runs",
            {
                "company_id": company,
                "run_id": run,
                "task": task,
                "definition_of_done": criteria,
                "consent": False,
            },
        )
        deadline = time.monotonic() + 75
        print(
            json.dumps({"private_fixture_pid": process.pid, "origin": base}), flush=True
        )
        last_report = 0.0
        while True:
            saved = request("/api/runs/" + run)
            if saved["status"] != "running":
                break
            if time.monotonic() - last_report > 10:
                print(
                    json.dumps(
                        {
                            "private_status": saved["status"],
                            "events": len(saved["events"]),
                            "last": [
                                {
                                    "kind": e["kind"],
                                    "message": e.get("message", "")[:600],
                                }
                                for e in saved["events"][-3:]
                            ],
                        }
                    ),
                    flush=True,
                )
                last_report = time.monotonic()
            assert time.monotonic() < deadline, json.dumps(
                {
                    "private_timeout": True,
                    "last_events": [
                        {"kind": e["kind"], "message": e.get("message", "")[:1200]}
                        for e in saved["events"][-10:]
                    ],
                }
            )
            time.sleep(0.2)
        assert saved["status"] == "completed", saved["status"]
        events = saved["events"]
        admission = [
            json.loads(e["message"])["proposal"]
            for e in events
            if e["kind"] == "admin_decision"
            and json.loads(e["message"])["proposal"].get("request_admission")
        ]
        assert len(admission) == 1
        assert admission[0]["frozen_definition_of_done"] == criteria
        assert admission[0]["frozen_planning_phase"]["completed"] is True
        assert admission[0]["frozen_planning_phase"]["authority_granted"] is False
        assert (
            admission[0]["frozen_planning_phase"]["plan_sha256"]
            == frozen_job["intake"]["ceo_plan_sha256"]
        )
        assert admission[0]["frozen_rust_checks"] == frozen_job["intake"].get(
            "required_rust_checks"
        )
        submissions = [
            json.loads(e["message"])
            for e in events
            if e["kind"] == "frozen_contract_submission"
        ]
        assert [row["stage"] for row in submissions] == [0, 1]
        assert all(len(row["verified_gate_sha256"]) == 64 for row in submissions)
        reviews = [
            json.loads(e["message"]) for e in events if e["kind"] == "completion_review"
        ]
        assert reviews and all(
            row["artifact_gate"]["named_artifact_source"] == "owner_frozen_bindings"
            for row in reviews
        )
        work = root / f"data/companies/{company}/runs/{run}"
        workspace = work / "workspace"
        assert (
            not (workspace / "STEM_results.json").exists()
            and not (workspace / "stem_results.json").exists()
        )
        assert (workspace / names[0]).read_bytes() == (
            workspace / names[1]
        ).read_bytes()
        checkpoint = json.loads((work / "autonomy.json").read_bytes())
        assert checkpoint["evidence"]["schema_version"] == 5
        for name in names:
            observed = checkpoint["evidence"]["artifacts"][name]
            assert (
                observed["state"] == "native_readback_observed"
                and observed["readback_step"] > observed["step"]
            )
        calls = [json.loads(path.read_bytes()) for path in work.glob("tool-*.json")]
        assert len(calls) == 5 and all(row["result"]["result"]["ok"] for row in calls)
        if APPROVED_WORKFLOW:
            assert all(
                row["result"]["result"]["mcp_receipt"]["contract_enforced"]
                for row in calls
            )
            decisions = [
                json.loads(e["message"])
                for e in events
                if e["kind"] == "authorization_decision"
            ]
            assert len(decisions) == 7 and all(
                d["verdict"]["decision"] == "allow"
                and d["authorization"]["model_called"] is False
                for d in decisions
            )
            assert not any(
                e["kind"] == "admin_decision"
                and json.loads(e["message"])["proposal"].get("request")
                for e in events
            )

        assert not any(
            e["kind"] == "call_started" and e["role"] == "worker" for e in events
        )
        print(
            "PASS: private real HTTP/native two-stage workflow completes using exact frozen names, canonical copies/readbacks and independently gated submissions; no invented template files; simulated planning and QA only; no model competence claim; owner_mode="
            + ("approved_workflow" if APPROVED_WORKFLOW else "model_review")
        )
    finally:
        process.terminate()
        assert process.wait(timeout=15) == 0

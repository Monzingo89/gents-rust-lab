"""Private HTTP/native Rust submission/call wiring; seeded intent and mock judges.

No local-model competence or benchmark qualification follows from this fixture.
The author model is deliberately unloadable: native PASS must compile a review
request before the controller tries another author generation.
The owned-call variant seeds both invocation and submission intents, then
verifies real native scope/completion gates and separate admin/QA review.
"""

import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path

import local_sqlite

assert all(arg in {"--sse", "--owned"} for arg in sys.argv[1:]), (
    "Unknown private fixture option"
)
owned = "--owned" in sys.argv[1:]
if sys.platform != "darwin":
    print("SKIP: actual isolated Rust execution needs the supported macOS adapter")
    raise SystemExit(0)
REPO = Path(__file__).resolve().parents[1]
transport = "sse" if "--sse" in sys.argv[1:] else "stdio"
compiler = (
    os.environ.get("GENTS_RUSTC")
    or subprocess.check_output(
        ["rustup", "which", "--toolchain", "stable", "rustc"], text=True
    ).strip()
)
assert Path(compiler).is_absolute() and Path(compiler).is_file()

with tempfile.TemporaryDirectory(prefix="gents-rust-submission-http-") as temporary:
    root = Path(temporary)
    binary_dir = root / "target/debug"
    binary_dir.mkdir(parents=True)
    for name in ["gents-local-mcp", "gents-sqlite-bridge"]:
        shutil.copyfile(REPO / "target/debug" / name, binary_dir / name)
        (binary_dir / name).chmod(0o500)
    actor = "Engineer Gent"
    judge = {"provider": "mock", "model": "mock-review-protocol"}
    worker = {
        "provider": "local",
        "model": "unloaded-private-fixture",
        "path": "models/absent-fixture.gguf",
        "context_tokens": 8192,
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": judge,
        "agents": [
            {
                "id": actor,
                "assignment": "Private protocol fixture only",
                "planner": judge,
                "worker": worker,
            }
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
    base = f"http://127.0.0.1:{port}"
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
        GENTS_MCP_TRANSPORT=transport,
        GENTS_RUSTC=compiler,
    )

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

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
                assert process.poll() is None
                if attempt == 99:
                    raise AssertionError("Private Rust submission startup failed")
                time.sleep(0.1)
        company, run, session = (
            "rust-submission-test",
            str(uuid.uuid4()),
            str(uuid.uuid4()),
        )
        op = lambda kind, **fields: request(
            f"/api/companies/{company}", {"op": kind, **fields}
        )
        op("register", name="Private controller regression, not company competence")
        task = "Submit this private protocol function after actual write, full readback and native acceptance. No other work is requested."
        criteria = [
            "Current protocol source passes its frozen native acceptance and full readback before independent review"
        ]
        tools = ["files:write", "files:read", "rust_verify"]
        if owned:
            task = "Invoke the frozen private protocol library with its exact input, then submit its native result to independent review."
            criteria = [
                "The exact owner-frozen library, function and input have a successful current native invocation"
            ]
            tools = ["owned_rust_call"]
        proposal = {
            "status": "ready",
            "question": "",
            "definition_of_done": criteria,
            "assignments": [
                {"actor": actor, "task": task, "tools": tools, "acceptance": criteria}
            ],
            "native_contracts": {"sources": [], "artifacts": []},
        }
        planning = {
            "id": session,
            "revision": 1,
            "status": "ready",
            "planning_mode": "implementation",
            "source_inputs": [],
            "source_files": None,
            "proposal": proposal,
            "messages": [{"role": "user", "kind": "requirements", "message": task}],
        }
        # Seed only this disposable controller fixture, never an issued task.
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
        source = "pub fn protocol_value()->u64{1}"
        tests = "#[test]fn protocol(){std::assert_eq!(candidate::protocol_value(),1);}"
        source_hash = hashlib.sha256(source.encode()).hexdigest()
        tests_hash = hashlib.sha256(tests.encode()).hexdigest()
        if owned:
            source = "pub fn protocol_value(x:u64)->u64{x}"
            source_hash = hashlib.sha256(source.encode()).hexdigest()
            description = json.loads(
                subprocess.check_output(
                    [str(REPO / "target/debug/gents-rust-component")],
                    input=json.dumps({"op": "describe", "source": source}),
                    text=True,
                    env=env,
                )
            )
        intake = {
            "idea": task,
            "acceptance": "\n".join(criteria),
            "exclusions": "paid services, downloads, external work",
            "timeline": "private regression",
            "company_signer": "Fixture",
            "provider_signer": "Fixture",
            "budget_usd": 0,
            "deliverable_type": "agent",
            "ceo_planning_id": session,
            "ceo_planning_revision": 1,
            "required_mcp_calls": [
                {"actor": actor, "tool": name} for name in ["files", "rust_verify"]
            ],
            "required_rust_checks": [
                {
                    "actor": actor,
                    "source_path": "protocol.rs",
                    "tests_path": "acceptance.rs",
                    "tests_sha256": tests_hash,
                }
            ],
        }
        if owned:
            intake.pop("required_rust_checks")
            intake["required_mcp_calls"] = [{"actor": actor, "tool": "owned_rust_call"}]
            intake["required_owned_rust_calls"] = [
                {
                    "id": "owner-protocol",
                    "actor": actor,
                    "source_path": "protocol.rs",
                    "source_sha256": source_hash,
                    "api_sha256": description["api_sha256"],
                    "function": "protocol_value",
                    "input_sha256": hashlib.sha256(b'["17"]').hexdigest(),
                    "owner_declared_review_package_id": "0" * 64,
                }
            ]
        state = op("intake", run_id=run, intake=intake)
        job = next(job for job in state["jobs"] if job["run_id"] == run)
        op("approve", run_id=run, digest=job["digest"])
        grant = {"tools": tools + ["finish"], "skills": [], "memory_from": []}
        op(
            "governance_install",
            run_id=run,
            expected_revision=0,
            policy={
                "revision": 1,
                "master": actor,
                "ceilings": {actor: grant},
                "grants": {actor: grant},
            },
        )
        work = root / f"data/companies/{company}/runs/{run}"
        workspace = work / "workspace"
        workspace.mkdir(parents=True, exist_ok=True, mode=0o700)
        if owned:
            (workspace / "protocol.rs").write_text(source)
        else:
            (workspace / "acceptance.rs").write_text(tests)
        # A pending proposal is not a receipt: the real controller must still
        # authorize and execute it, then derive read/verify and the finish request.
        queue = [
            {
                "action": "files",
                "arguments": {"op": "write", "path": "protocol.rs", "content": source},
                "summary": "SIMULATED unexecuted protocol intent, not authored benchmark code",
            }
        ]
        if owned:
            queue = [
                {
                    "action": "owned_rust_call",
                    "arguments": {"binding_id": "owner-protocol", "arguments": ["17"]},
                    "summary": "SIMULATED unexecuted owner protocol invocation",
                },
                {
                    "action": "finish",
                    "arguments": {
                        "deliverable": "Submit exact frozen protocol invocation",
                        "evidence": "Independent native gates must still pass",
                    },
                    "summary": "SIMULATED submission intent, not acceptance",
                },
            ]
        (work / "autonomy.json").write_text(json.dumps({"plan_queue": queue}))
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
        deadline = time.monotonic() + 45
        while True:
            saved = request("/api/runs/" + run)
            if saved["status"] != "running":
                break
            assert time.monotonic() < deadline, {
                "private_fixture_timeout": True,
                "kinds": [e["kind"] for e in saved["events"][-8:]],
            }
            time.sleep(0.2)
        assert saved["status"] == "completed", saved["status"]
        events = saved["events"]
        submissions = [
            json.loads(e["message"])
            for e in events
            if e["kind"] == "frozen_contract_submission"
        ]
        assert len(submissions) == (0 if owned else 1)
        if not owned:
            submission = submissions[0]
            assert submission["origin"] == "host_compiled_frozen_rust_submission"
            assert (
                submission["model_generated_summary"] is False
                and submission["authority_granted"] is False
            )
            assert (
                submission["independent_qa_required"] is True
                and len(submission["verified_gate_sha256"]) == 64
            )
            assert submission["rust_sources"] == [
                {
                    "actor": actor,
                    "source_path": "protocol.rs",
                    "source_sha256": source_hash,
                    "tests_sha256": tests_hash,
                }
            ]
        assert not any(
            e["kind"] == "call_started" and e["role"] == "worker" for e in events
        )
        decisions = [
            json.loads(e["message"]) for e in events if e["kind"] == "admin_decision"
        ]
        assert any(
            d.get("proposal", {}).get("request", {}).get("action") == "finish"
            and d["model_verdict"]["decision"] == "allow"
            for d in decisions
        )
        assert any(e["kind"] == "completion_review" for e in events)
        calls = [
            json.loads(path.read_bytes()) for path in sorted(work.glob("tool-*.json"))
        ]
        assert [c["request"]["action"] for c in calls] == (
            ["owned_rust_call"] if owned else ["files", "files", "rust_verify"]
        )
        if not owned:
            assert [c["request"]["arguments"].get("op") for c in calls[:2]] == [
                "write",
                "read",
            ]
        for call in calls:
            result = call["result"]["result"]
            assert result["ok"] and result["mcp_receipt"]["actor"] == actor
            assert result["mcp_receipt"]["policy_revision"] == 1
        assert calls[-1]["result"]["result"]["result"]["passed"] is True
        assert (workspace / "protocol.rs").read_text() == source
        if owned:
            result = calls[0]["result"]["result"]
            assert result["result"]["output"] == "17"
            assert result["result"]["source_sha256"] == source_hash
            assert result["mcp_receipt"]["tool"] == "owned_rust_call"
            print(
                f"PASS: actual HTTP/{transport} owner-frozen library/input native invocation and gated finish; admin and independent QA retained; zero author generations; seeded intents/mock judgments/protocol source only, no model competence or package qualification claim"
            )
        else:
            assert (workspace / "acceptance.rs").read_text() == tests
            print(
                f"PASS: actual HTTP/{transport} native write/readback/acceptance then one host-compiled finish; admin and independent QA retained; zero author generations; seeded pending intent/mock judgments only, no model competence claim"
            )
    finally:
        process.terminate()
        assert process.wait(timeout=15) == 0

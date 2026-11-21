"""Private factory HTTP contract check; seeded synthetic plan, no model/task execution."""

import copy
import json
import os
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

import local_sqlite

REPO = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


with tempfile.TemporaryDirectory(prefix="gents-factory-http-") as temporary:
    root = Path(temporary).resolve()
    model = {
        "provider": "local",
        "model": "unavailable-factory-fixture",
        "path": "models/not-installed.gguf",
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Worker",
                "assignment": "Synthetic fixture",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 256,
    }
    (root / "config.json").write_text(json.dumps(config))
    http_port, model_port = free_port(), free_port()
    env = dict(
        os.environ,
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{http_port}",
        CONFIG=str(root / "config.json"),
        RUN_STORE=str(root / "runs.sqlite3"),
        LOCAL_MODEL_PORT=str(model_port),
        GENTS_SQLITE_BRIDGE=str(REPO / "target/debug/gents-sqlite-bridge"),
        GENTS_LOCAL_ONLY="1",
        GENTS_DISABLE_SCHEDULER="1",
        GENTS_OFFICE_ACTIVITY="0",
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
    )
    base = f"http://127.0.0.1:{http_port}"

    def request(path, body=None, headers=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json", **(headers or {})},
        )
        with urllib.request.urlopen(req, timeout=15) as response:
            return json.load(response)

    def rejected(path, body, status=400, headers=None):
        try:
            request(path, body, headers)
        except urllib.error.HTTPError as error:
            assert error.code == status, error.code
            error.close()
        else:
            raise AssertionError("Expected explicit rejection")

    process = subprocess.Popen(
        [str(REPO / "target/debug/gents-harness")],
        cwd=root,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None, "private factory harness exited"
                if attempt == 99:
                    raise AssertionError("private factory startup timed out")
                time.sleep(0.05)
        for company in ("factory-owner", "factory-foreign"):
            request(
                f"/api/companies/{company}",
                {"op": "register", "name": "Synthetic factory HTTP fixture"},
            )
        sid = str(uuid.uuid4())
        plan = {
            "status": "ready",
            "revision": 2,
            "proposal": {
                "status": "ready",
                "question": "",
                "definition_of_done": ["Read actual fixture bytes"],
                "assignments": [
                    {
                        "actor": "Worker",
                        "task": "Read fixture",
                        "tools": ["files:read"],
                        "acceptance": ["Native read receipt"],
                    }
                ],
            },
        }
        database = root / "data/companies/factory-owner/brain.sqlite3"
        with local_sqlite.connect(database) as db:
            db.execute(
                "CREATE TABLE ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)"
            )
            db.execute(
                "INSERT INTO ceo_planning VALUES(?,2,'ready',?)",
                (sid, json.dumps(plan)),
            )
            counts = {
                table: (db.execute(f"SELECT COUNT(*) FROM {table}").fetchone() or (0,))[
                    0
                ]
                for table in ("jobs", "audit", "run_governance")
            }
        with local_sqlite.connect(
            root / "data/companies/factory-foreign/brain.sqlite3"
        ) as db:
            db.execute("UPDATE company SET owner='different-private-owner'")
        path = f"/api/companies/factory-owner/planning/{sid}/blueprint"
        body = {
            "expected_revision": 2,
            "specification": {
                "version": 1,
                "objective": "Read synthetic fixture",
                "inputs": ["fixture.txt"],
                "outputs": ["Read receipt"],
                "definition_of_done": ["Read actual fixture bytes"],
                "exclusions": ["Network and paid services"],
                "coordination": "ordered",
                "data_classification": "synthetic",
                "control_profiles": ["local_baseline"],
                "required_capabilities": [],
            },
        }
        result = request(path, body)
        assert result["blueprint"]["status"] == "ready_for_owner_review", result
        assert result["blueprint"]["architecture"]["id"] == "flow"
        assert (
            result["blueprint"]["preview_only"] is True
            and result["execution_authorized"] is False
        )
        assert len(result["blueprint_sha256"]) == 64
        for coordination, architecture in (
            ("adaptive_delegation", "orchestration"),
            ("peer_review", "collaboration"),
        ):
            changed = copy.deepcopy(body)
            changed_specification = changed["specification"]
            assert isinstance(changed_specification, dict)
            changed_specification["coordination"] = coordination
            blocked = request(path, changed)["blueprint"]
            assert (
                blocked["status"] == "missing_capability"
                and blocked["architecture"]["id"] == architecture
            )
            assert blocked["architecture"]["executor"] is None
            assert blocked["proposed_communication_controls"] is None
            assert {gap["code"] for gap in blocked["gaps"]} == {
                "executor_not_implemented"
            }
        changed = copy.deepcopy(body)
        changed_specification = changed["specification"]
        assert isinstance(changed_specification, dict)
        changed_specification["required_capabilities"] = ["owner_tool_not_yet_built"]
        assert "owned_tool_missing" in {
            gap["code"] for gap in request(path, changed)["blueprint"]["gaps"]
        }
        rejected(path, {**body, "expected_revision": 1})
        rejected(path, {**body, "execution_authorized": True})
        rejected(path, body, 403, {"Origin": "https://untrusted.example"})
        rejected(f"/api/companies/factory-foreign/planning/{sid}/blueprint", body)
        rejected(f"/api/companies/missing-company/planning/{sid}/blueprint", body)
        assert request("/api/runs")["runs"] == []
        with local_sqlite.connect(database) as db:
            planning_row = db.execute(
                "SELECT body FROM ceo_planning WHERE id=?", (sid,)
            ).fetchone()
            assert planning_row is not None
            planning_body = planning_row[0]
            assert isinstance(planning_body, (str, bytes, bytearray))
            assert json.loads(planning_body) == plan
            assert all(
                (db.execute(f"SELECT COUNT(*) FROM {table}").fetchone() or (0,))[0]
                == count
                for table, count in counts.items()
            )
        with socket.socket() as listener:
            assert listener.connect_ex(("127.0.0.1", model_port)) != 0, (
                "preview must not launch inference"
            )
    finally:
        process.terminate()
        process.wait(timeout=10)

print(
    "PASS: synthetic factory HTTP preview, ownership/revision/ingress checks, explicit missing capabilities; no inference, jobs or policy changes"
)

"""Real HTTP/CLI evidence path in a disposable root with synthetic media review.

No business SOW, user media approval or real model inference is performed here.
"""

import io
import json
import os
import socket
import subprocess
import time
import urllib.error
import urllib.request
import uuid
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

import gents
import task_evidence as task
from test_task_evidence import EvidenceBoundaries

ROOT = Path(__file__).resolve().parents[1]
fixture = EvidenceBoundaries()
fixture.setUp()
process = None
try:
    root, company, package = fixture.root, fixture.company, fixture.package
    directory, digest = fixture.approved_fixture()
    model = {"provider": "mock", "model": "mock"}
    config = {
        "company_mode": True,
        "agents": [
            {
                "id": "Test Gent",
                "assignment": "review",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 64,
        "max_run_seconds": 30,
    }
    (root / "config.json").write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    base = f"http://127.0.0.1:{port}"
    process = subprocess.Popen(
        [str(ROOT / "target/debug/gents-harness")],
        cwd=root,
        env=dict(
            os.environ,
            AUTH_MODE="local",
            BIND=f"127.0.0.1:{port}",
            CONFIG=str(root / "config.json"),
            RUN_STORE=":memory:",
        ),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    def request(path, body=None):
        req = urllib.request.Request(
            base + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def op(kind, **data):
        return request("/api/companies/" + company, {"op": kind, **data})

    for _ in range(100):
        try:
            request("/api/config")
            break
        except OSError:
            time.sleep(0.05)
    assert request("/api/config")["task_evidence_version"] == 1
    op("register", name="Fictional test tenant")
    run = str(uuid.uuid4())
    intake = {
        "idea": "Synthetic evidence audit",
        "acceptance": "Include all 12 employee decisions",
        "exclusions": "paid services",
        "timeline": "unknown",
        "company_signer": "simulation",
        "provider_signer": "simulation",
        "budget_usd": 0,
        "deliverable_type": "text",
        "task_evidence": {
            "profile": task.PROFILE,
            "package_id": package,
            "sha256": digest,
        },
    }
    state = op("intake", run_id=run, intake=intake)
    job = next(j for j in state["jobs"] if j["run_id"] == run)
    argv = ["gents.py", "--server", base, "--company", company, "start", run]
    with (
        patch.object(gents, "ROOT", root),
        patch("sys.argv", argv),
        redirect_stdout(io.StringIO()),
    ):
        try:
            gents.main()
        except ValueError as error:
            assert "Approve" in str(error)
        else:
            raise AssertionError("Unapproved audit must not execute")
    assert not (root / "data/companies" / company / "runs" / run / "audit").exists()
    op("approve", run_id=run, digest=job["digest"])
    # Typed runs cannot start without real checked artifacts, even after SOW approval.
    try:
        request(
            "/api/runs",
            {
                "company_id": company,
                "run_id": run,
                "task": intake["idea"],
                "consent": False,
                "definition_of_done": [intake["acceptance"]],
            },
        )
    except urllib.error.HTTPError as error:
        assert error.code in (400, 409)
    else:
        raise AssertionError("Typed audit started without evidence artifacts")
    with (
        patch.object(gents, "ROOT", root),
        patch("sys.argv", argv),
        redirect_stdout(io.StringIO()) as captured,
    ):
        gents.main()
    assert "Deterministic audit verified" in captured.getvalue()
    for _ in range(100):
        saved = request("/api/runs/" + run)
        if saved["status"] != "running":
            break
        time.sleep(0.05)
    assert any(
        e["kind"] == "tool_result" and e["agent"] == "Audit Tool"
        for e in saved["events"]
    )
    output = root / "data/companies" / company / "runs" / run / "audit"
    assert all(task.verify(directory, output).values())
    with (
        patch.object(gents, "ROOT", root),
        patch(
            "sys.argv",
            ["gents.py", "--server", base, "--company", company, "verify-audit", run],
        ),
        redirect_stdout(io.StringIO()),
    ):
        gents.main()
    print(
        "PASS: real HTTP/CLI approval, typed audit admission, frozen evidence, 12 verified decisions, run-scoped attachments and office tool event; mock role review is not real-model validation"
    )
finally:
    if process is not None:
        process.terminate()
        process.wait(timeout=10)
    fixture.tearDown()

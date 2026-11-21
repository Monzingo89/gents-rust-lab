"""Exercise tenant control boundaries against a real HTTP server in a disposable root."""

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

ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="gents-company-smoke-") as temporary:
    root = Path(temporary)
    model = {"provider": "mock", "model": "mock"}
    config = {
        "company_mode": True,
        "agents": [
            {"id": "Test Gent", "assignment": "test", "planner": model, "worker": model}
        ],
        "max_steps": 1,
        "max_tokens": 64,
    }
    path = root / "config.json"
    path.write_text(json.dumps(config))
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
            CONFIG=str(path),
            RUN_STORE=":memory:",
        ),
        stdout=subprocess.DEVNULL,
    )

    def request(path, body):
        req = urllib.request.Request(
            base + path,
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    def op(company, kind, **data):
        return request("/api/companies/" + company, {"op": kind, **data})

    def rejected(action):
        try:
            action()
        except urllib.error.HTTPError as error:
            assert error.code in (400, 409), error.code
        else:
            raise AssertionError("request should have failed")

    try:
        for _ in range(100):
            try:
                with urllib.request.urlopen(base + "/health", timeout=1):
                    break
            except OSError:
                time.sleep(0.05)
        for company in ("one", "two"):
            op(company, "register", name=company)
        op("one", "ingest", source="handbook.md", text="Company one private evidence")
        assert op("two", "status")["tiers"] == []
        rejected(
            lambda: op(
                "one", "ingest", source="owner-evaluation/secret.txt", text="excluded"
            )
        )
        rejected(lambda: request("/api/runs", {"task": "test", "consent": False}))
        run = str(uuid.uuid4())
        intake = {
            "idea": "test",
            "acceptance": "has evidence",
            "exclusions": "paid services",
            "timeline": "unknown",
            "company_signer": "A",
            "provider_signer": "B",
            "budget_usd": 0,
            "deliverable_type": "text",
        }
        rejected(
            lambda: op("one", "intake", run_id=run, intake=dict(intake, budget_usd=1))
        )
        state = op("one", "intake", run_id=run, intake=intake)
        job = state["jobs"][0]
        assert job["approval"] is None
        payload = {"company_id": "one", "run_id": run, "task": "test", "consent": False}
        rejected(lambda: request("/api/runs", payload))
        assert not (root / "data/companies/one/runs" / run / "checkpoint.json").exists()
        rejected(lambda: op("one", "approve", run_id=run, digest="bad"))
        op("one", "approve", run_id=run, digest=job["digest"])
        rejected(lambda: request("/api/runs", dict(payload, company_id="two")))
        rejected(lambda: request("/api/runs", dict(payload, task="changed scope")))
        # Mismatched frozen criteria fail before any model call.
        request("/api/runs", dict(payload, definition_of_done=["different"]))
        for _ in range(100):
            with urllib.request.urlopen(base + "/api/runs/" + run) as response:
                saved = json.load(response)
            if saved["status"] != "running":
                break
            time.sleep(0.05)
        assert saved["status"] == "failed", saved
        assert all(
            e["company_id"] == "one" and e["run_id"] == run for e in saved["events"]
        )
        assert (root / "data/companies/one/runs" / run / "checkpoint.json").is_file()
        state = op(
            "one",
            "capability",
            manifest={"id": "sample", "source": "local-reviewed-package"},
        )
        assert state["capabilities"][0]["status"] == "quarantined"
        assert op("two", "status")["capabilities"] == []
        op("one", "archive")
        assert all(t["tier"] == "PLTC" for t in op("one", "status")["tiers"])
        archived = op("one", "memory_archive_status", run_id="onboarding")
        assert archived["backend"] == "owned_local_sqlite" and archived["checkpoints"]
        assert archived["checkpoints"][0]["lifecycle"] == "inactive"
        assert "Company one private evidence" not in json.dumps(archived)
        assert (
            op("two", "memory_archive_status", run_id="onboarding")["checkpoints"] == []
        )
    finally:
        process.terminate()
        process.wait(timeout=10)
print(
    "PASS: company scope, frozen SOW, zero budget, approval, checkpoint scoping, quarantine, cold archive"
)

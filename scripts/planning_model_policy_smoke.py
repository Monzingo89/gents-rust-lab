"""Private HTTP proof of local planning-policy refusal; zero model calls."""
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


with tempfile.TemporaryDirectory(prefix="gents-planning-policy-") as temporary:
    root = Path(temporary)
    model = {"provider": "local", "model": "unavailable-policy-fixture", "path": "models/not-installed.gguf", "context_tokens": 8192}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Fixture Gent", "assignment": "unused", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512,
              "planning_model_policy": {"failures_before_next": 2, "candidates": [
                  {"id": "missing", "model": model, "sha256": "0" * 64, "minimum_memory_mib": 512}]}}
    (root / "config.json").write_text(json.dumps(config))
    http_port, model_port = port(), port()
    assert http_port != model_port
    env = {key: value for key, value in os.environ.items() if key in {"PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "WINDIR"}}
    env.update(AUTH_MODE="local", BIND=f"127.0.0.1:{http_port}", CONFIG=str(root / "config.json"),
               RUN_STORE=str(root / "runs.sqlite3"), LOCAL_MODEL_PORT=str(model_port),
               GENTS_SQLITE_BRIDGE=str(REPO / "target/debug/gents-sqlite-bridge"), GENTS_LOCAL_ONLY="1",
               GENTS_DISABLE_SCHEDULER="1", GENTS_OFFICE_ACTIVITY="0", HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1")
    base = f"http://127.0.0.1:{http_port}"

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=10) as response:
            return json.load(response)

    process = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env, stdout=subprocess.DEVNULL)
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None, "Private policy harness exited"
                if attempt == 99:
                    raise AssertionError("Private policy startup timed out")
                time.sleep(.05)
        company = "policy-fixture"
        public_policy = request("/api/config")["planning_model_policy"]
        assert public_policy["failures_before_next"] == 2 and public_policy["candidates"][0]["id"] == "missing"
        assert "not-installed.gguf" not in json.dumps(public_policy) and "key_env" not in json.dumps(public_policy)
        request(f"/api/companies/{company}", {"op": "register", "name": "Private no-inference policy fixture"})
        session = str(uuid.uuid4())
        path = f"/api/companies/{company}/planning/{session}"
        result = request(path, {"message": "Clarify an isolated synthetic task", "expected_revision": 0})
        assert result["status"] == "failed" and result["revision"] == 1
        failure = result["inference_failure"]
        assert failure["code"] == "local_planning_policy_unavailable" and failure["inference_dispatched"] is False
        assert failure["retryable_without_host_change"] is False
        selection = failure["selection"]
        assert selection.get("reason") == "process_capacity_unavailable" or selection["skipped"][0]["reason"] in ("model_not_installed_or_unreadable", "host_capacity_below_owner_floor")
        retained = request(path)
        assert retained["status"] == "failed" and len(retained["attempts"]) == 1
        assert retained["attempts"][0]["local_execution"] is None and retained["attempts"][0]["usage"] is None
        assert request("/api/runs")["runs"] == []
        # The caller/model cannot install candidates or forge selection receipts.
        for field in ["planning_model_policy", "planning_model_selection"]:
            try:
                request(path, {"message": "Untrusted candidate override", "expected_revision": 1, field: {"provider": "remote"}})
            except urllib.error.HTTPError as error:
                assert error.code == 400
            else:
                raise AssertionError("Planning caller changed owner model policy")
        assert request(path)["revision"] == 1
        with local_sqlite.connect((root / f"data/companies/{company}/brain.sqlite3").as_uri() + "?mode=ro", uri=True) as db:
            assert db.execute("SELECT COUNT(*) FROM jobs").fetchone()[0] == 0
            assert db.execute("SELECT COUNT(*) FROM audit WHERE kind='mcp_tool_call'").fetchone()[0] == 0
            assert db.execute("SELECT COUNT(*) FROM audit WHERE kind='ceo_planning_completed'").fetchone()[0] == 1
        with socket.socket() as probe:
            assert probe.connect_ex(("127.0.0.1", model_port)) != 0, "No model backend may start on policy refusal"
        print("PASS: actual HTTP model-policy refusal becomes a durable terminal checkpoint with no inference, task, native tool or forged owner policy; infrastructure only, no model competence claim")
    finally:
        process.terminate()
        assert process.wait(timeout=15) == 0

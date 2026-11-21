"""No paid calls. Verify persistence, replay protection, deadlines and acceptance veto."""

import json
import os
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def request(base, path, body=None, method=None, headers=None):
    req = urllib.request.Request(
        base + path,
        data=None if body is None else json.dumps(body).encode(),
        method=method,
        headers={"Content-Type": "application/json", **(headers or {})},
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        return json.loads(r.read())


def expect(code, fn):
    try:
        fn()
    except urllib.error.HTTPError as e:
        assert e.code == code, (e.code, code)
    else:
        raise AssertionError(f"expected HTTP {code}")


def launch(store, config):
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
    sock.close()
    env = dict(
        os.environ,
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{port}",
        RUN_STORE=store,
        CONFIG=str(config),
    )
    proc = subprocess.Popen(
        [str(ROOT / "target/debug/gents-harness")],
        cwd=ROOT,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    base = f"http://127.0.0.1:{port}"
    for _ in range(100):
        if proc.poll() is not None:
            raise RuntimeError("startup failed")
        try:
            with urllib.request.urlopen(base + "/health", timeout=1):
                return proc, base
        except (OSError, urllib.error.URLError):
            time.sleep(0.05)
    proc.kill()
    raise TimeoutError("startup")


def wait(base, id):
    for _ in range(400):
        run = request(base, "/api/runs/" + id)
        if run["status"] != "running":
            return run
        time.sleep(0.1)
    raise TimeoutError("run")


def stop(proc):
    proc.terminate()
    proc.wait(timeout=5)


with tempfile.TemporaryDirectory() as temp:
    store = str(Path(temp) / "runs.sqlite3")
    config = ROOT / "config/fixed.mock.json"
    proc, base = launch(store, config)
    body = {"task": "Persistence and replay test", "consent": False}
    headers = {"Idempotency-Key": "recovery-example"}
    try:
        id = request(base, "/api/runs", body, headers=headers)["id"]
        replay = request(base, "/api/runs", body, headers=headers)
        assert replay["id"] == id and replay["replayed"]
        expect(
            409,
            lambda: request(
                base, "/api/runs", dict(body, task="different"), headers=headers
            ),
        )
        expect(429, lambda: request(base, "/api/runs", body))
        expect(409, lambda: request(base, "/api/runs/" + id, method="DELETE"))
        time.sleep(0.2)
        assert any(e["call_id"] for e in request(base, "/api/runs/" + id)["events"])
        # Simulate a crash while a provider request is active.
        proc.kill()
        proc.wait(timeout=5)
    finally:
        if proc.poll() is None:
            stop(proc)
    proc, base = launch(store, config)
    try:
        recovered = request(base, "/api/runs/" + id)
        assert recovered["status"] == "interrupted", recovered
        assert recovered["events"][-1]["kind"] == "interrupted"
        assert request(base, "/api/runs", body, headers=headers)["id"] == id
        assert any(r["id"] == id for r in request(base, "/api/runs")["runs"])
        request(base, "/api/runs/" + id, method="DELETE")
        expect(404, lambda: request(base, "/api/runs/" + id))
        assert not request(base, "/api/runs")["runs"]
    finally:
        stop(proc)
    print(
        "PASS: persistent idempotency, conflict detection, owner admission, timestamps/call IDs, crash recovery without replay, history and deletion"
    )
    for scenario in ["deadline", "acceptance_veto"]:
        c = json.loads((ROOT / "config/mock.json").read_text())
        c["max_run_seconds"] = 1 if scenario == "deadline" else 120
        c["judges"]["max_rounds"] = 2
        file = Path(temp) / (scenario + ".json")
        file.write_text(json.dumps(c))
        proc, base = launch(":memory:", file)
        try:
            body = {"task": "Check hard acceptance", "consent": False}
            if scenario == "acceptance_veto":
                body["acceptance_checks"] = {"forbidden_substrings": ["MOCK OUTPUT"]}
            id = request(base, "/api/runs", body)["id"]
            run = wait(base, id)
            if scenario == "deadline":
                assert run["status"] == "timed_out", run
            else:
                assert run["status"] == "incomplete", run
                assert any(e["kind"] == "acceptance_veto" for e in run["events"])
                assert any(
                    e["kind"] == "completion_review"
                    and json.loads(e["message"])["verdict"] == "accepted"
                    for e in run["events"]
                )
        finally:
            stop(proc)
    print("PASS: wall-clock deadline and deterministic veto of AI acceptance")

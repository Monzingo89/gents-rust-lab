"""Exercise real local HTTP lifecycle. Run after `cargo build --workspace`."""

import json
import os
import socket
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def request(base, path, body=None):
    req = urllib.request.Request(
        base + path,
        data=None if body is None else json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        data = r.read()
        return (
            json.loads(data)
            if r.headers.get_content_type() == "application/json"
            else data.decode()
        )


def expect_status(base, path, body, status):
    try:
        request(base, path, body)
    except urllib.error.HTTPError as e:
        assert e.code == status, (e.code, status)
    else:
        raise AssertionError(f"expected {status}")


def launch(mode, config="config/fixed.mock.json"):
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
    sock.close()
    env = dict(
        os.environ,
        BIND=f"127.0.0.1:{port}",
        AUTH_MODE=mode,
        CONFIG=config,
        RUN_STORE=":memory:",
        TENANT_ID="11111111-1111-4111-8111-111111111111",
        API_CLIENT_ID="22222222-2222-4222-8222-222222222222",
        NATIVE_CLIENT_ID="33333333-3333-4333-8333-333333333333",
    )
    process = subprocess.Popen(
        [str(ROOT / "target/debug/gents-harness")],
        cwd=ROOT,
        env=env,
        stdout=subprocess.DEVNULL,
    )
    base = f"http://127.0.0.1:{port}"
    for _ in range(100):
        if process.poll() is not None:
            raise RuntimeError("server failed")
        try:
            request(base, "/health")
            return process, base
        except (OSError, urllib.error.URLError):
            time.sleep(0.05)
    process.terminate()
    raise TimeoutError("startup")


process, base = launch("local")
try:
    assert request(base, "/health") == "ok"
    assert len(request(base, "/api/config")["agents"]) == 2
    expect_status(base, "/api/runs", {"task": "", "consent": False}, 400)
    run = request(
        base, "/api/runs", {"task": "Compare the supplied vendors", "consent": False}
    )["id"]
    for _ in range(100):
        result = request(base, "/api/runs/" + run)
        if result["status"] != "running":
            break
        time.sleep(0.1)
    assert result["status"] == "completed", result
    calls = [e for e in result["events"] if e["reply"]]
    assert len(calls) == 6, len(calls)
    assert [e["seq"] for e in result["events"]] == list(range(len(result["events"])))
    assert all(e["reply"]["usage"]["input"] is None for e in calls)
    run = request(base, "/api/runs", {"task": "cancel test", "consent": False})["id"]
    request(base, "/api/runs/" + run + "/cancel", {})
    for _ in range(40):
        result = request(base, "/api/runs/" + run)
        if result["status"] != "running":
            break
        time.sleep(0.1)
    assert result["status"] == "cancelled", result
finally:
    process.terminate()
    process.wait(timeout=5)
process, base = launch("entra")
try:
    expect_status(base, "/api/runs", {"task": "test", "consent": False}, 401)
finally:
    process.terminate()
    process.wait(timeout=5)
print("PASS: fixed-mode lifecycle, cancellation, input checks and anonymous rejection")


def wait(base, run):
    for _ in range(400):
        state = request(base, "/api/runs/" + run)
        if state["status"] != "running":
            return state
        time.sleep(0.1)
    raise TimeoutError("run did not finish")


process, base = launch("local", "config/mock.json")
try:
    assert request(base, "/api/config")["judges"]["max_rounds"] == 3
    criteria = ["Use supplied facts", "Explain the result"]
    run = request(
        base,
        "/api/runs",
        {"task": "Compare vendors", "consent": False, "definition_of_done": criteria},
    )["id"]
    result = wait(base, run)
    assert result["status"] == "completed", result
    routes = [
        json.loads(e["message"])
        for e in result["events"]
        if e["kind"] == "route_accepted"
    ]
    reviews = [
        json.loads(e["message"])
        for e in result["events"]
        if e["kind"] == "completion_review"
    ]
    assert (
        len(routes) == 2
        and len(routes[0]["deployments"]) == 2
        and len(routes[1]["deployments"]) == 1
    )
    assert all(r["definition_of_done"] == criteria for r in routes)
    assert [r["verdict"] for r in reviews] == ["revise", "accepted"]
    assert routes[1]["deployments"][0]["context_from"] == ["previous_round", "feedback"]
    assert (
        routes[0]["deployments"][1]["planner_model"]
        != routes[1]["deployments"][0]["planner_model"]
    )
    calls = [e for e in result["events"] if e["reply"]]
    assert len(calls) == 13, len(calls)
    assert sum(e["role"] in ["router", "judge"] for e in calls) == 4
finally:
    process.terminate()
    process.wait(timeout=5)
print(
    "PASS: routing/model selection, frozen criteria, two-agent to one-agent repair, explicit context, independent review, judge metrics"
)

import tempfile

for scenario, expected in [
    ("round_limit", "incomplete"),
    ("call_limit", "incomplete"),
    ("bad_router", "failed"),
    ("bad_judge", "failed"),
]:
    config = json.loads((ROOT / "config/mock.json").read_text())
    if scenario == "round_limit":
        config["judges"]["max_rounds"] = 1
    if scenario == "call_limit":
        config["judges"]["max_calls"] = 3
    if scenario == "bad_router":
        config["judges"]["router"]["model"] = "mock-invalid-router"
    if scenario == "bad_judge":
        config["judges"]["completion"]["model"] = "mock-invalid-judge"
    with tempfile.NamedTemporaryFile(mode="w", suffix=".json") as fixture:
        json.dump(config, fixture)
        fixture.flush()
        process, base = launch("local", fixture.name)
        try:
            run = request(
                base, "/api/runs", {"task": "Bounded judge scenario", "consent": False}
            )["id"]
            result = wait(base, run)
            assert result["status"] == expected, (scenario, result)
            if scenario == "call_limit":
                assert sum(bool(e["reply"]) for e in result["events"]) == 3
            if scenario == "bad_router":
                assert not any(e["role"] == "worker" for e in result["events"])
        finally:
            process.terminate()
            process.wait(timeout=5)
print(
    "PASS: round/call exhaustion stays incomplete; invalid routing and completion decisions fail closed"
)

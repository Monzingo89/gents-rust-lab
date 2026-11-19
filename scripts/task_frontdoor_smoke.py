"""Actual HTTP/cache/corpus checks; no generative provider or model load."""
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]


def main():
    if any(arg != "--browser" for arg in sys.argv[1:]):
        raise SystemExit("unsupported fixture option")
    browser = "--browser" in sys.argv
    with tempfile.TemporaryDirectory(prefix="gents-task-frontdoor-") as temporary:
        folder = Path(temporary)
        if browser:
            (folder / "web").symlink_to(ROOT / "web", target_is_directory=True)
        model = {"provider": "mock", "model": "must-not-be-invoked"}
        config = {"company_mode": True, "company_tools": True, "admin_judge": model,
                  "agents": [{"id": "Engineer", "assignment": "fixture", "planner": model, "worker": model}],
                  "max_steps": 1, "max_tokens": 512, "max_run_seconds": 30}
        (folder / "config.json").write_text(json.dumps(config))
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"http://127.0.0.1:{port}"

        def request(path, body=None):
            data = None if body is None else json.dumps(body).encode()
            wire = urllib.request.Request(origin + path, data=data, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(wire, timeout=20) as result:
                return json.load(result)

        env = {k: v for k, v in os.environ.items() if k in {"PATH", "HOME", "USER", "TMPDIR", "LANG", "SHELL"}}
        env.update(AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(folder / "config.json"),
                   RUN_STORE=str(folder / "runs.sqlite3"), GENTS_DISABLE_SCHEDULER="1", GENTS_OFFICE_ACTIVITY="0", GENTS_LOCAL_ONLY="1")
        with (folder / "harness.log").open("wb") as log:
            child = subprocess.Popen([str(ROOT / "target/debug/gents-harness")], cwd=folder, env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 15
                while True:
                    try:
                        request("/api/config")
                        break
                    except OSError:
                        if time.monotonic() > deadline:
                            raise
                        time.sleep(.1)
                route = "/api/companies/one/tasks"
                request("/api/companies/one", {"op": "register", "name": "Task frontdoor fixture"})
                request("/api/companies/two", {"op": "register", "name": "Other private corpus"})
                if browser:
                    request("/api/companies/we-need-gents-lc", {"op": "register", "name": "Empty browser connection fixture"})
                now = time.time_ns() // 1_000_000
                ask = "Who was US president in 1976?"
                source = {"source_id": "chronology", "revision": "fixture-v1", "title": "Synthetic chronology fixture",
                          "domain": "history", "language": "en", "text": "Gerald Ford served in 1976. Carter won the election and took office in 1977.",
                          "observed_at_ms": now, "valid_until_ms": now + 60000,
                          "lookups": [{"exact_ask": ask, "temporal_scope": "1976", "context_independent": True, "start_byte": 0, "end_byte": 11}]}
                source["lookups"].append({**source["lookups"][0], "temporal_scope": ""})
                request(route, {"op": "ingest", "source": source})
                task = {"original_ask": ask, "temporal_scope": "1976", "output": {"format": "text", "language": "en"}}
                # SQLite immediate transactions serialize compatible duplicate lookups.
                # This is deterministic cache coalescence, not async model subscribers.
                with ThreadPoolExecutor(max_workers=8) as pool:
                    replies = list(pool.map(lambda _: request(route, {"op": "execute", "input": task}), range(8)))
                paths = [reply["result"]["actual_path"] for reply in replies]
                assert paths.count("deterministic_lookup") == 1, paths
                assert paths.count("exact_answer_cache") == 7, paths
                assert len({reply["request"]["task_id"] for reply in replies}) == 8
                assert all(reply["result"]["answer"] == "Gerald Ford" and reply["result"]["measurements"]["generative_model_calls"] == 0 for reply in replies)
                lookup = request(route, {"op": "search", "query": "Carter election", "domain": "history"})
                assert lookup["hits"] and lookup["generative_model_calls"] == 0
                assert request(route, {"op": "inspect_cache"})["exact_entries"] == 1
                saved_ceo = []
                for expected_path in ("deterministic_lookup", "exact_answer_cache"):
                    session = str(uuid.uuid4())
                    reply = request("/api/companies/one/planning/" + session, {"message": ask, "expected_revision": 0, "source_inputs": [], "source_files": [], "owned_invocations": []})
                    assert reply["status"] == "answered" and reply["task_request"]["original_ask"] == ask
                    assert reply["task_result"]["actual_path"] == expected_path
                    assert reply["task_result"]["measurements"]["generative_model_calls"] == 0
                    assert not reply["policy_installed"] and not reply["tools_executed"]
                    assert request("/api/companies/one/planning/" + session)["task_result"]["answer"] == "Gerald Ford"
                    saved_ceo.append(session)
                if browser:
                    subprocess.run(["node", "scripts/task_frontdoor_browser.mjs"], cwd=ROOT,
                                   env={**env, "GENTS_TASK_FIXTURE_URL": origin}, timeout=90, check=True)
                other = request("/api/companies/two/tasks", {"op": "execute", "input": task})
                assert other["result"]["answer"] is None
                for changed in ({**task, "original_ask": "Who won the 1976 election?"}, {**task, "intent": "action"},
                                {**task, "constraints": ["Answer with two independent sources"]}):
                    answer = request(route, {"op": "execute", "input": changed})
                    assert answer["result"]["status"] == "requires_planning"
                try:
                    request(route, {"op": "execute", "input": {**task, "max_model_calls": 100}})
                except urllib.error.HTTPError as error:
                    assert error.code == 422
                else:
                    raise AssertionError("Caller policy injection accepted")
                source.update(revision="fixture-v2", text="Corrected!! served in 1976. Carter won the election and took office in 1977.")
                request(route, {"op": "ingest", "source": source})
                corrected = request(route, {"op": "execute", "input": task})
                assert corrected["result"]["actual_path"] == "deterministic_lookup"
                assert corrected["result"]["answer"] == "Corrected!!"
                old = request(route, {"op": "status", "task_id": replies[0]["request"]["task_id"]})
                assert old["answer"] is None and old["status"] == "evidence_invalidated"
                request(route, {"op": "delete_source", "source_id": "chronology"})
                assert request(route, {"op": "execute", "input": task})["result"]["answer"] is None
                for session in saved_ceo:
                    saved = request("/api/companies/one/planning/" + session)
                    assert saved["task_result"]["answer"] is None
                    assert "Gerald Ford" not in saved["messages"][1]["message"]
                request(route, {"op": "invalidate_cache"})
                assert request(route, {"op": "inspect_cache"})["exact_entries"] == 0
                assert request("/api/runs")["runs"] == []
                print("PASS: real HTTP ingestion/retrieval/cache; eight concurrent requests (one extraction, seven hits); original CEO/UI planning endpoint returns source/cached answers BEFORE any model requirement, persists history and redacts deleted evidence; tenant separation and input checks; zero generative calls or runs. Historical data are fixtures.")
            finally:
                child.terminate()
                assert child.wait(timeout=15) == 0


if __name__ == "__main__":
    main()

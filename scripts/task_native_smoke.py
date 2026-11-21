#!/usr/bin/env python3
"""Native MCP stdio -> company queue -> shared harness executors/SQLite.

All workers are explicit mocks; the transports, scopes and receipts are real.
"""

import argparse
import concurrent.futures
import json
import os
import selectors
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class Mcp:
    def __init__(
        self,
        root,
        run,
        actor="Worker",
        company="one",
        owner="local-developer",
        transport="stdio",
    ):
        self.transport = transport
        self.token = str(uuid.uuid4())
        self.session = ""
        env = dict(os.environ)
        command = [
            str(ROOT / "target/debug/gents-local-mcp"),
            "--root",
            str(root),
            "--company",
            company,
            "--owner",
            owner,
            "--run",
            run,
            "--actor",
            actor,
            "--phase",
            "worker",
        ]
        if transport == "sse":
            command += ["--transport", "sse", "--bind", "127.0.0.1:0"]
            env["GENTS_MCP_TOKEN"] = self.token
        self.child = subprocess.Popen(
            command,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.seq = 0
        self.selector = selectors.DefaultSelector()
        self.selector.register(
            self.child.stderr if transport == "sse" else self.child.stdout,
            selectors.EVENT_READ,
        )
        if transport == "sse":
            assert self.selector.select(10), "owned SSE readiness deadline"
            self.endpoint = json.loads(self.child.stderr.readline())["mcp_endpoint"]
            url = urllib.parse.urlparse(self.endpoint)
            assert (
                url.scheme == "http"
                and url.hostname == "127.0.0.1"
                and url.path == "/mcp"
                and url.port
                and not url.username
                and not url.password
                and not url.query
                and not url.fragment
            )
            self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.rpc(
            "initialize",
            {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "native task fixture", "version": "1"},
            },
        )
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, frame):
        if self.transport == "sse":
            headers = {
                "Content-Type": "application/json",
                "Accept": "application/json, text/event-stream",
                "MCP-Protocol-Version": "2025-11-25",
                "Authorization": "Bearer " + self.token,
            }
            if self.session:
                headers["MCP-Session-Id"] = self.session
            with self.opener.open(
                urllib.request.Request(
                    self.endpoint, data=json.dumps(frame).encode(), headers=headers
                ),
                timeout=15,
            ) as response:
                if "id" not in frame:
                    assert response.status == 202
                    return
                self.session = response.headers["mcp-session-id"]
                assert response.headers["content-type"] == "text/event-stream"
                data = response.read(66001)
                assert len(data) <= 66000
                return json.loads(
                    next(
                        line[6:]
                        for line in data.decode().splitlines()
                        if line.startswith("data: ")
                    )
                )
        self.child.stdin.write(json.dumps(frame) + "\n")
        self.child.stdin.flush()
        if "id" not in frame:
            return
        assert self.selector.select(15), "native protocol response deadline"
        return json.loads(self.child.stdout.readline())

    def rpc(self, method, params):
        self.seq += 1
        value = self.send(
            {"jsonrpc": "2.0", "id": self.seq, "method": method, "params": params}
        )
        assert value["id"] == self.seq, value
        assert "error" not in value, value
        return value["result"]

    def call(self, name, args=None, rejected=False):
        reply = self.rpc(
            "tools/call", {"name": name, "arguments": {} if args is None else args}
        )
        if rejected:
            assert reply["isError"], reply
            return reply
        assert not reply["isError"], reply
        return reply["structuredContent"]["value"]

    def catalog(self):
        tools = []
        params = {}
        seen = set()
        while True:
            page = self.rpc("tools/list", params)
            tools.extend(page["tools"])
            cursor = page.get("nextCursor")
            if not cursor:
                return tools
            assert cursor not in seen
            seen.add(cursor)
            params = {"cursor": cursor}

    def close(self):
        self.selector.close()
        self.child.terminate()
        try:
            self.child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.child.kill()
            self.child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--adaptive", action="store_true")
    parser.add_argument("--sse", action="store_true")
    args = parser.parse_args()
    transport = "sse" if args.sse else "stdio"
    with tempfile.TemporaryDirectory(prefix="gents-native-task-") as temporary:
        folder = Path(temporary)
        run = str(uuid.uuid4())
        model = {"provider": "mock", "model": "mock-task-stage"}
        memory = {
            "weights_bytes": 1,
            "adapters_bytes": 0,
            "kv_bytes": 1,
            "context_bytes": 1,
            "runtime_bytes": 1,
            "headroom_bytes": 1,
            "provenance": "mock fixture only",
        }
        worker = {
            "model": model,
            "sha256": None,
            "memory_estimate": memory,
            "evaluated_profiles": ["general"],
            "evaluation_reference": "mock fixture only",
        }
        config = {
            "company_mode": True,
            "company_tools": True,
            "admin_judge": model,
            "agents": [
                {
                    "id": "Worker",
                    "assignment": "fixture",
                    "planner": model,
                    "worker": model,
                }
            ],
            "max_steps": 1,
            "max_tokens": 512,
            "task_workflows": {
                "models": {"fixture": worker},
                "default_model": "fixture",
                "max_calls": 12,
                "total_generation_tokens": 4096,
                "per_call_tokens": 128,
                "deadline_seconds": 60,
                "max_retries": 0,
                "max_pending_jobs": 4,
                "concurrency": 1,
                "resident_budget_bytes": 1048576,
                "allow_parametric_answers": True,
                "native_mcp": True,
                "native_companies": ["one"],
            },
        }
        if args.adaptive:
            worker["evaluated_profiles"].append("intake")
            config["task_workflows"]["adaptive"] = {
                "intake": {"model": "fixture", "tokens": 128, "max_repairs": 0},
                "max_replans": 0,
                "rules": [],
            }
        path = folder / "config.json"
        path.write_text(json.dumps(config))
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"http://127.0.0.1:{port}"
        env = {
            k: v
            for k, v in os.environ.items()
            if k in {"PATH", "HOME", "USER", "TMPDIR", "LANG", "SHELL"}
        }
        env.update(
            AUTH_MODE="local",
            BIND=f"127.0.0.1:{port}",
            CONFIG=str(path),
            RUN_STORE=str(folder / "runs.sqlite3"),
            GENTS_DISABLE_SCHEDULER="1",
            GENTS_OFFICE_ACTIVITY="0",
            GENTS_LOCAL_ONLY="1",
        )
        log = (folder / "harness.log").open("wb")
        child = None
        clients = []

        def start():
            p = subprocess.Popen(
                [str(ROOT / "target/debug/gents-harness")],
                cwd=folder,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
            deadline = time.monotonic() + 15
            while True:
                try:
                    req("/api/config")
                    return p
                except OSError:
                    if p.poll() is not None or time.monotonic() > deadline:
                        raise
                    time.sleep(0.05)

        def req(url, body=None):
            request = urllib.request.Request(
                origin + url,
                data=None if body is None else json.dumps(body).encode(),
                headers={"Content-Type": "application/json"},
            )
            try:
                with urllib.request.urlopen(request, timeout=15) as response:
                    return json.load(response)
            except urllib.error.HTTPError as error:
                raise AssertionError(
                    f"{url}: HTTP {error.code}: {error.read(2000).decode()}"
                ) from error

        def op(kind, **fields):
            return req("/api/companies/one", {"op": kind, **fields})

        def wait(client, task):
            deadline = time.monotonic() + 35
            while True:
                result = client.call("get_task_status", {"task_id": task})
                if result["status"] != "running":
                    return result
                assert time.monotonic() < deadline, result
                time.sleep(0.1)

        def wait_inflight(store, task):
            # Observe a committed request with no matching return. Merely
            # seeing the queue row does not establish mid-call revocation.
            deadline = time.monotonic() + 10
            while True:
                with sqlite3.connect(store) as db:
                    row = db.execute(
                        "select job from task_native_queue_v1 where task=?", (task,)
                    ).fetchone()
                    job = row[0] if row else None
                    events = (
                        []
                        if not job
                        else [
                            (kind, json.loads(body))
                            for kind, body in db.execute(
                                "select kind,body from audit where run_id=? and kind in ('task_model_dispatch','task_model_return') order by seq",
                                (job,),
                            )
                        ]
                    )
                dispatched = [
                    body["call_id"]
                    for kind, body in events
                    if kind == "task_model_dispatch"
                ]
                returned = {
                    body["call_id"]
                    for kind, body in events
                    if kind == "task_model_return"
                }
                if len(dispatched) == 1 and dispatched[0] not in returned:
                    return job, dispatched[0]
                assert time.monotonic() < deadline, (task, events)
                time.sleep(0.02)

        def wait_foreground(expected):
            deadline = time.monotonic() + 5
            while True:
                value = req("/api/office/activity")["foreground"]["async_task_jobs"]
                if value == expected:
                    return
                assert time.monotonic() < deadline, (value, expected)
                time.sleep(0.01)

        try:
            child = start()
            op("register", name="Native task fixture")
            req(
                "/api/companies/two",
                {"op": "register", "name": "Not enabled for native tasks"},
            )
            intake = {
                "idea": "Bounded native task fixture",
                "acceptance": "Typed results and no scope expansion",
                "exclusions": "Paid services",
                "timeline": "Synthetic",
                "company_signer": "Fixture",
                "provider_signer": "Fixture",
                "budget_usd": 0,
                "max_model_calls": 128,
            }
            state = op("intake", run_id=run, intake=intake)
            job = next(j for j in state["jobs"] if j["run_id"] == run)
            op("approve", run_id=run, digest=job["digest"])
            tools = [
                "execute_task",
                "destructure_task",
                "get_task_status",
                "cancel_task",
                "ingest_sources",
                "search_knowledge",
                "list_capabilities",
                "inspect_cache",
            ]
            access = {"tools": tools, "skills": [], "memory_from": []}
            policy = {
                "revision": 1,
                "master": "Worker",
                "ceilings": {"Worker": access, "Reader": access},
                "grants": {"Worker": access, "Reader": access},
            }
            op("governance_install", run_id=run, expected_revision=0, policy=policy)
            worker_client = Mcp(folder, run, transport=transport)
            reader = Mcp(folder, run, "Reader", transport=transport)
            clients.extend([worker_client, reader])
            deadline = time.monotonic() + 10
            while not worker_client.call("list_capabilities")["native_async"][
                "enabled"
            ]:
                assert time.monotonic() < deadline
                time.sleep(0.05)
            catalog = worker_client.catalog()
            listed = next(t for t in catalog if t["name"] == "execute_task")
            assert listed["outputSchema"]["properties"]["value"]["required"] == [
                "intake",
                "result",
            ]
            assert {t["name"] for t in catalog} == set(tools)
            now = int(time.time() * 1000)
            text = "Native reference evidence records a synthetic release literal v7."
            worker_client.call(
                "ingest_sources",
                {
                    "source": {
                        "source_id": "native-fixture",
                        "revision": "v1",
                        "title": "Synthetic native fixture",
                        "domain": "general",
                        "language": "en",
                        "text": text,
                        "observed_at_ms": now,
                        "valid_until_ms": now + 300000,
                        "lookups": [
                            {
                                "exact_ask": "Read native literal",
                                "temporal_scope": "",
                                "context_independent": True,
                                "start_byte": len(text) - 3,
                                "end_byte": len(text) - 1,
                            }
                        ],
                    }
                },
            )
            for expected in ["deterministic_lookup", "exact_answer_cache"]:
                result = worker_client.call(
                    "execute_task", {"input": {"original_ask": "Read native literal"}}
                )["result"]
                assert (
                    result["actual_path"] == expected
                    and result["measurements"]["generative_model_calls"] == 0
                ), result
            if args.adaptive:
                launched = worker_client.call(
                    "destructure_task",
                    {"input": {"original_ask": "Describe native reference evidence"}},
                )
                analyzed = wait(worker_client, launched["intake"]["request"]["task_id"])
                assert (
                    analyzed["status"] == "analyzed"
                    and analyzed["verification_state"] == "proposed_not_executed"
                ), analyzed
                missing = worker_client.call(
                    "execute_task",
                    {"input": {"original_ask": "missing-intent-fixture"}},
                )
                result = wait(worker_client, missing["intake"]["request"]["task_id"])
                assert (
                    result["status"] == "needs_clarification"
                    and result["clarification_questions"]
                ), result
                structured = launched["intake"]["request"]
                structured["task_id"] = str(uuid.uuid4())
                launched = worker_client.call(
                    "execute_task",
                    {
                        "request": structured,
                        "hints": {"template": "rag/v1", "evidence_required": True},
                    },
                )
                result = wait(worker_client, structured["task_id"])
                assert result["status"] == "generated" and not result.get("analysis"), (
                    result
                )
                with sqlite3.connect(folder / "data/companies/one/brain.sqlite3") as db:
                    body = json.loads(
                        db.execute(
                            "select body from task_native_queue_v1 where task=?",
                            (structured["task_id"],),
                        ).fetchone()[0]
                    )
                    assert body["skip_model_intake"]
                    reservations = db.execute(
                        "select count(*) from inference_attempt_reservations where run_id=?",
                        (run,),
                    ).fetchone()[0]
                    assert reservations == 3, reservations
                print(
                    json.dumps(
                        {
                            "transport": "actual native MCP "
                            + transport
                            + " and SQLite",
                            "worker": "mock only",
                            "analysis_only": True,
                            "clarification_blocks_worker": True,
                            "structured_intake_bypass": True,
                            "parent_reserved_calls": reservations,
                            "provider_cost_usd": 0,
                        }
                    ),
                    flush=True,
                )
                return
            records = []
            for template in [
                "direct/v1",
                "rag/v1",
                "tool_loop/v1",
                "pipeline/v1",
                "hub/v1",
                "hierarchy/v1",
                "peer/v1",
                "critique/v1",
                "explore/v1",
            ]:
                launched = worker_client.call(
                    "execute_task",
                    {
                        "input": {
                            "original_ask": "Describe native reference evidence using "
                            + template
                        },
                        "hints": {
                            "template": template,
                            "domain": "general",
                            "evidence_required": False,
                        },
                    },
                )
                assert launched["result"]["status"] == "running", launched
                task = launched["intake"]["request"]["task_id"]
                result = wait(worker_client, task)
                assert (
                    result["status"] == "generated"
                    and result["verification_state"] != "verified"
                ), (template, result)
                assert result["measurements"]["generative_model_calls"] == 0, result
                assert (
                    result["progress"]["phase"] == "finished"
                    and result["progress"]["pending_model_requests"] == 0
                ), result
                assert (
                    result["progress"]["model_dispatches"]
                    == result["progress"]["model_returns"]
                    >= len(result["plan"]["nodes"])
                ), result
                assert reader.call("get_task_status", {"task_id": task}) is None
                records.append(
                    {"template": template, "nodes": len(result["plan"]["nodes"])}
                )
            usage = req("/api/companies/one/tasks", {"op": "usage"})
            assert (
                usage["coverage_complete"]
                and usage["generative_model_calls"] == 0
                and usage["provider_cost_usd"] == 0
            ), usage
            assert (
                usage["sampled_jobs"] == len(records)
                and usage["observed_native_replies"] == 0
            ), usage
            assert (
                usage["workflow_return_records"]
                == usage["observed_fixture_replies"]
                >= sum(r["nodes"] for r in records)
            ), usage
            duplicate = {
                "input": {
                    "original_ask": "Describe native reference evidence for duplicate subscribers"
                },
                "hints": {"template": "pipeline/v1", "evidence_required": False},
            }
            first = worker_client.call("execute_task", duplicate)
            second = worker_client.call("execute_task", duplicate)
            a, b = [v["intake"]["request"]["task_id"] for v in [first, second]]
            # Wait for both durable queue rows to attach to the same executor.
            store = folder / "data/companies/one/brain.sqlite3"
            deadline = time.monotonic() + 10
            while True:
                with sqlite3.connect(store) as db:
                    jobs = [
                        row[0]
                        for row in db.execute(
                            "select job from task_native_queue_v1 where task in (?,?)",
                            (a, b),
                        )
                    ]
                if len(jobs) == 2 and all(jobs):
                    break
                assert time.monotonic() < deadline
                time.sleep(0.02)
            assert jobs[0] == jobs[1], jobs
            wait_foreground(1)
            assert (
                worker_client.call("cancel_task", {"task_id": a})["status"]
                == "cancelled"
            )
            assert wait(worker_client, b)["status"] == "generated"
            assert wait(worker_client, a)["status"] == "cancelled"
            wait_foreground(0)
            # Revoke only AFTER a committed dispatch, before its fixture reply.
            # New current epoch preserves status access, not work authority.
            pending = worker_client.call(
                "execute_task",
                {
                    "input": {
                        "original_ask": "Describe native reference evidence while revoking permission"
                    },
                    "hints": {"template": "pipeline/v1", "evidence_required": False},
                },
            )
            task = pending["intake"]["request"]["task_id"]
            revoked_job, revoked_call = wait_inflight(store, task)
            policy = json.loads(json.dumps(policy))
            policy["revision"] = 2
            policy["grants"]["Worker"]["tools"].remove("execute_task")

            def revise():
                try:
                    op(
                        "governance_install",
                        run_id=run,
                        expected_revision=1,
                        policy=policy,
                    )
                    return "saved"
                except AssertionError as error:
                    return str(error)

            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                updates = list(pool.map(lambda _: revise(), range(2)))
            assert (
                updates.count("saved") == 1
                and sum(
                    "stale governance update rejected" in result for result in updates
                )
                == 1
            ), updates
            result = wait(worker_client, task)
            assert (
                result["status"] == "evidence_invalidated" and result["answer"] is None
            ), result
            assert result.get("progress") is None, result
            assert result["measurements"]["generative_model_calls"] is None, result
            # Wait for the executor itself to settle. Status redaction alone
            # cannot prove cancellation, nor can a reservation prove inference.
            deadline = time.monotonic() + 10
            while True:
                with sqlite3.connect(store) as db:
                    state, checkpoint = db.execute(
                        "select state,result from task_workflow_jobs_v1 where id=?",
                        (revoked_job,),
                    ).fetchone()
                    settled = state == "completed"
                if settled:
                    break
                assert time.monotonic() < deadline, "revoked executor did not settle"
                time.sleep(0.02)
            checkpoint = json.loads(checkpoint)
            assert (
                checkpoint["status"] == "evidence_invalidated"
                and checkpoint["answer"] is None
            ), checkpoint
            assert (
                not checkpoint["evidence"]
                and not checkpoint["artifacts"]
                and checkpoint["plan"] is None
            ), checkpoint
            assert (
                checkpoint["measurements"]["input_tokens"] is None
                and checkpoint["measurements"]["output_tokens"] is None
            ), checkpoint
            wait_foreground(0)
            policy["revision"] = 3
            policy["grants"]["Worker"]["tools"].append("execute_task")
            op("governance_install", run_id=run, expected_revision=2, policy=policy)
            stale = wait(worker_client, task)
            assert (
                stale["status"] == "evidence_invalidated" and stale["answer"] is None
            ), stale
            with sqlite3.connect(store) as db:
                revoked_dispatches = db.execute(
                    "select count(*) from audit where run_id=? and kind='task_model_dispatch'",
                    (revoked_job,),
                ).fetchone()[0]
                revoked_returns = db.execute(
                    "select count(*) from audit where run_id=? and kind='task_model_return'",
                    (revoked_job,),
                ).fetchone()[0]
            assert revoked_dispatches == 1 and revoked_returns == 0, (
                revoked_dispatches,
                revoked_returns,
            )
            uncertain = req("/api/companies/one/tasks", {"op": "usage"})
            assert (
                uncertain["generative_model_calls"] is None
                and uncertain["provider_cost_usd"] is None
            ), uncertain
            assert uncertain["unresolved_workflow_requests"] >= 1, uncertain
            restarting = worker_client.call(
                "execute_task",
                {
                    "input": {
                        "original_ask": "Describe native reference evidence for interrupted worker"
                    },
                    "hints": {"template": "pipeline/v1", "evidence_required": False},
                },
            )
            restart_task = restarting["intake"]["request"]["task_id"]
            restart_job, _ = wait_inflight(store, restart_task)
            with sqlite3.connect(store) as db:
                old_instance = json.loads(
                    db.execute("select body from task_native_worker_v1").fetchone()[0]
                )["instance"]
            child.terminate()
            child.wait(timeout=20)
            child = start()
            deadline = time.monotonic() + 12
            while True:
                with sqlite3.connect(store) as db:
                    instance = json.loads(
                        db.execute("select body from task_native_worker_v1").fetchone()[
                            0
                        ]
                    )["instance"]
                if instance != old_instance:
                    break
                assert time.monotonic() < deadline
                time.sleep(0.1)
            result = wait(worker_client, restart_task)
            assert result["status"] == "interrupted" and result["answer"] is None, (
                result
            )
            with sqlite3.connect(store) as db:
                assert (
                    db.execute(
                        "select count(*) from audit where run_id=? and kind='task_model_dispatch'",
                        (restart_job,),
                    ).fetchone()[0]
                    == 1
                )
            budget_run = str(uuid.uuid4())
            budget_intake = {**intake, "max_model_calls": 1}
            state = op("intake", run_id=budget_run, intake=budget_intake)
            budget_job = next(j for j in state["jobs"] if j["run_id"] == budget_run)
            op("approve", run_id=budget_run, digest=budget_job["digest"])
            budget_policy = {**policy, "revision": 1}
            op(
                "governance_install",
                run_id=budget_run,
                expected_revision=0,
                policy=budget_policy,
            )
            budget_client = Mcp(folder, budget_run, transport=transport)
            clients.append(budget_client)
            limited = budget_client.call(
                "execute_task",
                {
                    "input": {
                        "original_ask": "Describe a fictional reply with a strict parent call allowance"
                    },
                    "hints": {"template": "pipeline/v1", "evidence_required": False},
                },
            )
            limited = wait(budget_client, limited["intake"]["request"]["task_id"])
            assert limited["status"] == "blocked" and any(
                "budget" in b.lower() for b in limited["blockers"]
            ), limited
            with sqlite3.connect(store) as db:
                assert (
                    db.execute(
                        "select count(*) from inference_attempt_reservations where run_id=?",
                        (budget_run,),
                    ).fetchone()[0]
                    == 1
                )
            with sqlite3.connect(store) as db:
                reservations = db.execute(
                    "select count(*) from inference_attempt_reservations where run_id=?",
                    (run,),
                ).fetchone()[0]
                events = db.execute(
                    "select count(*) from audit where kind='task_model_dispatch' and run_id in (select job from task_native_queue_v1 where json_extract(body,'$.custody.scope.run')=?)",
                    (run,),
                ).fetchone()[0]
                assert reservations >= events > 0, (reservations, events)
                assert (
                    db.execute(
                        "select count(*) from task_sources_v1 where subject='local-developer'"
                    ).fetchone()[0]
                    == 0
                )
                assert (
                    db.execute(
                        "select count(*) from task_workflow_plan_revisions_v1"
                    ).fetchone()[0]
                    >= 9
                )
            with sqlite3.connect(folder / "data/companies/two/brain.sqlite3") as db:
                assert (
                    db.execute(
                        "select count(*) from sqlite_master where name='task_native_worker_v1'"
                    ).fetchone()[0]
                    == 0
                )
            print(
                json.dumps(
                    {
                        "transport": "actual native MCP "
                        + transport
                        + ", HTTP owner governance and SQLite",
                        "worker": "mock only; no model weights loaded",
                        "templates": records,
                        "exact_repeat_zero_generation": True,
                        "actor_private": True,
                        "coalesced_subscriber_cancellation": True,
                        "shared_job_priority_count": 1,
                        "priority_released_after_completion": True,
                        "revocation_invalidated": True,
                        "revocation_after_dispatch": True,
                        "revoked_call_id": revoked_call,
                        "revoked_job_dispatches": revoked_dispatches,
                        "revoked_job_returns": revoked_returns,
                        "revoked_executor_settled": True,
                        "regrant_did_not_revive": True,
                        "concurrent_owner_update_stale_rejected": True,
                        "restart_did_not_replay": True,
                        "parent_budget_enforced": True,
                        "parent_budget_reservations": reservations,
                        "dispatch_reservations_not_inference_proof": events,
                        "other_company_not_enabled": True,
                        "provider_cost_usd": 0,
                    }
                ),
                flush=True,
            )
        except Exception:
            log.flush()
            print((folder / "harness.log").read_text()[-4000:])
            raise
        finally:
            for client in clients:
                client.close()
            if child is not None:
                child.terminate()
                child.wait(timeout=20)
            log.close()


if __name__ == "__main__":
    main()

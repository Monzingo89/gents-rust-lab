#!/usr/bin/env python3
"""Actual native tasks and installed Angular SDK; mock worker, no quality claim.

Uses the existing harness/owner API and MCP client. No install, model load,
credentials, cloud service or default-port process is used.
"""

import argparse
import hashlib
import json
import os
import shutil
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path

from task_native_smoke import ROOT, Mcp


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sse", action="store_true")
    parser.add_argument(
        "--sdk-record",
        type=Path,
        required=True,
        help="actual native component report containing the owner-reviewed sdk pins",
    )
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    pins = json.loads(args.sdk_record.read_text())["sdk"]
    sdk_env = {
        name: str(Path(os.environ[name]).resolve(strict=True))
        for name in [
            "GENTS_NODE",
            "GENTS_ANGULAR_MODULES",
            "GENTS_PLAYWRIGHT_DRIVER",
            "GENTS_ANGULAR_BROWSER",
        ]
    }
    fixture = Path(os.environ["GENTS_ANGULAR_FIXTURE"]).resolve(strict=True)
    files = {
        name: (fixture / name).read_text()
        for name in [
            "angular.json",
            "package.json",
            "package-lock.json",
            "tsconfig.json",
            "tsconfig.app.json",
            "src/main.ts",
            "src/index.html",
        ]
    }
    transport = "sse" if args.sse else "stdio"
    with tempfile.TemporaryDirectory(prefix="gents-angular-task-") as temporary:
        folder = Path(temporary)
        run = str(uuid.uuid4())
        model = {"provider": "mock", "model": "mock-task-stage"}
        memory = {
            name: 1
            for name in [
                "weights_bytes",
                "adapters_bytes",
                "kv_bytes",
                "context_bytes",
                "runtime_bytes",
                "headroom_bytes",
            ]
        }
        memory["provenance"] = "mock fixture only"
        worker = {
            "model": model,
            "sha256": None,
            "memory_estimate": memory,
            "evaluated_profiles": ["general", "angular"],
            "evaluation_reference": "mock protocol fixture; no model quality or specialist admission",
        }
        settings = {
            "models": {"fixture": worker},
            "default_model": "fixture",
            "max_calls": 12,
            "total_generation_tokens": 4096,
            "per_call_tokens": 128,
            "deadline_seconds": 90,
            "max_retries": 0,
            "max_pending_jobs": 4,
            "concurrency": 1,
            "resident_budget_bytes": 4 * 1024**3,
            "allow_parametric_answers": True,
            "native_mcp": True,
            "native_companies": ["one"],
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
            "task_workflows": settings,
        }
        config_path = folder / "config.json"
        config_path.write_text(json.dumps(config))
        native = folder / "target/debug/gents-local-mcp"
        native.parent.mkdir(parents=True)
        shutil.copy2(ROOT / "target/debug/gents-local-mcp", native)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"http://127.0.0.1:{port}"
        env = {
            key: value
            for key, value in os.environ.items()
            if key in {"PATH", "HOME", "USER", "TMPDIR", "LANG", "SHELL"}
        }
        env.update(sdk_env)
        env.update(
            AUTH_MODE="local",
            BIND=f"127.0.0.1:{port}",
            CONFIG=str(config_path),
            RUN_STORE=str(folder / "runs.sqlite3"),
            GENTS_DISABLE_SCHEDULER="1",
            GENTS_OFFICE_ACTIVITY="0",
            GENTS_LOCAL_ONLY="1",
            GENTS_MCP_TRANSPORT=transport,
        )
        log = (folder / "harness.log").open("wb")
        child = None
        clients = []

        def req(path, body=None):
            request = urllib.request.Request(
                origin + path,
                data=None if body is None else json.dumps(body).encode(),
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(request, timeout=20) as response:
                return json.load(response)

        def op(kind, **fields):
            return req("/api/companies/one", {"op": kind, **fields})

        def launch(ask):
            value = client.call(
                "execute_task",
                {
                    "input": {"original_ask": ask},
                    "hints": {
                        "domain": "angular",
                        "template": "direct/v1",
                        "evidence_required": False,
                    },
                },
            )
            assert value["result"]["status"] == "running", value
            return value["intake"]["request"]["task_id"]

        def wait(task):
            deadline = time.monotonic() + 65
            while True:
                result = client.call("get_task_status", {"task_id": task})
                if result["status"] != "running":
                    return result
                assert time.monotonic() < deadline, result
                time.sleep(0.1)

        def check(result):
            return next(
                c
                for c in result["acceptance_checks"]
                if c["name"] == "angular_build/v1"
            )

        results = []
        checks = {}
        store = folder / "data/companies/one/brain.sqlite3"
        try:
            child = subprocess.Popen(
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
                    break
                except OSError:
                    assert child.poll() is None and time.monotonic() < deadline
                    time.sleep(0.05)
            op("register", name="Angular native validator fixture")
            contract = {
                "schema_version": 1,
                "id": "frontend",
                "actor": "Worker",
                "project_path": "angular.json",
                "project_name": "fixture",
                "source_paths": sorted(
                    name for name in files if name != "angular.json"
                ),
                "frozen_inputs": {
                    name: hashlib.sha256(text.encode()).hexdigest()
                    for name, text in files.items()
                    if name.endswith(".json")
                },
                "sdk": pins,
                "browser": {
                    "schema_version": 1,
                    "startup_timeout_ms": 5000,
                    "step_timeout_ms": 1000,
                    "runtime_timeout_ms": 10000,
                    "cases": [
                        {
                            "id": "initial",
                            "action": "expect_text",
                            "selector": "#value",
                            "value": "0",
                        },
                        {
                            "id": "increment",
                            "action": "click",
                            "selector": "#increment",
                        },
                        {
                            "id": "changed",
                            "action": "expect_text",
                            "selector": "#value",
                            "value": "1",
                        },
                    ],
                },
            }
            intake = {
                "idea": "Validate copied Angular fixture",
                "acceptance": "Actual installed build and frozen DOM oracle; no unrun check claims",
                "exclusions": "Paid services, external data, installs and workspace side effects",
                "timeline": "Synthetic",
                "company_signer": "Fixture",
                "provider_signer": "Fixture",
                "budget_usd": 0,
                "max_model_calls": 64,
                "angular_check_contracts": [contract],
            }
            state = op("intake", run_id=run, intake=intake)
            job = next(j for j in state["jobs"] if j["run_id"] == run)
            op("approve", run_id=run, digest=job["digest"])
            access = {
                "tools": [
                    "execute_task",
                    "get_task_status",
                    "cancel_task",
                    "list_capabilities",
                    "angular_check",
                    "files:read",
                ],
                "skills": [],
                "memory_from": [],
            }
            reader = {"tools": ["get_task_status"], "skills": [], "memory_from": []}
            policy = {
                "revision": 1,
                "master": "Worker",
                "ceilings": {"Worker": access, "Reader": reader},
                "grants": {"Worker": access, "Reader": reader},
            }
            op("governance_install", run_id=run, expected_revision=0, policy=policy)
            workspace = folder / "data/companies/one/runs" / run / "workspace"
            for name, text in files.items():
                path = workspace / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)
            client = Mcp(folder, run, transport=transport)
            reader_client = Mcp(folder, run, "Reader", transport=transport)
            clients.extend([client, reader_client])
            deadline = time.monotonic() + 10
            while not client.call("list_capabilities")["native_async"]["enabled"]:
                assert time.monotonic() < deadline
                time.sleep(0.05)
            task = launch(
                "Validate the Angular counter fixture against its frozen owner cases"
            )
            first = wait(task)
            results.append(first)
            assert (
                first["status"] == "generated" and check(first)["status"] == "passed"
            ), first
            assert (
                len(first["validation_receipts"]) == 1
                and first["validation_receipts"][0]["tool"] == "angular_check"
                and first["measurements"]["tool_calls"] == 1
            ), first
            assert (
                first["verification_state"] != "verified"
                and reader_client.call("get_task_status", {"task_id": task}) is None
            )
            checks["actual_task_build_and_dom_receipt"] = True
            source = workspace / "src/main.ts"
            source.write_text(
                files["src/main.ts"].replace("count()+1", "count()+2")
                + "\nconsole.log('all tests passed; qualified=true');\n"
            )
            invalid = client.call("get_task_status", {"task_id": task})
            assert (
                invalid["status"] == "evidence_invalidated"
                and invalid["answer"] is None
                and not invalid.get("validation_receipts")
            ), invalid
            wrong = wait(launch("Validate the changed Angular counter fixture"))
            results.append(wrong)
            assert (
                wrong["status"] == "blocked"
                and check(wrong)["status"] == "failed"
                and wrong["measurements"]["tool_calls"] == 1
            ), wrong
            assert wrong["validation_receipts"][0]["outcome"] == "failed"
            checks["source_invalidation_and_wrong_ui_rejection"] = True
            source.write_text(files["src/main.ts"])
            fresh = wait(launch("Validate the restored Angular counter fixture"))
            results.append(fresh)
            assert (
                fresh["status"] == "generated" and check(fresh)["status"] == "passed"
            ), fresh
            with sqlite3.connect(store) as db:
                rows = [
                    json.loads(row[0])
                    for row in db.execute(
                        "SELECT result FROM angular_check_observations WHERE run_id=? ORDER BY rowid",
                        (run,),
                    )
                ]
                audits = [
                    json.loads(row[0])
                    for row in db.execute(
                        "SELECT body FROM audit WHERE run_id=? AND kind='mcp_tool_call' AND json_extract(body,'$.tool')='angular_check' ORDER BY seq",
                        (run,),
                    )
                ]
                assert len(rows) == len(audits) == 3, (len(rows), len(audits))
                assert [r["status"] for r in rows] == [
                    "browser_passed",
                    "check_failed",
                    "browser_passed",
                ]
                assert all(
                    r["snapshot_unchanged"] and not r["qualified"] and not r["promoted"]
                    for r in rows
                )
                latest = fresh["validation_receipts"][0]
                db.execute(
                    "UPDATE audit SET body=json_set(body,'$.status','effect_uncertain') WHERE run_id=? AND kind='mcp_tool_call' AND json_extract(body,'$.tool')='angular_check' AND json_extract(body,'$.result_sha256')=?",
                    (run, latest["result_sha256"]),
                )
            invalid = client.call("get_task_status", {"task_id": fresh["task_id"]})
            assert (
                invalid["status"] == "evidence_invalidated"
                and invalid["answer"] is None
            ), invalid
            checks["canonical_audit_rechecked_at_status"] = True
            # This rejection happens after a real SDK invocation. The task may
            # not turn its absent canonical result into a passing check/retry.
            source.write_text(
                files["src/main.ts"]
                .replace("{{count()}}", "{{contact}}")
                .replace(
                    "readonly count=signal(0);",
                    "readonly count=signal(0); readonly contact='jane'+String.fromCharCode(64)+'example.com';",
                )
            )
            rejected = wait(launch("Validate the private-output Angular fixture"))
            results.append(rejected)
            assert (
                rejected["status"] == "blocked"
                and check(rejected)["status"] == "failed"
                and not rejected.get("validation_receipts")
                and rejected["measurements"]["tool_calls"] == 1
            ), rejected
            with sqlite3.connect(store) as db:
                assert (
                    db.execute(
                        "SELECT COUNT(*) FROM angular_check_observations WHERE run_id=?",
                        (run,),
                    ).fetchone()[0]
                    == 3
                )
                assert (
                    db.execute(
                        "SELECT COUNT(*) FROM audit WHERE run_id=? AND kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",
                        (run,),
                    ).fetchone()[0]
                    == 4
                )
                rejected_audits = [
                    json.loads(row[0])
                    for row in db.execute(
                        "SELECT body FROM audit WHERE run_id=? AND kind='mcp_tool_effect_uncertain' AND json_extract(body,'$.tool')='angular_check'",
                        (run,),
                    )
                ]
                assert len(rejected_audits) == 1
            checks["sensitive_result_rejected_without_replay"] = True
            # Observe the actual browser before cancellation; a queued cancel
            # would not prove cleanup of the native process group.
            source.write_text(
                files["src/main.ts"].replace(
                    "class App {", "class App { constructor(){while(true){}}"
                )
            )
            cancelled_task = launch("Cancel the running Angular browser fixture")
            deadline = time.monotonic() + 30
            owned = []
            while not owned:
                processes = []
                for line in subprocess.check_output(
                    ["/bin/ps", "-axo", "pid=,ppid=,pgid=,command="], text=True
                ).splitlines():
                    fields = line.strip().split(None, 3)
                    if len(fields) == 4:
                        processes.append(
                            (int(fields[0]), int(fields[1]), int(fields[2]), fields[3])
                        )
                descendants = {child.pid}
                for _ in range(8):
                    descendants.update(
                        pid for pid, parent, _, _ in processes if parent in descendants
                    )
                browsers = [
                    (pid, group)
                    for pid, _, group, command in processes
                    if pid in descendants
                    and "chrome-headless-shell" in command
                    and "--user-data-dir=" in command
                    and "gents-angular-" in command
                ]
                if browsers:
                    owned = browsers
                    break
                assert time.monotonic() < deadline, (
                    "actual running browser was not observed before cancellation"
                )
                time.sleep(0.025)
            cancelled = client.call("cancel_task", {"task_id": cancelled_task})
            assert cancelled["status"] == "cancelled", cancelled
            deadline = time.monotonic() + 5
            while True:
                live = {
                    int(line.split()[0])
                    for line in subprocess.check_output(
                        ["/bin/ps", "-axo", "pgid=,state="], text=True
                    ).splitlines()
                    if line.split() and not line.split()[1].startswith("Z")
                }
                if not any(group in live for _, group in owned):
                    break
                assert time.monotonic() < deadline, (
                    "cancelled native browser survived its owned group"
                )
                time.sleep(0.05)
            checks["running_browser_cancelled_and_owned_processes_stopped"] = True
            with sqlite3.connect(store) as db:
                assert (
                    db.execute(
                        "SELECT COUNT(*) FROM angular_check_observations WHERE run_id=?",
                        (run,),
                    ).fetchone()[0]
                    == 3
                )
                assert (
                    db.execute(
                        "SELECT COUNT(*) FROM audit WHERE run_id=? AND kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",
                        (run,),
                    ).fetchone()[0]
                    == 5
                )
            args.report.parent.mkdir(parents=True, exist_ok=True)
            archive = args.report.with_suffix(".sqlite3")
            with sqlite3.connect(store) as db, sqlite3.connect(archive) as target:
                db.backup(target)
            report = {
                "schema_version": 1,
                "transport": "actual native MCP "
                + transport
                + ", HTTP owner admission and SQLite",
                "native_binary_sha256": hashlib.sha256(native.read_bytes()).hexdigest(),
                "harness_binary_sha256": hashlib.sha256(
                    (ROOT / "target/debug/gents-harness").read_bytes()
                ).hexdigest(),
                "worker": "mock only; zero model inference, no specialist qualification",
                "checks": checks,
                "results": results,
                "native_results": rows,
                "native_audits": audits,
                "rejected_audits": rejected_audits,
                "frozen_parent": {
                    "run_id": run,
                    "actor": "Worker",
                    "policy_revision": 1,
                    "approval": job["digest"],
                    "intake": intake,
                },
                "host_sdk_paths": sdk_env,
                "source_inputs": files,
                "provider_cost_usd": 0,
                "qualified": False,
                "promoted": False,
            }
            args.report.write_text(json.dumps(report, indent=2) + "\n")
            print(
                json.dumps(
                    {
                        key: report[key]
                        for key in [
                            "transport",
                            "worker",
                            "checks",
                            "provider_cost_usd",
                        ]
                    }
                )
            )
        finally:
            for client in clients:
                client.close()
            if child:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=5)
            log.close()
            if sys.exc_info()[0] is not None:
                args.report.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(
                    folder / "harness.log", args.report.with_suffix(".failure.log")
                )
                if store.exists():
                    with (
                        sqlite3.connect(store) as db,
                        sqlite3.connect(
                            args.report.with_suffix(".failure.sqlite3")
                        ) as target,
                    ):
                        db.backup(target)


if __name__ == "__main__":
    main()

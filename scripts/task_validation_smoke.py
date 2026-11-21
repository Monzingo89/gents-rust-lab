#!/usr/bin/env python3
"""Real native task validation. Workers are mocks; --dotnet-live uses installed SDK.

No model downloads or network package restore. --api-live separately grants
execution of the copied application through an inherited loopback socket.
"""

import argparse
import hashlib
import json
import os
import shutil
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.request
import uuid
from pathlib import Path

from task_native_smoke import ROOT, Mcp

PROJECT = '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><UseAppHost>false</UseAppHost><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>'
GOOD = 'Console.WriteLine("Synthetic API build fixture; no app execution");'
API_PROJECT = '<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup><TargetFramework>net10.0</TargetFramework><UseAppHost>false</UseAppHost><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>'
API_GOOD = """var builder=WebApplication.CreateSlimBuilder(args);
builder.WebHost.ConfigureKestrel(o=>o.ListenHandle(ulong.Parse(Environment.GetEnvironmentVariable("GENTS_API_LISTEN_FD")!)));
var app=builder.Build();app.MapGet("/health",()=>new{ok=true});
app.MapPost("/sum",(Operands p)=>new{sum=p.left+p.right});app.Run();
record Operands(int left,int right);"""


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dotnet-live", action="store_true")
    parser.add_argument("--api-live", action="store_true")
    parser.add_argument("--mixed-contract-live", action="store_true")
    parser.add_argument("--sse", action="store_true")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if args.mixed_contract_live:
        args.api_live = True
    if args.api_live:
        args.dotnet_live = True
    project = API_PROJECT if args.api_live else PROJECT
    good = API_GOOD if args.api_live else GOOD
    transport = "sse" if args.sse else "stdio"
    executable = None
    if args.dotnet_live:
        executable = Path(os.environ["GENTS_DOTNET"]).resolve(strict=True)
        assert executable.is_absolute() and executable.name == "dotnet"
    with tempfile.TemporaryDirectory(prefix="gents-task-validation-") as temporary:
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
            "evaluated_profiles": ["general", "dotnet", "angular", "cosmos"],
            "evaluation_reference": "mock protocol fixture only; no specialist qualification",
        }
        settings = {
            "models": {"fixture": worker},
            "default_model": "fixture",
            "max_calls": 12,
            "total_generation_tokens": 4096,
            "per_call_tokens": 128,
            "deadline_seconds": 60,
            "max_retries": 0,
            "max_pending_jobs": 4,
            "concurrency": 1,
            "resident_budget_bytes": 3 * 1024**3 if args.dotnet_live else 1048576,
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
        path = folder / "config.json"
        path.write_text(json.dumps(config))
        native_binary = folder / "target/debug/gents-local-mcp"
        native_binary.parent.mkdir(parents=True)
        shutil.copy2(ROOT / "target/debug/gents-local-mcp", native_binary)
        binary_sha = hashlib.sha256(native_binary.read_bytes()).hexdigest()
        harness_binary = ROOT / "target/debug/gents-harness"
        harness_sha = hashlib.sha256(harness_binary.read_bytes()).hexdigest()
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"http://127.0.0.1:{port}"
        env = {
            k: v
            for k, v in os.environ.items()
            if k in {"PATH", "USER", "TMPDIR", "LANG", "SHELL"}
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
        env["GENTS_MCP_TRANSPORT"] = transport
        if executable:
            env["GENTS_DOTNET"] = str(executable)
        log = (folder / "harness.log").open("wb")
        child = None
        clients = []

        def req(url, body=None):
            request = urllib.request.Request(
                origin + url,
                data=None if body is None else json.dumps(body).encode(),
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(request, timeout=15) as response:
                return json.load(response)

        def op(kind, **fields):
            return req("/api/companies/one", {"op": kind, **fields})

        def wait(client, task):
            deadline = time.monotonic() + 45
            while True:
                result = client.call("get_task_status", {"task_id": task})
                if result["status"] != "running":
                    return result
                assert time.monotonic() < deadline, result
                time.sleep(0.1)

        def launch(ask, profile="dotnet", template="direct/v1"):
            value = client.call(
                "execute_task",
                {
                    "input": {"original_ask": ask},
                    "hints": {
                        "domain": profile,
                        "template": template,
                        "evidence_required": False,
                    },
                },
            )
            assert value["result"]["status"] == "running", value
            return value["intake"]["request"]["task_id"]

        def check(result, name):
            return next(
                c for c in result["acceptance_checks"] if c["name"] == name + "/v1"
            )

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
            op("register", name="Task validator fixture")
            intake = {
                "idea": "Typed native validator fixture",
                "acceptance": "Actual scoped receipts; no unrun check claims",
                "exclusions": "Paid services, external network and execution outside frozen API checks"
                if args.api_live
                else "Paid services and application execution",
                "timeline": "Synthetic",
                "company_signer": "Fixture",
                "provider_signer": "Fixture",
                "budget_usd": 0,
                "max_model_calls": 64,
            }
            if args.dotnet_live:
                intake["dotnet_check_contracts"] = [
                    {
                        "id": "api-build",
                        "actor": "Worker",
                        "project_path": "api/Api.csproj",
                        "source_paths": ["api/Program.cs"],
                        "sdk_version": "10.0.401",
                        "dotnet_sha256": hashlib.sha256(
                            executable.read_bytes()
                        ).hexdigest(),
                        "artifact_mode": "managed_dll",
                        "frozen_inputs": {
                            "api/Api.csproj": hashlib.sha256(
                                project.encode()
                            ).hexdigest()
                        },
                    }
                ]
                if args.api_live:
                    intake["dotnet_check_contracts"][0]["api"] = {
                        "schema_version": 1,
                        "listen_mode": "inherited_loopback_socket",
                        "startup_timeout_ms": 3000,
                        "request_timeout_ms": 1000,
                        "runtime_timeout_ms": 8000,
                        "cases": [
                            {
                                "id": "health",
                                "method": "GET",
                                "path": "/health",
                                "expected_status": 200,
                                "expected_json": {"ok": True},
                            },
                            {
                                "id": "sum",
                                "method": "POST",
                                "path": "/sum",
                                "request_json": {"left": 7, "right": 5},
                                "expected_status": 200,
                                "expected_json": {"sum": 12},
                            },
                        ],
                    }
            library_project = PROJECT.replace("<OutputType>Exe</OutputType>", "")
            library_source = "public static class Arithmetic { public static int Sum(int a,int b)=>a+b; }"
            if args.mixed_contract_live:
                contract = dict(intake["dotnet_check_contracts"][0])
                contract.pop("api")
                contract.update(
                    id="compile-only",
                    project_path="library/Library.csproj",
                    source_paths=["library/Value.cs"],
                    frozen_inputs={
                        "library/Library.csproj": hashlib.sha256(
                            library_project.encode()
                        ).hexdigest()
                    },
                )
                intake["dotnet_check_contracts"].append(contract)
            state = op("intake", run_id=run, intake=intake)
            job = next(j for j in state["jobs"] if j["run_id"] == run)
            op("approve", run_id=run, digest=job["digest"])
            tools = [
                "execute_task",
                "get_task_status",
                "cancel_task",
                "list_capabilities",
            ]
            if args.dotnet_live:
                tools += ["dotnet_check", "files:read"]
            if args.api_live:
                tools += ["dotnet_api_check"]
            access = {"tools": tools, "skills": [], "memory_from": []}
            reader = {"tools": ["get_task_status"], "skills": [], "memory_from": []}
            policy = {
                "revision": 1,
                "master": "Worker",
                "ceilings": {"Worker": access, "Reader": reader},
                "grants": {"Worker": access, "Reader": reader},
            }
            op("governance_install", run_id=run, expected_revision=0, policy=policy)
            workspace = folder / "data/companies/one/runs" / run / "workspace/api"
            if args.dotnet_live:
                workspace.mkdir(parents=True)
                (workspace / "Api.csproj").write_text(project)
                (workspace / "Program.cs").write_text(good)
            if args.mixed_contract_live:
                library = workspace.parent / "library"
                library.mkdir()
                (library / "Library.csproj").write_text(library_project)
                (library / "Value.cs").write_text(library_source)
            client = Mcp(folder, run, transport=transport)
            clients.append(client)
            reader_client = Mcp(folder, run, "Reader", transport=transport)
            clients.append(reader_client)
            deadline = time.monotonic() + 10
            while not client.call("list_capabilities")["native_async"]["enabled"]:
                assert time.monotonic() < deadline
                time.sleep(0.05)
            store = folder / "data/companies/one/brain.sqlite3"
            results = []
            checks = {}
            native_results = []
            native_audits = []
            rejected_api_task = None
            rejected_api_audits = []
            rejected_program = None
            if not args.dotnet_live:
                for profile, name, status in [
                    ("general", "output_shape", "passed"),
                    ("dotnet", "dotnet_build", "not_run"),
                    ("angular", "angular_build", "not_run"),
                    ("cosmos", "cosmos_receipts", "unsupported"),
                ]:
                    task = launch(
                        "Describe a synthetic " + profile + " validation boundary",
                        profile,
                    )
                    result = wait(client, task)
                    assert result["status"] == "generated", result
                    assert check(result, name)["status"] == status, result
                    assert result["measurements"]["tool_calls"] == 0 and not result.get(
                        "validation_receipts"
                    ), result
                    assert result["verification_state"] != "verified"
                    results.append(
                        {"profile": profile, "check": name, "status": status}
                    )
                checks["unsupported_and_unrun_checks_are_explicit"] = True
            else:
                ask = (
                    "Describe the API fixture using an actual frozen SDK build boundary"
                )
                task = launch(ask, template="pipeline/v1")
                first = wait(client, task)
                results.append(first)
                assert (
                    first["status"] == "generated"
                    and check(first, "dotnet_build")["status"] == "passed"
                ), first
                assert len(first["validation_receipts"]) == (
                    3 if args.mixed_contract_live else 2 if args.api_live else 1
                ) and first["measurements"]["tool_calls"] == (
                    2 if args.mixed_contract_live else 1
                ), first
                assert all(
                    c["status"] == "passed"
                    for c in first["acceptance_checks"]
                    if c["name"] == "dotnet_build/v1"
                ), first
                if args.mixed_contract_live:
                    assert sorted(
                        (r["contract_id"], r["validator"])
                        for r in first["validation_receipts"]
                    ) == [
                        ("api-build", "dotnet_api"),
                        ("api-build", "dotnet_build"),
                        ("compile-only", "dotnet_build"),
                    ], first
                if args.api_live:
                    assert check(first, "dotnet_api")["status"] == "passed", first
                    assert (
                        first["validation_receipts"][0]["result_sha256"]
                        == first["validation_receipts"][1]["result_sha256"]
                    )
                assert (
                    len(first["plan"]["nodes"]) == 3
                    and first["verification_state"] != "verified"
                )
                assert reader_client.call("get_task_status", {"task_id": task}) is None
                bad = (
                    good.replace("p.left+p.right", "p.left-p.right")
                    if args.api_live
                    else "Console.WriteLine(UnknownSymbol);"
                )
                (workspace / "Program.cs").write_text(bad)
                invalid = client.call("get_task_status", {"task_id": task})
                assert (
                    invalid["status"] == "evidence_invalidated"
                    and invalid["answer"] is None
                    and not invalid.get("validation_receipts")
                ), invalid
                failed = wait(
                    client,
                    launch(
                        "Describe the deliberate API behavior failure"
                        if args.api_live
                        else "Describe the deliberate C# compiler failure"
                    ),
                )
                results.append(failed)
                assert failed["status"] == "blocked" and failed["answer"] is None, (
                    failed
                )
                assert check(failed, "dotnet_build")["status"] == (
                    "passed" if args.api_live else "failed"
                ), failed
                if args.api_live:
                    assert check(failed, "dotnet_api")["status"] == "failed", failed
                (workspace / "Program.cs").write_text(good)
                fresh = wait(client, launch(ask, template="pipeline/v1"))
                results.append(fresh)
                assert (
                    fresh["status"] == "generated"
                    and check(fresh, "dotnet_build")["status"] == "passed"
                ), fresh
                with sqlite3.connect(store) as db:
                    tool = "dotnet_api_check" if args.api_live else "dotnet_check"
                    calls = db.execute(
                        "select count(*) from audit where run_id=? and kind='mcp_tool_call' and json_extract(body,'$.tool')=?",
                        (run, tool),
                    ).fetchone()[0]
                    assert calls == 3, calls
                    if args.api_live:
                        assert db.execute(
                            "select count(*) from audit where run_id=? and kind='mcp_tool_call' and json_extract(body,'$.tool')='dotnet_check'",
                            (run,),
                        ).fetchone()[0] == (3 if args.mixed_contract_live else 0)
                    assert (
                        db.execute(
                            "select count(*) from inference_attempt_reservations where run_id=?",
                            (run,),
                        ).fetchone()[0]
                        == 7
                    )
                    ref = fresh["validation_receipts"][0]
                    native_results = [
                        json.loads(row[0])
                        for row in db.execute(
                            "select result from dotnet_build_observations where run_id=? and actor=? order by rowid",
                            (run, "Worker"),
                        )
                    ]
                    native_audits = [
                        json.loads(row[0])
                        for row in db.execute(
                            "select body from audit where run_id=? and kind='mcp_tool_call' and json_extract(body,'$.tool') in ('dotnet_check','dotnet_api_check') order by seq",
                            (run,),
                        )
                    ]
                    stored = json.loads(
                        db.execute(
                            "select result from dotnet_build_observations where verification_identity=? order by rowid desc limit 1",
                            (ref["source_identity"],),
                        ).fetchone()[0]
                    )
                    assert (
                        hashlib.sha256(
                            json.dumps(
                                stored, sort_keys=True, separators=(",", ":")
                            ).encode()
                        ).hexdigest()
                        == ref["result_sha256"]
                    )
                    db.execute(
                        "delete from audit where kind='mcp_tool_call' and json_extract(body,'$.result_sha256')=?",
                        (ref["result_sha256"],),
                    )
                missing = client.call("get_task_status", {"task_id": fresh["task_id"]})
                assert (
                    missing["status"] == "evidence_invalidated"
                    and missing["answer"] is None
                ), missing
                # Source changes after the first stage must stop the same
                # admission before SDK dispatch; no detached replay follows.
                pending = launch(
                    "Describe an API snapshot changed while its pipeline runs",
                    template="pipeline/v1",
                )
                deadline = time.monotonic() + 10
                while True:
                    with sqlite3.connect(store) as db:
                        row = db.execute(
                            "select job from task_native_queue_v1 where task=?",
                            (pending,),
                        ).fetchone()
                        active = row[0]
                        started = (
                            active
                            and db.execute(
                                "select count(*) from audit where run_id=? and kind='task_model_dispatch'",
                                (active,),
                            ).fetchone()[0]
                        )
                    if started:
                        break
                    assert time.monotonic() < deadline
                    time.sleep(0.02)
                (workspace / "Program.cs").write_text(
                    "Console.WriteLine(ChangedWhileQueued);"
                )
                changed = wait(client, pending)
                assert changed["answer"] is None and changed["status"] in {
                    "evidence_invalidated",
                    "interrupted",
                    "blocked",
                }, changed
                with sqlite3.connect(store) as db:
                    assert (
                        db.execute(
                            "select count(*) from audit where run_id=? and kind='mcp_tool_dispatch' and json_extract(body,'$.tool')=?",
                            (run, tool),
                        ).fetchone()[0]
                        == 3
                    )
                if args.api_live:
                    # Synthetic test PAN is assembled only inside the candidate
                    # to prove the unchanged result gate rejects it after real
                    # execution, without automatically recompiling the snapshot.
                    rejected_program = (
                        'Console.WriteLine(string.Join(" ",new[]{"4111","1111","1111","1111"}));'
                        + good
                    )
                    (workspace / "Program.cs").write_text(rejected_program)
                    rejected_api_task = wait(
                        client,
                        launch(
                            "Describe a native API diagnostic rejected before persistence"
                        ),
                    )
                    assert (
                        rejected_api_task["status"] == "blocked"
                        and rejected_api_task["answer"] is None
                    ), rejected_api_task
                    assert (
                        check(rejected_api_task, "dotnet_api")["status"] == "failed"
                    ), rejected_api_task
                    assert (
                        check(rejected_api_task, "dotnet_build")["status"] == "not_run"
                    ), rejected_api_task
                    assert rejected_api_task["measurements"]["tool_calls"] == (
                        2 if args.mixed_contract_live else 1
                    ), rejected_api_task
                    assert all(
                        r["contract_id"] != "api-build"
                        for r in rejected_api_task.get("validation_receipts", [])
                    ), rejected_api_task
                    with sqlite3.connect(store) as db:
                        assert (
                            db.execute(
                                "select count(*) from audit where run_id=? and kind='mcp_tool_dispatch' and json_extract(body,'$.tool')='dotnet_api_check'",
                                (run,),
                            ).fetchone()[0]
                            == 4
                        )
                        assert db.execute(
                            "select count(*) from audit where run_id=? and kind='mcp_tool_call' and json_extract(body,'$.tool')='dotnet_check'",
                            (run,),
                        ).fetchone()[0] == (4 if args.mixed_contract_live else 0)
                        rejected_api_audits = [
                            json.loads(row[0])
                            for row in db.execute(
                                "select body from audit where run_id=? and kind='mcp_tool_effect_uncertain' and json_extract(body,'$.tool')='dotnet_api_check'",
                                (run,),
                            )
                        ]
                        assert len(rejected_api_audits) == 1, rejected_api_audits
                        # Retain prior canonical receipts captured before the
                        # deliberate deletion, plus any independent library
                        # build from this final negative task.
                        known_results = {
                            hashlib.sha256(
                                json.dumps(
                                    v, sort_keys=True, separators=(",", ":")
                                ).encode()
                            ).hexdigest()
                            for v in native_results
                        }
                        for row in db.execute(
                            "select result from dotnet_build_observations where run_id=? and actor=? order by rowid",
                            (run, "Worker"),
                        ):
                            value = json.loads(row[0])
                            digest = hashlib.sha256(
                                json.dumps(
                                    value, sort_keys=True, separators=(",", ":")
                                ).encode()
                            ).hexdigest()
                            if digest not in known_results:
                                native_results.append(value)
                                known_results.add(digest)
                        known_audits = {v["result_sha256"] for v in native_audits}
                        for row in db.execute(
                            "select body from audit where run_id=? and kind='mcp_tool_call' and json_extract(body,'$.tool') in ('dotnet_check','dotnet_api_check') order by seq",
                            (run,),
                        ):
                            value = json.loads(row[0])
                            if value["result_sha256"] not in known_audits:
                                native_audits.append(value)
                                known_audits.add(value["result_sha256"])
                    results.append(rejected_api_task)
                    checks["failed_native_api_is_not_automatically_recompiled"] = True
                checks.update(
                    actual_sdk_build=True,
                    three_stage_one_validation=True,
                    shared_tool_budget=True,
                    source_change_invalidated=True,
                    compiler_failure_blocked=True,
                    completed_build_not_replayed_or_cached=True,
                    canonical_native_receipt_required=True,
                    wrong_actor_denied=True,
                    inflight_source_change_stopped_sdk=True,
                )
                if args.mixed_contract_live:
                    checks.pop("three_stage_one_validation")
                    checks.update(
                        three_stage_one_validation_per_contract=True,
                        compile_facet_reuse_is_contract_scoped=True,
                        compile_only_contract_is_not_dropped=True,
                    )
                if args.api_live:
                    checks.pop("compiler_failure_blocked")
                    checks.update(
                        actual_application_http=True,
                        compile_success_separate_from_api_failure=True,
                        compile_and_api_share_one_native_invocation=True,
                    )
            assert hashlib.sha256(native_binary.read_bytes()).hexdigest() == binary_sha
            assert (
                hashlib.sha256(harness_binary.read_bytes()).hexdigest() == harness_sha
            )
            report = {
                "schema_version": 1,
                "transport": "actual native MCP "
                + transport
                + ", HTTP owner admission and SQLite",
                "native_binary_sha256": binary_sha,
                "harness_binary_sha256": harness_sha,
                "worker": "mock only; zero model inference and no specialist qualification",
                "sdk": "actual installed SDK 10.0.401"
                if args.dotnet_live
                else "not executed",
                "checks": checks,
                "results": results,
                "frozen_parent": {
                    "run_id": run,
                    "actor": "Worker",
                    "policy_revision": 1,
                    "approved_digest": job["digest"],
                    "intake": intake,
                },
                "source_inputs": {
                    "api/Api.csproj": project,
                    "good_program": good,
                    "failed_program": bad,
                }
                if args.dotnet_live
                else None,
                "native_results": native_results,
                "native_audits": native_audits,
                "rejected_api_task": rejected_api_task,
                "rejected_api_audits": rejected_api_audits,
                "rejected_program": rejected_program,
                "provider_cost_usd": 0,
                "qualified": False,
                "promoted": False,
            }
            if args.mixed_contract_live:
                report["source_inputs"].update(
                    {
                        "library/Library.csproj": library_project,
                        "library/Value.cs": library_source,
                    }
                )
            if args.report:
                args.report.write_text(json.dumps(report, indent=2) + "\n")
            print(
                json.dumps(
                    {
                        k: report[k]
                        for k in [
                            "schema_version",
                            "transport",
                            "native_binary_sha256",
                            "worker",
                            "sdk",
                            "checks",
                            "provider_cost_usd",
                            "qualified",
                            "promoted",
                        ]
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

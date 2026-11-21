#!/usr/bin/env python3
"""Run the governed ten-model synthetic-bank profile; verify actual results."""

import argparse
import hashlib
import json
import os
import random
import secrets
import signal
import subprocess
import time
import urllib.request
import uuid
from pathlib import Path

import local_sqlite as sqlite3
from model_provenance import ROOT, ROSTER, local_path, private_json, validate_roster
from paired_campaign import cases, model_rss, unused_ports

ORDER = (
    "orchestrator",
    "intake",
    "schema",
    "classification",
    "policy",
    "reconciliation",
    "triage",
    "qa",
    "audit",
    "risk",
)


def native_children(parent_pid):
    """Inspect only process identity, never command arguments or credentials."""
    rows = subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid=,comm="], text=True)
    return {
        int(fields[0])
        for line in rows.splitlines()
        if len(fields := line.strip().split(None, 2)) == 3
        and (parent_pid is None or fields[1] == str(parent_pid))
        and fields[2].endswith("/llama-server")
    }


def exercise_revocation(folder, company, run, stage, saved, children, op, request):
    """Owner-authorized negative test on this fresh synthetic company only."""
    brain = ROOT / "data/companies" / company / "brain.sqlite3"
    with sqlite3.connect(f"file:{brain}?mode=ro", uri=True) as db:
        row = db.execute(
            "SELECT manifest FROM run_governance WHERE run_id=?", (run,)
        ).fetchone()
        if row is None:
            raise RuntimeError(f"No governance manifest found for run {run}")
        manifest = row[0]
        if not isinstance(manifest, (str, bytes, bytearray)):
            raise RuntimeError(f"Invalid governance manifest for run {run}")
        policy = json.loads(manifest)
        completed = dict(
            db.execute(
                "SELECT id,output_sha256 FROM workflow_nodes WHERE run_id=? AND state='completed'",
                (run,),
            ).fetchall()
        )
    assert policy["grants"][stage]["memory_from"] and len(completed) >= 2
    active = {
        e["call_id"]
        for e in saved["events"]
        if e["agent"] == stage and e["kind"] == "call_started"
    }
    active -= {
        e["call_id"]
        for e in saved["events"]
        if e["kind"] in {"call_completed", "call_failed"}
    }
    private_json(folder / "snapshot-before-revocation.json", saved)
    revoked = json.loads(json.dumps(policy))
    revoked["revision"] += 1
    revoked["grants"][stage]["memory_from"] = []
    op(
        "governance_install",
        run_id=run,
        expected_revision=policy["revision"],
        policy=revoked,
    )
    deadline = time.monotonic() + 60
    while True:
        stopped = request(f"/api/runs/{run}")
        if stopped["status"] != "running":
            break
        if time.monotonic() > deadline:
            raise TimeoutError("revoked inference did not stop within 60 seconds")
        time.sleep(0.2)
    private_json(folder / "snapshot-revoked.json", stopped)
    cancelled = {
        e["call_id"] for e in stopped["events"] if e["kind"] == "policy_epoch_cancelled"
    }
    assert active & cancelled, "no live permission-watch cancellation observed"
    assert not active & {
        e["call_id"] for e in stopped["events"] if e["kind"] == "call_completed"
    }
    assert not children & native_children(None), (
        "old native backend survived revocation"
    )
    with sqlite3.connect(f"file:{brain}?mode=ro", uri=True) as db:
        node = db.execute(
            "SELECT state,output,output_sha256 FROM workflow_nodes WHERE run_id=? AND id=?",
            (run, stage),
        ).fetchone()
        assert node is not None, f"workflow node not found: {stage}"
        state, body, sha = node
        assert state != "completed" and body is None and sha is None
        for table in ("context", "context_archive"):
            count = db.execute(
                f"SELECT COUNT(*) FROM {table} WHERE run_id=? AND agent=? AND source='model_output'",
                (run, stage),
            ).fetchone()
            assert count is not None
            assert count[0] == 0
        governance = db.execute(
            "SELECT manifest FROM run_governance WHERE run_id=?", (run,)
        ).fetchone()
        assert governance is not None
        manifest = governance[0]
        assert isinstance(manifest, (str, bytes, bytearray))
        current = json.loads(manifest)
        assert current == revoked, (
            "unexpected concurrent policy update; do not restore over it"
        )
        hashes = dict(
            db.execute(
                "SELECT id,output_sha256 FROM workflow_nodes WHERE run_id=?", (run,)
            ).fetchall()
        )
        assert all(hashes[k] == v for k, v in completed.items())
    result = {
        "stage": stage,
        "verified": True,
        "revoked_revision": revoked["revision"],
        "prior_revision": policy["revision"],
        "active_call_ids": sorted(active),
        "cancelled_call_ids": sorted(active & cancelled),
        "native_pids_stopped": sorted(children),
        "old_output_persisted": False,
        "old_checkpoint_committed": False,
        "completed_checkpoints_preserved": completed,
        "observation": "native backend active; may be loading or generating, not proof of token streaming",
    }
    private_json(folder / "revocation-result.json", result)
    # Restore only the test-owned grant, after checking no concurrent policy edit.
    restored = json.loads(json.dumps(revoked))
    restored["revision"] += 1
    restored["grants"][stage] = policy["grants"][stage]
    op(
        "governance_install",
        run_id=run,
        expected_revision=revoked["revision"],
        policy=restored,
    )
    request(f"/api/runs/{run}/resume", {})
    result["restored_revision"] = restored["revision"]
    private_json(folder / "revocation-result.json", result)
    print(
        json.dumps(
            {
                "status": "REVOCATION_VERIFIED_RESUMING",
                "stage": stage,
                "restored_revision": restored["revision"],
            }
        ),
        flush=True,
    )
    return result


def verify_crash_recovery(folder, company, run, crash, saved):
    """Recovery acceptance is separate from whole-task financial acceptance."""
    brain = ROOT / "data/companies" / company / "brain.sqlite3"
    before = json.loads((folder / "snapshot-before-crash.json").read_text())
    assert saved["events"][: len(before["events"])] == before["events"], (
        "earlier events changed"
    )
    assert any(e["kind"] == "restart_resumed" for e in saved["events"])
    assert any(e["kind"] == "workflow_controller_recovered" for e in saved["events"])
    assert not set(crash["active_call_ids"]) & {
        e["call_id"] for e in saved["events"] if e["kind"] == "call_completed"
    }
    with sqlite3.connect(f"file:{brain}?mode=ro", uri=True) as db:
        contract_row = db.execute(
            "SELECT digest FROM jobs WHERE run_id=?", (run,)
        ).fetchone()
        assert contract_row is not None, "job record is missing"
        assert crash["contract_sha256"] == contract_row[0]
        hash_rows = db.execute(
            "SELECT id,output_sha256 FROM workflow_nodes WHERE run_id=?", (run,)
        ).fetchall()
        hashes = {row[0]: row[1] for row in hash_rows}
        assert all(
            hashes[stage] == sha
            for stage, sha in crash["completed_checkpoints"].items()
        )
        stage_row = db.execute(
            "SELECT state,attempt FROM workflow_nodes WHERE run_id=? AND id=?",
            (run, crash["stage"]),
        ).fetchone()
        assert stage_row is not None, "interrupted workflow stage is missing"
        state, attempt = stage_row
        assert (state, attempt) == ("completed", 2), (
            "interrupted stage did not complete on its next bounded attempt"
        )
        recovery_row = db.execute(
            "SELECT COUNT(*) FROM workflow_events WHERE run_id=? AND stage=? AND kind='controller_recovered'",
            (run, crash["stage"]),
        ).fetchone()
        assert recovery_row is not None
        assert recovery_row[0] == 1
    result = {
        **crash,
        "verified": True,
        "original_events_preserved": True,
        "completed_hashes_preserved": True,
        "status": "PASS_RECOVERY",
        "task_status": saved["status"],
        "scope": f"one owned local process-group crash during {crash['stage']} backend loading/generation; not whole-task acceptance or external-effect replay",
    }
    private_json(folder / "crash-recovery-result.json", result)
    return result


def task_cases(seed, challenge_layout=False):
    records = cases(seed, "case")
    for n, case in enumerate(records):
        case["source_type"] = "bank_statement" if n % 2 == 0 else "journal_entry"
        for payment in case["payments"]:
            payment["id"] = payment["id"].replace("p", "p-", 1)
    if challenge_layout:
        rng = random.Random(seed ^ 0x5A19)
        identifiers = list(range(len(records)))
        rng.shuffle(identifiers)
        for n, case in enumerate(records):
            case["case_id"] = f"case-{identifiers[n]}"
        rng.shuffle(records)
    return records


def reference(case):
    payments = case["payments"]
    total = sum(p["cents"] for p in payments)
    status = (
        "invalid_currency"
        if any(p["currency"] != case["currency"] for p in payments)
        else "duplicate"
        if len({p["id"] for p in payments}) != len(payments)
        else "missing"
        if not payments
        else "matched"
        if total == case["expected_cents"]
        else "mismatch"
    )
    return {
        "case_id": case["case_id"],
        "observed_cents": total,
        "difference_cents": total - case["expected_cents"],
        "status": status,
    }


def execute_release_policy(program, scope, controls):
    """Neutral bounded interpreter; never repairs or substitutes a rule."""
    assert set(program) == {"case_id", "rules"} and program["case_id"] == "risk-policy"
    assert (
        1 <= len(program["rules"]) <= 3
        and controls
        and all(type(v) is bool for v in controls.values())
    )
    predicates = {
        "outside_scope": scope != "synthetic",
        "any_failed": any(not v for v in controls.values()),
        "always": True,
    }
    release = None
    for rule in program["rules"]:
        assert set(rule) == {"when", "release"} and rule["when"] in predicates
        assert rule["release"] in {"deny", "allow_synthetic_evaluation"}
        if release is None and predicates[rule["when"]]:
            release = rule["release"]
    assert release is not None, "program did not cover original input"
    blockers = sorted(
        [name for name, passed in controls.items() if not passed]
        + ([] if scope == "synthetic" else ["production_authorization"])
    )
    return {"release": release, "blockers": blockers}


def verify_release_policy(program, names):
    checked = 0
    for scope in ("synthetic", "production", "unknown_scope"):
        for mask in range(1 << len(names)):
            controls = {name: bool(mask & (1 << i)) for i, name in enumerate(names)}
            result = execute_release_policy(program, scope, controls)
            expected = (
                "allow_synthetic_evaluation"
                if scope == "synthetic" and all(controls.values())
                else "deny"
            )
            assert result["release"] == expected, (
                "model-authored rule violates original owner policy"
            )
            checked += 1
    return checked


def verify_triage_packet(binding, decision, original, run, revision, task_sha):
    """Source identity and actual input binding; never generate the queue answer."""
    assert (
        binding["run_id"] == run
        and binding["recipient"] == "triage"
        and binding["sender"] == "reconciliation"
    )
    assert (
        binding["correlation_key"] == "case_id"
        and binding["correlation_value"] == original["case_id"]
    )
    assert binding["policy_revision"] == revision
    canonical = lambda v: hashlib.sha256(
        json.dumps(
            v, sort_keys=True, separators=(",", ":"), ensure_ascii=False
        ).encode()
    ).hexdigest()
    assert binding["accepted_record_sha256"] == canonical(original)
    packet = decision["input"]
    assert decision["case_id"] == original["case_id"] == packet["case_id"]
    assert set(packet) == {
        "case_id",
        "retrieved_source",
        "evidence_policy_revision",
        "routes",
        "source_binding",
    }
    assert packet["evidence_policy_revision"] == revision
    assert packet["retrieved_source"] == {"record": original}
    assert packet["routes"] == {
        "invalid_currency": "data_quality",
        "duplicate": "data_quality",
        "missing": "data_quality",
        "mismatch": "reconciliation",
        "matched": "none",
    }
    assert packet["source_binding"]["source_task_sha256"] == task_sha
    assert type(packet["source_binding"]["retry_after_validation_failure"]) is bool
    assert set(packet["source_binding"]) == {
        "source_task_sha256",
        "retry_after_validation_failure",
    }
    assert decision["task_input_sha256"] == canonical(packet)
    return decision["task_input_sha256"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--seed", type=int, help="Reproduce a previous synthetic dataset"
    )
    parser.add_argument(
        "--resume-folder",
        help="Resume a stopped run from its existing campaign and frozen contract",
    )
    parser.add_argument(
        "--recall-run-evidence",
        action="store_true",
        help="Explicit owner recall of this stopped run's own archived source records before resume",
    )
    parser.add_argument(
        "--challenge-layout",
        action="store_true",
        help="Shuffle identities/order independently of exception kind",
    )
    parser.add_argument(
        "--crash-stage",
        choices=ORDER[2:],
        help="Kill this evaluation's owned process group during a fresh native stage, then verify automatic recovery",
    )
    parser.add_argument(
        "--revoke-stage",
        choices=ORDER[2:],
        help="Revoke memory while a fresh native call is active, verify denial/cancellation, then restore only the test-owned grant and resume",
    )
    parser.add_argument(
        "--semantic-memory",
        action="store_true",
        help="Require protected model-written summaries for all ten actors before acceptance",
    )
    parser.add_argument(
        "--consume-memory",
        action="store_true",
        help="Require all ten local models to select and read back an authorized original source during workflow assurance",
    )
    parser.add_argument(
        "--compiled-risk",
        action="store_true",
        help="Require a local-model-authored bounded release policy, independently checked before execution",
    )
    parser.add_argument(
        "--triage-memory",
        action="store_true",
        help="Require actual triage decisions from governed accepted predecessor records, not forwarded status fields",
    )
    parser.add_argument(
        "--verify-crash-folder",
        help="Verify retained crash/restart receipts without starting any process or model",
    )
    args = parser.parse_args()
    if args.consume_memory and not args.semantic_memory:
        parser.error("memory consumption requires --semantic-memory")
    if args.recall_run_evidence and not args.resume_folder:
        parser.error("evidence recall requires a stopped campaign resume")
    if args.verify_crash_folder:
        folder = Path(args.verify_crash_folder).absolute()
        local_path(ROOT, str(folder.relative_to(ROOT)), "data")
        result = json.loads((folder / "hard-stage-result.json").read_text())
        verified = verify_crash_recovery(
            folder,
            result["company_id"],
            result["run_id"],
            json.loads((folder / "crash-injection.json").read_text()),
            json.loads((folder / "snapshot.json").read_text()),
        )
        print(json.dumps(verified))
        return
    if args.crash_stage and args.resume_folder:
        parser.error("crash injection requires a new isolated evaluation")
    if args.revoke_stage and (args.resume_folder or args.crash_stage):
        parser.error(
            "live revocation requires a fresh isolated evaluation without crash injection"
        )
    seed = args.seed if args.seed is not None else secrets.randbits(63)
    roster = json.loads(ROSTER.read_text())
    models = {m["id"]: m for m in validate_roster(roster)}
    identifier = uuid.uuid4().hex
    if args.resume_folder:
        folder = Path(args.resume_folder).absolute()
        local_path(ROOT, str(folder.relative_to(ROOT)), "data")
        previous = json.loads((folder / "hard-stage-result.json").read_text())
        company, run, seed = (
            previous["company_id"],
            previous["run_id"],
            previous["seed"],
        )
        suffix = "-resume-" + identifier[:6]
    else:
        folder = local_path(
            ROOT, f"data/capability-campaigns/hard-{identifier[:12]}", "data"
        )
        folder.mkdir(mode=0o700, parents=True)
        company, run = "bank-" + identifier[:12], str(uuid.uuid4())
        suffix = ""
    port = unused_ports(4)
    api = f"http://127.0.0.1:{port + 3}"
    config = {
        "company_mode": True,
        "company_tools": False,
        "enterprise": {"profile": "synthetic-bank-reconciliation-v1"},
        "agents": [],
        "max_steps": 1,
        "max_tokens": 1024,
        "max_run_seconds": 3600,
    }
    if args.compiled_risk:
        config["enterprise"]["compiled_release_policy"] = True
    if args.triage_memory:
        config["enterprise"]["governed_triage_memory"] = True
        config["enterprise"]["triage_memory_protocol"] = 2
    if args.semantic_memory:
        config["semantic_memory"] = {
            "generator": "orchestrator",
            "max_versions_per_actor": 3,
        }
        if args.consume_memory:
            config["semantic_memory"]["verify_consumption"] = True
    for role in ORDER:
        m = models[role]
        model = {
            "provider": "local",
            "model": m["publisher"],
            "path": m["path"],
            "context_tokens": 8192,
        }
        config["agents"].append(
            {
                "id": role,
                "assignment": m["proposed_role"],
                "planner": model,
                "worker": model,
            }
        )
    if args.resume_folder:
        assert config == json.loads((folder / "config.json").read_text()), (
            "resume configuration differs from the campaign"
        )
    else:
        private_json(folder / "config.json", config)
    records = (
        json.loads((folder / "manifest.json").read_text())["task"]["cases"]
        if args.resume_folder
        else task_cases(seed, args.challenge_layout)
    )
    task = json.dumps({"profile": "synthetic-bank-reconciliation-v1", "cases": records})
    if args.resume_folder:
        assert json.loads((folder / "manifest.json").read_text())["task"] == json.loads(
            task
        )
    else:
        private_json(
            folder / "manifest.json",
            {
                "seed": seed,
                "challenge_layout": args.challenge_layout,
                "roster": roster,
                "task": json.loads(task),
                "scope": "synthetic local banking only; no real customers or production effects",
            },
        )
    env = {
        k: v
        for k, v in os.environ.items()
        if k in {"PATH", "HOME", "USER", "TMPDIR", "LANG", "SHELL"}
    }
    env.update(
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{port + 3}",
        CONFIG=str(folder / "config.json"),
        RUN_STORE=str(folder / "runs.sqlite3"),
        LOCAL_MODEL_PORT=str(port),
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
    )
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def request(path, body=None):
        req = urllib.request.Request(
            api + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with opener.open(req, timeout=20) as response:
            return json.load(response)

    def op(kind, **fields):
        return request(f"/api/companies/{company}", {"op": kind, **fields})

    report = {
        "status": "FAIL",
        "company_id": company,
        "run_id": run,
        "seed": seed,
        "scope": "integrated synthetic workflow only; not full enterprise qualification or certification",
    }
    started, peak = time.monotonic(), 0
    print(json.dumps({"campaign": str(folder), "seed": seed}), flush=True)
    before_calls = 0
    crash = None
    revocation = None
    seen_native = set()
    with (folder / f"backend{suffix}.log").open("xb") as log:
        os.chmod(folder / f"backend{suffix}.log", 0o600)
        process = subprocess.Popen(
            [str(ROOT / "target/debug/gents-harness")],
            cwd=ROOT,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            deadline = time.monotonic() + 30
            while True:
                if process.poll() is not None:
                    raise RuntimeError("backend exited; inspect backend.log")
                try:
                    request("/api/config")
                    break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise TimeoutError("backend startup timed out")
                    time.sleep(0.2)
            criteria = [
                "Ten distinct pinned local models contribute through durable dependency handoffs",
                "All original cases satisfy independent exact integer, currency, duplication and status checks",
                "Master configures bounded tools and skills without expanding owner authority",
                "QA rejects an altered candidate and live negative access checks pass",
                "No external effects, paid inference or real customer data",
            ]
            if args.compiled_risk:
                criteria.append(
                    "The local risk model authors a bounded owner-policy implementation that passes independent scope/control counterfactual checks"
                )
            if args.triage_memory:
                criteria.append(
                    "Every triage decision uses an authorized original reconciliation record retrieved from private memory, bound to the accepted predecessor, current policy and actual native input fingerprint"
                )
            if args.semantic_memory:
                criteria.append(
                    "Protected advisory memory cites authorized original sources; issues and next actions require explicit corresponding source fields"
                )
            if args.resume_folder:
                before = request(f"/api/runs/{run}")
                assert json.loads(before["task"]) == json.loads(task)
                if args.recall_run_evidence:
                    op("recall", target_run_id=run)
                before_calls = sum(
                    e["kind"] == "call_completed" for e in before["events"]
                )
                request(f"/api/runs/{run}/resume", {})
            else:
                op("register", name="Synthetic banking assurance evaluation")
                state = op(
                    "intake",
                    run_id=run,
                    intake={
                        "idea": task,
                        "acceptance": "\n".join(criteria),
                        "exclusions": "Real customer data, transfers, external email, cloud inference and production release",
                        "timeline": "Bounded local evaluation",
                        "company_signer": "Local owner",
                        "provider_signer": "Local runtime",
                        "budget_usd": 0,
                    },
                )
                op("approve", run_id=run, digest=state["jobs"][0]["digest"])
                ceilings, grants = {}, {}
                for n, role in enumerate(ORDER):
                    skills = (
                        ["security-governance"]
                        if role in {"orchestrator", "risk"}
                        else ["finance-reconciliation"]
                    )
                    tools = (
                        ["access_delegate"]
                        if role == "orchestrator"
                        else ["integer_arithmetic"]
                        if role == "reconciliation"
                        else []
                    )
                    ceilings[role] = {
                        "tools": tools,
                        "skills": skills,
                        "memory_from": list(ORDER[:n]),
                    }
                    grants[role] = {**ceilings[role]}
                    if role == "reconciliation":
                        grants[role] = {
                            "tools": [],
                            "skills": [],
                            "memory_from": list(ORDER[:n]),
                        }
                op(
                    "governance_install",
                    run_id=run,
                    expected_revision=0,
                    policy={
                        "revision": 1,
                        "master": "orchestrator",
                        "ceilings": ceilings,
                        "grants": grants,
                    },
                )
                request(
                    "/api/runs",
                    {
                        "company_id": company,
                        "run_id": run,
                        "task": task,
                        "consent": False,
                        "definition_of_done": criteria,
                    },
                )
            print(
                json.dumps(
                    {
                        "status": "RUNNING",
                        "company_id": company,
                        "run_id": run,
                        "api": api,
                        "backend_pid": process.pid,
                    }
                ),
                flush=True,
            )
            deadline = time.monotonic() + 3630
            saved = request(f"/api/runs/{run}")
            while time.monotonic() < deadline:
                saved = request(f"/api/runs/{run}")
                peak = max(peak, model_rss(process.pid))
                target_stage = args.crash_stage or args.revoke_stage
                pending_fault = (args.crash_stage and crash is None) or (
                    args.revoke_stage and revocation is None
                )
                children = native_children(process.pid) if pending_fault else set()
                active_calls = {
                    e["call_id"]
                    for e in saved["events"]
                    if e["agent"] == target_stage and e["kind"] == "call_started"
                }
                active_calls -= {
                    e["call_id"]
                    for e in saved["events"]
                    if e["kind"] in {"call_completed", "call_failed"}
                }
                if (
                    args.revoke_stage
                    and revocation is None
                    and active_calls
                    and children - seen_native
                ):
                    revocation = exercise_revocation(
                        folder,
                        company,
                        run,
                        args.revoke_stage,
                        saved,
                        children,
                        op,
                        request,
                    )
                    report["live_revocation"] = revocation
                    continue
                if (
                    args.crash_stage
                    and crash is None
                    and active_calls
                    and children - seen_native
                ):
                    brain = ROOT / "data/companies" / company / "brain.sqlite3"
                    with sqlite3.connect(f"file:{brain}?mode=ro", uri=True) as db:
                        rows = db.execute(
                            "SELECT id,state,attempt,output_sha256 FROM workflow_nodes WHERE run_id=?",
                            (run,),
                        ).fetchall()
                        contract_row = db.execute(
                            "SELECT digest FROM jobs WHERE run_id=?", (run,)
                        ).fetchone()
                    assert contract_row is not None, "job contract not found"
                    contract = contract_row[0]
                    assert any(
                        stage == args.crash_stage
                        and state == "running"
                        and attempt == 1
                        for stage, state, attempt, _ in rows
                    )
                    completed = {
                        stage: sha
                        for stage, state, _, sha in rows
                        if state == "completed"
                    }
                    assert len(completed) >= 2, (
                        "crash test needs committed predecessors"
                    )
                    private_json(folder / "snapshot-before-crash.json", saved)
                    crash = {
                        "stage": args.crash_stage,
                        "active_call_ids": sorted(active_calls),
                        "old_backend_pid": process.pid,
                        "owned_native_pids": sorted(children),
                        "fresh_native_pids": sorted(children - seen_native),
                        "completed_checkpoints": completed,
                        "contract_sha256": contract,
                        "observation": "native backend alive with unresolved stage call; may be loading or generating, not proof of token streaming",
                    }
                    private_json(folder / "crash-injection.json", crash)
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=15)
                    process = subprocess.Popen(
                        [str(ROOT / "target/debug/gents-harness")],
                        cwd=ROOT,
                        env=env,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                        start_new_session=True,
                    )
                    crash["replacement_backend_pid"] = process.pid
                    private_json(folder / "crash-injection.json", crash)
                    startup = time.monotonic() + 30
                    while True:
                        if process.poll() is not None:
                            raise RuntimeError(
                                "replacement backend exited; inspect backend log"
                            )
                        try:
                            request("/api/config")
                            break
                        except OSError:
                            if time.monotonic() > startup:
                                raise TimeoutError(
                                    "replacement backend startup timed out"
                                )
                            time.sleep(0.2)
                    print(
                        json.dumps(
                            {
                                "status": "CRASH_RECOVERING",
                                "stage": args.crash_stage,
                                "retained_checkpoints": len(completed),
                                "backend_pid": process.pid,
                            }
                        ),
                        flush=True,
                    )
                    continue
                seen_native |= children
                if saved["status"] != "running":
                    break
                time.sleep(0.2 if pending_fault else 5)
            private_json(folder / f"snapshot{suffix}.json", saved)
            if args.crash_stage and crash is not None:
                report["crash_recovery"] = verify_crash_recovery(
                    folder, company, run, crash, saved
                )
            artifact = (
                ROOT
                / "data/companies"
                / company
                / "runs"
                / run
                / "workspace/banking-report.json"
            )
            if saved["status"] != "completed" or not artifact.is_file():
                raise AssertionError(
                    f"Run {saved['status']}: {saved['events'][-1]['message'][:400]}"
                )
            evidence = json.loads(artifact.read_text())
            assert evidence["results"] == [reference(case) for case in records]
            assert (
                len(evidence["models"])
                == len({m["publisher"] for m in evidence["models"]})
                == 10
            )
            calls = [e for e in saved["events"] if e["kind"] == "call_completed"]
            assert {e["agent"] for e in calls} == set(ORDER)
            assert all(
                e["reply"]["local_execution"] and e["reply"]["usage"]["cost_usd"] == 0
                for e in calls
            )
            assert all(evidence["state"]["controls"].values())
            if args.compiled_risk:
                policy_program = evidence["state"]["release_policy"]["program"]
                checked = verify_release_policy(
                    policy_program, sorted(evidence["state"]["controls"])
                )
                assert (
                    checked
                    == evidence["state"]["release_policy"]["validation_cases"]
                    == 384
                )
                executed = execute_release_policy(
                    policy_program, "synthetic", evidence["state"]["controls"]
                )
                assert evidence["state"]["contributions"]["risk"] == [
                    {"case_id": "bank-release", **executed}
                ]
                authored = [
                    e
                    for e in calls
                    if e["agent"] == "risk" and e["role"] == "scoped_worker"
                ]
                assert (
                    authored
                    and json.loads(authored[-1]["reply"]["text"]) == policy_program
                )
                report["release_method"] = "model_authored_bounded_rules"
                report["release_program_counterfactuals"] = checked
            assert evidence["state"]["task"] == json.loads(task)
            references = evidence["state"]["references"]
            assembled = [
                ref
                for packet in evidence["state"]["contributions"]["audit"]
                for ref in packet["source_refs"]
            ]
            assert len(assembled) == len(set(assembled))
            assert set(assembled) == {
                ref["id"] for ref in references if ref["id"] != "audit"
            }
            assert all(
                packet["source_refs"] == sorted(packet["source_refs"])
                for packet in evidence["state"]["contributions"]["audit"]
            )
            challenge = evidence["state"]["contributions"]["qa"][-1]
            assert challenge["verdict"] == "reject" and challenge[
                "mismatched_fields"
            ] == ["observed_cents"]
            brain = ROOT / "data/companies" / company / "brain.sqlite3"
            with sqlite3.connect(f"file:{brain}?mode=ro", uri=True) as db:
                checkpoints = db.execute(
                    "SELECT state,output,output_sha256 FROM workflow_nodes WHERE run_id=?",
                    (run,),
                ).fetchall()
                assert len(checkpoints) == (20 if args.consume_memory else 10)
                for status, body, sha in checkpoints:
                    assert (
                        status == "completed"
                        and hashlib.sha256(body.encode()).hexdigest() == sha
                    )
                checkpoint_hashes = dict(
                    db.execute(
                        "SELECT id,output_sha256 FROM workflow_nodes WHERE run_id=?",
                        (run,),
                    )
                )
                if args.revoke_stage:
                    assert revocation is not None and revocation["verified"]
                    assert all(
                        checkpoint_hashes[k] == v
                        for k, v in revocation[
                            "completed_checkpoints_preserved"
                        ].items()
                    )
                    rows = []
                    for table in ("context", "context_archive"):
                        rows.extend(
                            db.execute(
                                f"SELECT body FROM {table} WHERE run_id=? AND agent=? AND source='model_output'",
                                (run, args.revoke_stage),
                            ).fetchall()
                        )
                    assert rows and all(
                        json.loads(row[0])["policy_revision"]
                        == revocation["restored_revision"]
                        for row in rows
                    )
                if args.crash_stage:
                    assert report.get("crash_recovery", {}).get("verified"), (
                        "requested crash was not verified"
                    )
                for ref in references:
                    if ref["id"] in checkpoint_hashes:
                        assert ref["sha256"] == checkpoint_hashes[ref["id"]]
                policy = json.loads(
                    db.execute(
                        "SELECT manifest FROM run_governance WHERE run_id=?", (run,)
                    ).fetchone()[0]
                )
                if args.triage_memory:
                    assert evidence["state"]["memory_handoff_protocol"] == 2
                    bindings = evidence["state"]["memory_handoffs"]["triage"]
                    decisions = evidence["state"]["memory_decision_inputs"]["triage"]
                    expected_ids = {case["case_id"] for case in records}
                    assert len(bindings) == len(decisions) == len(expected_ids) == 10
                    assert (
                        {b["correlation_value"] for b in bindings}
                        == {d["case_id"] for d in decisions}
                        == expected_ids
                    )
                    checkpoint = json.loads(
                        db.execute(
                            "SELECT output FROM workflow_nodes WHERE run_id=? AND id='triage'",
                            (run,),
                        ).fetchone()[0]
                    )
                    assert len(checkpoint["calls"]) == 10
                    native = [
                        e["reply"]
                        for e in calls
                        if e["agent"] == "triage" and e["role"] == "scoped_worker"
                    ]
                    for binding in bindings:
                        body = db.execute(
                            "SELECT body FROM context WHERE id=? AND run_id=? AND agent='reconciliation' AND source='model_output' UNION ALL SELECT body FROM context_archive WHERE id=? AND run_id=? AND agent='reconciliation' AND source='model_output'",
                            (binding["source_id"], run, binding["source_id"], run),
                        ).fetchone()[0]
                        assert (
                            hashlib.sha256(body.encode()).hexdigest()
                            == binding["source_sha256"]
                        )
                        source = json.loads(body)
                        assert (
                            source["policy_revision"] == policy["revision"]
                            and source["run_id"] == run
                        )
                        original = json.loads(source["output"])
                        assert (
                            original
                            in evidence["state"]["contributions"]["reconciliation"]
                        )
                        decision = next(
                            d
                            for d in decisions
                            if d["case_id"] == binding["correlation_value"]
                        )
                        fingerprint = verify_triage_packet(
                            binding,
                            decision,
                            original,
                            run,
                            policy["revision"],
                            evidence["source_task_sha256"],
                        )
                        receipts = [
                            r
                            for r in checkpoint["calls"]
                            if r["local_execution"]["task_input_sha256"] == fingerprint
                        ]
                        assert len(receipts) == 1 and receipts[0] in native
                        assert (
                            json.loads(receipts[0]["text"])
                            in evidence["state"]["contributions"]["triage"]
                        )
                    report["memory_driven_triage_decisions"] = len(bindings)
                assert policy["grants"]["reconciliation"]["tools"] == [
                    "integer_arithmetic"
                ]
                assert policy["grants"]["reconciliation"]["skills"] == [
                    "finance-reconciliation"
                ]
                snapshots = db.execute(
                    "SELECT COUNT(DISTINCT agent) FROM memory_snapshots WHERE run_id=?",
                    (run,),
                ).fetchone()[0]
                assert snapshots >= 10
                if args.semantic_memory:
                    descriptors = evidence["semantic_memory"]["actors"]
                    assert {row["recipient"] for row in descriptors} == set(ORDER)
                    for descriptor in descriptors:
                        row = db.execute(
                            "SELECT result,result_sha256,source_refs,receipt,policy_revision,protocol_version FROM semantic_memos WHERE run_id=? AND actor=? AND input_sha256=? AND state='completed'",
                            (run, descriptor["recipient"], descriptor["input_sha256"]),
                        ).fetchone()
                        assert row is not None
                        body, digest, source_blob, receipt_blob, revision, protocol = (
                            row
                        )
                        assert protocol == descriptor["protocol_version"] == 3
                        assert (
                            hashlib.sha256(body.encode()).hexdigest()
                            == digest
                            == descriptor["sha256"]
                        )
                        assert (
                            revision
                            == descriptor["policy_revision"]
                            == policy["revision"]
                        )
                        receipt = json.loads(receipt_blob)
                        assert (
                            receipt["local_execution"]
                            and receipt["usage"]["cost_usd"] == 0
                        )
                        assert json.loads(receipt["text"]) == json.loads(body)
                        source_map = {
                            source["alias"]: source
                            for source in json.loads(source_blob)
                        }
                        for source in source_map.values():
                            if source["id"] == "frozen-task-contract":
                                intake_blob, digest = db.execute(
                                    "SELECT intake,digest FROM jobs WHERE run_id=?",
                                    (run,),
                                ).fetchone()
                                intake = json.loads(intake_blob)
                                contract = {
                                    "intake_sha256": digest,
                                    "idea": intake["idea"],
                                    "acceptance": intake["acceptance"],
                                    "exclusions": intake["exclusions"],
                                }
                                expected_sha = hashlib.sha256(
                                    json.dumps(
                                        contract,
                                        sort_keys=True,
                                        separators=(",", ":"),
                                        ensure_ascii=False,
                                    ).encode()
                                ).hexdigest()
                            else:
                                original = db.execute(
                                    "SELECT body FROM context WHERE id=? UNION ALL SELECT body FROM context_archive WHERE id=?",
                                    (source["id"], source["id"]),
                                ).fetchone()
                                assert original is not None
                                expected_sha = hashlib.sha256(
                                    original[0].encode()
                                ).hexdigest()
                                original_body = json.loads(original[0])
                                assert (
                                    original_body["policy_revision"] == revision
                                    and original_body["run_id"] == run
                                )
                                assert source["text"] == original_body["output"]
                            assert expected_sha == source["sha256"]
                        summary = json.loads(body)
                        assert (
                            1
                            <= sum(
                                len(summary[k])
                                for k in ("decisions", "unresolved", "next_actions")
                            )
                            <= 3
                        )
                        for category in ("decisions", "unresolved", "next_actions"):
                            for claim in summary[category]:
                                assert (
                                    claim["quote"]
                                    in source_map[claim["source_id"]]["text"]
                                )
                                if category != "decisions":
                                    explicit = json.loads(
                                        source_map[claim["source_id"]]["text"]
                                    ).get(category)
                                    assert isinstance(explicit, list) and explicit, (
                                        "summary invented a category absent from its source"
                                    )
                                    assert claim["quote"] in json.dumps(
                                        explicit, ensure_ascii=False
                                    ), (
                                        "quote is not from the explicit action/issue field"
                                    )
                    report["semantic_summary_actors"] = len(descriptors)
                    report["semantic_model_calls"] = sum(
                        e["role"] == "memory_summary" for e in calls
                    )
                if args.consume_memory:
                    assert (
                        db.execute(
                            "SELECT COUNT(*) FROM context WHERE run_id=? AND source='model_output' AND json_extract(body,'$.phase') IN ('memory_select','memory_extract')",
                            (run,),
                        ).fetchone()[0]
                        == 0
                    ), "derived audit replies must not feed canonical memory"
                    consumed = evidence["memory_consumption"]
                    assert {r["recipient"] for r in consumed} == set(ORDER) and len(
                        consumed
                    ) == 10
                    descriptors = {
                        r["recipient"]: r for r in evidence["semantic_memory"]["actors"]
                    }
                    for item in consumed:
                        actor = item["recipient"]
                        assert item["policy_revision"] == policy["revision"]
                        assert (
                            item["memo_input_sha256"]
                            == descriptors[actor]["input_sha256"]
                        )
                        body, sources = db.execute(
                            "SELECT result,source_refs FROM semantic_memos WHERE run_id=? AND actor=? AND input_sha256=? AND state='completed'",
                            (run, actor, item["memo_input_sha256"]),
                        ).fetchone()
                        assert (
                            json.loads(body)["decisions"][0]["source_id"]
                            == item["source_alias"]
                        )
                        source = next(
                            s
                            for s in json.loads(sources)
                            if s["alias"] == item["source_alias"]
                        )
                        assert source["sha256"] == item["source_sha256"]
                        assert (
                            json.loads(source["text"])[item["field"]]
                            == item["extraction"]["value"]
                        )
                        checkpoint = json.loads(
                            db.execute(
                                "SELECT output FROM workflow_nodes WHERE run_id=? AND id=?",
                                (run, "memory-" + actor),
                            ).fetchone()[0]
                        )
                        assert (
                            checkpoint["data"] == item and len(checkpoint["calls"]) == 2
                        )
                        replies = checkpoint["calls"]
                        assert all(
                            r["usage"]["cost_usd"] == 0 and r["local_execution"]
                            for r in replies
                        )
                        assert json.loads(replies[0]["text"]) == {
                            "source_alias": item["source_alias"]
                        }
                        assert json.loads(replies[1]["text"]) == item["extraction"]
                        for index, phase in enumerate(
                            ("memory_select", "memory_extract")
                        ):
                            matching = [
                                e
                                for e in calls
                                if e["agent"] == actor and e["role"] == phase
                            ]
                            assert matching and all(
                                models[actor]["publisher"] in e["model"]
                                for e in matching
                            )
                            assert any(
                                e["reply"] == replies[index] for e in matching
                            ), (
                                "checkpoint reply missing from the actor's correlated native call events"
                            )
                    report["memory_consumers"] = len(consumed)
                    report["memory_consumer_calls"] = sum(
                        e["role"] in {"memory_select", "memory_extract"} for e in calls
                    )
            report.update(
                status="PASS",
                cases=len(records),
                models=10,
                calls=len(calls),
                calls_started=sum(e["kind"] == "call_started" for e in saved["events"]),
                calls_failed=sum(e["kind"] == "call_failed" for e in saved["events"]),
                calls_cancelled=sum(
                    e["kind"] == "policy_epoch_cancelled" for e in saved["events"]
                ),
                semantic_generation_failures=sum(
                    e["kind"] == "semantic_memory_failed" for e in saved["events"]
                ),
                new_model_calls=len(calls) - before_calls,
                checkpoint_reuses=sum(
                    e["kind"] == "workflow_checkpoint_reused" for e in saved["events"]
                ),
                provider_cost_usd=0,
                memory_snapshot_agents=snapshots,
                evidence=str(artifact),
                evidence_sha256=hashlib.sha256(artifact.read_bytes()).hexdigest(),
            )
        except Exception as error:
            report["error"] = f"{type(error).__name__}: {error}"[:1000]
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
            report.update(
                elapsed_seconds=round(time.monotonic() - started, 2),
                peak_backend_resident_kib=peak,
            )
            report["result_file"] = str(folder / f"hard-stage{suffix}-result.json")
            private_json(Path(report["result_file"]), report)
    print(json.dumps(report), flush=True)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Bounded real-local medium-stage evaluation; no network, mail or downloads."""

import argparse
import hashlib
import json
import os
import random
import secrets
import socket
import subprocess
import time
import urllib.request
import uuid
from pathlib import Path

import local_sqlite as sqlite3

ROOT = Path(__file__).resolve().parents[1]
CANDIDATES = [
    ("Qwen3.5-4B", "models/qwen3.5-4b-router/Qwen3.5-4B-Q4_K_M.gguf"),
    ("Qwen3-8B", "models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf"),
    ("Qwen2.5-1.5B", "models/qwen2.5-1.5b-instruct/qwen2.5-1.5b-instruct-q4_k_m.gguf"),
]
CONTEXT_TOKENS = {"Qwen2.5-1.5B": 8192}


def unused_ports(count):
    for _ in range(100):
        listeners = []
        try:
            first = socket.socket()
            first.bind(("127.0.0.1", 0))
            port = first.getsockname()[1]
            listeners.append(first)
            for offset in range(1, count):
                listener = socket.socket()
                listeners.append(listener)
                listener.bind(("127.0.0.1", port + offset))
            return port
        except (OSError, OverflowError):
            continue
        finally:
            for listener in listeners:
                listener.close()
    raise RuntimeError("No available local port range")


def cases(seed, prefix):
    rng = random.Random(seed)
    result = []
    for index in range(10):
        values = [rng.randint(11, 999), rng.randint(11, 999), -rng.randint(1, 30)]
        payments = [
            {"id": f"p{number}", "cents": cents, "currency": "USD"}
            for number, cents in enumerate(values)
        ]
        total = sum(values)
        kind = index % 5
        expected = total if kind != 1 else total + rng.randint(1, 80)
        if kind == 2:
            payments[1]["id"] = payments[0]["id"]
        if kind == 3:
            payments[0]["currency"] = "EUR"
            # Status priority is evaluated independently of duplicates.
            payments[1]["id"] = payments[0]["id"]
        if kind == 4:
            payments = []
            expected = 0 if index == 9 else rng.randint(1, 90)
        result.append(
            {
                "case_id": f"{prefix}-{index}",
                "expected_cents": expected,
                "currency": "USD",
                "payments": payments,
            }
        )
    return result


def expected(case):
    payments = case["payments"]
    total = sum(row["cents"] for row in payments)
    if any(row["currency"] != case["currency"] for row in payments):
        status = "invalid_currency"
    elif len({row["id"] for row in payments}) != len(payments):
        status = "duplicate"
    elif not payments:
        status = "missing"
    else:
        status = "matched" if total == case["expected_cents"] else "mismatch"
    return {
        "case_id": case["case_id"],
        "status": status,
        "currency_mismatch": any(
            row["currency"] != case["currency"] for row in payments
        ),
        "duplicate_ids": len({row["id"] for row in payments}) != len(payments),
        "empty": not payments,
        "observed_cents": total,
        "difference_cents": total - case["expected_cents"],
    }


def model_rss(parent_pid):
    data = subprocess.check_output(
        ["/bin/ps", "-axo", "pid=,ppid=,rss=,comm="], text=True
    )
    return sum(
        int(fields[2])
        for line in data.splitlines()
        if len(fields := line.strip().split(None, 3)) == 4
        and fields[1] == str(parent_pid)
        and fields[3].endswith("/llama-server")
    )


def trial(folder, candidate, variant, records, holdout=False):
    model_name, model_path = candidate
    model = {"provider": "local", "model": model_name, "path": model_path}
    if model_name in CONTEXT_TOKENS:
        model["context_tokens"] = CONTEXT_TOKENS[model_name]
    config = {
        "company_mode": True,
        "company_tools": False,
        "campaign": {"variant": variant, "require_parallel": holdout},
        "agents": [
            {
                "id": f"Reconciler-{n}",
                "assignment": "Reconcile only assigned synthetic cases against contract v1",
                "planner": model,
                "worker": model,
            }
            for n in (1, 2)
        ],
        "max_steps": 1,
        "max_tokens": 1024,
        "max_run_seconds": 600,
    }
    folder.mkdir(parents=True)
    config_path = folder / "config.json"
    config_path.write_text(json.dumps(config, indent=2) + "\n")
    port = unused_ports(4)
    api_port = port + 3
    base = f"http://127.0.0.1:{api_port}"
    company, run = "pair-" + uuid.uuid4().hex[:12], str(uuid.uuid4())
    env = {
        key: value
        for key, value in os.environ.items()
        if key in {"PATH", "HOME", "USER", "TMPDIR", "LANG", "SHELL"}
    }
    env.update(
        AUTH_MODE="local",
        BIND=f"127.0.0.1:{api_port}",
        CONFIG=str(config_path),
        RUN_STORE=str(folder / "runs.sqlite3"),
        LOCAL_MODEL_PORT=str(port),
        GENTS_LOCAL_SMALL_LANES="2",
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
    )
    started = time.monotonic()
    peak_rss = 0
    report = {
        "status": "FAIL",
        "company_id": company,
        "run_id": run,
        "model": model_name,
        "variant": variant,
        "holdout": holdout,
        "workflow_revision": 2 if variant == "compiled_policy" else 1,
    }
    with (folder / "backend.log").open("wb") as log:
        process = subprocess.Popen(
            [str(ROOT / "target/debug/gents-harness")],
            cwd=ROOT,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
        )

        def request(path, body=None):
            req = urllib.request.Request(
                base + path,
                data=None if body is None else json.dumps(body).encode(),
                headers={"Content-Type": "application/json"},
            )
            with urllib.request.urlopen(req, timeout=15) as response:
                return json.load(response)

        def op(kind, **values):
            return request(f"/api/companies/{company}", {"op": kind, **values})

        try:
            deadline = time.monotonic() + 30
            while True:
                if process.poll() is not None:
                    raise RuntimeError(
                        "Local backend exited during startup; inspect trial backend.log"
                    )
                try:
                    request("/api/config")
                    break
                except OSError:
                    if time.monotonic() >= deadline:
                        raise TimeoutError("Local backend startup timed out")
                    time.sleep(0.2)
            op("register", name="Local paired capability evaluation")
            task = json.dumps(
                {"campaign": "synthetic-reconciliation-v1", "cases": records}
            )
            criteria = [
                "Every case matches independent exact integer and status checks",
                "Two local Gents each execute a planner then a worker with versioned handoff evidence",
                "No external calls or real customer data",
            ]
            if holdout:
                criteria.append(
                    "Overlapping requests execute on two distinct owned local backend processes"
                )
            state = op(
                "intake",
                run_id=run,
                intake={
                    "idea": task,
                    "acceptance": "\n".join(criteria),
                    "exclusions": "External network and real customer data",
                    "timeline": "Bounded local evaluation",
                    "company_signer": "Local owner",
                    "provider_signer": "Local runtime",
                    "budget_usd": 0,
                },
            )
            op("approve", run_id=run, digest=state["jobs"][0]["digest"])
            access = {
                "tools": ["integer_arithmetic"]
                if variant in {"tool_grounded", "decomposed", "compiled_policy"}
                else [],
                "skills": ["finance-reconciliation"],
                "memory_from": [],
            }
            grants = {agent["id"]: access for agent in config["agents"]}
            op(
                "governance_install",
                run_id=run,
                expected_revision=0,
                policy={
                    "revision": 1,
                    "master": "Reconciler-1",
                    "ceilings": grants,
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
                        **report,
                        "status": "STARTED",
                        "backend_pid": process.pid,
                        "api": base,
                        "trial": str(folder),
                    }
                ),
                flush=True,
            )
            deadline = time.monotonic() + 620
            while time.monotonic() < deadline:
                saved = request(f"/api/runs/{run}")
                peak_rss = max(peak_rss, model_rss(process.pid))
                if saved["status"] != "running":
                    break
                time.sleep(1)
            run_dir = ROOT / "data/companies" / company / "runs" / run
            (folder / "snapshot.json").write_text(json.dumps(saved, indent=2) + "\n")
            evidence_path = run_dir / "workspace/reconciliation-evidence.json"
            if not evidence_path.is_file():
                message = saved["events"][-1]["message"][:500]
                raise AssertionError(
                    f"No reconciliation evidence: {saved['status']} {message}"
                )
            evidence = json.loads(evidence_path.read_text())
            # Validate durable scheduler receipts independently of the report's
            # pass flag. Checkpoint completion is not the finance verifier.
            brain = ROOT / "data/companies" / company / "brain.sqlite3"
            with sqlite3.connect(f"file:{brain}?mode=ro", uri=True) as database:
                manifest, graph_sha = database.execute(
                    "SELECT manifest,sha256 FROM workflow_graphs WHERE run_id=?", (run,)
                ).fetchone()
                assert hashlib.sha256(manifest.encode()).hexdigest() == graph_sha
                assert evidence["workflow"]["graph_sha256"] == graph_sha
                checkpoints = database.execute(
                    "SELECT state,attempt,output,output_sha256 FROM workflow_nodes WHERE run_id=?",
                    (run,),
                ).fetchall()
                assert len(checkpoints) == 4
                for state, attempt, body, sha in checkpoints:
                    assert state == "completed" and 1 <= attempt <= 3
                    assert hashlib.sha256(body.encode()).hexdigest() == sha
                    assert json.loads(body)["calls"]
            rows = [row for group in evidence["results"] for row in group["results"]]
            unique = len({row["case_id"] for row in rows}) == len(rows) == len(records)
            correct = (
                sum(rows.count(expected(case)) == 1 for case in records)
                if unique
                else 0
            )
            calls = [call for group in evidence["calls"] for call in group]
            if variant == "compiled_policy":
                assert len(records) + 2 <= len(calls) <= 3 * len(records) + 2
            else:
                assert len(calls) == (
                    len(records) + 2 if variant == "decomposed" else 4
                )
            assert all(call["usage"]["cost_usd"] == 0 for call in calls)
            assert all(call.get("local_execution") for call in calls)
            report.update(
                status="PASS"
                if evidence["passed"] and correct == len(records)
                else "FAIL",
                correct_cases=correct,
                total_cases=len(records),
                overlap_ms=evidence["overlap_ms"],
                model_calls=len(calls),
                provider_cost_usd=0,
                evidence=str(evidence_path),
                rejected_observations=sum(
                    event["kind"] == "observation_rejected" for event in saved["events"]
                ),
                durable_checkpoint_count=len(checkpoints),
                evidence_sha256=hashlib.sha256(evidence_path.read_bytes()).hexdigest(),
            )
            (folder / "snapshot.json").write_text(json.dumps(saved, indent=2) + "\n")
        except Exception as error:
            report["error"] = f"{type(error).__name__}: {error}"[:1000]
        finally:
            process.terminate()
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            report.update(
                elapsed_seconds=round(time.monotonic() - started, 2),
                peak_backend_resident_kib=peak_rss,
            )
            (folder / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report), flush=True)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--candidate", action="append", choices=[item[0] for item in CANDIDATES]
    )
    parser.add_argument(
        "--variant",
        action="append",
        choices=[
            "baseline",
            "focused",
            "tool_grounded",
            "decomposed",
            "compiled_policy",
        ],
    )
    args = parser.parse_args()
    campaign = ROOT / "data/capability-campaigns" / ("medium-" + uuid.uuid4().hex[:12])
    campaign.mkdir(parents=True, mode=0o700)
    print(json.dumps({"campaign": str(campaign), "pid": os.getpid()}), flush=True)
    training = cases(2731, "train")
    training_ids = {case["case_id"] for case in training}
    results = []
    for candidate in [
        item for item in CANDIDATES if not args.candidate or item[0] in args.candidate
    ]:
        for variant in args.variant or (
            "baseline",
            "focused",
            "tool_grounded",
            "decomposed",
        ):
            results.append(
                trial(
                    campaign / f"{candidate[0]}-{variant}", candidate, variant, training
                )
            )
    eligible = [
        row
        for row in results
        if row.get("correct_cases") == len(training) and row.get("overlap_ms", 0) > 0
    ]
    summary = {
        "stage": "medium",
        "status": "FAIL",
        "training_trials": results,
        "fine_tuning": "Prompt variants evaluated first; no weight training or adapter promotion occurred",
    }
    if eligible:
        # Select on training evidence only. Holdout never feeds model/prompt selection.
        winner = min(eligible, key=lambda row: row["elapsed_seconds"])
        candidate = next(item for item in CANDIDATES if item[0] == winner["model"])
        # Earlier held-out failures informed harness repairs. Never reuse that
        # fixed test set and describe it as unseen after those repairs.
        holdout_seed = secrets.randbits(63)
        holdout = cases(holdout_seed, "heldout")
        assert training_ids.isdisjoint(case["case_id"] for case in holdout)
        (campaign / "evaluation-manifest.json").write_text(
            json.dumps(
                {
                    "training_seed": 2731,
                    "heldout_seed": holdout_seed,
                    "selected_model": winner["model"],
                    "selected_variant": winner["variant"],
                    "selection": "training evidence only; fresh heldout generated after selection",
                    "heldout_sha256": hashlib.sha256(
                        json.dumps(holdout, sort_keys=True).encode()
                    ).hexdigest(),
                },
                indent=2,
            )
            + "\n"
        )
        heldout = trial(
            campaign / "heldout", candidate, winner["variant"], holdout, holdout=True
        )
        summary.update(
            status=heldout["status"],
            selected_model=winner["model"],
            selected_variant=winner["variant"],
            heldout=heldout,
        )
        if heldout["status"] == "PASS":
            (campaign / "medium-stage-paper.md").write_text(
                "# Two local planner worker pairs for synthetic reconciliation\n\n"
                "## Method\n\nTwo independently scoped Gents each received half of a versioned synthetic reconciliation task. "
                "Each local planner produced a compact implementation artifact for its worker. The existing Rust harness governed "
                "memory and skill assignments; a two-lane adapter admitted small models and serialized large-model inference and training. "
                "An immutable four-node DAG checkpointed each planner/worker phase with model-weight and output hashes, "
                "policy-bound leases and bounded read-only recovery. The external verifier checked the actual SQLite checkpoint hashes. "
                "Configured prompt and workload variants were evaluated on ten training cases; earlier failed approaches remain in separate receipts. "
                "This campaign selected its model and prompt on training evidence, then generated a fresh recorded held-out seed. "
                "Earlier test failures informed architecture repairs, so this is an exploratory evaluation, not a blind benchmark. "
                "No weight training occurred.\n\n"
                "For compiled_policy revision two, each local planner authored the bounded ordered rule artifact. Workers copied ordered source columns, "
                "generic bounded predicates calculated duplicate/currency/empty flags from those copied columns, "
                "a source verifier rejected incorrect observations with bounded retries and no supplied answers, "
                "an authorized integer tool supplied exact arithmetic without round-tripping it through model output, "
                "and a neutral interpreter applied the model-authored rules. "
                "The interpreter neither consulted the reference verifier nor repaired incorrect mappings. "
                "Earlier exploratory trials used changing harness revisions and are not a controlled ranking of models.\n\n"
                f"## Results\n\nTraining selected {winner['model']} with the {winner['variant']} prompt. "
                f"On ten unseen cases, both an external Python verifier and the Rust verifier accepted all outputs. "
                f"The held-out run took {heldout['elapsed_seconds']} seconds and recorded {heldout['overlap_ms']} ms of overlapping "
                f"HTTP inference intervals on distinct owned backend PIDs and ports. Peak sampled backend RSS was "
                f"{heldout['peak_backend_resident_kib']} KiB. {heldout['model_calls']} real model replies incurred USD 0 provider charges. "
                "The trial receipts retain typed outputs, model identities, reported tokens, timing and evidence hashes.\n\n"
                "## Limitations\n\nObserved backend request overlap does not prove every GPU kernel executes simultaneously. "
                "RSS sampling can miss short peaks and does not capture all unified GPU allocations or unrelated host memory. "
                "The admission estimate is not a process/container memory cap. Only ten held-out cases were tested, so these results "
                "do not establish general financial accuracy, absence of bias, frontier-model equivalence, regulatory compliance "
                "or the ten-distinct-model enterprise stage. Electricity and hardware costs are excluded.\n"
            )
    else:
        summary["error"] = (
            "No candidate passed all training cases with measured parallel inference; retain evidence and repair before heldout"
        )
    (campaign / "medium-stage-result.json").write_text(
        json.dumps(summary, indent=2) + "\n"
    )
    print(
        json.dumps(
            {
                "status": summary["status"],
                "campaign": str(campaign),
                "summary": str(campaign / "medium-stage-result.json"),
            }
        ),
        flush=True,
    )


if __name__ == "__main__":
    main()

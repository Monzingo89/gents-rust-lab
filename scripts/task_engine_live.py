#!/usr/bin/env python3
"""Bounded existing-native-model RAG check; no downloads, cloud or production admission."""

import argparse
import copy
import hashlib
import json
import sqlite3
import time
import uuid
from pathlib import Path

from local_trial import local_trial, port

__all__ = ["MODEL", "MODELS", "ROOT", "file_sha", "local_trial", "main", "port"]

ROOT = Path(__file__).resolve().parents[1]
MODEL = "models/qwen3-4b-instruct/Qwen3-4B-Q4_K_M.gguf"
MODELS = {
    "qwen3-4b": (
        MODEL,
        "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
    ),
    "qwen3-8b": (
        "models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf",
        "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785",
    ),
}


def file_sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main(
    model_key="qwen3-4b",
    *,
    workers=None,
    default_worker="trial",
    observed_at=None,
    resident_budget=None,
    output_dir=None,
):
    relative, expected_sha = MODELS[model_key]
    path = ROOT / relative
    if not path.is_file() or path.is_symlink():
        raise SystemExit(
            "Existing bounded local model unavailable; no download attempted"
        )
    sha = file_sha(path)
    if sha != expected_sha:
        raise ValueError("Installed trial model pin changed; no download attempted")
    run = str(uuid.uuid4())
    company = "task-live-" + run[:8]
    output = (
        ROOT / "data/task-engine-live" / run if output_dir is None else Path(output_dir)
    )
    output.mkdir(parents=True, mode=0o700)
    display = "Qwen3-4B" if model_key == "qwen3-4b" else "Qwen3-8B"
    model = {
        "provider": "local",
        "model": display + " existing bounded RAG trial",
        "path": relative,
        "context_tokens": 8192,
        "local_reasoning_tokens": 512,
        "local_final_tokens": 512,
    }
    worker = {
        "model": model,
        "sha256": sha,
        "memory_estimate": {
            "weights_bytes": path.stat().st_size,
            "adapters_bytes": 0,
            "kv_bytes": 512 * 1024**2,
            "context_bytes": 32 * 1024**2,
            "runtime_bytes": 1024**3,
            "headroom_bytes": 2 * 1024**3,
            "provenance": "conservative explicit trial estimate; measured residency reported separately",
        },
        "evaluated_profiles": ["general", "history"],
        "evaluation_reference": "bounded trial only; this isolated harness creates evaluation evidence, not production admission",
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Live Trial",
                "assignment": "bounded read-only trial",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 512,
        "max_run_seconds": 180,
        "task_workflows": {
            "models": {"trial": worker},
            "default_model": "trial",
            "max_calls": 1,
            "total_generation_tokens": 1024,
            "per_call_tokens": 1024,
            "deadline_seconds": 120,
            "max_retries": 0,
            "max_pending_jobs": 2,
            "concurrency": 1,
            "resident_budget_bytes": 6 * 1024**3,
            "allow_parametric_answers": False,
        },
    }
    settings = config["task_workflows"]
    settings["resident_budget_bytes"] = (
        resident_budget
        if resident_budget is not None
        else (6 if model_key == "qwen3-4b" else 9) * 1024**3
    )
    if workers is not None:
        settings["models"] = copy.deepcopy(workers)
        settings["default_model"] = default_worker
    configpath = output / "config.json"
    configpath.write_text(json.dumps(config, indent=2) + "\n")
    with (ROOT / "target/debug/gents-harness").open("rb") as stream:
        binary_sha = hashlib.file_digest(stream, "sha256").hexdigest()
    began = time.monotonic()
    try:
        with local_trial(ROOT, configpath, output) as trial:
            req = trial.api
            req(
                "/api/companies/" + company,
                {"op": "register", "name": "Isolated bounded local RAG evaluation"},
            )
            route = "/api/companies/" + company + "/tasks"
            now = int(time.time() * 1000) if observed_at is None else observed_at
            text = "Gerald Ford held the US presidency from August 9, 1974 until January 20, 1977, following election loss to Jimmy Carter."
            source = {
                "source_id": "ford-library-paraphrase",
                "revision": "2026-11-18-trial-v1",
                "title": "Ford Library facts page; bounded paraphrase; https://www.fordlibrarymuseum.gov/the-fords/gerald-r-ford/facts-and-favorites",
                "domain": "history",
                "language": "en",
                "text": text,
                "observed_at_ms": now,
                "valid_until_ms": now + 600000,
                "lookups": [],
            }
            taskinput = {
                "original_ask": "Who was US president in 1976? Return only the full name.",
                "temporal_scope": "1976",
                "output": {"format": "text", "language": "en"},
            }
            # Preserve the existing factual oracle and one-call ceiling. Freeze
            # them before any generator is admitted; failure cannot edit them.
            criteria = {
                "input": taskinput,
                "source": source,
                "accepted_answer_forms": [
                    "gerald ford",
                    "gerald r. ford",
                    "gerald r ford",
                ],
                "expected_status": "generated",
                "expected_model_calls": 1,
                "max_calls": 1,
                "max_retries": 0,
                "max_generation_tokens": 1024,
                "deadline_seconds": 120,
                "model_sha256": sha,
                "harness_sha256": binary_sha,
                "production_profile_admission": False,
            }
            criteria.update(
                config_sha256=file_sha(configpath),
                probe_sha256=file_sha(Path(__file__)),
                lifecycle_sha256=file_sha(ROOT / "scripts/local_trial.py"),
            )
            (output / "probe-source.py").write_bytes(Path(__file__).read_bytes())
            criteria_bytes = (
                json.dumps(criteria, sort_keys=True, indent=2) + "\n"
            ).encode()
            (output / "criteria.json").write_bytes(criteria_bytes)
            req(route, {"op": "ingest", "source": source})
            task_began = time.monotonic()
            started = req(route, {"op": "execute", "input": taskinput})
            task = started["request"]["task_id"]
            deadline = time.monotonic() + 150
            while True:
                result = req(route, {"op": "status", "task_id": task})
                if result["status"] != "running":
                    break
                if time.monotonic() > deadline:
                    req(route, {"op": "cancel_task", "task_id": task})
                    raise AssertionError("live deadline")
                time.sleep(0.2)
            rag = result
            factual_pass = (rag.get("answer") or "").strip().rstrip(".").lower() in {
                "gerald ford",
                "gerald r. ford",
                "gerald r ford",
            }
            task_elapsed_ms = (time.monotonic() - task_began) * 1000
            with sqlite3.connect(
                ROOT / "data/companies" / company / "brain.sqlite3"
            ) as db:
                receipts = [
                    {"kind": kind, "body": json.loads(body)}
                    for kind, body in db.execute(
                        "SELECT kind,body FROM audit WHERE kind IN ('task_model_dispatch','task_model_return') ORDER BY seq"
                    )
                ]
            native_returns = [
                row["body"] for row in receipts if row["kind"] == "task_model_return"
            ]
            native = (
                native_returns[0].get("local_execution", {}) if native_returns else {}
            )
            # Install a separate reviewed extraction binding only AFTER the live
            # RAG check, then prove exact repetition returns without generation.
            source["revision"] = "2026-11-18-trial-v2"
            source["lookups"] = [
                {
                    "exact_ask": "Return the recorded president name.",
                    "temporal_scope": "1976",
                    "context_independent": True,
                    "start_byte": 0,
                    "end_byte": 11,
                }
            ]
            req(route, {"op": "ingest", "source": source})
            taskinput = {
                "original_ask": "Return the recorded president name.",
                "temporal_scope": "1976",
                "output": {"format": "text", "language": "en"},
            }
            first = req(route, {"op": "execute", "input": taskinput})["result"]
            second = req(route, {"op": "execute", "input": taskinput})["result"]
            assert (
                first["actual_path"] == "deterministic_lookup"
                and second["actual_path"] == "exact_answer_cache"
            )
            assert second["measurements"]["generative_model_calls"] == 0
            hardware = req("/api/companies/" + company + "/hardware")
            report = {
                "factual_acceptance_passed": factual_pass,
                "expected_answer": "Gerald Ford",
                "schema_version": 1,
                "company": company,
                "model_path": MODEL,
                "model_sha256": sha,
                "criteria_sha256": hashlib.sha256(criteria_bytes).hexdigest(),
                "harness_sha256": binary_sha,
                "rag_result": rag,
                "call_receipts": receipts,
                "configured_context_tokens": model["context_tokens"],
                "observed_slot_context_tokens": native.get("observed_context_tokens"),
                "rag_model_input_tokens": rag["measurements"].get("input_tokens"),
                "rag_model_output_tokens": rag["measurements"].get("output_tokens"),
                "resident_memory_bytes": rag["measurements"].get("memory_bytes"),
                "memory_scope": "Native backend RSS observed at reply, not peak/GPU allocation",
                "exact_first": first,
                "exact_repeat": second,
                "exact_cache_requests": 2,
                "exact_cache_hits": 1,
                "exact_cache_hit_ratio": 0.5,
                "paid_calls": 0,
                "actual_provider_cost_usd": 0,
                "estimated_total_cost_usd": None,
                "hardware": hardware,
                "elapsed_seconds": time.monotonic() - began,
                "provider_cost_usd": 0,
                "production_profile_admission": False,
                "quality_scope": "One source-grounded historical question against the existing fixed oracle; no general expert or routing quality claim.",
            }
            selected = native_returns[0].get("model") if native_returns else None
            actual = settings["models"].get(selected, {})
            report.update(
                selected_model=selected,
                task_elapsed_ms=task_elapsed_ms,
                model_path=actual.get("model", {}).get("path"),
                model_sha256=actual.get("sha256"),
            )
            (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
            print(
                json.dumps(
                    {
                        "report": str(output / "report.json"),
                        "rag_status": rag["status"],
                        "rag_answer": rag["answer"],
                        "rag_verification": rag["verification_state"],
                        "rag_measurements": rag["measurements"],
                        "exact_repeat_calls": second["measurements"][
                            "generative_model_calls"
                        ],
                        "provider_cost_usd": 0,
                    }
                )
            )
            assert rag["status"] == "generated" and factual_pass, rag
            assert rag["measurements"]["generative_model_calls"] == 1
            assert (
                len(native_returns) == 1
                and native_returns[0]["native_dispatch_receipt"]
            )
            assert (output / "criteria.json").read_bytes() == criteria_bytes
    finally:
        checkpoint = ROOT / "data/companies" / company
        if checkpoint.is_dir() and not checkpoint.is_symlink():
            checkpoint.rename(output / "company-checkpoint")
    return report


def compare():
    """Owner-frozen three-call experiment, never a production admission write."""
    folder = ROOT / "data/task-engine-routing" / str(uuid.uuid4())
    folder.mkdir(parents=True, mode=0o700)
    plan = {
        "schema_version": 1,
        "max_model_calls": 3,
        "max_retries": 0,
        "max_paid_calls": 0,
        "phases": ["qwen3-4b", "qwen3-8b", "measured-routing-validation"],
        "models": MODELS,
        "source_observed_at_ms": int(time.time() * 1000),
        "task": "Who was US president in 1976? Return only the full name.",
        "source_text": "Gerald Ford held the US presidency from August 9, 1974 until January 20, 1977, following election loss to Jimmy Carter.",
        "accepted_answer_forms": ["gerald ford", "gerald r. ford", "gerald r ford"],
        "resident_budget_bytes": 9 * 1024**3,
        "per_phase_calls": 1,
        "per_phase_generation_tokens": 1024,
        "per_phase_deadline_seconds": 120,
        "harness_sha256": file_sha(ROOT / "target/debug/gents-harness"),
        "probe_sha256": file_sha(Path(__file__)),
        "lifecycle_sha256": file_sha(ROOT / "scripts/local_trial.py"),
        "production_profile_admission": False,
        "selection": "Only after both native literal-oracle trials pass, rank their recorded execute-through-status milliseconds; default 4B breaks ties. Validate the existing selector once in a new company. No retries.",
    }
    frozen = (json.dumps(plan, sort_keys=True, indent=2) + "\n").encode()
    (folder / "criteria.json").write_bytes(frozen)
    (folder / "probe-source.py").write_bytes(Path(__file__).read_bytes())
    record = {
        "schema_version": 1,
        "passed": False,
        "criteria_sha256": hashlib.sha256(frozen).hexdigest(),
        "phase_reports": [],
        "generative_model_calls": None,
        "paid_calls": None,
        "actual_provider_cost_usd": None,
        "estimated_total_cost_usd": None,
        "production_profile_admission": False,
    }
    began = time.monotonic()
    try:
        measured = {}
        for key in plan["phases"][:2]:
            phase = folder / key
            result = main(
                key,
                observed_at=plan["source_observed_at_ms"],
                resident_budget=plan["resident_budget_bytes"],
                output_dir=phase,
            )
            descriptor = json.loads((phase / "config.json").read_bytes())[
                "task_workflows"
            ]["models"]["trial"]
            descriptor["measured_total_latency_ms"] = int(
                result["task_elapsed_ms"] + 0.5
            )
            descriptor["evaluation_reference"] = (
                f"Owner frozen literal trial only: {phase / 'report.json'} sha256:{file_sha(phase / 'report.json')}; not general profile qualification"
            )
            measured[key] = descriptor
            record["phase_reports"].append(str(phase / "report.json"))
        expected = min(
            measured,
            key=lambda key: (
                measured[key]["measured_total_latency_ms"],
                key != "qwen3-4b",
                measured[key]["memory_estimate"]["weights_bytes"],
            ),
        )
        policy = {
            "models": measured,
            "default_model": "qwen3-4b",
            "expected_selected": expected,
            "scope": "This question, source and fixed limits only; no statistical optimum or production profile qualification",
        }
        (folder / "measured-policy.json").write_text(
            json.dumps(policy, indent=2) + "\n"
        )
        phase = folder / "measured-routing-validation"
        result = main(
            expected,
            workers=measured,
            default_worker="qwen3-4b",
            observed_at=plan["source_observed_at_ms"],
            resident_budget=plan["resident_budget_bytes"],
            output_dir=phase,
        )
        record["phase_reports"].append(str(phase / "report.json"))
        criteria = [
            json.loads(Path(p).with_name("criteria.json").read_bytes())
            for p in record["phase_reports"]
        ]
        for key in (
            "input",
            "source",
            "accepted_answer_forms",
            "max_calls",
            "max_retries",
            "max_generation_tokens",
            "deadline_seconds",
            "harness_sha256",
        ):
            assert all(c[key] == criteria[0][key] for c in criteria), key
        assert criteria[0]["input"]["original_ask"] == plan["task"]
        assert criteria[0]["source"]["text"] == plan["source_text"]
        assert all(
            c["probe_sha256"] == plan["probe_sha256"]
            and c["lifecycle_sha256"] == plan["lifecycle_sha256"]
            for c in criteria
        )
        assert result["selected_model"] == expected
        assert result["rag_result"]["plan"]["nodes"][0]["provider_model"] == expected
        assert all(c["max_calls"] == 1 for c in criteria)
        assert (folder / "criteria.json").read_bytes() == frozen
        record.update(
            passed=True,
            expected_selected=expected,
            actual_selected=result["selected_model"],
            measured_latency_ms={
                k: v["measured_total_latency_ms"] for k, v in measured.items()
            },
            generative_model_calls=3,
            paid_calls=0,
            actual_provider_cost_usd=0,
        )
    except Exception as error:
        record["failure_type"] = type(error).__name__
        raise
    finally:
        record["elapsed_seconds"] = time.monotonic() - began
        # Include partial/failed phase records without claiming known usage.
        record["retained_reports"] = [str(p) for p in folder.glob("*/report.json")]
        (folder / "report.json").write_text(json.dumps(record, indent=2) + "\n")
        print(
            json.dumps(
                {
                    "comparison_report": str(folder / "report.json"),
                    "passed": record["passed"],
                }
            ),
            flush=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group()
    choice.add_argument("--model", choices=MODELS, default="qwen3-4b")
    choice.add_argument(
        "--compare",
        action="store_true",
        help="Opt in to at most three local calls; frozen comparison, no promotion",
    )
    args = parser.parse_args()
    compare() if args.compare else main(args.model)

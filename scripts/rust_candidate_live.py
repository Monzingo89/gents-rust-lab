"""One installed local author call, existing governed custody and frozen Rust acceptance.

Opt in explicitly; run under local_evaluate.run_check for owned process-group bounds.
No model download, retry, production qualification or source promotion.
"""

import argparse
import hashlib
import json
import time
import uuid
from pathlib import Path

import local_sqlite as sqlite3
from experiment_loop import Controller
from local_trial import local_trial

ROOT = Path(__file__).resolve().parents[1]
MODELS = {
    "qwen3-4b": (
        "models/qwen3-4b-instruct/Qwen3-4B-Q4_K_M.gguf",
        "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
    ),
    "qwen3-8b": (
        "models/qwen3-8b/Qwen3-8B-Q4_K_M.gguf",
        "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785",
    ),
    "qwen3.8-27b": (
        "models/qwen3.8-27b-planner-judge/Qwen3.8-27B-UD-Q4_K_M.gguf",
        "322e194ff79741c7baa497c240f677f54b201b0efab44ca8e50f122b39123482",
    ),
}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", choices=MODELS, required=True)
    parser.add_argument(
        "--prompt-profile",
        choices=("full", "focused"),
        default="full",
        help="Existing owner-frozen candidate context mode; default preserves the original example",
    )
    args = parser.parse_args()
    model_relative, model_sha = MODELS[args.model]
    model_path = ROOT / model_relative
    if model_path.is_symlink() or digest(model_path) != model_sha:
        raise ValueError("Installed trial model pin mismatch; no download attempted")
    identifier = "rust-live-" + uuid.uuid4().hex[:20]
    output = ROOT / "data/local-evaluation" / identifier
    output.mkdir(parents=True, mode=0o700)
    inputs = ROOT / "data/experiment-inputs" / identifier
    inputs.mkdir(parents=True, mode=0o700)
    tests = ROOT / "scripts/fixtures/chunk-boundary-acceptance.rs"
    # Existing native adapter owns the candidate module; remove only the
    # standalone fixture's include, exactly as docs/145 specifies.
    wrapper = 'mod candidate {include!(env!("GENTS_CANDIDATE_SOURCE"));}\n'
    assert tests.read_text().count(wrapper) == 1
    suite = tests.read_text().replace(wrapper, "")
    (inputs / "acceptance.rs").write_text(suite)
    signal = inputs / "evidence.json"
    save(signal, {"owner_evaluation": "frozen; not yet dispatched"})
    plan = json.loads((ROOT / "config/experiment-loop.rust-example.json").read_bytes())
    plan["company"] = identifier
    plan["evidence_paths"] = [str(signal.relative_to(ROOT))]
    if args.prompt_profile == "focused":
        plan["experiments"][0]["candidate_workflow"]["prompt_profile"] = "focused"
    spec = plan["experiments"][0]["evaluation"]
    assert digest(inputs / "acceptance.rs") == spec["tests_sha256"], (
        "Original acceptance changed"
    )
    spec["tests_path"] = str((inputs / "acceptance.rs").relative_to(ROOT))
    model = {
        "provider": "local",
        "model": args.model + " bounded Rust candidate trial",
        "path": model_relative,
        "context_tokens": 8192,
        "local_reasoning_tokens": 512,
        "local_final_tokens": 2048,
    }
    config = {
        "company_mode": True,
        "company_tools": True,
        "admin_judge": model,
        "agents": [
            {
                "id": "Engineer Gent",
                "assignment": "frozen quarantined author",
                "planner": model,
                "worker": model,
            }
        ],
        "max_steps": 1,
        "max_tokens": 2048,
        "max_run_seconds": 180,
    }
    save(output / "config.json", config)
    criteria = {
        "plan": plan,
        "config": config,
        "model_sha256": model_sha,
        "harness_sha256": digest(ROOT / "target/debug/gents-harness"),
        "probe_sha256": digest(Path(__file__)),
        "acceptance_fixture_sha256": digest(tests),
        "example_plan_sha256": digest(
            ROOT / "config/experiment-loop.rust-example.json"
        ),
        "deadline_seconds": 210,
        "production_profile_admission": False,
    }
    (output / "probe-source.py").write_bytes(Path(__file__).read_bytes())
    save(output / "criteria.json", criteria)
    criteria_sha = digest(output / "criteria.json")
    report = {
        "schema_version": 1,
        "criteria_sha256": criteria_sha,
        "model_sha256": model_sha,
        "model_path": model_relative,
        "prompt_profile": args.prompt_profile,
        "verified_task_completion": False,
        "generative_model_calls": None,
        "paid_calls": None,
        "actual_provider_cost_usd": None,
        "estimated_total_cost_usd": None,
        "cache_hit_ratio": None,
        "promotion_authorized": False,
        "production_profile_admission": False,
        "scope": "One pure Rust task against the unchanged frozen owner suite",
    }
    began = time.monotonic()
    trial = None
    try:
        with local_trial(ROOT, output / "config.json", output) as trial:
            api = trial.api
            controller = Controller(ROOT, identifier, api)
            api(
                "/api/companies/" + identifier,
                {"op": "register", "name": "Isolated native Rust trial"},
            )
            report["freeze"] = controller.freeze(plan)
            # Explicit host signal after freezing, not model-selected scheduling.
            save(signal, {"owner_evaluation": "dispatch the frozen one-call trial"})
            dispatched = controller.poll()
            report["dispatch"] = dispatched
            assert dispatched["status"] == "dispatched", dispatched
            run = dispatched["run_id"]
            started = time.monotonic()
            deadline = started + 210
            while True:
                observed = controller.poll()
                report["observed"] = observed
                if observed["status"] == "result":
                    break
                if observed["status"] != "observing" or time.monotonic() > deadline:
                    api(f"/api/runs/{run}/cancel", {})
                    raise AssertionError(
                        "Frozen trial did not reach a confirmed result; no replay"
                    )
                time.sleep(0.2)
            report["task_elapsed_seconds"] = time.monotonic() - started
            report["controller"] = controller.status()
            snapshot = json.loads(
                (controller.folder / run / "observed-run.json").read_bytes()
            )
            calls = [
                event for event in snapshot["events"] if event.get("reply") is not None
            ]
            report["call_receipts"] = calls
            with sqlite3.connect(
                ROOT / "data/companies" / identifier / "brain.sqlite3"
            ) as db:
                reservations = db.execute(
                    "SELECT COUNT(*) FROM inference_attempt_reservations"
                ).fetchone()[0]
            report["inference_attempt_reservations"] = reservations
            native = len(calls) == 1 and isinstance(
                calls[0]["reply"].get("local_execution"), dict
            )
            if native:
                reply = calls[0]["reply"]
                local = reply["local_execution"]
                report.update(
                    generative_model_calls=1,
                    paid_calls=0,
                    actual_provider_cost_usd=reply["usage"].get("cost_usd"),
                    input_tokens=reply["usage"].get("input"),
                    output_tokens=reply["usage"].get("output"),
                    observed_context_tokens=local.get("observed_context_tokens"),
                    resident_memory_bytes=None
                    if local.get("resident_kib") is None
                    else local["resident_kib"] * 1024,
                    memory_scope="Backend RSS at reply, not peak or GPU allocation",
                )
            assert (
                digest(output / "criteria.json") == criteria_sha
                and digest(model_path) == model_sha
            )
            report["verified_task_completion"] = (
                native
                and reservations == 1
                and observed["result"]["independently_accepted"] is True
                and report["actual_provider_cost_usd"] == 0
            )
    except Exception as error:
        report["error_type"] = type(error).__name__
        raise
    finally:
        report["harness_exit_code"] = None if trial is None else trial.child.returncode
        report["shutdown_forced"] = None if trial is None else trial.shutdown_forced
        report["startup_id"] = None if trial is None else trial.startup_id
        report["elapsed_seconds"] = time.monotonic() - began
        report["experiment_ledger"] = "data/experiment-loops/" + identifier
        save(output / "report.json", report)
        print(
            json.dumps(
                {
                    "report": str(output / "report.json"),
                    "verified_task_completion": report["verified_task_completion"],
                }
            ),
            flush=True,
        )
    return 0 if report["verified_task_completion"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

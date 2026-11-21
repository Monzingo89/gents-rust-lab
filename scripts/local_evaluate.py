"""Bounded offline integration baseline; no credentials, downloads or live models."""

import hashlib
import json
import os
import signal
import subprocess
import sys
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# Existing independent checks retain their assertions and acceptance boundaries.
CHECKS = [
    ("build", ["cargo", "build", "--workspace", "--offline", "--locked"], 180),
    ("lifecycle", ["node", "--test", "scripts/company-lifecycle.test.mjs"], 30),
    (
        "probe-lifecycle",
        [
            sys.executable,
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_local_trial.py",
        ],
        30,
    ),
    ("rust-lifecycle", ["node", "scripts/company-lifecycle-smoke.mjs"], 30),
    ("frontdoor", [sys.executable, "scripts/task_frontdoor_smoke.py"], 45),
    ("grounded", [sys.executable, "scripts/task_workflows_smoke.py", "--grounded"], 45),
    ("routing", [sys.executable, "scripts/task_workflows_smoke.py", "--routing"], 45),
    ("workflows", [sys.executable, "scripts/task_workflows_smoke.py"], 120),
    ("native-stdio", [sys.executable, "scripts/task_native_smoke.py"], 120),
    ("native-sse", [sys.executable, "scripts/task_native_smoke.py", "--sse"], 120),
    (
        "screens",
        [
            "node",
            "--test",
            "scripts/office-activity.test.mjs",
            "scripts/task-activity.test.mjs",
            "scripts/room-activity.test.mjs",
            "scripts/ceo-execution.test.mjs",
        ],
        30,
    ),
]


def run_check(name, command, timeout, folder, env):
    started = time.perf_counter()
    timed_out = False
    log_path = folder / f"{name}.log"
    with log_path.open("wb") as log:
        try:
            child = subprocess.Popen(
                command,
                cwd=ROOT,
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        except OSError as error:
            log.write(f"Could not start {command[0]}: {error}\n".encode())
            return {
                "name": name,
                "command": command,
                "passed": False,
                "exit_code": None,
                "timed_out": False,
                "log": log_path.name,
            }
        try:
            code = child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            code = None
        finally:
            # Smokes own subprocess trees. Kill the group even if its leader
            # exited abnormally; never leave an orphan harness/MCP behind.
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
    raw = log_path.read_bytes()
    observations = []
    for line in raw.decode(errors="replace").splitlines():
        try:
            value = json.loads(line)
            if isinstance(value, dict):
                observations.append(value)
        except ValueError:
            pass
    return {
        "name": name,
        "command": command,
        "passed": code == 0 and not timed_out,
        "exit_code": code,
        "timed_out": timed_out,
        "elapsed_ms": round((time.perf_counter() - started) * 1000, 3),
        "log": log_path.name,
        "log_sha256": hashlib.sha256(raw).hexdigest(),
        "observations": observations,
    }


def main():
    if sys.argv[1:]:
        raise SystemExit(
            "Usage: python3 scripts/local_evaluate.py (fixed offline fixture suite)"
        )
    if os.name != "posix":
        raise SystemExit("This process-group baseline currently requires macOS/Linux")
    folder = ROOT / "data/local-evaluation" / str(uuid.uuid4())
    folder.mkdir(parents=True, mode=0o700)
    env = {
        k: v
        for k, v in os.environ.items()
        if k
        in {
            "PATH",
            "HOME",
            "USER",
            "TMPDIR",
            "LANG",
            "SHELL",
            "RUSTUP_HOME",
            "CARGO_HOME",
        }
    }
    env.update(
        GENTS_LOCAL_ONLY="1",
        GENTS_OFFICE_ACTIVITY="0",
        GENTS_DISABLE_SCHEDULER="1",
        CARGO_NET_OFFLINE="true",
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
    )
    report = {
        "schema_version": 1,
        "scope": "synthetic integration fixtures; not real-model task qualification",
        "head": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "git_status": subprocess.check_output(
            ["git", "status", "--short"], cwd=ROOT, text=True
        ),
        "measurements": {
            "verified_real_tasks": None,
            "generative_model_calls": 0,
            "paid_calls": 0,
            "actual_provider_cost_usd": 0,
            "estimated_total_cost_usd": None,
            "resident_bytes": None,
            "model_context_tokens": None,
            "cache_hit_ratio": None,
        },
        "checks": [],
    }
    started = time.perf_counter()
    try:
        for name, command, timeout in CHECKS:
            print(f"Checking {name}...", flush=True)
            result = run_check(name, command, timeout, folder, env)
            report["checks"].append(result)
            if not result["passed"]:
                break
    finally:
        binary = ROOT / "target/debug/gents-harness"
        if binary.is_file():
            with binary.open("rb") as stream:
                report["harness_sha256"] = hashlib.file_digest(
                    stream, "sha256"
                ).hexdigest()
        report["elapsed_ms"] = round((time.perf_counter() - started) * 1000, 3)
        report["passed"] = len(report["checks"]) == len(CHECKS) and all(
            r["passed"] for r in report["checks"]
        )
        # Do not claim zero runtime usage when the expected suite was not run.
        if not report["passed"]:
            for key in (
                "generative_model_calls",
                "paid_calls",
                "actual_provider_cost_usd",
            ):
                report["measurements"][key] = None
        target = folder / "report.json"
        target.write_text(json.dumps(report, indent=2) + "\n")
        print(target.relative_to(ROOT), flush=True)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

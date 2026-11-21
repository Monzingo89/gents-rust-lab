"""Probe lifecycle safety; real Rust control-plane cases perform no inference."""
import json
import importlib
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

import local_sqlite as sqlite3
from local_trial import local_trial, stop_group, wait_ready

ROOT = Path(__file__).resolve().parents[1]


class ImportContractTests(unittest.TestCase):
    def test_existing_live_probe_imports_do_not_start_processes(self):
        with patch("subprocess.Popen", side_effect=AssertionError("Import started a process")):
            for name in ("task_intake_live", "task_mlx_live", "task_workspace_live"):
                with self.subTest(module=name):
                    module = importlib.import_module(name)
                    self.assertTrue(callable(module.port))
                    self.assertEqual(module.ROOT, ROOT)

    def test_routing_comparison_freezes_before_dispatch_and_stops_on_failure(self):
        import task_engine_live as live
        with tempfile.TemporaryDirectory(prefix="gents-routing-budget-") as temporary:
            root = Path(temporary)
            binary = root / "target/debug/gents-harness"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"not executed")
            helper = root / "scripts/local_trial.py"
            helper.parent.mkdir()
            helper.write_bytes(b"not executed")
            def fail_before_generation(*args, **kwargs):
                plan = json.loads((kwargs["output_dir"].parent / "criteria.json").read_bytes())
                self.assertEqual(plan["max_model_calls"], 3)
                self.assertEqual(plan["max_retries"], 0)
                self.assertEqual(plan["max_paid_calls"], 0)
                raise RuntimeError("injected first-phase failure; no model called")
            with patch.object(live, "ROOT", root), patch.object(live, "main", side_effect=fail_before_generation) as run:
                with self.assertRaises(RuntimeError):
                    live.compare()
                self.assertEqual(run.call_count, 1)
            report = json.loads(next((root / "data/task-engine-routing").glob("*/report.json")).read_bytes())
            self.assertFalse(report["passed"])
            self.assertIsNone(report["generative_model_calls"])
            self.assertEqual(report["phase_reports"], [])


@unittest.skipUnless(os.name == "posix", "owned readiness deadline uses POSIX timers")
class ReadinessTests(unittest.TestCase):
    def test_deadline_interrupts_a_probe_that_ignores_its_request_timeout(self):
        child = Mock()
        child.poll.return_value = None
        previous = signal.getsignal(signal.SIGALRM)
        def stalled(*args, **kwargs):
            time.sleep(5)
        started = time.monotonic()
        with self.assertRaises(TimeoutError):
            wait_ready(child, stalled, "owned", .05)
        self.assertLess(time.monotonic() - started, .5)
        self.assertIs(signal.getsignal(signal.SIGALRM), previous)
        self.assertEqual(signal.getitimer(signal.ITIMER_REAL), (0.0, 0.0))

    def test_process_exit_cannot_be_hidden_by_a_successful_response(self):
        config = {"startup_id": "owned", "auth": "local", "local_only": True,
                  "company_mode": True, "company_tools": True}
        child = Mock()
        child.poll.side_effect = [None, 7]
        with self.assertRaisesRegex(RuntimeError, "exited"):
            wait_ready(child, Mock(return_value=config), "owned", .2)

    def test_unavailable_service_has_a_bounded_deadline(self):
        child = Mock()
        child.poll.return_value = None
        started = time.monotonic()
        with self.assertRaises(TimeoutError):
            wait_ready(child, Mock(side_effect=ConnectionRefusedError), "owned", .05)
        self.assertLess(time.monotonic() - started, .5)

    def test_foreign_listener_is_rejected_before_any_write(self):
        api = Mock(return_value={"startup_id": "incumbent", "auth": "local",
                                 "local_only": True, "company_mode": True, "company_tools": True})
        child = Mock()
        child.poll.return_value = None
        with self.assertRaisesRegex(ValueError, "instance"):
            wait_ready(child, api, "owned-instance", .2)
        self.assertEqual(api.call_args.args, ("/api/config",))
        self.assertLessEqual(api.call_args.kwargs["timeout"], .2)

    def test_matching_nonce_does_not_override_local_policy(self):
        for field, invalid in [("auth", "none"), ("local_only", False),
                               ("company_mode", False), ("company_tools", False)]:
            config = {"startup_id": "owned", "auth": "local", "local_only": True,
                      "company_mode": True, "company_tools": True, field: invalid}
            child = Mock()
            child.poll.return_value = None
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "local"):
                wait_ready(child, Mock(return_value=config), "owned", .2)


@unittest.skipUnless(os.name == "posix", "owned process groups require POSIX")
class HarnessTests(unittest.TestCase):
    def test_group_permission_error_requires_independent_liveness_evidence(self):
        child = Mock(pid=12345)
        with patch("local_trial.os.killpg", side_effect=[None, PermissionError(), PermissionError()]), \
             patch("local_trial.active_group", side_effect=[True, False]) as observed:
            self.assertFalse(stop_group(child))
        self.assertEqual(observed.call_count, 2)
        child.wait.assert_called_once()

    def test_a_live_group_permission_denial_cannot_pass_cleanup(self):
        child = Mock(pid=12345)
        with patch("local_trial.os.killpg", side_effect=[None, PermissionError(), PermissionError()]), \
             patch("local_trial.active_group", return_value=True), \
             patch("local_trial.time.monotonic", side_effect=[0, 3]):
            with self.assertRaises(PermissionError):
                stop_group(child)
        child.wait.assert_not_called()

    def config(self, folder):
        model = {"provider": "mock", "model": "no-inference-lifecycle-fixture"}
        path = folder / "config.json"
        path.write_text(json.dumps({"company_mode": True, "company_tools": True, "admin_judge": model,
            "agents": [{"id": "Fixture", "assignment": "no dispatch", "planner": model, "worker": model}],
            "max_steps": 1, "max_tokens": 128, "max_run_seconds": 30}))
        return path

    def test_actual_rust_readiness_and_shutdown_preserve_the_incumbent(self):
        with tempfile.TemporaryDirectory(prefix="gents-trial-lifecycle-") as temporary:
            folder = Path(temporary)
            first, second = folder / "first", folder / "second"
            first.mkdir(); second.mkdir()
            previous = signal.getsignal(signal.SIGTERM)
            with local_trial(ROOT, self.config(first), first) as incumbent:
                config = incumbent.api("/api/config")
                self.assertEqual(config["startup_id"], incumbent.startup_id)
                port = int(incumbent.api.server.rsplit(":", 1)[1])
                with self.assertRaises((ValueError, RuntimeError)):
                    with local_trial(ROOT, self.config(second), second, http_port=port):
                        self.fail("occupied address was accepted")
                self.assertIsNone(incumbent.child.poll())
                self.assertEqual(incumbent.api("/api/config")["startup_id"], incumbent.startup_id)
            self.assertEqual(incumbent.child.returncode, 0)
            self.assertFalse(incumbent.shutdown_forced)
            self.assertIs(signal.getsignal(signal.SIGTERM), previous)
            record = json.loads((first / "lifecycle.json").read_bytes())
            rejected = json.loads((second / "lifecycle.json").read_bytes())
            self.assertTrue(record["control_plane_ready"])
            self.assertFalse(rejected["control_plane_ready"])
            self.assertNotEqual(record["startup_id"], rejected["startup_id"])
            with sqlite3.connect(first / "runs.sqlite3") as db:
                self.assertEqual(db.execute("SELECT COUNT(*) FROM runs").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT COUNT(*) FROM run_events").fetchone()[0], 0)

    def test_sigterm_of_the_probe_cleans_up_its_detached_harness(self):
        with tempfile.TemporaryDirectory(prefix="gents-trial-term-") as temporary:
            folder = Path(temporary)
            config = self.config(folder)
            marker = folder / "owned-pid"
            code = """import sys,time
from pathlib import Path
sys.path.insert(0,sys.argv[1]+'/scripts')
from local_trial import local_trial
with local_trial(sys.argv[1],sys.argv[2],sys.argv[3]) as trial:
    Path(sys.argv[4]).write_text(str(trial.child.pid))
    time.sleep(60)
"""
            child = subprocess.Popen([sys.executable, "-c", code, str(ROOT), str(config),
                                      str(folder), str(marker)], start_new_session=True)
            try:
                deadline = time.monotonic() + 20
                while not marker.exists():
                    self.assertIsNone(child.poll())
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.025)
                pid = int(marker.read_text())
                child.terminate()
                self.assertEqual(child.wait(timeout=5), 128 + signal.SIGTERM)
                with self.assertRaises(ProcessLookupError):
                    os.kill(pid, 0)
                record = json.loads((folder / "lifecycle.json").read_bytes())
                self.assertEqual(record["failure_type"], "SystemExit")
                self.assertEqual(record["child_exit_code"], 0)
            finally:
                if child.poll() is None:
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill(); child.wait(timeout=2)

    def test_ignored_term_descendant_is_killed_after_leader_exit(self):
        with tempfile.TemporaryDirectory(prefix="gents-trial-descendant-") as temporary:
            marker = Path(temporary) / "leaf-pid"
            leaf = ("import os,signal,time; from pathlib import Path; "
                    "signal.signal(signal.SIGTERM,signal.SIG_IGN); "
                    f"Path({str(marker)!r}).write_text(str(os.getpid())); time.sleep(30)")
            leader = f"import subprocess,sys,time; subprocess.Popen([sys.executable,'-c',{leaf!r}]); time.sleep(30)"
            child = subprocess.Popen([sys.executable, "-c", leader], start_new_session=True)
            try:
                deadline = time.monotonic() + 5
                while not marker.exists():
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.025)
                pid = int(marker.read_text())
                self.assertTrue(stop_group(child))
                self.assertIsNotNone(child.returncode)
                # An orphan can briefly remain a zombie until the OS reaps it;
                # it must no longer be executable/running after group cleanup.
                observed = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)],
                                          capture_output=True, text=True, check=False)
                self.assertIn(observed.returncode, (0, 1))
                self.assertTrue(not observed.stdout.strip() or observed.stdout.lstrip().startswith("Z"))
            finally:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=2)


if __name__ == "__main__":
    unittest.main()

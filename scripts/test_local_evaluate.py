"""Failure/timeout reporting for the offline evaluator, without a live company."""

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "local_evaluate", Path(__file__).with_name("local_evaluate.py")
)
assert spec is not None and spec.loader is not None
evaluate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluate)


@unittest.skipUnless(os.name == "posix", "process-group runner supports macOS/Linux")
class EvaluationTests(unittest.TestCase):
    def test_nonzero_exit_cannot_be_overridden_by_successful_json(self):
        with tempfile.TemporaryDirectory() as folder:
            result = evaluate.run_check(
                "failure",
                [
                    sys.executable,
                    "-c",
                    "print('{\"passed\": true}'); raise SystemExit(7)",
                ],
                3,
                Path(folder),
                os.environ.copy(),
            )
            self.assertFalse(result["passed"])
            self.assertEqual(result["exit_code"], 7)
            self.assertEqual(result["observations"], [{"passed": True}])
            self.assertTrue((Path(folder) / result["log"]).is_file())

    def test_timeout_terminates_owned_process_and_cannot_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            result = evaluate.run_check(
                "timeout",
                [
                    sys.executable,
                    "-c",
                    'import time; print("started", flush=True); time.sleep(20)',
                ],
                0.1,
                Path(folder),
                os.environ.copy(),
            )
            self.assertFalse(result["passed"])
            self.assertTrue(result["timed_out"])
            self.assertIsNone(result["exit_code"])
            self.assertLess(result["elapsed_ms"], 6000)


if __name__ == "__main__":
    unittest.main()

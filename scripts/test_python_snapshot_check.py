import hashlib
import socket
import sys
import tempfile
import unittest
from pathlib import Path

import python_snapshot_check as check


class SnapshotInputs(unittest.TestCase):
    def test_inputs_are_explicit_bounded_and_content_addressed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "f.py").write_text("value = 1")
            (root / "other.py").write_text("not selected")
            first = check.snapshot(root, ["f.py"])
            self.assertEqual(list(first), ["f.py"])
            (root / "f.py").write_text("value = 2")
            self.assertNotEqual(
                check.manifest(first), check.manifest(check.snapshot(root, ["f.py"]))
            )
            for paths in (
                [],
                ["../f.py"],
                ["/f.py"],
                [".f.py"],
                ["f.py", "f.py"],
                ["a/../f.py"],
                ["a//f.py"],
                ["other.txt"],
            ):
                with self.assertRaises((ValueError, OSError)):
                    check.snapshot(root, paths)
            (root / "alias.py").symlink_to(root / "f.py")
            with self.assertRaises((ValueError, OSError)):
                check.snapshot(root, ["alias.py"])
            import os

            os.link(root / "f.py", root / "link.py")
            with self.assertRaises(ValueError):
                check.snapshot(root, ["link.py"])
            (root / "large.py").write_bytes(b"x" * (check.MAX_BYTES + 1))
            with self.assertRaises(ValueError):
                check.snapshot(root, ["large.py"])


@unittest.skipUnless(sys.platform == "darwin", "macOS Seatbelt experiment")
class IsolatedChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / "candidate.py"
        self.suite = self.root / "test_candidate.py"

    def tearDown(self):
        self.temp.cleanup()

    def run_suite(self, source, tests, **options):
        self.source.write_text(source)
        self.suite.write_text(tests)
        return check.verify(
            self.root,
            ["candidate.py", "test_candidate.py"],
            "test_candidate.py",
            hashlib.sha256(self.suite.read_bytes()).hexdigest(),
            **options,
        )

    def test_same_suite_retests_changed_source_without_touching_originals(self):
        tests = (
            "import unittest,candidate\nclass T(unittest.TestCase):\n"
            " def test_value(self): self.assertEqual(candidate.value, 2)\n"
        )
        first = self.run_suite("value = 1", tests)
        second = self.run_suite("value = 2", tests)
        third = self.run_suite("value = 2", tests)
        self.assertEqual(first["status"], "reported_fail", first)
        self.assertEqual(second["status"], "reported_pass", second)
        self.assertEqual(third["status"], "reported_pass", third)
        self.assertNotEqual(first["source_identity"], second["source_identity"])
        self.assertEqual(second["source_identity"], third["source_identity"])
        self.assertEqual(first["suite_sha256"], second["suite_sha256"])
        self.assertTrue(second["snapshot_unchanged"])
        self.assertFalse(second["qualified"])
        self.assertFalse(second["authority_granted"])
        self.assertEqual(second["model_calls"], 0)
        self.assertEqual(self.source.read_text(), "value = 2")
        self.assertFalse((self.root / "__pycache__").exists())

    def test_network_host_reads_writes_source_mutation_and_subprocesses_denied(self):
        outside = self.root / "outside.txt"
        outside.write_text("untouched")
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
            tests = f"""import unittest,pathlib,socket,subprocess
class T(unittest.TestCase):
 def test_denials(self):
  with self.assertRaises(OSError): pathlib.Path({str(outside)!r}).read_text()
  with self.assertRaises(OSError): pathlib.Path({str(outside)!r}).write_text('changed')
  with self.assertRaises(OSError): pathlib.Path('candidate.py').write_text('changed')
  with self.assertRaises(OSError): socket.create_connection(('127.0.0.1',{port}), timeout=.2)
  with self.assertRaises(OSError): subprocess.run(['/bin/echo','forbidden'],check=True)
"""
            result = self.run_suite("value = 2", tests)
        self.assertEqual(result["status"], "reported_pass", result)
        self.assertEqual(outside.read_text(), "untouched")

    def test_timeout_empty_suite_import_failure_and_excess_output_cannot_pass(self):
        for source, tests, options, reason in (
            (
                "while True: pass",
                "import candidate",
                {"timeout_seconds": 0.3},
                "timed_out",
            ),
            ("value=2", "", {}, None),
            ("import missing_package", "import candidate", {}, None),
            ('print("x" * 100000)', "import candidate", {}, "output_truncated"),
            (
                "x=bytearray(300*1024*1024)\nimport time\ntime.sleep(2)",
                "import candidate",
                {},
                "resident_memory_limit",
            ),
        ):
            result = self.run_suite(source, tests, **options)
            self.assertNotEqual(result["status"], "reported_pass", result)
            if reason:
                self.assertTrue(result[reason], result)
                self.assertEqual(result["status"], "indeterminate", result)

    def test_test_code_can_spoof_exit_and_is_never_independent_acceptance(self):
        result = self.run_suite("import os\nos._exit(0)", "import candidate")
        self.assertEqual(result["status"], "reported_pass", result)
        self.assertFalse(result["qualified"])
        self.assertFalse(result["promoted"])

    def test_owner_scratch_allows_real_atomic_io_but_not_source_or_host_writes(self):
        outside = self.root / "outside.txt"
        outside.write_text("untouched")
        tests = f"""import os,pathlib,tempfile,unittest
class T(unittest.TestCase):
 def test_io(self):
  with tempfile.TemporaryDirectory() as d:
   p=pathlib.Path(d); (p/'a').write_text('verified'); os.replace(p/'a',p/'b')
   self.assertEqual((p/'b').read_text(),'verified')
   with self.assertRaises(OSError): pathlib.Path('candidate.py').write_text('changed')
   with self.assertRaises(OSError): pathlib.Path({str(outside)!r}).write_text('changed')
   (p/'alias').symlink_to({str(outside)!r})
   with self.assertRaises(OSError): (p/'alias').read_text()
   with self.assertRaises(OSError): os.link({str(outside)!r},p/'host-link')
   try: os.link('candidate.py',p/'source-link')
   except OSError: pass
   else:
    with self.assertRaises(OSError): os.chmod(p/'source-link',0o600)
    with self.assertRaises(OSError): (p/'source-link').write_text('changed')
"""
        result = self.run_suite("value=2", tests, scratch_bytes=1_048_576)
        self.assertEqual(result["status"], "reported_pass", result)
        self.assertEqual(result["scratch_limit_bytes"], 1_048_576)
        self.assertTrue(result["snapshot_unchanged"])
        self.assertEqual(outside.read_text(), "untouched")
        result = self.run_suite("value=2", tests)
        self.assertEqual(result["status"], "reported_fail", result)

    def test_scratch_byte_and_entry_limits_cannot_report_pass(self):
        for source, budget in [
            (
                "import os,pathlib,time\np=pathlib.Path(os.environ['TMPDIR'])\nfor i in range(4): (p/str(i)).write_bytes(b'x'*10000)\ntime.sleep(1)",
                20000,
            ),
            (
                "import os,pathlib,time\np=pathlib.Path(os.environ['TMPDIR'])\nfor i in range(300): (p/str(i)).touch()\ntime.sleep(1)",
                1_048_576,
            ),
        ]:
            result = self.run_suite(source, "import candidate", scratch_bytes=budget)
            self.assertEqual(result["status"], "indeterminate", result)
            self.assertTrue(result["scratch_limit_exceeded"], result)

    def test_suite_hash_and_invalid_limits_fail_before_execution(self):
        self.source.write_text("value=2")
        self.suite.write_text('raise RuntimeError("must not execute")')
        for options in (
            {},
            {"timeout_seconds": float("nan")},
            {"timeout_seconds": True},
            {"timeout_seconds": 11},
        ):
            with self.assertRaises(ValueError):
                check.verify(
                    self.root,
                    ["candidate.py", "test_candidate.py"],
                    "test_candidate.py",
                    "0" * 64,
                    **options,
                )


if __name__ == "__main__":
    unittest.main()

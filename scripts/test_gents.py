import argparse
import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

import gents


class IntakeTaskInput(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="gents-intake-test-")
        self.root = Path(self.temporary.name)
        self.task_file = self.root / "task.txt"
        self.task = (
            "Audit all 12 contractors.\n\nPreserve evidence and report unknowns."
        )
        self.task_file.write_text(self.task, encoding="utf-8")

    def tearDown(self):
        self.temporary.cleanup()

    def load(self, path):
        with redirect_stdout(io.StringIO()):
            return gents.task_answer(argparse.Namespace(task_file=str(path)))

    def test_file_preserves_multiline_task(self):
        self.assertEqual(self.load(self.task_file), self.task)

    def test_at_path_works_at_delivery_question(self):
        with (
            patch("builtins.input", return_value="@" + str(self.task_file)),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(gents.task_answer(argparse.Namespace()), self.task)

    def test_inline_answer_still_works(self):
        with patch("builtins.input", return_value="A short task"):
            self.assertEqual(gents.task_answer(argparse.Namespace()), "A short task")

    def test_limit_counts_utf8_bytes(self):
        self.task_file.write_text("é" * 5000, encoding="utf-8")
        self.assertEqual(len(self.load(self.task_file)), 5000)
        self.task_file.write_text("é" * 5001, encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "10000 UTF-8 bytes"):
            self.load(self.task_file)

    def test_empty_binary_invalid_and_missing_files_rejected(self):
        for content in (b"   \n", b"a\x00b", b"\xff"):
            self.task_file.write_bytes(content)
            with self.assertRaises(ValueError):
                self.load(self.task_file)
        with self.assertRaisesRegex(ValueError, "not found"):
            self.load(self.root / "missing.txt")

    def test_owner_evaluation_cannot_enter_prompt(self):
        private = self.root / "owner-evaluation"
        private.mkdir()
        target = private / "key.txt"
        target.write_text("private")
        alias = self.root / "alias.txt"
        alias.symlink_to(target)
        for path in (target, alias):
            with self.assertRaisesRegex(ValueError, "Owner evaluation"):
                self.load(path)

    def test_intake_flag_passes_full_task_to_frozen_intake_and_sow(self):
        def fake_company(args, op, **data):
            self.assertEqual(op, "intake")
            self.assertEqual(data["intake"]["idea"], self.task)
            return {
                "name": "Fictional Company",
                "jobs": [{"run_id": data["run_id"], "digest": "test-digest"}],
            }

        answers = [
            "text",
            "Cite evidence",
            "",
            "",
            "",
            "Company simulation",
            "Gents simulation",
            "",
            "",
        ]
        command = [
            "gents.py",
            "--company",
            "we-need-gents-lc",
            "intake",
            "--task-file",
            str(self.task_file),
        ]
        with (
            patch.object(gents, "ROOT", self.root),
            patch.object(gents, "company", side_effect=fake_company) as company,
            patch("sys.argv", command),
            patch("builtins.input", side_effect=answers),
            redirect_stdout(io.StringIO()),
        ):
            gents.main()
        self.assertEqual(company.call_count, 1)
        sow = next(
            (self.root / "data/companies/we-need-gents-lc/runs").glob("*/SOW.md")
        )
        self.assertIn(self.task, sow.read_text())


if __name__ == "__main__":
    unittest.main()

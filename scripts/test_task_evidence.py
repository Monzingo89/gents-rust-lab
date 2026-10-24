import csv
import datetime as dt
import json
from pathlib import Path
import shutil
import tempfile
import unittest
import uuid

import task_evidence as task

REPO = Path(__file__).resolve().parents[1]


class EvidenceBoundaries(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="gents-evidence-test-")
        self.root = Path(self.temporary.name)
        self.package = str(uuid.uuid4())
        self.company = task.COMPANY

    def tearDown(self):
        self.temporary.cleanup()

    def fixture(self):
        directory = task.package_dir(self.root, self.company, self.package)
        directory.mkdir(parents=True)
        for folder, names in (("task", task.TASK_FILES), ("company", task.COMPANY_FILES)):
            for name in names:
                destination = directory / "originals" / folder / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                if Path(name).suffix in (".mp4", ".wav"):
                    destination.write_bytes(b"synthetic unit-test media, not real extraction")
                else:
                    shutil.copyfile(REPO / "docs/companies" / task.PACK / folder / name, destination)
        task.write_json(directory / "ocr.json", [])
        for stem in ("authorized-policy-update", "onboarding-screen-demo"):
            task.write_json(directory / (stem + ".json"), {"segments": [{"start": 0, "end": 1, "text": "Synthetic test transcript"}]})
        task.write_json(directory / "media-review.json", {"reviewed": False, "reviewer": "", "corrections": {}})
        manifest = {"profile": task.PROFILE, "company_id": self.company, "package_id": self.package,
                    "files": {str(p.relative_to(directory)): task.sha(p) for p in directory.rglob("*") if p.is_file() and p.name != "media-review.json"},
                    "status": "review_required", "media_review_sha256": None}
        task.write_json(directory / "manifest.json", manifest)
        return directory

    def approved_fixture(self):
        directory = self.fixture()
        task.review(self.root, self.company, self.package, "Synthetic test operator", "2026-10-01", 7, True)
        return directory, task.sha(directory / "manifest.json")

    def test_tenant_traversal_and_symlink_rejected(self):
        with self.assertRaises(ValueError):
            task.package_dir(self.root, "another-company", self.package)
        with self.assertRaises(ValueError):
            task.confined(self.root, self.root / "../outside")
        with self.assertRaises(ValueError):
            task.confined(self.root, self.root / "owner-evaluation/secret.txt")
        alias = self.root / "alias"
        alias.symlink_to(self.root)
        with self.assertRaises(ValueError):
            task.confined(self.root, alias / "anything")

    def test_unreviewed_media_cannot_execute(self):
        directory = self.fixture()
        with self.assertRaisesRegex(ValueError, "explicitly reviewed"):
            task.execute(self.root, self.company, str(uuid.uuid4()), self.package, task.sha(directory / "manifest.json"))

    def test_review_and_source_changes_invalidate_frozen_evidence(self):
        directory, digest = self.approved_fixture()
        review = directory / "media-review.json"
        review.write_text(review.read_text() + " ")
        with self.assertRaisesRegex(ValueError, "explicitly reviewed"):
            task.execute(self.root, self.company, str(uuid.uuid4()), self.package, digest)
        source = directory / "originals/task/employee-records.csv"
        source.write_text(source.read_text() + "\n")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            task.load(self.root, self.company, self.package)

    def test_full_synthetic_audit_and_idempotent_reuse(self):
        directory, digest = self.approved_fixture()
        run = str(uuid.uuid4())
        summary, output, hit = task.execute(self.root, self.company, run, self.package, digest)
        self.assertFalse(hit)
        self.assertEqual(summary["status"], "verified")
        self.assertEqual(summary["employee_count"], 12)
        self.assertTrue(all(summary["checks"].values()))
        self.assertTrue((output / "report.html").is_file())
        before = task.sha(output / "decisions.csv")
        _, again, hit = task.execute(self.root, self.company, run, self.package, digest)
        self.assertTrue(hit)
        self.assertEqual(again, output)
        self.assertEqual(task.sha(output / "decisions.csv"), before)
        decisions = task.read_csv(output / "decisions.csv", "employee_id")
        # A blank date in this synthetic fixture stays UNKNOWN; no fabricated video correction.
        self.assertEqual(decisions["E005"]["training_decision"], "UNKNOWN")

    def test_verifier_rejects_forged_gate_even_with_updated_hash(self):
        directory, digest = self.approved_fixture()
        _, output, _ = task.execute(self.root, self.company, str(uuid.uuid4()), self.package, digest)
        path = output / "decisions.csv"
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream)
            names, rows = reader.fieldnames, list(reader)
        rows[0]["hr_gate"] = "HOLD" if rows[0]["hr_gate"] == "PASS" else "PASS"
        with path.open("w", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=names)
            writer.writeheader()
            writer.writerows(rows)
        summary = json.loads((output / "run-summary.json").read_text())
        summary["artifact_sha256"]["decisions.csv"] = task.sha(path)
        task.write_json(output / "run-summary.json", summary)
        with self.assertRaisesRegex(ValueError, "Department gate"):
            task.verify(directory, output)

    def test_changed_manifest_cannot_be_bound_to_old_sow(self):
        directory, digest = self.approved_fixture()
        manifest = json.loads((directory / "manifest.json").read_text())
        manifest["new_field"] = "new version"
        task.write_json(directory / "manifest.json", manifest)
        with self.assertRaisesRegex(ValueError, "SOW was frozen"):
            task.execute(self.root, self.company, str(uuid.uuid4()), self.package, digest)

    def test_verifier_rejects_invented_source_fact_even_with_updated_hash(self):
        directory, digest = self.approved_fixture()
        _, output, _ = task.execute(self.root, self.company, str(uuid.uuid4()), self.package, digest)
        path = output / "evidence.json"
        rows = json.loads(path.read_text())
        employee = next(r for r in rows if r["source_file"] == "task/employee-records.csv")
        employee["observed_fact"]["manager_approved"] = "invented"
        task.write_json(path, rows)
        summary = json.loads((output / "run-summary.json").read_text())
        summary["artifact_sha256"]["evidence.json"] = task.sha(path)
        task.write_json(output / "run-summary.json", summary)
        with self.assertRaisesRegex(ValueError, "CSV fact"):
            task.verify(directory, output)

    def test_missing_register_fact_is_unknown_and_known_failure_dominates(self):
        self.assertEqual(task.gate(None, True), "UNKNOWN")
        self.assertEqual(task.gate(None, False), "HOLD")

    def test_changed_company_policy_requires_a_new_reviewed_rule_profile(self):
        directory, _ = self.approved_fixture()
        policy = directory / "originals/company/departments/Finance-manual.txt"
        policy.write_text(policy.read_text() + "\nA new, unsupported policy amendment.\n")
        with self.assertRaisesRegex(ValueError, "Unsupported policy source hash"):
            task.rule_sources(directory)

    def test_inclusive_deadline_and_finance_limit_and_insurance_date(self):
        employee = {"employee_id": "E777", "hire_date": "2026-10-01", "training_completed_date": "2026-10-08", "manager_approved": "yes"}
        request = {"cost_center": "C1", "requested_cents": "120000", "charge_type": "MOB-IND", "role_code": "FIELD-READ"}
        policy = {"effective_hire_date": "2026-10-01", "training_days": 7, "manager_approval_required": True}
        result = task.decide(employee, request, {"clearance_code": "HRC-2", "assignment_state": "ACTIVE"}, {"status": "ACTIVE"},
                             {"mfa_state": "VERIFIED"}, {"status": "APPROVED", "insurance_valid_through": "2026-10-15"}, policy, {}, dt.date(2026, 10, 15))
        self.assertEqual(result["training_decision"], "PASS")
        self.assertEqual(result["release_decision"], "ALLOW")
        self.assertEqual(result["release_ticket"], "WNG|E777|C1|READY")
        employee["training_completed_date"] = "2026-10-09"
        result = task.decide(employee, request, {}, {}, {}, {}, policy, {}, dt.date(2026, 10, 15))
        self.assertEqual(result["release_decision"], "HOLD")
        self.assertEqual(result["release_ticket"], "")


if __name__ == "__main__":
    unittest.main()

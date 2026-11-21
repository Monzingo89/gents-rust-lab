import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import role_probe_campaign as campaign


class ReadinessIndexTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.model = {
            "id": "a",
            "publisher": "owner/a",
            "source_repo": "owner/source",
            "source_revision": "a" * 40,
            "path": "models/a.gguf",
            "sha256": "b" * 64,
            "context_tokens": 8192,
        }
        self.contract = {
            "id": "a",
            "instruction": "unit fixture, not real inference evidence",
            "cases": [
                {"input": {"case_id": "one"}, "expected": {"accepted": True}},
                {"input": {"case_id": "two"}, "expected": {"accepted": False}},
            ],
        }
        (self.root / "config").mkdir()
        (self.root / "config/enterprise-role-probes.json").write_text(
            json.dumps({"roles": [self.contract]})
        )
        self.patcher = patch.object(campaign, "ROOT", self.root)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)

    def recorded_campaign(self, name, passed, timestamp):
        folder = self.root / "data/model-registry/role-campaigns" / name
        folder.mkdir(parents=True)
        (folder / "manifest.json").write_text(
            json.dumps(
                {
                    "roster": {"models": [self.model]},
                    "protocol": {"roles": [self.contract]},
                }
            )
        )
        calls = []
        for case in self.contract["cases"]:
            raw = json.dumps(
                case["input"], sort_keys=True, separators=(",", ":"), ensure_ascii=False
            ).encode()
            calls.append(
                {
                    "case_id": case["input"]["case_id"],
                    "source_sha256": hashlib.sha256(raw).hexdigest(),
                    "reply": {
                        "text": json.dumps(case["expected"]),
                        "usage": {"cost_usd": 0},
                        "local_execution": {"fixture": "simulated unit receipt only"},
                    },
                }
            )
        evidence = folder / "fixture.json"
        evidence.write_text(
            json.dumps(
                {
                    "id": "a",
                    "weight_sha256": self.model["sha256"],
                    "role_contract_sha256": hashlib.sha256(
                        json.dumps(
                            self.contract,
                            sort_keys=True,
                            separators=(",", ":"),
                            ensure_ascii=False,
                        ).encode()
                    ).hexdigest(),
                    "model": {"provider": "local", "path": self.model["path"]},
                    "calls": calls,
                }
            )
        )
        report = folder / "report.json"
        report.write_text(
            json.dumps(
                {"models": [{"id": "a", "passed": passed, "evidence": str(evidence)}]}
            )
        )
        os.utime(report, (timestamp, timestamp))
        return evidence

    def test_latest_matching_failure_cannot_be_hidden_by_a_prior_pass(self):
        self.recorded_campaign("older-pass", True, 100)
        self.recorded_campaign("newer-failure", False, 200)
        campaign.summarize([self.model])
        report = json.loads(
            (self.root / "data/model-registry/readiness-index.json").read_text()
        )
        self.assertEqual(report["ready_models"], 0)
        self.assertEqual(report["enterprise_qualified_models"], 0)

    def test_source_hash_mismatch_is_rejected_even_when_report_claims_pass(self):
        evidence = self.recorded_campaign("pass", True, 100)
        value = json.loads(evidence.read_text())
        value["calls"][0]["source_sha256"] = "0" * 64
        evidence.write_text(json.dumps(value))
        with self.assertRaises(AssertionError):
            campaign.summarize([self.model])

    def test_current_manifest_cannot_make_a_legacy_or_stale_receipt_ready(self):
        for index, digest in enumerate((None, "0" * 64)):
            with self.subTest(digest=digest):
                evidence = self.recorded_campaign(f"current-{index}", True, 200 + index)
                value = json.loads(evidence.read_text())
                value["role_contract_sha256"] = digest
                evidence.write_text(json.dumps(value))
                campaign.summarize([self.model])
                report = json.loads(
                    (self.root / "data/model-registry/readiness-index.json").read_text()
                )
                self.assertEqual(report["ready_models"], 0)


if __name__ == "__main__":
    unittest.main()

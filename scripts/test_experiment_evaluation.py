"""Host custody/protocol boundary checks; journal entries here are simulated."""

import json
import tempfile
import unittest
import uuid
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import experiment_continuation as continuation
import experiment_evaluation as e
import local_sqlite as sql
from experiment_loop import bounded_file, canonical


class EvaluationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.run = str(uuid.uuid4())
        self.company = "one"
        self.trial = {
            "actor": "Engineer",
            "task": "Actual frozen task",
            "criteria": ["proposal-only"],
            "max_model_calls": 1,
            "candidate_workflow": {
                "version": 1,
                "actor": "Engineer",
                "path": "candidate.rs",
                "max_bytes": 8192,
                "max_attempts": 1,
            },
        }
        workspace = self.root / "data/companies/one/runs" / self.run / "workspace"
        workspace.mkdir(parents=True)
        self.source = workspace / "candidate.rs"
        self.source.write_text(
            "pub fn remaining(used:usize,limit:usize)->usize{limit.saturating_sub(used)}"
        )
        self.dbpath = self.root / "data/companies/one/brain.sqlite3"
        intake = {
            "idea": self.trial["task"],
            "acceptance": "\n".join(self.trial["criteria"]),
            "candidate_workflow": self.trial["candidate_workflow"],
        }
        encoded = canonical(intake)
        contract = e.sha(encoded.encode())
        approval = {
            "owner": "local-developer",
            "sha256": contract,
            "type": "local_operator_approval",
        }
        native = lambda value: json.dumps(
            value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
        )
        receipts = []
        for args in [
            {"op": "write", "path": "candidate.rs", "content": self.source.read_text()},
            {
                "op": "read",
                "path": "candidate.rs",
                "offset_bytes": 0,
                "max_bytes": 8192,
            },
        ]:
            receipts.append(
                {
                    "actor": "Engineer",
                    "phase": "worker",
                    "tool": "files",
                    "policy_revision": 1,
                    "status": "completed",
                    "arguments_sha256": e.sha(native(args).encode()),
                    "result_sha256": "b" * 64,
                }
            )
        state = {
            "stage": "submitted",
            "source_sha256": e.sha(self.source.read_bytes()),
            "source": self.source.read_text(),
            "offset": self.source.stat().st_size,
            "write_receipt": receipts[0],
            "read_receipts": [receipts[1]],
        }
        body = canonical(state)
        policy = {
            "revision": 1,
            "grants": {"Engineer": {"tools": ["files:read", "files:write"]}},
        }
        with sql.connect(self.dbpath) as db:
            db.execute("CREATE TABLE company(owner TEXT)")
            db.execute("INSERT INTO company VALUES('local-developer')")
            db.execute(
                "CREATE TABLE jobs(run_id TEXT,intake TEXT,digest TEXT,approval TEXT,state TEXT)"
            )
            db.execute(
                "INSERT INTO jobs VALUES(?,?,?,?,?)",
                (
                    self.run,
                    encoded,
                    contract,
                    canonical(approval),
                    "candidate_submitted",
                ),
            )
            db.execute(
                "CREATE TABLE run_governance(run_id TEXT,revision INTEGER,manifest TEXT)"
            )
            db.execute(
                "INSERT INTO run_governance VALUES(?,1,?)",
                (self.run, canonical(policy)),
            )
            db.execute(
                "CREATE TABLE candidate_workflows(run_id TEXT,contract TEXT,revision INTEGER,state TEXT,state_sha256 TEXT)"
            )
            db.execute(
                "INSERT INTO candidate_workflows VALUES(?,?,1,?,?)",
                (self.run, contract, body, e.sha(body.encode())),
            )
            db.execute(
                "CREATE TABLE audit(seq INTEGER,run_id TEXT,kind TEXT,body TEXT)"
            )
            for i, receipt in enumerate(receipts):
                db.execute(
                    "INSERT INTO audit VALUES(?,?,'mcp_tool_call',?)",
                    (i + 1, self.run, canonical(receipt)),
                )

    def tearDown(self):
        self.temporary.cleanup()

    def custody(self):
        return e.custody(self.root, self.company, self.run, self.trial, bounded_file)

    def mutate(self, statement, args=()):
        with sql.connect(self.dbpath) as db:
            db.execute(statement, args)

    def test_custody_requires_exact_current_bytes_and_canonical_scope_receipts(self):
        source, receipt = self.custody()
        self.assertEqual(source, self.source.read_text())
        self.assertEqual(receipt["native_audit_sequences"], [1, 2])
        self.source.write_text("pub fn changed()->u32{0}")
        with self.assertRaisesRegex(ValueError, "exact confirmed"):
            self.custody()

    def test_receipt_loss_does_not_become_acceptance(self):
        self.mutate("DELETE FROM audit WHERE seq=2")
        with self.assertRaisesRegex(ValueError, "canonical journal"):
            self.custody()

    def test_an_identical_confirmed_read_does_not_invalidate_existing_custody(self):
        self.mutate(
            "INSERT INTO audit SELECT 3,run_id,kind,body FROM audit WHERE seq=2"
        )
        self.assertEqual(self.custody()[1]["native_audit_sequences"], [1, 2])

    def test_changed_epoch_or_revoked_actor_stops_review(self):
        self.mutate(
            "UPDATE run_governance SET revision=2,manifest=?",
            (canonical({"revision": 2, "grants": {"Engineer": {"tools": []}}}),),
        )
        with self.assertRaises(ValueError):
            self.custody()

    def test_changed_checkpoint_or_approval_stops_review(self):
        self.mutate("UPDATE candidate_workflows SET state_sha256='corrupt'")
        with self.assertRaisesRegex(ValueError, "checkpoint identity"):
            self.custody()

    def test_foreign_local_owner_cannot_supply_another_companys_candidate(self):
        self.mutate("UPDATE company SET owner='someone-else'")
        with self.assertRaisesRegex(ValueError, "owner identity"):
            self.custody()

    def test_different_frozen_task_cannot_reuse_the_source_receipts(self):
        self.trial["task"] = "A different requested implementation"
        with self.assertRaisesRegex(ValueError, "frozen experiment"):
            self.custody()

    def test_absent_or_nonterminal_submission_is_not_eligible(self):
        self.mutate("UPDATE jobs SET state='running'")
        with self.assertRaisesRegex(ValueError, "owner-approved submission"):
            self.custody()

    def test_evaluator_checks_current_custody_after_actual_execution_boundary(self):
        self.trial["evaluation"] = {
            "frozen_tests": "owner tests",
            "tests_sha256": e.sha(b"owner tests"),
            "runtime": {},
        }

        def changed(*args):
            self.source.write_text("pub fn changed()->u32{0}")
            return {"passed": True}

        with (
            patch.object(e, "run_native", side_effect=changed),
            self.assertRaises(ValueError),
        ):
            e.evaluate(self.root, self.company, self.run, self.trial, bounded_file)

    def test_frozen_tests_are_checked_before_execution(self):
        self.trial["evaluation"] = {
            "frozen_tests": "changed",
            "tests_sha256": e.sha(b"owner tests"),
            "runtime": {},
        }
        with (
            patch.object(e, "run_native") as execute,
            self.assertRaisesRegex(ValueError, "acceptance snapshot"),
        ):
            e.evaluate(self.root, self.company, self.run, self.trial, bounded_file)
        execute.assert_not_called()

    def test_native_runtime_pin_change_is_detected_before_subprocess(self):
        runtime = {
            "adapter": {"path": "/fake/adapter", "sha256": "a"},
            "compiler": {"path": "/fake/compiler", "sha256": "b"},
        }
        with (
            patch.object(
                e,
                "executable_pin",
                return_value={"path": "/fake/adapter", "sha256": "changed"},
            ),
            patch.object(e.subprocess, "run") as run,
        ):
            with self.assertRaisesRegex(ValueError, "executable changed"):
                e.run_native(runtime, {})
            run.assert_not_called()

    def test_forged_or_mismatched_native_pass_is_rejected(self):
        runtime = {
            "adapter": {"path": "/fake/adapter", "sha256": "a"},
            "compiler": {"path": "/fake/compiler", "sha256": "b"},
        }
        request = {"source_sha256": "c" * 64, "tests_sha256": "d" * 64}
        good = {
            "passed": True,
            **request,
            "promotion_authorized": False,
            "provider_cost_usd": 0,
            "stage": "acceptance",
            "diagnostics": {"passed": True},
            "tests_count": 1,
        }
        bads = [
            dict(good, source_sha256="wrong"),
            dict(good, stage="admission"),
            dict(good, diagnostics={"passed": False}),
            dict(good, tests_count=0),
            dict(good, promotion_authorized=True),
            dict(good, provider_cost_usd=True),
        ]
        pins = lambda path: (
            runtime["adapter"] if path.endswith("adapter") else runtime["compiler"]
        )
        with patch.object(e, "executable_pin", side_effect=pins):
            for bad in bads:
                result = SimpleNamespace(
                    stdout=json.dumps(bad).encode(), stderr=b"", returncode=0
                )
                with (
                    self.subTest(bad=bad),
                    patch.object(e.subprocess, "run", return_value=result),
                    self.assertRaises(ValueError),
                ):
                    e.run_native(runtime, request)

    def test_invalid_owner_evaluator_descriptor_does_not_run_a_program(self):
        for descriptor in (
            {
                "kind": "shell",
                "tests_path": "data/experiment-inputs/t",
                "tests_sha256": "a" * 64,
            },
            {
                "kind": "rust_acceptance",
                "tests_path": "config/secrets",
                "tests_sha256": "a" * 64,
            },
        ):
            with (
                patch.object(e.subprocess, "run") as run,
                self.assertRaises(ValueError),
            ):
                e.freeze(self.root, descriptor, self.trial, bounded_file)
            run.assert_not_called()

    def test_acceptance_byte_change_is_rejected_before_preflight_or_inference(self):
        descriptor = {
            "kind": "rust_acceptance",
            "tests_path": "data/experiment-inputs/acceptance.rs",
            "tests_sha256": "a" * 64,
        }
        with (
            patch.object(e.subprocess, "run") as run,
            self.assertRaisesRegex(ValueError, "acceptance hash mismatch"),
        ):
            e.freeze(
                self.root, descriptor, self.trial, lambda *args: b"changed owner tests"
            )
        run.assert_not_called()

    def test_published_example_preserves_the_original_finite_acceptance_body(self):
        root = Path(__file__).resolve().parents[1]
        example = json.loads(
            (root / "config/experiment-loop.rust-example.json").read_text()
        )
        original = (root / "scripts/fixtures/chunk-boundary-acceptance.rs").read_text()
        wrapper = 'mod candidate {include!(env!("GENTS_CANDIDATE_SOURCE"));}\n'
        self.assertEqual(original.count(wrapper), 1)
        self.assertEqual(
            e.sha(original.replace(wrapper, "").encode()),
            example["experiments"][0]["evaluation"]["tests_sha256"],
        )
        self.assertEqual(example["max_model_calls"], 1)

    def continuation_result(self, passed=False):
        _source, custody = self.custody()
        runtime = {
            "adapter": {"path": "/fake/adapter", "sha256": "a"},
            "compiler": {"path": "/fake/compiler", "sha256": "b"},
        }
        self.trial["evaluation"] = {
            "kind": "rust_acceptance",
            "tests_sha256": "c" * 64,
            "runtime": runtime,
        }
        checked = {
            "kind": "rust_acceptance",
            "passed": passed,
            "runtime": runtime,
            "custody": custody,
            "tests_sha256": "c" * 64,
            "native_result": {
                "passed": passed,
                "source_sha256": custody["source_sha256"],
                "tests_sha256": "c" * 64,
                "stage": "acceptance",
                "diagnostics": {"stdout": "α" * 1000, "stderr": ""},
            },
        }
        return {
            "runtime_status": "candidate_submitted",
            "independently_accepted": passed,
            "evaluation": checked,
        }

    def prepare(self, result, condition="rejected", prior_sources=None):
        runtime = self.trial["evaluation"]["runtime"]
        with patch.object(
            e,
            "executable_pin",
            side_effect=lambda path: (
                runtime["adapter"] if path.endswith("adapter") else runtime["compiler"]
            ),
        ):
            return continuation.prepare(
                self.root,
                "one",
                self.run,
                self.trial,
                result,
                condition,
                prior_sources or set(),
                "d" * 64,
                "e" * 64,
                bounded_file,
            )

    def test_continuation_relays_exact_source_with_bounded_utf8_diagnostic(self):
        result = self.continuation_result()
        prepared = self.prepare(result)
        self.assertEqual(prepared["decision"], "continue")
        feedback = prepared["feedback"]
        self.assertEqual(feedback["source"], self.source.read_text())
        self.assertEqual(feedback["prior_run"], self.run)
        self.assertLessEqual(len(feedback["diagnostics_excerpt"].encode()), 1200)
        self.assertEqual(feedback["evaluation_sha256"], "e" * 64)

    def test_condition_mismatch_and_repeated_source_do_not_author_another_revision(
        self,
    ):
        result = self.continuation_result(passed=True)
        self.assertEqual(self.prepare(result)["decision"], "skip")
        self.assertEqual(
            self.prepare(result, "accepted", {e.sha(self.source.read_bytes())})[
                "decision"
            ],
            "skip",
        )

    def test_continuation_rechecks_custody_and_cannot_use_changed_source(self):
        result = self.continuation_result()
        self.source.write_text("pub fn changed()->u64{0}")
        with self.assertRaises(ValueError):
            self.prepare(result)

    def test_runtime_failure_is_not_an_independent_rejection_for_repair(self):
        result = self.continuation_result()
        result["runtime_status"] = "budget_exhausted"
        self.assertEqual(self.prepare(result)["decision"], "skip")

    def test_followup_does_not_accept_another_criterion_hash(self):
        parent = {
            **self.trial,
            "evaluation": {"kind": "rust_acceptance", "tests_sha256": "a" * 64},
        }
        trial = {**parent, "after": {"experiment": "parent", "outcome": "rejected"}}
        continuation.validate(trial, {"parent": parent})
        trial["evaluation"] = {"kind": "rust_acceptance", "tests_sha256": "b" * 64}
        with self.assertRaisesRegex(ValueError, "preserve"):
            continuation.validate(trial, {"parent": parent})

    def test_continuation_rejects_forward_edges_and_changed_tasks(self):
        parent = {
            **self.trial,
            "evaluation": {"kind": "rust_acceptance", "tests_sha256": "a" * 64},
        }
        trial = {**parent, "after": {"experiment": "parent", "outcome": "rejected"}}
        with self.assertRaises(ValueError):
            continuation.validate(trial, {})
        trial["task"] = "A different task"
        with self.assertRaises(ValueError):
            continuation.validate(trial, {"parent": parent})


if __name__ == "__main__":
    unittest.main()

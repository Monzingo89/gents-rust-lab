"""Coordinator invariants. Model proposals/outcomes here are explicit fixtures."""

import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import research_agenda as m
from test_experiment_loop import API

PROPOSAL = {
    "hypothesis": "Handle empty remainder before retreating a boundary.",
    "procedure": [
        "Validate the offset and positive limit first.",
        "Return the end offset when already at end.",
    ],
    "prediction": "The original end-of-input failure will pass.",
}


class Driver:
    def __init__(self, owner):
        self.owner, self.calls, self.pending = owner, [], None
        self.proposals, self.sources = [json.dumps(PROPOSAL)] * 2, ["repaired"] * 2
        self.accepted = True

    def step(self, identifier, plan, expected, config):
        self.calls.append((identifier, copy.deepcopy(plan), copy.deepcopy(expected)))
        if self.pending:
            return {"status": self.pending, "run_id": "original-uncertain-run"}, None
        index = int(identifier.split("-c")[-1].split("-")[0])
        research = identifier.endswith("research")
        source = self.proposals[index] if research else self.sources[index]
        run = identifier + "-run"
        proof = self.owner.proof(source)
        self.owner.artifacts[run] = (source, proof)
        result = {
            "runtime_status": "candidate_submitted",
            "independently_accepted": False,
        }
        if not research:
            result["evaluation"] = self.owner.outcome(source, self.accepted)
            result["independently_accepted"] = self.accepted
        return {"status": "result"}, {"run_id": run, "result": result}


class AgendaTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.api = API()
        self.api.config["agents"].append(
            {
                "id": "Researcher",
                "worker_provider": "local",
                "planner_provider": "local",
            }
        )
        self.runtime = {
            "adapter": {"path": "/adapter", "sha256": "a" * 64},
            "compiler": {"path": "/compiler", "sha256": "b" * 64},
        }
        self.spec = {
            "kind": "rust_acceptance",
            "tests_path": "data/experiment-inputs/tests.rs",
            "tests_sha256": "c" * 64,
        }
        self.sealed = {
            **self.spec,
            "runtime": self.runtime,
            "frozen_tests": "owner tests",
            "parser_executable": {"path": "/parser", "sha256": "d" * 64},
        }
        self.plan = {
            "version": 1,
            "company": "one",
            "researcher": "Researcher",
            "author": "Engineer",
            "case": {
                "task": "Owner task including exact end-of-input behavior",
                "source_path": "candidate.rs",
                "evaluation": self.spec,
            },
            "max_cycles": 2,
            "max_model_calls": 4,
            "failed_reference": {
                "company": "failed",
                "actor": "Engineer",
                "run_id": "failed-run",
            },
            "teacher_reference": {
                "company": "teacher",
                "actor": "Engineer",
                "run_id": "teacher-run",
            },
        }
        self.artifacts = {
            "failed-run": ("original", self.proof("original")),
            "teacher-run": ("teacher", self.proof("teacher")),
        }
        self.driver = Driver(self)
        self.agenda = m.Agenda(self.root, "fixture", self.api, self.driver)
        for name, replacement in [
            (
                "reference_trial",
                lambda root, reference, case: {
                    "actor": reference["actor"],
                    "task": case["task"],
                },
            ),
            ("evaluation.freeze", lambda *args: copy.deepcopy(self.sealed)),
            (
                "evaluation.evaluate",
                lambda root, company, run, *args: self.outcome(
                    self.artifacts[run][0], run == "teacher-run"
                ),
            ),
            (
                "evaluation.custody",
                lambda root, company, run, *args: copy.deepcopy(self.artifacts[run]),
            ),
            (
                "evaluation.executable_pin",
                lambda path: copy.deepcopy(
                    next(
                        pin
                        for pin in [
                            *self.runtime.values(),
                            self.sealed["parser_executable"],
                        ]
                        if pin["path"] == path
                    )
                ),
            ),
        ]:
            context = patch("research_agenda." + name, replacement)
            context.start()
            self.addCleanup(context.stop)
        self.agenda.freeze(self.plan)
        self.api.calls.clear()

    def proof(self, source):
        return {
            "source_sha256": m.evaluation.sha(source.encode()),
            "policy_revision": 1,
            "native_audit_sequences": [8, 9],
            "intake_sha256": "e" * 64,
        }

    def outcome(self, source, passed):
        return {
            "kind": "rust_acceptance",
            "passed": passed,
            "custody": self.proof(source),
            "runtime": self.runtime,
            "tests_sha256": self.spec["tests_sha256"],
            "promotion_authorized": False,
            "native_result": {
                "passed": passed,
                "source_sha256": self.proof(source)["source_sha256"],
                "tests_sha256": self.spec["tests_sha256"],
                "diagnostics": {"stdout": "fixture diagnostic", "stderr": ""},
            },
        }

    def restart(self):
        self.agenda = m.Agenda(self.root, "fixture", self.api, self.driver)

    def test_ready_and_accepted_restart_idle_without_api_or_driver(self):
        for _ in range(20):
            self.assertEqual(self.agenda.poll()["status"], "ready")
        self.assertEqual(self.api.calls, [])
        self.agenda.start()
        self.assertEqual(self.agenda.poll()["cycles"][0]["stage"], "trial")
        self.restart()
        self.assertEqual(self.agenda.poll()["status"], "accepted")
        self.assertEqual(len(self.driver.calls), 2)
        self.api.calls.clear()
        before = (self.agenda.storage.folder / "ledger.sqlite3").read_bytes()
        for _ in range(20):
            self.restart()
            self.assertEqual(self.agenda.poll()["status"], "accepted")
        self.assertEqual(self.api.calls, [])
        self.assertEqual(len(self.driver.calls), 2)
        self.assertEqual(
            (self.agenda.storage.folder / "ledger.sqlite3").read_bytes(), before
        )
        self.assertEqual(self.agenda.start()["status"], "accepted")

    def test_compiler_preserves_objective_evaluator_grants_and_hides_teacher_from_author(
        self,
    ):
        self.agenda.start()
        self.agenda.poll()
        self.agenda.poll()
        research, author = [row[1] for row in self.driver.calls]
        task = json.loads(author["experiments"][0]["task"])
        self.assertEqual(task["objective"], self.plan["case"]["task"])
        self.assertNotIn("reviewed_teacher_source", task)
        self.assertEqual(
            json.loads(research["experiments"][0]["task"])["reviewed_teacher_source"],
            "teacher",
        )
        for plan in (research, author):
            trial = plan["experiments"][0]
            self.assertEqual(plan["max_model_calls"], 1)
            self.assertEqual(trial["tools"], ["files:read", "files:write"])
            self.assertEqual(trial["criteria"], [m.ACCEPTANCE])
            self.assertEqual(trial["candidate_workflow"]["max_attempts"], 1)
        self.assertEqual(author["experiments"][0]["evaluation"], self.spec)
        self.assertEqual(
            self.driver.calls[1][2]["experiments"][0]["evaluation"], self.sealed
        )

    def test_unknown_dispatch_keeps_same_child_and_reservation_after_restart(self):
        self.driver.pending = "needs_reconciliation"
        self.agenda.start()
        first = self.agenda.poll()
        self.restart()
        for _ in range(4):
            self.assertEqual(self.agenda.poll(), first)
        self.assertEqual(first["reserved_model_calls"], 2)
        self.assertEqual(len({c[0] for c in self.driver.calls}), 1)

    def test_invalid_proposal_stops_before_author_and_never_refunds(self):
        self.driver.proposals[0] = json.dumps({**PROPOSAL, "tools": ["shell"]})
        self.agenda.start()
        result = self.agenda.poll()
        self.assertEqual(result["status"], "proposal_rejected")
        self.assertEqual(result["reserved_model_calls"], 2)
        self.assertEqual(len(self.driver.calls), 1)
        self.api.calls.clear()
        self.assertEqual(self.agenda.poll(), result)
        self.assertEqual(self.api.calls, [])

    def test_repeated_source_stops_without_second_research_call(self):
        self.driver.accepted = False
        self.driver.sources[0] = "original"
        self.agenda.start()
        self.agenda.poll()
        self.assertEqual(self.agenda.poll()["status"], "stalled_source")
        self.assertEqual(len(self.driver.calls), 2)

    def test_repeated_procedure_stops_next_author(self):
        self.driver.accepted = False
        self.agenda.start()
        self.agenda.poll()
        self.agenda.poll()
        result = self.agenda.poll()
        self.assertEqual(result["status"], "stalled_proposal")
        self.assertEqual(result["reserved_model_calls"], 4)
        self.assertEqual(len(self.driver.calls), 3)

    def test_new_rejected_source_and_procedure_can_follow_until_frozen_limit(self):
        self.driver.accepted = False
        self.driver.sources[1] = "second repaired"
        self.driver.proposals[1] = json.dumps(
            {**PROPOSAL, "procedure": ["Different step one.", "Different step two."]}
        )
        self.agenda.start()
        for _ in range(4):
            self.agenda.poll()
        result = self.agenda.poll()
        self.assertEqual(result["status"], "budget_exhausted")
        self.assertEqual(result["reserved_model_calls"], 4)
        self.assertEqual(len(self.driver.calls), 4)
        next_task = json.loads(self.driver.calls[2][1]["experiments"][0]["task"])
        self.assertEqual(next_task["failed_source"], "repaired")

    def test_custody_change_in_note_blocks_author(self):
        self.agenda.start()
        self.agenda.poll()
        run = next(r for r in self.artifacts if r.endswith("research-run"))
        self.artifacts[run] = ("tampered", self.proof("tampered"))
        with self.assertRaisesRegex(ValueError, "note custody"):
            self.agenda.poll()
        self.assertEqual(len(self.driver.calls), 1)

    def test_prior_trial_custody_change_blocks_new_research(self):
        self.driver.accepted = False
        self.agenda.start()
        self.agenda.poll()
        self.agenda.poll()
        run = next(r for r in self.artifacts if r.endswith("trial-run"))
        self.artifacts[run] = ("tampered", self.proof("tampered"))
        with self.assertRaisesRegex(ValueError, "previous trial custody"):
            self.agenda.poll()
        self.assertEqual(len(self.driver.calls), 2)

    def test_reference_and_configuration_changes_block_dispatch(self):
        self.agenda.start()
        self.artifacts["teacher-run"] = ("altered", self.proof("altered"))
        with self.assertRaisesRegex(ValueError, "reference custody"):
            self.agenda.poll()
        self.assertEqual(self.driver.calls, [])
        self.api.config["agents"][0]["worker_provider"] = "cloud"
        with self.assertRaisesRegex(ValueError, "configuration changed"):
            self.agenda.poll()

    def test_frozen_and_checkpoint_tamper_fail_without_budget_reset(self):
        with self.agenda.storage.locked() as db:
            db.execute("UPDATE research_agenda SET state='{}'")
            db.commit()
        with self.assertRaisesRegex(ValueError, "integrity changed"):
            self.agenda.poll()
        self.assertEqual(self.driver.calls, [])

    def test_stop_disarms_and_start_does_not_renew(self):
        self.agenda.start()
        self.assertEqual(self.agenda.stop()["status"], "stopped")
        self.assertEqual(self.agenda.start()["status"], "stopped")
        self.api.calls.clear()
        self.assertEqual(self.agenda.poll()["reserved_model_calls"], 0)
        self.assertEqual(self.api.calls, [])

    def test_oversized_compilation_refuses_without_dispatch(self):
        self.agenda.start()
        self.artifacts["failed-run"] = ("x" * 8200, self.proof("x" * 8200))
        # Re-freeze with the larger trusted reference in an independent fixture.
        self.agenda = m.Agenda(self.root, "large", self.api, self.driver)
        self.agenda.freeze(self.plan)
        self.agenda.start()
        with self.assertRaisesRegex(ValueError, "no truncation"):
            self.agenda.poll()
        self.assertEqual(self.driver.calls, [])
        self.assertEqual(self.agenda.status()["reserved_model_calls"], 0)


class ProposalTests(unittest.TestCase):
    def test_strict_schema_bounds_and_duplicate_keys(self):
        self.assertEqual(m.parse_proposal(json.dumps(PROPOSAL)), PROPOSAL)
        invalid = [
            "null",
            "[]",
            '{"hypothesis":"a","hypothesis":"b","procedure":[],"prediction":"x"}',
            json.dumps({**PROPOSAL, "procedure": ["only one"]}),
            json.dumps({**PROPOSAL, "hypothesis": "a" * 385}),
            json.dumps({**PROPOSAL, "prediction": "```code```"}),
            json.dumps({**PROPOSAL, "procedure": ["pub fn x(){}", "step"]}),
            json.dumps({**PROPOSAL, "hypothesis": "line\rbreak"}),
        ]
        for source in invalid:
            with self.subTest(source=source), self.assertRaises(ValueError):
                m.parse_proposal(source)

    def test_whitespace_case_changes_do_not_make_novel_procedure(self):
        changed = {
            **PROPOSAL,
            "procedure": [
                "  " + s.upper().replace(" ", "  ") for s in PROPOSAL["procedure"]
            ],
        }
        self.assertEqual(m.procedure_key(PROPOSAL), m.procedure_key(changed))


if __name__ == "__main__":
    unittest.main()

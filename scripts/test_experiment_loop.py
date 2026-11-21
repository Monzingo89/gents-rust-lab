"""Durability, admission and no-inference checks; API decisions here are fixtures."""

import copy
import json
import os
import tempfile
import threading
import unittest
import urllib.error
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from unittest.mock import patch

import experiment_loop as m


class API:
    def __init__(self):
        self.calls = []
        self.jobs = []
        self.runs = {}
        self.config = {
            "local_only": True,
            "company_mode": True,
            "company_tools": True,
            "auth": "local",
            "agents": [
                {
                    "id": "Engineer",
                    "worker_provider": "local",
                    "planner_provider": "local",
                }
            ],
        }
        self.fail = None
        self.lost_dispatch_response = False

    def __call__(self, path, body=None):
        self.calls.append((path, copy.deepcopy(body)))
        if self.fail and self.fail(path, body):
            raise OSError("fixture interruption")
        if path == "/api/config":
            return copy.deepcopy(self.config)
        if path.startswith("/api/companies/"):
            if body["op"] == "intake":
                self.jobs.append(
                    {
                        "run_id": body["run_id"],
                        "digest": "owner-digest",
                        "state": "draft",
                    }
                )
            return {"jobs": copy.deepcopy(self.jobs)}
        if path == "/api/runs":
            run = body["run_id"]
            self.runs[run] = {
                "status": "completed",
                "events": [{"reply": {"usage": {"cost_usd": 0}}}],
            }
            if self.lost_dispatch_response:
                raise OSError("response lost after dispatch")
            return {"id": run}
        run = path.split("/")[3]
        if run not in self.runs:
            raise urllib.error.HTTPError(path, 404, "absent", {}, None)
        if path.endswith("/attention"):
            return {"attention": None}
        return copy.deepcopy(self.runs[run])


class ControllerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.input = self.root / "data/experiment-inputs/research.json"
        self.input.parent.mkdir(parents=True)
        self.input.write_text("baseline")
        self.api = API()
        trial = {
            "id": "first",
            "actor": "Engineer",
            "task": "Author bounded candidate source",
            "criteria": ["Independent review remains required"],
            "tools": ["files:write", "files:read"],
            "max_model_calls": 1,
        }
        self.plan = {
            "version": 1,
            "company": "one",
            "evidence_paths": ["data/experiment-inputs/research.json"],
            "max_model_calls": 2,
            "experiments": [trial, {**trial, "id": "second"}],
        }
        self.controller = m.Controller(self.root, "example", self.api)
        self.controller.freeze(self.plan)
        self.api.calls.clear()

    def tearDown(self):
        self.temporary.cleanup()

    def change(self, body="new research"):
        self.input.write_text(body)

    def post_count(self):
        return sum(path == "/api/runs" for path, _ in self.api.calls)

    def finish(self):
        admitted = self.controller.poll()
        self.assertEqual(admitted["status"], "dispatched")
        report = self.controller.poll()
        self.assertEqual(report["status"], "result")
        return admitted["run_id"]

    def test_initial_unchanged_and_restarted_polls_do_not_even_read_api(self):
        for _ in range(20):
            self.assertEqual(self.controller.poll()["status"], "unchanged")
        restarted = m.Controller(self.root, "example", self.api)
        self.assertEqual(restarted.poll()["status"], "unchanged")
        self.assertEqual(self.api.calls, [])
        self.assertEqual(restarted.status()["reserved_model_calls"], 0)

    def test_one_claim_per_distinct_evidence_and_finite_plan_never_renews(self):
        self.change()
        run = self.finish()
        self.assertFalse(
            self.controller.status()["trials"][0]["result"]["independently_accepted"]
        )
        self.assertEqual(self.controller.poll()["status"], "unchanged")
        self.change("baseline")
        self.assertEqual(self.controller.poll()["status"], "unchanged")
        self.change("new research")
        self.assertEqual(self.controller.poll()["status"], "unchanged")
        self.change("another independent observation")
        self.assertNotEqual(self.finish(), run)
        self.assertEqual(self.controller.poll()["status"], "exhausted")
        self.assertEqual(self.controller.status()["reserved_model_calls"], 2)
        self.assertEqual(self.post_count(), 2)

    def test_allowance_and_exact_grants_reach_actual_intake_and_policy_requests(self):
        self.change()
        self.controller.poll()
        intake = next(
            body["intake"]
            for _, body in self.api.calls
            if body and body.get("op") == "intake"
        )
        self.assertEqual(intake["max_model_calls"], 1)
        self.assertEqual(intake["budget_usd"], 0)
        policy = next(
            body["policy"]
            for _, body in self.api.calls
            if body and body.get("op") == "governance_install"
        )
        self.assertEqual(
            policy["grants"]["Engineer"]["tools"], self.plan["experiments"][0]["tools"]
        )

    def test_preparation_failure_burns_claim_and_never_replays_any_mutation(self):
        self.change()
        self.api.fail = lambda _, body: body and body.get("op") == "approve"
        first = self.controller.poll()
        self.assertEqual(first["status"], "needs_reconciliation")
        self.api.fail = None
        writes = [call for call in self.api.calls if call[1] is not None]
        for _ in range(3):
            self.assertEqual(self.controller.poll()["status"], "needs_reconciliation")
        self.assertEqual(
            [call for call in self.api.calls if call[1] is not None], writes
        )
        self.assertEqual(self.controller.status()["reserved_model_calls"], 1)
        self.assertEqual(self.post_count(), 0)

    def test_lost_dispatch_response_reconciles_by_original_run_without_post(self):
        self.change()
        self.api.lost_dispatch_response = True
        first = self.controller.poll()
        self.assertEqual(first["status"], "needs_reconciliation")
        restarted = m.Controller(self.root, "example", self.api)
        report = restarted.poll()
        self.assertEqual(report["status"], "result")
        self.assertEqual(report["run_id"], first["run_id"])
        self.assertEqual(self.post_count(), 1)

    def test_crash_after_durable_claim_before_intake_does_not_replay(self):
        self.change()

        def crash(_, body):
            if body and body.get("op") == "intake":
                raise KeyboardInterrupt()

        self.api.fail = crash
        with self.assertRaises(KeyboardInterrupt):
            self.controller.poll()
        self.api.fail = None
        self.api.calls.clear()
        self.assertEqual(self.controller.poll()["status"], "needs_reconciliation")
        self.assertTrue(all(body is None for _, body in self.api.calls))
        self.assertEqual(self.controller.status()["reserved_model_calls"], 1)

    def test_active_and_interrupted_runs_remain_outstanding_without_resume(self):
        self.change()
        run = self.controller.poll()["run_id"]
        for status in (
            "running",
            "queued",
            "interrupted",
            "needs_reconciliation",
            "timed_out",
        ):
            self.api.runs[run]["status"] = status
            self.assertEqual(self.controller.poll()["status"], "observing")
            self.assertEqual(
                self.controller.status()["trials"][0]["state"], "observing"
            )
        self.assertEqual(self.post_count(), 1)
        self.assertFalse(any(path.endswith("/resume") for path, _ in self.api.calls))

    def test_incomplete_runtime_is_a_failed_result_with_no_acceptance(self):
        self.change()
        run = self.controller.poll()["run_id"]
        self.api.runs[run]["status"] = "incomplete"
        result = self.controller.poll()
        self.assertEqual(result["status"], "result")
        self.assertEqual(result["result"]["runtime_status"], "incomplete")
        self.assertFalse(result["result"]["independently_accepted"])

    def test_attention_is_retained_without_starting_another_trial(self):
        self.change()
        run = self.controller.poll()["run_id"]
        self.api.runs[run]["status"] = "running"
        original = self.controller.api
        self.controller.api = lambda path, body=None: (
            {"attention": {"answer": None}}
            if path.endswith("/attention")
            else original(path, body)
        )
        report = self.controller.poll()
        self.assertEqual(report["status"], "observing")
        self.assertEqual(report["result"]["status"], "attention")
        self.change("different evidence")
        self.assertEqual(self.controller.poll()["status"], "observing")
        self.assertEqual(self.post_count(), 1)

    def test_cost_validation_failure_never_releases_claim(self):
        self.change()
        run = self.controller.poll()["run_id"]
        self.api.runs[run]["events"][0]["reply"]["usage"]["cost_usd"] = 1
        with self.assertRaises(ValueError):
            self.controller.poll()
        self.assertEqual(self.controller.status()["trials"][0]["state"], "observing")
        self.assertEqual(self.post_count(), 1)

    def test_unknown_runtime_status_fails_closed(self):
        self.change()
        run = self.controller.poll()["run_id"]
        self.api.runs[run]["status"] = "invented-success"
        with self.assertRaisesRegex(ValueError, "unrecognized"):
            self.controller.poll()
        self.assertEqual(self.controller.status()["trials"][0]["state"], "observing")

    def test_configuration_drift_yields_no_claim_and_company_work_takes_priority(self):
        self.change()
        self.api.config["auth"] = "entra"
        with self.assertRaisesRegex(ValueError, "configuration changed"):
            self.controller.poll()
        self.assertEqual(self.controller.status()["reserved_model_calls"], 0)
        self.api.config["auth"] = "local"
        self.api.jobs.append({"state": "running", "run_id": "unrelated"})
        self.assertEqual(self.controller.poll()["status"], "yielding_to_company_work")
        self.assertEqual(self.controller.status()["reserved_model_calls"], 0)

    def test_concurrent_poller_cannot_reconcile_an_in_progress_preparation(self):
        self.change()
        entered, release = threading.Event(), threading.Event()
        original = self.api

        def slow(path, body=None):
            if body and body.get("op") == "intake":
                entered.set()
                if not release.wait(10):
                    raise RuntimeError("fixture timeout")
            return original(path, body)

        self.controller.api = slow
        reports = []
        thread = threading.Thread(target=lambda: reports.append(self.controller.poll()))
        thread.start()
        try:
            self.assertTrue(entered.wait(10))
            self.assertEqual(
                m.Controller(self.root, "example", self.api).poll()["status"], "busy"
            )
        finally:
            release.set()
            thread.join(10)
        self.assertEqual(reports[0]["status"], "dispatched")
        self.assertEqual(self.post_count(), 1)

    def test_frozen_state_cannot_be_overwritten_or_silently_corrupted(self):
        with self.assertRaisesRegex(ValueError, "already frozen"):
            self.controller.freeze(self.plan)
        with self.controller.locked() as db:
            db.execute("UPDATE frozen SET baseline='{}'")
            db.commit()
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            self.controller.poll()
        self.assertEqual(self.api.calls, [])

    def test_trial_reservation_corruption_cannot_create_a_new_allowance(self):
        self.change()
        self.finish()
        with self.controller.locked() as db:
            db.execute("UPDATE trials SET reserved_calls=0")
            db.commit()
        self.change("new signal")
        with self.assertRaisesRegex(ValueError, "ledger mismatch"):
            self.controller.poll()
        self.assertEqual(self.post_count(), 1)

    def evaluation_controller(self):
        trial = copy.deepcopy(self.plan["experiments"][0])
        trial["evaluation"] = {
            "kind": "rust_acceptance",
            "tests_path": "data/experiment-inputs/tests.rs",
            "tests_sha256": "a" * 64,
        }
        plan = {**self.plan, "experiments": [trial], "max_model_calls": 1}
        controller = m.Controller(self.root, "evaluated", self.api)
        with patch.object(
            m.experiment_evaluation,
            "freeze",
            side_effect=lambda root, spec, trial, read: spec,
        ):
            controller.freeze(plan)
        return controller

    def test_independent_pass_and_failure_are_separate_from_runtime_submission(self):
        controller = self.evaluation_controller()
        self.change()
        run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "candidate_submitted"
        with patch.object(
            m.experiment_evaluation,
            "evaluate",
            return_value={"passed": True, "scope": "frozen suite only"},
        ) as evaluate:
            report = controller.poll()
            evaluate.assert_called_once()
        self.assertEqual(report["result"]["runtime_status"], "candidate_submitted")
        self.assertTrue(report["result"]["independently_accepted"])
        self.assertFalse(report["result"]["promotion_authorized"])
        self.assertEqual(controller.status()["independently_accepted_trials"], 1)
        self.assertEqual(controller.poll()["status"], "exhausted")

    def test_evaluation_rejection_is_a_result_and_does_not_grant_acceptance(self):
        controller = self.evaluation_controller()
        self.change()
        run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "candidate_submitted"
        with patch.object(
            m.experiment_evaluation,
            "evaluate",
            return_value={"passed": False, "scope": "frozen suite only"},
        ):
            report = controller.poll()
        self.assertEqual(report["status"], "result")
        self.assertFalse(report["result"]["independently_accepted"])
        self.assertEqual(controller.status()["independently_accepted_trials"], 0)

    def test_uncertain_evaluation_is_not_replayed_or_accepted_on_restart(self):
        controller = self.evaluation_controller()
        self.change()
        run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "candidate_submitted"
        with patch.object(
            m.experiment_evaluation, "evaluate", side_effect=OSError("lost result")
        ) as evaluate:
            self.assertEqual(controller.poll()["status"], "needs_reconciliation")
            restarted = m.Controller(self.root, "evaluated", self.api)
            self.assertEqual(restarted.poll()["status"], "needs_reconciliation")
            evaluate.assert_called_once()
        self.assertEqual(restarted.status()["independently_accepted_trials"], 0)
        self.assertEqual(restarted.status()["reserved_model_calls"], 1)

    def test_crash_at_evaluation_checkpoint_does_not_repeat_native_execution(self):
        controller = self.evaluation_controller()
        self.change()
        run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "candidate_submitted"
        with patch.object(
            m.experiment_evaluation, "evaluate", side_effect=KeyboardInterrupt()
        ) as evaluate:
            with self.assertRaises(KeyboardInterrupt):
                controller.poll()
            self.assertEqual(controller.poll()["status"], "needs_reconciliation")
            evaluate.assert_called_once()
        self.assertEqual(
            controller.status()["trials"][0]["state"], "evaluation_pending"
        )

    def test_runtime_failure_cannot_bypass_submission_custody_for_evaluation(self):
        controller = self.evaluation_controller()
        self.change()
        run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "budget_exhausted"
        with patch.object(m.experiment_evaluation, "evaluate") as evaluate:
            report = controller.poll()
            evaluate.assert_not_called()
        self.assertEqual(
            report["result"]["evaluation"]["status"], "not_run_runtime_not_submitted"
        )
        self.assertFalse(report["result"]["independently_accepted"])

    def continuation_controller(self):
        parent = copy.deepcopy(self.plan["experiments"][0])
        parent["evaluation"] = {
            "kind": "rust_acceptance",
            "tests_path": "data/experiment-inputs/tests.rs",
            "tests_sha256": "a" * 64,
        }
        parent["candidate_workflow"] = {
            "version": 1,
            "actor": "Engineer",
            "path": "candidate.rs",
            "max_bytes": 8192,
            "max_attempts": 1,
        }
        child = {
            **copy.deepcopy(parent),
            "id": "repair",
            "after": {"experiment": parent["id"], "outcome": "rejected"},
        }
        final = {
            **copy.deepcopy(child),
            "id": "unused",
            "after": {"experiment": child["id"], "outcome": "rejected"},
        }
        plan = {
            **self.plan,
            "max_model_calls": 3,
            "experiments": [parent, child, final],
        }
        controller = m.Controller(self.root, "continuation", self.api)
        with patch.object(
            m.experiment_evaluation,
            "freeze",
            side_effect=lambda root, spec, trial, read: spec,
        ):
            controller.freeze(plan)
        self.change()
        run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "candidate_submitted"
        with patch.object(
            m.experiment_evaluation,
            "evaluate",
            return_value={
                "kind": "rust_acceptance",
                "passed": False,
                "scope": "fixed suite",
                "native_result": {},
            },
        ):
            self.assertEqual(controller.poll()["status"], "result")
        return controller, plan

    def test_independent_result_can_trigger_frozen_followup_without_input_edit(self):
        controller, plan = self.continuation_controller()
        original = self.input.read_bytes()
        feedback = {"source": "untrusted source", "source_sha256": "a" * 64}
        with patch.object(
            m.experiment_continuation,
            "prepare",
            return_value={
                "decision": "continue",
                "feedback": feedback,
                "predecessor_result_sha256": "b" * 64,
            },
        ):
            report = controller.poll()
        self.assertEqual(report["status"], "dispatched")
        self.assertEqual(self.input.read_bytes(), original)
        intake = [
            body["intake"]
            for _, body in self.api.calls
            if body and body.get("op") == "intake"
        ][-1]
        self.assertEqual(intake["candidate_feedback"], feedback)
        self.assertEqual(intake["idea"], plan["experiments"][1]["task"])
        self.assertEqual(intake["max_model_calls"], 1)
        self.assertEqual(controller.status()["reserved_model_calls"], 2)

    def test_skipped_conditions_consume_no_calls_or_fake_runs(self):
        controller, _ = self.continuation_controller()
        with patch.object(
            m.experiment_continuation,
            "prepare",
            return_value={"decision": "skip", "reason": "condition mismatch"},
        ):
            self.assertEqual(controller.poll()["status"], "skipped")
        self.assertEqual(controller.poll()["status"], "skipped")
        self.assertEqual(controller.poll()["status"], "exhausted")
        status = controller.status()
        self.assertEqual(len(status["trials"]), 1)
        self.assertEqual(len(status["skipped_trials"]), 2)
        self.assertEqual(status["reserved_model_calls"], 1)

    def test_corrupt_predecessor_result_cannot_choose_a_branch(self):
        controller, _ = self.continuation_controller()
        with controller.locked() as db:
            result = json.loads(
                db.execute("SELECT result FROM trials WHERE position=0").fetchone()[0]
            )
            result["independently_accepted"] = True
            db.execute(
                "UPDATE trials SET result=? WHERE position=0", (m.canonical(result),)
            )
            db.commit()
        with self.assertRaisesRegex(ValueError, "result integrity"):
            controller.poll()
        self.assertEqual(self.post_count(), 1)

    def test_changed_predecessor_custody_stops_before_spending_followup_allowance(self):
        controller, _ = self.continuation_controller()
        with patch.object(
            m.experiment_continuation,
            "prepare",
            side_effect=ValueError("source changed"),
        ):
            self.assertEqual(controller.poll()["status"], "needs_reconciliation")
        self.assertEqual(controller.status()["reserved_model_calls"], 1)

    def test_lost_followup_dispatch_does_not_repeat_its_feedback_or_claim(self):
        controller, _ = self.continuation_controller()
        self.api.lost_dispatch_response = True
        with patch.object(
            m.experiment_continuation,
            "prepare",
            return_value={
                "decision": "continue",
                "feedback": {"source": "reference"},
                "predecessor_result_sha256": "a" * 64,
            },
        ):
            first = controller.poll()
        self.assertEqual(first["status"], "needs_reconciliation")
        self.assertEqual(controller.poll()["status"], "result")
        self.assertEqual(self.post_count(), 2)
        self.assertEqual(controller.status()["reserved_model_calls"], 2)

    def test_multistep_repair_reuses_its_own_approved_feedback_binding(self):
        controller, _ = self.continuation_controller()
        feedback = {"source": "first rejected source"}
        with patch.object(
            m.experiment_continuation,
            "prepare",
            return_value={
                "decision": "continue",
                "feedback": feedback,
                "predecessor_result_sha256": "a" * 64,
            },
        ):
            run = controller.poll()["run_id"]
        self.api.runs[run]["status"] = "candidate_submitted"
        with patch.object(
            m.experiment_evaluation,
            "evaluate",
            return_value={
                "kind": "rust_acceptance",
                "passed": False,
                "scope": "fixed suite",
                "native_result": {},
            },
        ):
            self.assertEqual(controller.poll()["status"], "result")
        with patch.object(
            m.experiment_continuation,
            "prepare",
            return_value={"decision": "skip", "reason": "bounded stop"},
        ) as prepare:
            controller.poll()
        self.assertEqual(prepare.call_args.args[3]["candidate_feedback"], feedback)

    def test_feedback_tampering_is_detected_before_observation_or_dispatch(self):
        controller, _ = self.continuation_controller()
        with patch.object(
            m.experiment_continuation,
            "prepare",
            return_value={
                "decision": "continue",
                "feedback": {"source": "reference"},
                "predecessor_result_sha256": "a" * 64,
            },
        ):
            controller.poll()
        count = len(self.api.calls)
        with controller.locked() as db:
            db.execute("UPDATE trial_feedback SET body='{}'")
            db.commit()
        with self.assertRaisesRegex(ValueError, "feedback integrity"):
            controller.poll()
        self.assertEqual(len(self.api.calls), count)

    def test_same_bytes_with_new_mtime_are_not_new_evidence(self):
        ledger = self.controller.folder / "ledger.sqlite3"
        before = ledger.stat().st_mtime_ns
        self.input.touch()
        self.assertEqual(self.controller.poll()["status"], "unchanged")
        self.assertEqual(self.api.calls, [])
        self.assertEqual(ledger.stat().st_mtime_ns, before)

    def test_special_oversized_hardlinked_and_traversal_inputs_fail_before_api(self):
        for name in (
            "data/experiment-inputs/../outside",
            "/absolute",
            "data/experiment-inputs/.hidden",
        ):
            with self.assertRaises(ValueError):
                m.bounded_file(self.root, name)
        hardlink = self.input.with_name("hardlink")
        os.link(self.input, hardlink)
        with self.assertRaises(ValueError):
            self.controller.poll()
        hardlink.unlink()
        with self.assertRaises(ValueError):
            m.bounded_file(self.root, str(self.input.relative_to(self.root)), limit=1)
        self.input.unlink()
        os.mkfifo(self.input)
        with self.assertRaises(ValueError):
            self.controller.poll()
        self.assertEqual(self.api.calls, [])

    def test_untrusted_evidence_and_linked_state_are_rejected(self):
        self.input.unlink()
        self.input.symlink_to(self.root / "absent")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.controller.poll()
        self.assertEqual(self.api.calls, [])
        other = self.root / "data/experiment-loops/linked"
        other.symlink_to(self.controller.folder, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "linked state"):
            m.Controller(self.root, "linked", self.api).poll()


class ValidationTests(unittest.TestCase):
    def test_api_does_not_follow_an_actual_http_redirect(self):
        class Redirect(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(302)
                self.send_header("Location", "http://localhost:1/forbidden")
                self.end_headers()

            def log_message(self, *args):
                pass

        with HTTPServer(("127.0.0.1", 0), Redirect) as server:
            thread = threading.Thread(target=server.serve_forever)
            thread.start()
            try:
                with self.assertRaises(urllib.error.HTTPError) as caught:
                    m.LoopbackAPI("http://127.0.0.1:" + str(server.server_port))(
                        "/api/config"
                    )
                self.assertEqual(caught.exception.code, 302)
                caught.exception.close()
            finally:
                server.shutdown()
                thread.join(5)

    def test_nonloopback_redirect_credentials_and_proxy_targets_are_rejected(self):
        for server in (
            "https://127.0.0.1",
            "http://localhost:8788",
            "http://127.0.0.1/path",
            "http://user@127.0.0.1",
            "http://127.0.0.1?x",
        ):
            with self.assertRaises(ValueError):
                m.LoopbackAPI(server)

    def test_external_authority_and_unbounded_trials_are_rejected(self):
        api = API()
        trial = {
            "id": "one",
            "actor": "Engineer",
            "task": "bounded",
            "criteria": ["review"],
            "tools": [],
            "max_model_calls": 1,
        }
        base = {
            "version": 1,
            "company": "one",
            "evidence_paths": ["data/experiment-inputs/a"],
            "max_model_calls": 1,
            "experiments": [trial],
        }
        mutations = [
            dict(base, max_model_calls=True),
            dict(base, max_model_calls=0),
            dict(base, version=True),
            dict(base, evidence_paths=["config/secrets"]),
            dict(base, extra="authority"),
            dict(base, experiments=[dict(trial, tools=["email_send"])]),
            dict(base, experiments=[dict(trial, max_model_calls=True)]),
            dict(base, experiments=[trial, trial]),
        ]
        for plan in mutations:
            with self.subTest(plan=plan), self.assertRaises(ValueError):
                m.validate_plan(plan, api.config)


if __name__ == "__main__":
    unittest.main()

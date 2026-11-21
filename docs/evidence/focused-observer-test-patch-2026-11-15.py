import json
import os
import pathlib
import tempfile

import showcase_observer as m


def test_terminal_run_counts_calls_and_reports_zero_cost(self):
    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        for status in ("completed", "blocked", "candidate_submitted"):
            calls = []

            def api(path, status=status):
                calls.append(path)
                if path.endswith("/attention"):
                    return {"attention": None}
                return {
                    "status": status,
                    "events": [{"reply": {"usage": {"cost_usd": 0}}}, {"reply": None}],
                }

            report = m.observe_run(
                api,
                "r1",
                root,
                clock=lambda: 0,
                sleep=lambda s: self.fail("sleep"),
                timeout_seconds=1,
                poll_seconds=1,
            )
            self.assertEqual(report["status"], status)
            self.assertEqual(report["runtime_status"], status)
            self.assertEqual(report["model_calls"], 1)
            self.assertEqual(report["provider_cost_usd"], 0)
            self.assertEqual(calls, ["/api/runs/r1", "/api/runs/r1/attention"])
            self.assertEqual(json.loads((root / "report.json").read_text()), report)


def test_pending_attention_returns_attention_and_answered_attention_continues(self):
    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        calls = []

        def api(path):
            calls.append(path)
            if path.endswith("/attention"):
                return {"attention": {"question": "q", "answer": None}}
            return {
                "status": "running",
                "events": [{"reply": {"usage": {"cost_usd": 0}}}],
            }

        report = m.observe_run(
            api,
            "r2",
            root,
            clock=lambda: 0,
            sleep=lambda s: self.fail("sleep"),
            timeout_seconds=1,
            poll_seconds=1,
        )
        self.assertEqual(report["status"], "attention")
        self.assertEqual(report["runtime_status"], "running")
        self.assertEqual(report["model_calls"], 1)
        self.assertEqual(report["provider_cost_usd"], 0)
        self.assertEqual(calls, ["/api/runs/r2", "/api/runs/r2/attention"])
        calls = []
        sleeps = []
        state = {"poll": 0}

        def api2(path):
            calls.append(path)
            if path.endswith("/attention"):
                return {"attention": {"question": "q", "answer": "a"}}
            state["poll"] += 1
            status = "running" if state["poll"] == 1 else "completed"
            return {"status": status, "events": [{"reply": {"usage": {"cost_usd": 0}}}]}

        report = m.observe_run(
            api2,
            "r2",
            root,
            clock=lambda: 0,
            sleep=lambda s: sleeps.append(s),
            timeout_seconds=1,
            poll_seconds=1,
        )
        self.assertEqual(report["status"], "completed")
        self.assertEqual(report["runtime_status"], "completed")
        self.assertEqual(report["model_calls"], 1)
        self.assertEqual(report["provider_cost_usd"], 0)
        self.assertEqual(
            calls,
            [
                "/api/runs/r2",
                "/api/runs/r2/attention",
                "/api/runs/r2",
                "/api/runs/r2/attention",
            ],
        )
        self.assertEqual(sleeps, [1])
        self.assertEqual(json.loads((root / "report.json").read_text()), report)


def test_rejects_invalid_costs_and_unsafe_paths(self):
    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        store = m.EvidenceStore(root)

        def run(cost):
            calls = []

            def api(path):
                calls.append(path)
                if path.endswith("/attention"):
                    return {"attention": None}
                return {
                    "status": "completed",
                    "events": [{"reply": {"usage": {"cost_usd": cost}}}],
                }

            return m.observe_run(
                api,
                "r3",
                root,
                clock=lambda: 0,
                sleep=lambda s: self.fail("sleep"),
                timeout_seconds=1,
                poll_seconds=1,
            ), calls

        for bad in (True, False, 1, -1, float("nan"), float("inf"), "0", None):
            with self.assertRaises(ValueError):
                run(bad)
        for bad in ({}, {"usage": {}}):
            with self.assertRaises(ValueError):

                def api(path):
                    if path.endswith("/attention"):
                        return {"attention": None}
                    return {"status": "completed", "events": [{"reply": bad}]}

                m.observe_run(
                    api,
                    "r3",
                    root,
                    clock=lambda: 0,
                    sleep=lambda s: self.fail("sleep"),
                    timeout_seconds=1,
                    poll_seconds=1,
                )
        report, calls = run(0)
        self.assertEqual(report["status"], "completed")
        self.assertEqual(report["runtime_status"], "completed")
        self.assertEqual(report["model_calls"], 1)
        self.assertEqual(report["provider_cost_usd"], 0)
        self.assertEqual(calls, ["/api/runs/r3", "/api/runs/r3/attention"])
        for name in ("", ".", "..", "a/b", "a\\b"):
            with self.assertRaises(ValueError):
                store.write_if_changed(name, {"ok": True})
        link = root / "link"
        target = root / "target"
        target.write_text("x")
        os.symlink(target, link)
        with self.assertRaises(ValueError):
            store.write_if_changed("link", {"ok": True})
        for name in ("a..b", "-", "_", "a b"):
            store.write_if_changed(name, {"name": name})
            self.assertEqual(json.loads((root / name).read_text()), {"name": name})

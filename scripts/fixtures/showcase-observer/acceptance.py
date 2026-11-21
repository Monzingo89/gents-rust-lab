import json
import os
import tempfile
import unittest
from pathlib import Path

import showcase_observer as m


class Tests(unittest.TestCase):
    def test_terminal_counts_current_snapshot_and_never_dispatches(self):
        calls = []

        def api(path):
            calls.append(path)
            return (
                {
                    "status": "completed",
                    "events": [{"reply": {"usage": {"cost_usd": 0}}}],
                }
                if path.endswith("/r")
                else {"attention": None}
            )

        with tempfile.TemporaryDirectory() as d:
            r = m.observe_run(
                api,
                "r",
                Path(d),
                clock=lambda: 0,
                sleep=lambda _: self.fail("terminal slept"),
            )
            self.assertEqual(r["status"], "completed")
            self.assertEqual(r["runtime_status"], "completed")
            self.assertEqual(r["model_calls"], 1)
            self.assertEqual(r["provider_cost_usd"], 0)
            self.assertTrue((Path(d) / "observed-run.json").is_file())
            self.assertTrue(
                all(p in ["/api/runs/r", "/api/runs/r/attention"] for p in calls)
            )

    def test_unchanged_snapshot_is_cached_but_changed_one_is_persisted(self):
        with tempfile.TemporaryDirectory() as d:
            store = m.EvidenceStore(Path(d))
            self.assertTrue(store.write_if_changed("snapshot.json", {"b": 2, "a": 1}))
            p = Path(d) / "snapshot.json"
            os.utime(p, ns=(1_000_000_000, 1_000_000_000))
            self.assertFalse(store.write_if_changed("snapshot.json", {"a": 1, "b": 2}))
            self.assertEqual(p.stat().st_mtime_ns, 1_000_000_000)
            self.assertTrue(store.write_if_changed("snapshot.json", {"a": 2}))
            self.assertEqual(json.loads(p.read_text()), {"a": 2})

    def test_timeout_does_not_mark_running_as_complete(self):
        tick = [0]

        def sleep(_):
            tick[0] += 1

        def api(path):
            return (
                {"attention": None}
                if path.endswith("/attention")
                else {"status": "running", "events": []}
            )

        with tempfile.TemporaryDirectory() as d:
            r = m.observe_run(
                api,
                "r",
                Path(d),
                clock=lambda: tick[0],
                sleep=sleep,
                timeout_seconds=2,
                poll_seconds=1,
            )
            self.assertEqual(r["status"], "observation_expired")
            self.assertEqual(r["runtime_status"], "running")
            self.assertGreaterEqual(r["elapsed_seconds"], 2)

    def test_unanswered_attention_stops_observation(self):
        def api(path):
            return (
                {"attention": {"question": "Inspect a retained effect", "answer": None}}
                if path.endswith("/attention")
                else {"status": "running", "events": []}
            )

        with tempfile.TemporaryDirectory() as d:
            r = m.observe_run(
                api,
                "r",
                Path(d),
                clock=lambda: 0,
                sleep=lambda _: self.fail("attention slept"),
            )
            self.assertEqual(r["status"], "attention")
            self.assertEqual(r["runtime_status"], "running")

    def test_provider_cost_fails_closed(self):
        for cost in [1, -1, float("nan"), float("inf")]:

            def api(path):
                return (
                    {"attention": None}
                    if path.endswith("/attention")
                    else {
                        "status": "completed",
                        "events": [{"reply": {"usage": {"cost_usd": cost}}}],
                    }
                )

            with (
                tempfile.TemporaryDirectory() as d,
                self.assertRaises((ValueError, RuntimeError, AssertionError)),
            ):
                m.observe_run(api, "r", Path(d), clock=lambda: 0, sleep=lambda _: None)

    def test_unsafe_evidence_names_are_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            store = m.EvidenceStore(Path(d))
            for name in ["../outside.json", "/tmp/escape.json", "sub/file.json"]:
                with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                    store.write_if_changed(name, {})


class Extra(unittest.TestCase):
    def test_cache_checks_disk_content_not_stale_memory_or_mtime(self):
        with tempfile.TemporaryDirectory() as d:
            s = m.EvidenceStore(Path(d))
            s.write_if_changed("s.json", {"v": 1})
            p = Path(d) / "s.json"
            p.write_text('{"v":2}')
            self.assertTrue(s.write_if_changed("s.json", {"v": 1}))
            self.assertEqual(json.loads(p.read_text()), {"v": 1})
            p.unlink()
            self.assertTrue(s.write_if_changed("s.json", {"v": 1}))

    def test_reject_invalid_names_nonfinite_and_symlink_targets(self):
        with tempfile.TemporaryDirectory() as d:
            s = m.EvidenceStore(Path(d))
            for name in ["", ".", "..", "a\\b", "a/b"]:
                with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                    s.write_if_changed(name, {})
            with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                s.write_if_changed("bad.json", {"x": float("nan")})
            outside = Path(d) / "outside"
            outside.write_text("ORIGINAL")
            (Path(d) / "link.json").symlink_to(outside)
            with self.assertRaises((ValueError, RuntimeError, AssertionError)):
                s.write_if_changed("link.json", {})
            self.assertEqual(outside.read_text(), "ORIGINAL")

    def test_repeated_snapshots_count_once_and_current_snapshot_replaces(self):
        ticks = [0]
        seen = []

        def api(p):
            seen.append(p)
            if p.endswith("/attention"):
                return {"attention": None}
            return {
                "status": "running" if ticks[0] < 2 else "completed",
                "events": [{"reply": {"usage": {"cost_usd": 0}}}],
            }

        def sleep(_):
            ticks[0] += 1

        with tempfile.TemporaryDirectory() as d:
            r = m.observe_run(
                api,
                "task-1",
                Path(d),
                clock=lambda: ticks[0],
                sleep=sleep,
                timeout_seconds=10,
                poll_seconds=1,
            )
            self.assertEqual(r["status"], "completed")
            self.assertEqual(r["model_calls"], 1)
            self.assertEqual(r["elapsed_seconds"], 2)
            self.assertTrue(
                set(seen) <= {"/api/runs/task-1", "/api/runs/task-1/attention"}
            )

    def test_unknown_costs_fail_closed(self):
        for reply in [
            {"usage": {}},
            {},
            {"usage": {"cost_usd": "0"}},
            {"usage": {"cost_usd": False}},
            {"usage": {"cost_usd": None}},
        ]:

            def api(p):
                return (
                    {"attention": None}
                    if p.endswith("/attention")
                    else {"status": "completed", "events": [{"reply": reply}]}
                )

            with (
                tempfile.TemporaryDirectory() as d,
                self.assertRaises(
                    (ValueError, RuntimeError, AssertionError, TypeError)
                ),
            ):
                m.observe_run(api, "r", Path(d), clock=lambda: 0, sleep=lambda _: None)

    def test_run_id_and_poll_budget_validate_before_api(self):
        for rid in ["", "../x", "x/y", "x?query", "x#fragment", "a\\b"]:
            with (
                tempfile.TemporaryDirectory() as d,
                self.assertRaises((ValueError, RuntimeError, AssertionError)),
            ):
                m.observe_run(
                    lambda _: self.fail("invalid path reached api"),
                    rid,
                    Path(d),
                    clock=lambda: 0,
                    sleep=lambda _: None,
                )
        for timeout, poll in [(float("nan"), 1), (float("inf"), 1), (1, 0), (1, -1)]:
            with (
                tempfile.TemporaryDirectory() as d,
                self.assertRaises((ValueError, RuntimeError, AssertionError)),
            ):
                m.observe_run(
                    lambda _: self.fail("invalid time reached api"),
                    "r",
                    Path(d),
                    timeout_seconds=timeout,
                    poll_seconds=poll,
                )

    def test_original_evidence_survives_api_failure(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "observed-run.json"
            p.write_text('{"kept":true}')

            def api(_):
                raise OSError("offline")

            with self.assertRaises(OSError):
                m.observe_run(api, "r", Path(d))
            self.assertEqual(json.loads(p.read_text()), {"kept": True})


class RuntimeStates(unittest.TestCase):
    def test_all_nonactive_statuses_return_without_sleep(self):
        for status in [
            "candidate_submitted",
            "blocked",
            "rejected",
            "new_terminal_state",
        ]:

            def api(path):
                return (
                    {"attention": None}
                    if path.endswith("/attention")
                    else {"status": status, "events": []}
                )

            with tempfile.TemporaryDirectory() as d:
                result = m.observe_run(
                    api,
                    "r",
                    Path(d),
                    clock=lambda: 0,
                    sleep=lambda _: self.fail("terminal slept"),
                )
                self.assertEqual(result["status"], status)
                self.assertEqual(result["runtime_status"], status)

    def test_queued_is_active_and_timeout_is_not_completion(self):
        def api(path):
            return (
                {"attention": None}
                if path.endswith("/attention")
                else {"status": "queued", "events": []}
            )

        with tempfile.TemporaryDirectory() as d:
            result = m.observe_run(
                api,
                "r",
                Path(d),
                clock=lambda: 0,
                sleep=lambda _: self.fail("expired queued slept"),
                timeout_seconds=0,
            )
            self.assertEqual(result["status"], "observation_expired")
            self.assertEqual(result["runtime_status"], "queued")


import unittest


class Priority(unittest.TestCase):
    def test_nonactive_precedes_timeout(self):
        for status in ["candidate_submitted", "blocked", "new_terminal_state"]:

            def api(path):
                return (
                    {"attention": None}
                    if path.endswith("/attention")
                    else {"status": status, "events": []}
                )

            with tempfile.TemporaryDirectory() as d:
                result = m.observe_run(
                    api,
                    "r",
                    Path(d),
                    clock=lambda: 0,
                    timeout_seconds=0,
                    sleep=lambda _: self.fail("terminal slept"),
                )
                self.assertEqual(result["status"], status)
                self.assertEqual(result["runtime_status"], status)

    def test_nonactive_precedes_unanswered_attention(self):
        for status in ["candidate_submitted", "blocked", "new_terminal_state"]:

            def api(path):
                return (
                    {"attention": {"question": "q", "answer": None}}
                    if path.endswith("/attention")
                    else {"status": status, "events": []}
                )

            with tempfile.TemporaryDirectory() as d:
                result = m.observe_run(
                    api,
                    "r",
                    Path(d),
                    clock=lambda: 0,
                    sleep=lambda _: self.fail("terminal slept"),
                )
                self.assertEqual(result["status"], status)
                self.assertEqual(result["runtime_status"], status)

import json
import math
import os
import pathlib
import tempfile
import unittest
from unittest import mock

import showcase_observer as m


def _run_response(status, events):
    return {"status": status, "events": events}


def _attention_response(attention):
    return {"attention": attention}


def _fake_api(routes):
    calls = []

    def api(path):
        calls.append(path)
        if path not in routes:
            raise AssertionError(f"unexpected api path: {path}")
        return routes[path]

    api.calls = calls
    return api


def _fake_clock(start, step):
    state = {"value": start}

    def clock():
        state["value"] += step
        return state["value"]

    clock.state = state
    return clock


def _fake_sleep():
    calls = []

    def sleep(seconds):
        calls.append(seconds)

    sleep.calls = calls
    return sleep


class ShowcaseObserverTests(unittest.TestCase):
    def test_evidence_store_preserves_mtime_and_notices_external_changes(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = pathlib.Path(tmp)
            store = m.EvidenceStore(directory)
            target = directory / "test.json"

            first = {"b": 2, "a": 1}
            self.assertTrue(store.write_if_changed("test.json", first))
            first_bytes = target.read_bytes()
            first_mtime = target.stat().st_mtime_ns

            self.assertFalse(store.write_if_changed("test.json", {"a": 1, "b": 2}))
            self.assertEqual(target.read_bytes(), first_bytes)
            self.assertEqual(target.stat().st_mtime_ns, first_mtime)

            target.write_bytes(b'{"a": 999, "b": 2}')
            self.assertTrue(store.write_if_changed("test.json", first))
            self.assertEqual(target.read_bytes(), first_bytes)

            target.unlink()
            self.assertTrue(store.write_if_changed("test.json", first))
            self.assertEqual(target.read_bytes(), first_bytes)

    def test_evidence_store_atomic_replace_failure_keeps_original_and_cleans_temp(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = pathlib.Path(tmp)
            store = m.EvidenceStore(directory)
            target = directory / "test.json"
            original = b'{"original": true}'
            target.write_bytes(original)

            with mock.patch.object(m.os, "replace", side_effect=OSError("replace failed")):
                with self.assertRaises(OSError):
                    store.write_if_changed("test.json", {"changed": True})

            self.assertEqual(target.read_bytes(), original)
            leftovers = [p.name for p in directory.iterdir() if p.name != "test.json"]
            self.assertEqual(leftovers, [])

    def test_terminal_run_counts_calls_and_reports_zero_cost(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            for status in ('completed', 'blocked', 'candidate_submitted'):
                calls = []
                def api(path, status=status):
                    calls.append(path)
                    if path.endswith('/attention'):
                        return {'attention': None}
                    return {'status': status, 'events': [{'reply': {'usage': {'cost_usd': 0}}}, {'reply': None}]}
                report = m.observe_run(api, 'r1', root, clock=lambda: 0, sleep=lambda s: self.fail('sleep'), timeout_seconds=1, poll_seconds=1)
                self.assertEqual(report['status'], status)
                self.assertEqual(report['runtime_status'], status)
                self.assertEqual(report['model_calls'], 1)
                self.assertEqual(report['provider_cost_usd'], 0)
                self.assertEqual(calls, ['/api/runs/r1', '/api/runs/r1/attention'])
                self.assertEqual(json.loads((root / 'report.json').read_text()), report)

    def test_timeout_preserves_running_runtime_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = pathlib.Path(tmp)
            run_path = "/api/runs/run-2"
            attention_path = "/api/runs/run-2/attention"
            api = _fake_api({
                run_path: _run_response("running", []),
                attention_path: _attention_response(None),
            })
            clock = _fake_clock(0.0, 10.0)
            sleep = _fake_sleep()

            report = m.observe_run(
                api,
                "run-2",
                directory,
                clock=clock,
                sleep=sleep,
                timeout_seconds=5,
                poll_seconds=1,
            )

            self.assertEqual(report["status"], "observation_expired")
            self.assertEqual(report["runtime_status"], "running")
            self.assertGreaterEqual(report["elapsed_seconds"], 5.0)
            self.assertGreaterEqual(api.calls.count(run_path), 1)

            saved_report = json.loads((directory / "report.json").read_text(encoding="utf-8"))
            self.assertEqual(saved_report["status"], "observation_expired")
            self.assertEqual(saved_report["runtime_status"], "running")

    def test_pending_attention_returns_attention_and_answered_attention_continues(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            calls = []
            def api(path):
                calls.append(path)
                if path.endswith('/attention'):
                    return {'attention': {'question': 'q', 'answer': None}}
                return {'status': 'running', 'events': [{'reply': {'usage': {'cost_usd': 0}}}]}
            report = m.observe_run(api, 'r2', root, clock=lambda: 0, sleep=lambda s: self.fail('sleep'), timeout_seconds=1, poll_seconds=1)
            self.assertEqual(report['status'], 'attention')
            self.assertEqual(report['runtime_status'], 'running')
            self.assertEqual(report['model_calls'], 1)
            self.assertEqual(report['provider_cost_usd'], 0)
            self.assertEqual(calls, ['/api/runs/r2', '/api/runs/r2/attention'])
            calls = []
            sleeps = []
            state = {'poll': 0}
            def api2(path):
                calls.append(path)
                if path.endswith('/attention'):
                    return {'attention': {'question': 'q', 'answer': 'a'}}
                state['poll'] += 1
                status = 'running' if state['poll'] == 1 else 'completed'
                return {'status': status, 'events': [{'reply': {'usage': {'cost_usd': 0}}}]}
            report = m.observe_run(api2, 'r2', root, clock=lambda: 0, sleep=lambda s: sleeps.append(s), timeout_seconds=1, poll_seconds=1)
            self.assertEqual(report['status'], 'completed')
            self.assertEqual(report['runtime_status'], 'completed')
            self.assertEqual(report['model_calls'], 1)
            self.assertEqual(report['provider_cost_usd'], 0)
            self.assertEqual(calls, ['/api/runs/r2', '/api/runs/r2/attention', '/api/runs/r2', '/api/runs/r2/attention'])
            self.assertEqual(sleeps, [1])
            self.assertEqual(json.loads((root / 'report.json').read_text()), report)

    def test_rejects_invalid_costs_and_unsafe_paths(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            store = m.EvidenceStore(root)
            def run(cost):
                calls = []
                def api(path):
                    calls.append(path)
                    if path.endswith('/attention'):
                        return {'attention': None}
                    return {'status': 'completed', 'events': [{'reply': {'usage': {'cost_usd': cost}}}]}
                return m.observe_run(api, 'r3', root, clock=lambda: 0, sleep=lambda s: self.fail('sleep'), timeout_seconds=1, poll_seconds=1), calls
            for bad in (True, False, 1, -1, float('nan'), float('inf'), '0', None):
                with self.assertRaises(ValueError):
                    run(bad)
            for bad in ({}, {'usage': {}}):
                with self.assertRaises(ValueError):
                    def api(path):
                        if path.endswith('/attention'):
                            return {'attention': None}
                        return {'status': 'completed', 'events': [{'reply': bad}]}
                    m.observe_run(api, 'r3', root, clock=lambda: 0, sleep=lambda s: self.fail('sleep'), timeout_seconds=1, poll_seconds=1)
            report, calls = run(0)
            self.assertEqual(report['status'], 'completed')
            self.assertEqual(report['runtime_status'], 'completed')
            self.assertEqual(report['model_calls'], 1)
            self.assertEqual(report['provider_cost_usd'], 0)
            self.assertEqual(calls, ['/api/runs/r3', '/api/runs/r3/attention'])
            for name in ('', '.', '..', 'a/b', 'a\\b'):
                with self.assertRaises(ValueError):
                    store.write_if_changed(name, {'ok': True})
            link = root / 'link'
            target = root / 'target'
            target.write_text('x')
            os.symlink(target, link)
            with self.assertRaises(ValueError):
                store.write_if_changed('link', {'ok': True})
            for name in ('a..b', '-', '_', 'a b'):
                store.write_if_changed(name, {'name': name})
                self.assertEqual(json.loads((root / name).read_text()), {'name': name})


if __name__ == "__main__":
    unittest.main()

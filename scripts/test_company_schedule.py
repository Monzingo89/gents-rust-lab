import datetime as dt
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch
import company_schedule as schedule
import company_tools


class ScheduleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="gents-schedule-")
        self.root = Path(self.temp.name)
        self.patch = patch.object(company_tools, "ROOT", self.root)
        self.patch.start()
        self.now = dt.datetime(2026, 10, 22, 10, tzinfo=dt.timezone.utc)
        self.args = SimpleNamespace(company="one", server="http://127.0.0.1:8788")
        self.spec = dict(op="create", id="daily-test", time="06:00", timezone="America/Chicago", task="Perform the authorized recurring task", acceptance="Actual evidence required")
        self.jobs = [{"run_id": "parent", "state": "completed", "digest": "frozen", "approval": json.dumps({"sha256": "frozen"})}]

    def tearDown(self):
        self.patch.stop()
        self.temp.cleanup()

    def create(self):
        with patch.object(schedule, "clock", return_value=self.now):
            return schedule.operation("one", "parent", self.spec)

    def test_create_is_persistent_idempotent_and_does_not_overwrite_scope(self):
        first = self.create()
        self.assertEqual(first, self.create())
        self.assertEqual(first["schedules"][0]["next_run_at"], "2026-10-22T11:00:00+00:00")
        with self.assertRaises(ValueError):
            schedule.operation("one", "parent", dict(self.spec, task="Changed task"))
        self.assertEqual(schedule.operation("two", "other", {"op": "list"})["schedules"], [])

    def test_timezone_dst_changes_and_past_times_move_to_tomorrow(self):
        spec = dict(self.spec, time="08:00")
        before = dt.datetime(2026, 10, 31, 14, tzinfo=dt.timezone.utc)
        self.assertEqual(schedule.next_at(spec, before).isoformat(), "2026-11-01T14:00:00+00:00")
        self.assertEqual(schedule.at(spec, dt.date(2026, 10, 31)).hour, 13)
        self.assertEqual(schedule.at(spec, dt.date(2026, 11, 1)).hour, 14)
        self.assertEqual(schedule.next_at(dict(spec, first_date="2026-11-02"), before).isoformat(), "2026-11-02T14:00:00+00:00")

    def test_update_requires_original_creator_and_preserves_pause_and_occurrences(self):
        self.create()
        schedule.operation("one", "parent", {"op": "pause", "id": "daily-test"})
        with schedule.connect("one") as db:
            db.execute("INSERT INTO occurrences VALUES('daily-test','2026-10-21','old-run','completed')")
        updated = dict(self.spec, op="update", time="03:00", task="Prepare verified report before 08:00 delivery", acceptance="Fetch sources\nVerify report\nInspect email receipt")
        with self.assertRaises(ValueError):
            schedule.operation("one", "child", updated)
        with patch.object(schedule, "clock", return_value=self.now):
            state = schedule.operation("one", "parent", updated)
        self.assertEqual(len(state["schedules"]), 1)
        self.assertEqual(state["schedules"][0]["time"], "03:00")
        self.assertFalse(state["schedules"][0]["enabled"])
        self.assertEqual(state["occurrences"][0]["run_id"], "old-run")
        with self.assertRaisesRegex(ValueError, "no changes"):
            schedule.operation("one", "parent", updated)
        with self.assertRaises(ValueError):
            schedule.operation("one", "parent", dict(updated, id="missing"))

    def test_pause_resume_and_invalid_input(self):
        self.create()
        self.assertFalse(schedule.operation("one", "parent", {"op": "pause", "id": "daily-test"})["schedules"][0]["enabled"])
        self.assertTrue(schedule.operation("one", "parent", {"op": "resume", "id": "daily-test"})["schedules"][0]["enabled"])
        for field, value in [("id", "../escape"), ("time", "25:00"), ("task", ""), ("timezone", "missing/place")]:
            with self.assertRaises((ValueError, KeyError)):
                schedule.operation("one", "parent", dict(self.spec, **{field: value}))

    def company(self, args, op, **data):
        if op == "intake":
            self.jobs.append(dict(run_id=data["run_id"], state="draft", digest="child", intake=data["intake"]))
        if op == "approve":
            next(j for j in self.jobs if j["run_id"] == data["run_id"])["state"] = "approved"
        return {"jobs": self.jobs}

    def test_poll_admits_once_and_retry_after_ambiguous_api_result_does_not_duplicate(self):
        self.create()
        due = self.now + dt.timedelta(hours=2)
        calls = []
        def api(args, path, body):
            calls.append(body)
            next(j for j in self.jobs if j["run_id"] == body["run_id"])["state"] = "running"
            raise TimeoutError("simulated response lost after admission")
        with patch.object(schedule.gents, "company", side_effect=self.company), patch.object(schedule.gents, "api", side_effect=api):
            with self.assertRaises(TimeoutError):
                schedule.poll(self.args, due)
            schedule.poll(self.args, due)
            schedule.poll(self.args, due)
        self.assertEqual(len(calls), 1)
        self.assertEqual(len(self.jobs), 2)
        self.assertIn("2026-10-22 (America/Chicago)", calls[0]["task"])
        self.assertEqual(schedule.operation("one", "parent", {"op": "list"})["occurrences"][0]["state"], "started")

    def test_busy_company_or_missing_parent_approval_cannot_start(self):
        self.create()
        with patch.object(schedule.gents, "company", side_effect=self.company), patch.object(schedule.gents, "api") as api:
            self.jobs[0]["state"] = "running"
            schedule.poll(self.args, self.now + dt.timedelta(hours=2))
            self.jobs[0]["state"] = "completed"
            self.jobs[0]["approval"] = None
            schedule.poll(self.args, self.now + dt.timedelta(hours=2))
            api.assert_not_called()

    def test_old_missed_days_skip_without_catchup_burst(self):
        self.create()
        with patch.object(schedule.gents, "company", side_effect=self.company), patch.object(schedule.gents, "api") as api:
            # Offline three days; today's preparation time has not arrived.
            schedule.poll(self.args, self.now + dt.timedelta(days=3))
            api.assert_not_called()
        self.assertEqual(schedule.operation("one", "parent", {"op": "list"})["schedules"][0]["next_run_at"], "2026-10-25T11:00:00+00:00")


if __name__ == "__main__":
    unittest.main()

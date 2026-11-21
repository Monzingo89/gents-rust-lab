#!/usr/bin/env python3
"""Durable daily triggers for local company agents. No report generation here."""

import argparse
import datetime as dt
import json
import os
import re
import time
import uuid
from contextlib import contextmanager
from zoneinfo import ZoneInfo

import company_tools
import gents
import local_sqlite as sqlite3

UTC = dt.UTC


def clock():
    return dt.datetime.now(UTC)


@contextmanager
def connect(company):
    folder = company_tools.confined(company_tools.ROOT, f"data/companies/{company}")
    folder.mkdir(parents=True, exist_ok=True)
    path = company_tools.confined(folder, "schedules.sqlite3")
    db = sqlite3.connect(path, timeout=10)
    db.row_factory = sqlite3.Row
    db.execute(
        "CREATE TABLE IF NOT EXISTS schedules(id TEXT PRIMARY KEY,spec TEXT NOT NULL,source_run TEXT NOT NULL,enabled INTEGER NOT NULL,created_at TEXT NOT NULL,next_at TEXT NOT NULL)"
    )
    db.execute(
        "CREATE TABLE IF NOT EXISTS occurrences(schedule_id TEXT,date TEXT,run_id TEXT UNIQUE,state TEXT NOT NULL,PRIMARY KEY(schedule_id,date))"
    )
    db.commit()
    try:
        with db:
            yield db
    finally:
        db.close()


def at(spec, date):
    return dt.datetime.combine(
        date, dt.time.fromisoformat(spec["time"]), ZoneInfo(spec["timezone"])
    ).astimezone(UTC)


def next_at(spec, now):
    date = now.astimezone(ZoneInfo(spec["timezone"])).date()
    if spec.get("first_date"):
        date = max(date, dt.date.fromisoformat(spec["first_date"]))
    candidate = at(spec, date)
    return candidate if candidate > now else at(spec, date + dt.timedelta(days=1))


def operation(company, source_run, args):
    now = clock()
    op = args.get("op", "list")
    with connect(company) as db:
        if op in ("create", "update"):
            identifier = args.get("id", "")
            if not re.fullmatch(r"[a-z0-9-]{1,60}", identifier):
                raise ValueError(
                    "schedule id must be 1..60 lowercase letters, digits or dashes"
                )
            spec = {
                key: args.get(key)
                for key in (
                    "id",
                    "time",
                    "timezone",
                    "first_date",
                    "task",
                    "acceptance",
                )
            }
            if not isinstance(spec["time"], str) or not re.fullmatch(
                r"(?:[01]\d|2[0-3]):[0-5]\d", spec["time"]
            ):
                raise ValueError("daily start time must be HH:MM in the named timezone")
            ZoneInfo(spec["timezone"])
            if spec["first_date"] is not None and (
                not isinstance(spec["first_date"], str)
                or dt.date.fromisoformat(spec["first_date"]).isoformat()
                != spec["first_date"]
            ):
                raise ValueError(
                    "first_date must be YYYY-MM-DD in the schedule timezone"
                )
            for key in ("task", "acceptance"):
                if (
                    not isinstance(spec[key], str)
                    or not spec[key].strip()
                    or len(spec[key].encode()) > 8500
                ):
                    raise ValueError(f"{key} must be nonempty text up to 8500 bytes")
            previous = db.execute(
                "SELECT spec,source_run FROM schedules WHERE id=?", (identifier,)
            ).fetchone()
            previous_spec = previous[0] if previous else None
            if previous_spec is not None and not isinstance(
                previous_spec, (str, bytes, bytearray)
            ):
                raise ValueError("invalid stored schedule specification")
            if op == "update":
                if not previous or previous_spec is None or previous[1] != source_run:
                    raise ValueError(
                        "only the creating run can update its existing schedule after admin review"
                    )
                if json.loads(previous_spec) == spec:
                    raise ValueError(
                        "schedule update makes no changes; inspect the existing result and change the incorrect preparation time or recurring requirements, or advance another unmet task criterion"
                    )
                db.execute(
                    "UPDATE schedules SET spec=?,next_at=? WHERE id=?",
                    (json.dumps(spec), next_at(spec, now).isoformat(), identifier),
                )
            elif previous and (
                previous_spec is None
                or json.loads(previous_spec) != spec
                or previous[1] != source_run
            ):
                raise ValueError(
                    "schedule already exists; inspect/pause it rather than overwrite its authorization"
                )
            if not previous:
                count_row = db.execute("SELECT count(*) FROM schedules").fetchone()
                schedule_count = count_row[0] if count_row is not None else 0
                if not isinstance(schedule_count, int):
                    raise ValueError("invalid stored schedule count")
                if schedule_count >= 20:
                    raise ValueError("company schedule limit reached")
            db.execute(
                "INSERT OR IGNORE INTO schedules VALUES(?,?,?,1,?,?)",
                (
                    identifier,
                    json.dumps(spec),
                    source_run,
                    now.isoformat(),
                    next_at(spec, now).isoformat(),
                ),
            )
        elif op in ("pause", "resume"):
            row = db.execute(
                "SELECT spec FROM schedules WHERE id=?", (args.get("id"),)
            ).fetchone()
            if row is None:
                raise ValueError("unknown schedule")
            stored_spec = row[0]
            if not isinstance(stored_spec, (str, bytes, bytearray)):
                raise ValueError("invalid stored schedule spec")
            db.execute(
                "UPDATE schedules SET enabled=?,next_at=? WHERE id=?",
                (
                    int(op == "resume"),
                    next_at(json.loads(stored_spec), now).isoformat(),
                    args["id"],
                ),
            )
        elif op != "list":
            raise ValueError(
                "schedule op must be create, update, list, pause or resume"
            )
        rows = db.execute("SELECT * FROM schedules ORDER BY id").fetchall()
        return {
            "schedules": [
                dict(
                    json.loads(r["spec"]),
                    source_run=r["source_run"],
                    enabled=bool(r["enabled"]),
                    next_run_at=r["next_at"],
                )
                for r in rows
            ],
            "occurrences": [
                {
                    "schedule_id": r[0],
                    "date": r[1],
                    "run_id": r[2],
                    "state": r[3],
                }
                for r in db.execute(
                    "SELECT schedule_id,date,run_id,state FROM occurrences ORDER BY rowid DESC LIMIT 20"
                )
            ],
            "now_utc": now.isoformat(),
            "requires": "Keep npm run company and the computer running. Start time is not guaranteed completion time. Missed old dates are skipped, not replayed in a burst.",
        }


def poll(args, now=None):
    now = now or clock()
    with connect(args.company) as db:
        rows = db.execute(
            "SELECT * FROM schedules WHERE enabled=1 ORDER BY next_at"
        ).fetchall()
        if not rows:
            return
        state = gents.company(args, "status")
        for job in state["jobs"]:
            if job["state"] not in ("draft", "approved", "running"):
                db.execute(
                    "UPDATE occurrences SET state=? WHERE run_id=?",
                    (job["state"], job["run_id"]),
                )
        for row in rows:
            if dt.datetime.fromisoformat(row["next_at"]) > now:
                continue
            spec = json.loads(row["spec"])
            date = now.astimezone(ZoneInfo(spec["timezone"])).date()
            if at(spec, date) > now:
                # We were offline on an earlier date. Do not send old reports now.
                db.execute(
                    "UPDATE schedules SET next_at=? WHERE id=?",
                    (at(spec, date).isoformat(), row["id"]),
                )
                continue
            identity = f"gents-schedule:{args.company}:{row['id']}:{date.isoformat()}"
            run = str(uuid.uuid5(uuid.NAMESPACE_URL, identity))
            existing = next((j for j in state["jobs"] if j["run_id"] == run), None)
            if any(
                j["state"] == "running" and j["run_id"] != run for j in state["jobs"]
            ):
                continue
            # Recheck inherited authority before any intake or retry.
            parent = next(
                (j for j in state["jobs"] if j["run_id"] == row["source_run"]), None
            )
            approval = json.loads(parent.get("approval") or "{}") if parent else {}
            if (
                not parent
                or parent["state"] in ("draft", "cancelled", "blocked", "inactive")
                or approval.get("sha256") != parent["digest"]
            ):
                continue
            task = f"{spec['task']}\n\nRecurring occurrence: {date.isoformat()} ({spec['timezone']}). Daily preparation start: {spec['time']}. Use resources for the actual current clock. This occurrence must not create another recurring schedule."
            if existing is None:
                intake = {
                    "idea": task,
                    "acceptance": spec["acceptance"],
                    "exclusions": "Unrequested spending, secrets, unrelated scope, duplicate recurring schedules",
                    "timeline": f"Occurrence {date.isoformat()} in {spec['timezone']}",
                    "company_signer": "Inherited admin-reviewed recurring task",
                    "provider_signer": "Local Gents agents",
                    "budget_usd": 0,
                    "deliverable_type": "agent",
                }
                state = gents.company(args, "intake", run_id=run, intake=intake)
                existing = next(j for j in state["jobs"] if j["run_id"] == run)
            if existing["state"] == "draft":
                gents.company(args, "approve", run_id=run, digest=existing["digest"])
            db.execute(
                "INSERT OR IGNORE INTO occurrences VALUES(?,?,?,'admitting')",
                (row["id"], date.isoformat(), run),
            )
            db.commit()
            if existing["state"] in ("draft", "approved"):
                gents.api(
                    args,
                    "/api/runs",
                    {
                        "company_id": args.company,
                        "run_id": run,
                        "task": existing["intake"]["idea"],
                        "consent": False,
                        "definition_of_done": existing["intake"][
                            "acceptance"
                        ].splitlines(),
                    },
                )
            db.execute(
                "UPDATE occurrences SET state=? WHERE run_id=?",
                (
                    "started"
                    if existing["state"] in ("draft", "approved", "running")
                    else existing["state"],
                    run,
                ),
            )
            db.execute(
                "UPDATE schedules SET next_at=? WHERE id=?",
                (at(spec, date + dt.timedelta(days=1)).isoformat(), row["id"]),
            )
            db.commit()
            print(f"Scheduled local-agent run admitted: {run}", flush=True)
            break


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", default="http://127.0.0.1:8788")
    args = parser.parse_args()
    parent = os.getppid()
    while os.getppid() == parent:
        for path in (company_tools.ROOT / "data/companies").glob("*/schedules.sqlite3"):
            try:
                if not re.fullmatch(r"[a-z0-9-]{1,80}", path.parent.name):
                    continue
                args.company = path.parent.name
                poll(args)
            except Exception as error:
                # Never echo provider bodies or credentials from an integration.
                print(
                    f"Company scheduler waiting ({type(error).__name__}).", flush=True
                )
        time.sleep(15)


if __name__ == "__main__":
    main()

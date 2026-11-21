"""Owner-frozen, evidence-triggered local trials. Runtime outcomes are not acceptance.

No model selects authority, trials, triggers or budgets. No retries of uncertain
mutations. The private state directory and repository are owner-controlled.
"""

import argparse
import copy
import fcntl
import hashlib
import json
import os
import re
import stat
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from contextlib import contextmanager
from pathlib import Path

import experiment_continuation
import experiment_evaluation
import local_sqlite as sqlite3
from showcase_observer import observe_run

ROOT = Path(__file__).resolve().parents[1]
IDENTIFIER = re.compile(r"[a-z0-9-]{1,60}\Z")
HOLD = {"interrupted", "needs_reconciliation", "timed_out"}
TERMINAL = {
    "completed",
    "incomplete",
    "blocked",
    "failed",
    "cancelled",
    "candidate_submitted",
    "budget_exhausted",
}
EVALUATION_ERRORS = (
    json.JSONDecodeError,
    KeyError,
    OSError,
    sqlite3.Error,
    subprocess.TimeoutExpired,
    UnicodeError,
    ValueError,
)
CONTINUATION_ERRORS = (
    json.JSONDecodeError,
    KeyError,
    OSError,
    sqlite3.Error,
    UnicodeError,
    ValueError,
)
DISPATCH_ERRORS = (
    json.JSONDecodeError,
    KeyError,
    OSError,
    sqlite3.Error,
    StopIteration,
    TimeoutError,
    urllib.error.HTTPError,
    urllib.error.URLError,
    UnicodeError,
    ValueError,
)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def digest(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def bounded_file(root, relative, limit=16 * 1024 * 1024):
    """Owner input validation, not confinement against hostile host rename races."""
    parts = relative.split("/") if isinstance(relative, str) else []
    if not parts or any(
        not p or p.startswith(".") or "\\" in p or "\0" in p for p in parts
    ):
        raise ValueError("ordinary relative path required")
    path = root
    for part in parts:
        path = path / part
        if path.is_symlink():
            raise ValueError("symlink input rejected")
    flags = os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW
    with os.fdopen(os.open(path, flags), "rb") as stream:
        before = os.fstat(stream.fileno())
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_size > limit
        ):
            raise ValueError("bounded single-link regular input required")
        body = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    identity = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
    if (
        len(body) > limit
        or len(body) != before.st_size
        or identity(before) != identity(after)
    ):
        raise ValueError("input changed while hashing")
    return body


def evidence(root, paths):
    return {
        path: hashlib.sha256(bounded_file(root, path)).hexdigest() for path in paths
    }


class LoopbackAPI:
    def __init__(self, server):
        parsed = urllib.parse.urlsplit(server)
        if (
            parsed.scheme != "http"
            or parsed.hostname != "127.0.0.1"
            or parsed.username
            or parsed.password
            or parsed.path
            or parsed.query
            or parsed.fragment
        ):
            raise ValueError("plain 127.0.0.1 harness URL required")
        self.server = server

        # Never follow an API redirect or inherit a proxy to another host.
        class NoRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, *args, **kwargs):
                return None

        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}), NoRedirect()
        )

    def __call__(self, path, body=None, *, timeout=30):
        request = urllib.request.Request(
            self.server + path,
            data=None if body is None else canonical(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with self.opener.open(request, timeout=timeout) as response:
            data = response.read(8 * 1024 * 1024 + 1)
        if len(data) > 8 * 1024 * 1024:
            raise ValueError("API response too large")
        return json.loads(data)


def validate_plan(plan, config):
    required = {
        "version",
        "company",
        "evidence_paths",
        "max_model_calls",
        "experiments",
    }
    if (
        not isinstance(plan, dict)
        or set(plan) != required
        or type(plan["version"]) is not int
        or plan["version"] != 1
    ):
        raise ValueError("exact version-1 plan required")
    if not isinstance(plan["company"], str) or not IDENTIFIER.fullmatch(
        plan["company"]
    ):
        raise ValueError("invalid company")
    if not (
        config.get("local_only") is True
        and config.get("company_mode") is True
        and config.get("company_tools") is True
        and config.get("auth") == "local"
    ):
        raise ValueError("local-auth, local-only company harness required")
    paths = plan["evidence_paths"]
    if (
        not isinstance(paths, list)
        or not 1 <= len(paths) <= 32
        or any(
            not isinstance(p, str) or not p.startswith("data/experiment-inputs/")
            for p in paths
        )
        or len(set(paths)) != len(paths)
    ):
        raise ValueError(
            "1..32 distinct owner evidence paths under data/experiment-inputs required"
        )
    experiments = plan["experiments"]
    if not isinstance(experiments, list) or not 1 <= len(experiments) <= 32:
        raise ValueError("1..32 frozen experiments required")
    actors = {a["id"]: a for a in config["agents"]}
    identifiers = set()
    earlier = {}
    calls = 0
    for trial in experiments:
        keys = {"id", "actor", "task", "criteria", "tools", "max_model_calls"}
        if (
            not isinstance(trial, dict)
            or not keys <= set(trial)
            or set(trial) - keys - {"candidate_workflow", "evaluation", "after"}
        ):
            raise ValueError("invalid frozen experiment fields")
        if (
            not isinstance(trial["id"], str)
            or not IDENTIFIER.fullmatch(trial["id"])
            or trial["id"] in identifiers
        ):
            raise ValueError("unique experiment ID required")
        identifiers.add(trial["id"])
        actor = actors.get(trial["actor"])
        if not actor or any(
            actor.get(k) not in ("local", "mlx", "mock")
            for k in ("worker_provider", "planner_provider")
        ):
            raise ValueError("configured local actor required")
        if (
            not isinstance(trial["task"], str)
            or not trial["task"].strip()
            or len(trial["task"].encode()) > 8500
        ):
            raise ValueError("bounded nonempty task required")
        criteria = trial["criteria"]
        if (
            not isinstance(criteria, list)
            or not criteria
            or any(not isinstance(c, str) or not c.strip() for c in criteria)
            or len("\n".join(criteria).encode()) > 8500
        ):
            raise ValueError("bounded nonempty criteria required")
        tools = trial["tools"]
        if (
            not isinstance(tools, list)
            or any(not isinstance(t, str) or not t for t in tools)
            or len(set(tools)) != len(tools)
        ):
            raise ValueError("exact tool grant required")
        # This first controller cannot grant external writes, downloads, runtime
        # administration, schedule creation, or source promotion.
        allowed = {
            "files:read",
            "files:list",
            "files:write",
            "files:rename",
            "finish",
            "source_read",
            "skill_list",
            "skill_read",
            "memory_search",
            "memory_append",
        }
        if not set(tools) <= allowed:
            raise ValueError("experiment tool outside bounded local allowlist")
        limit = trial["max_model_calls"]
        if type(limit) is not int or not 1 <= limit <= 1024:
            raise ValueError("trial call allowance must be 1..1024")
        calls += limit
        if "candidate_workflow" in trial:
            candidate = trial["candidate_workflow"]
            if (
                not isinstance(candidate, dict)
                or candidate.get("actor") != trial["actor"]
            ):
                raise ValueError("candidate actor must equal frozen actor")
        experiment_continuation.validate(trial, earlier)
        earlier[trial["id"]] = trial
    if (
        type(plan["max_model_calls"]) is not int
        or not calls <= plan["max_model_calls"] <= 1024
    ):
        raise ValueError(
            "total allowance must cover all frozen trial reservations and be <=1024"
        )
    if len(canonical(plan).encode()) > 256 * 1024:
        raise ValueError("plan too large")


class Controller:
    def __init__(self, root, identifier, api):
        if not IDENTIFIER.fullmatch(identifier):
            raise ValueError("invalid loop ID")
        self.root = Path(root).resolve(strict=True)
        self.folder = self.root / "data/experiment-loops" / identifier
        self.api = api

    @contextmanager
    def locked(self):
        # State directories are trusted owner-created private directories. Reject
        # pre-existing links; native MCP tools are not granted this filesystem.
        path = self.root
        for part in ("data", "experiment-loops", self.folder.name):
            path = path / part
            if path.is_symlink():
                raise ValueError("linked state directory rejected")
            path.mkdir(mode=0o700, exist_ok=True)
            info = path.stat()
            if info.st_uid != os.getuid() or info.st_mode & 0o022:
                raise ValueError("owner-controlled state directory required")
        descriptor = os.open(
            self.folder / "controller.lock",
            os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW,
            0o600,
        )
        with os.fdopen(descriptor, "r+") as lock:
            info = os.fstat(lock.fileno())
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                raise ValueError("ordinary private lock required")
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                yield None
                return
            path = self.folder / "ledger.sqlite3"
            if path.exists() or path.is_symlink():
                info = path.lstat()
                if (
                    not stat.S_ISREG(info.st_mode)
                    or info.st_nlink != 1
                    or info.st_uid != os.getuid()
                    or info.st_mode & 0o077
                ):
                    raise ValueError("private ordinary ledger required")
            db = sqlite3.connect(path, timeout=5)
            try:
                os.chmod(path, 0o600)
                db.execute(
                    "CREATE TABLE IF NOT EXISTS frozen(plan TEXT NOT NULL, sha256 TEXT NOT NULL, config TEXT NOT NULL, baseline TEXT NOT NULL)"
                )
                db.execute(
                    "CREATE TABLE IF NOT EXISTS trials(position INTEGER PRIMARY KEY,run_id TEXT UNIQUE NOT NULL,evidence_sha256 TEXT UNIQUE NOT NULL,evidence TEXT NOT NULL,state TEXT NOT NULL,reserved_calls INTEGER NOT NULL,result TEXT)"
                )
                db.execute(
                    "CREATE TABLE IF NOT EXISTS trial_outcomes(position INTEGER PRIMARY KEY,sha256 TEXT NOT NULL)"
                )
                db.execute(
                    "CREATE TABLE IF NOT EXISTS trial_feedback(position INTEGER PRIMARY KEY,body TEXT NOT NULL,sha256 TEXT NOT NULL)"
                )
                db.execute(
                    "CREATE TABLE IF NOT EXISTS skipped_trials(position INTEGER PRIMARY KEY,body TEXT NOT NULL,sha256 TEXT NOT NULL)"
                )
                db.commit()
                yield db
            finally:
                db.close()

    def freeze(self, plan):
        with self.locked() as db:
            if db is None:
                return {"status": "busy"}
            frozen_row = db.execute("SELECT COUNT(*) FROM frozen").fetchone()
            if frozen_row is not None and frozen_row[0]:
                raise ValueError(
                    "loop already frozen; existing authority cannot be overwritten"
                )
            config = self.api("/api/config")
            validate_plan(plan, config)
            plan = copy.deepcopy(plan)
            for trial in plan["experiments"]:
                if "evaluation" in trial:
                    trial["evaluation"] = experiment_evaluation.freeze(
                        self.root, trial["evaluation"], trial, bounded_file
                    )
            baseline = evidence(self.root, plan["evidence_paths"])
            self.api("/api/companies/" + plan["company"], {"op": "status"})
            frozen_hash = digest({"plan": plan, "config": config, "baseline": baseline})
            db.execute(
                "INSERT INTO frozen VALUES(?,?,?,?)",
                (canonical(plan), frozen_hash, canonical(config), canonical(baseline)),
            )
            db.commit()
            return {
                "status": "frozen",
                "plan_sha256": digest(plan),
                "baseline_sha256": digest(baseline),
                "max_model_calls": plan["max_model_calls"],
                "initial_dispatch": False,
            }

    def state(self, db):
        records = db.execute(
            "SELECT plan,sha256,config,baseline FROM frozen"
        ).fetchall()
        if len(records) != 1:
            raise ValueError("owner must freeze a plan first")
        frozen = records[0]
        plan, plan_hash, config, baseline = frozen
        plan, config, baseline = (
            json.loads(plan),
            json.loads(config),
            json.loads(baseline),
        )
        if digest({"plan": plan, "config": config, "baseline": baseline}) != plan_hash:
            raise ValueError("frozen snapshot digest mismatch; no automatic repair")
        # Never interpret a partial/corrupt ledger as a fresh call allowance.
        rows = db.execute(
            "SELECT position,run_id,evidence_sha256,evidence,state,reserved_calls FROM trials ORDER BY position"
        ).fetchall()
        skipped = db.execute(
            "SELECT position,body,sha256 FROM skipped_trials ORDER BY position"
        ).fetchall()
        positions = [r[0] for r in rows] + [r[0] for r in skipped]
        if len(positions) > len(plan["experiments"]) or sorted(positions) != list(
            range(len(positions))
        ):
            raise ValueError("trial ledger exceeds frozen plan")
        for row in rows:
            index, run, fingerprint, observed, state, calls = row
            if (
                str(uuid.UUID(run)) != run
                or fingerprint != digest(json.loads(observed))
                or calls != plan["experiments"][index]["max_model_calls"]
                or state
                not in {
                    "claimed",
                    "dispatching",
                    "observing",
                    "needs_reconciliation",
                    "evaluation_pending",
                    "evaluation_uncertain",
                    "result",
                }
            ):
                raise ValueError("trial ledger mismatch; no automatic repair")
            if "after" in plan["experiments"][index]:
                feedback = db.execute(
                    "SELECT body,sha256 FROM trial_feedback WHERE position=?", (index,)
                ).fetchone()
                if (
                    not feedback
                    or digest(json.loads(feedback[0])) != feedback[1]
                    or json.loads(observed).get("feedback_sha256") != feedback[1]
                ):
                    raise ValueError("continuation feedback integrity mismatch")
        for index, body, checksum in skipped:
            if (
                "after" not in plan["experiments"][index]
                or digest(json.loads(body)) != checksum
                or json.loads(body).get("experiment")
                != plan["experiments"][index]["id"]
            ):
                raise ValueError("skipped continuation integrity mismatch")
        if any("after" in trial for trial in plan["experiments"]):
            for index, result in db.execute(
                "SELECT position,result FROM trials WHERE state='result'"
            ):
                checksum = db.execute(
                    "SELECT sha256 FROM trial_outcomes WHERE position=?", (index,)
                ).fetchone()
                if not checksum or digest(json.loads(result)) != checksum[0]:
                    raise ValueError("predecessor result integrity mismatch")
        return plan, config, baseline

    def status(self):
        with self.locked() as db:
            if db is None:
                return {"status": "busy"}
            plan, _, _ = self.state(db)
            rows = db.execute(
                "SELECT position,run_id,evidence_sha256,state,reserved_calls,result FROM trials ORDER BY position"
            ).fetchall()
            skipped = []
            for (body,) in db.execute(
                "SELECT body FROM skipped_trials ORDER BY position"
            ):
                if isinstance(body, (str, bytes, bytearray)):
                    skipped.append(json.loads(body))
                elif isinstance(body, dict):
                    skipped.append(body)
                else:
                    raise ValueError("invalid skipped trial body")
            return {
                "status": "exhausted"
                if len(rows) + len(skipped) == len(plan["experiments"])
                and all(r[3] == "result" for r in rows)
                else "watching",
                "plan_sha256": digest(plan),
                "reserved_model_calls": sum(r[4] for r in rows),
                "max_model_calls": plan["max_model_calls"],
                "trials": [
                    dict(
                        zip(
                            (
                                "position",
                                "run_id",
                                "evidence_sha256",
                                "state",
                                "reserved_calls",
                                "result",
                            ),
                            (*r[:5], json.loads(r[5]) if r[5] else None),
                        )
                    )
                    for r in rows
                ],
                "independently_accepted_trials": sum(
                    bool(
                        r[5] and json.loads(r[5]).get("independently_accepted") is True
                    )
                    for r in rows
                ),
                "skipped_trials": skipped,
                "promotion_authorized": False,
            }

    def observe(self, db, run):
        directory = self.folder / run
        directory.mkdir(mode=0o700, exist_ok=True)
        try:
            report = observe_run(self.api, run, directory, timeout_seconds=0)
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
            error.close()
            db.execute(
                "UPDATE trials SET state='needs_reconciliation' WHERE run_id=?", (run,)
            )
            db.commit()
            return {
                "status": "needs_reconciliation",
                "run_id": run,
                "reason": "Original run absent; uncertain setup/dispatch is never replayed",
            }
        runtime = report["runtime_status"]
        if runtime not in TERMINAL | HOLD | {"running", "queued"}:
            raise ValueError("unrecognized runtime status; retain original claim")
        final = runtime in TERMINAL and report["status"] != "attention"
        result = {
            **report,
            "independently_accepted": False,
            "promotion_authorized": False,
        }
        if final:
            plan, _, _ = self.state(db)
            position = db.execute(
                "SELECT position FROM trials WHERE run_id=?", (run,)
            ).fetchone()[0]
            trial = plan["experiments"][position]
            if "after" in trial:
                trial = copy.deepcopy(trial)
                trial["candidate_feedback"] = json.loads(
                    db.execute(
                        "SELECT body FROM trial_feedback WHERE position=?", (position,)
                    ).fetchone()[0]
                )
            if "evaluation" in trial:
                if runtime == "candidate_submitted":
                    # A crash/response loss after this checkpoint never causes
                    # another verification dispatch or an unproved PASS.
                    db.execute(
                        "UPDATE trials SET state='evaluation_pending',result=? WHERE run_id=?",
                        (canonical(result), run),
                    )
                    db.commit()
                    try:
                        checked = experiment_evaluation.evaluate(
                            self.root, plan["company"], run, trial, bounded_file
                        )
                    except EVALUATION_ERRORS:
                        db.execute(
                            "UPDATE trials SET state='evaluation_uncertain' WHERE run_id=?",
                            (run,),
                        )
                        db.commit()
                        return {
                            "status": "needs_reconciliation",
                            "run_id": run,
                            "reason": "Independent evaluation unconfirmed; original claim retained without replay",
                        }
                    result["evaluation"] = checked
                    result["independently_accepted"] = checked["passed"]
                    result["independent_acceptance_scope"] = checked["scope"]
                else:
                    result["evaluation"] = {
                        "kind": "rust_acceptance",
                        "status": "not_run_runtime_not_submitted",
                        "passed": False,
                    }
        db.execute(
            "UPDATE trials SET state=?,result=? WHERE run_id=?",
            ("result" if final else "observing", canonical(result), run),
        )
        if final:
            position = db.execute(
                "SELECT position FROM trials WHERE run_id=?", (run,)
            ).fetchone()[0]
            db.execute(
                "INSERT OR REPLACE INTO trial_outcomes VALUES(?,?)",
                (position, digest(result)),
            )
        db.commit()
        return {
            "status": "result" if final else "observing",
            "run_id": run,
            "result": result,
        }

    def poll(self):
        with self.locked() as db:
            if db is None:
                return {"status": "busy"}
            plan, config, baseline = self.state(db)
            rows = db.execute(
                "SELECT position,run_id,evidence_sha256,state,reserved_calls FROM trials ORDER BY position"
            ).fetchall()
            unfinished = [r for r in rows if r[3] != "result"]
            if unfinished:
                if unfinished[0][3] in {"evaluation_pending", "evaluation_uncertain"}:
                    return {
                        "status": "needs_reconciliation",
                        "run_id": unfinished[0][1],
                        "reason": "Independent evaluation checkpoint retained; no automatic replay",
                    }
                return self.observe(db, unfinished[0][1])
            skipped = db.execute("SELECT position FROM skipped_trials").fetchall()
            position = len(rows) + len(skipped)
            if position == len(plan["experiments"]):
                return {"status": "exhausted"}
            trial = plan["experiments"][position]
            feedback = None
            if "after" in trial:
                indices = {
                    trial["id"]: i for i, trial in enumerate(plan["experiments"])
                }
                parent_position = indices[trial["after"]["experiment"]]
                parent = db.execute(
                    "SELECT run_id,state,result FROM trials WHERE position=?",
                    (parent_position,),
                ).fetchone()
                if parent is None:
                    decision = {"decision": "skip", "reason": "predecessor was skipped"}
                else:
                    parent_trial = copy.deepcopy(plan["experiments"][parent_position])
                    if "after" in parent_trial:
                        feedback_row = db.execute(
                            "SELECT body FROM trial_feedback WHERE position=?",
                            (parent_position,),
                        ).fetchone()
                        if feedback_row is None or not isinstance(feedback_row[0], str):
                            raise ValueError("stored trial feedback must be JSON text")
                        parent_trial["candidate_feedback"] = json.loads(feedback_row[0])
                    parent_result = json.loads(parent[2])
                    prior_sources = set()
                    ancestor = parent_trial
                    while "after" in ancestor:
                        index = indices[ancestor["after"]["experiment"]]
                        prior = db.execute(
                            "SELECT result FROM trials WHERE position=? AND state='result'",
                            (index,),
                        ).fetchone()
                        if prior:
                            value = (
                                json.loads(prior[0])
                                .get("evaluation", {})
                                .get("custody", {})
                                .get("source_sha256")
                            )
                            if value:
                                prior_sources.add(value)
                        ancestor = plan["experiments"][index]
                    try:
                        decision = experiment_continuation.prepare(
                            self.root,
                            plan["company"],
                            parent[0],
                            parent_trial,
                            parent_result,
                            trial["after"]["outcome"],
                            prior_sources,
                            digest(parent_result),
                            digest(parent_result.get("evaluation")),
                            bounded_file,
                        )
                    except CONTINUATION_ERRORS:
                        return {
                            "status": "needs_reconciliation",
                            "run_id": parent[0],
                            "reason": "Predecessor custody or evaluator identity changed; no follow-up dispatched",
                        }
                if decision["decision"] == "skip":
                    body = {
                        "experiment": trial["id"],
                        "after": trial["after"],
                        "reason": decision["reason"],
                    }
                    db.execute(
                        "INSERT INTO skipped_trials VALUES(?,?,?)",
                        (position, canonical(body), digest(body)),
                    )
                    db.commit()
                    return {"status": "skipped", **body}
                feedback = decision["feedback"]
                current = {
                    "kind": "independent_trial_result",
                    "experiment": trial["id"],
                    "predecessor_run": parent[0],
                    "predecessor_result_sha256": decision["predecessor_result_sha256"],
                    "feedback_sha256": digest(feedback),
                }
            else:
                current = evidence(self.root, plan["evidence_paths"])
            fingerprint = digest(current)
            if fingerprint == digest(baseline) or fingerprint in {r[2] for r in rows}:
                return {"status": "unchanged", "evidence_sha256": fingerprint}
            if self.api("/api/config") != config:
                raise ValueError(
                    "public harness configuration changed; freeze a reviewed new plan"
                )
            route = "/api/companies/" + plan["company"]
            company = self.api(route, {"op": "status"})
            if any(j["state"] in ("approved", "running") for j in company["jobs"]):
                return {"status": "yielding_to_company_work"}
            reserved = sum(r[4] for r in rows) + trial["max_model_calls"]
            if reserved > plan["max_model_calls"]:
                raise ValueError("frozen total call allowance exhausted")
            run = str(uuid.uuid4())
            # Claim and burn the full allowance BEFORE the first external write.
            # No claim is deleted, renewed, automatically retried or refunded.
            db.execute(
                "INSERT INTO trials VALUES(?,?,?,?,'claimed',?,NULL)",
                (
                    position,
                    run,
                    fingerprint,
                    canonical(current),
                    trial["max_model_calls"],
                ),
            )
            if feedback is not None:
                db.execute(
                    "INSERT INTO trial_feedback VALUES(?,?,?)",
                    (position, canonical(feedback), digest(feedback)),
                )
            db.commit()
            try:
                intake = {
                    "idea": trial["task"],
                    "acceptance": "\n".join(trial["criteria"]),
                    "exclusions": "Unrequested spending, tools outside frozen grant, source promotion",
                    "timeline": "Bounded local experiment; no completion-time promise",
                    "company_signer": "Owner-frozen evidence-triggered experiment",
                    "provider_signer": "Local Gents company",
                    "budget_usd": 0,
                    "deliverable_type": "agent",
                    "max_model_calls": trial["max_model_calls"],
                }
                if "candidate_workflow" in trial:
                    intake.update(
                        candidate_workflow=trial["candidate_workflow"],
                        deliverable_type="candidate",
                    )
                if feedback is not None:
                    intake["candidate_feedback"] = feedback
                state = self.api(
                    route, {"op": "intake", "run_id": run, "intake": intake}
                )
                job = next(j for j in state["jobs"] if j["run_id"] == run)
                self.api(
                    route, {"op": "approve", "run_id": run, "digest": job["digest"]}
                )
                access = {
                    a["id"]: {
                        "tools": trial["tools"] if a["id"] == trial["actor"] else [],
                        "skills": [],
                        "memory_from": [],
                    }
                    for a in config["agents"]
                }
                policy = {
                    "revision": 1,
                    "master": trial["actor"],
                    "ceilings": access,
                    "grants": access,
                }
                self.api(
                    route,
                    {
                        "op": "governance_install",
                        "run_id": run,
                        "expected_revision": 0,
                        "policy": policy,
                    },
                )
                db.execute(
                    "UPDATE trials SET state='dispatching' WHERE run_id=?", (run,)
                )
                db.commit()
                self.api(
                    "/api/runs",
                    {
                        "company_id": plan["company"],
                        "run_id": run,
                        "task": trial["task"],
                        "definition_of_done": trial["criteria"],
                        "consent": False,
                    },
                )
                db.execute("UPDATE trials SET state='observing' WHERE run_id=?", (run,))
                db.commit()
            except DISPATCH_ERRORS:
                db.execute(
                    "UPDATE trials SET state='needs_reconciliation' WHERE run_id=?",
                    (run,),
                )
                db.commit()
                # Do not expose API error bodies or automatically replay a POST.
                return {
                    "status": "needs_reconciliation",
                    "run_id": run,
                    "reason": "Unconfirmed preparation or dispatch; inspect original company job and run",
                }
            return {
                "status": "dispatched",
                "run_id": run,
                "evidence_sha256": fingerprint,
                "reserved_model_calls": reserved,
            }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--id", required=True)
    parser.add_argument("--server", default="http://127.0.0.1:8788")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("freeze").add_argument("--plan", type=Path, required=True)
    commands.add_parser("poll")
    commands.add_parser("status")
    commands.add_parser("watch")
    args = parser.parse_args()
    controller = Controller(ROOT, args.id, LoopbackAPI(args.server))
    if args.command == "freeze":
        print(canonical(controller.freeze(json.loads(args.plan.read_bytes()))))
    elif args.command == "status":
        print(canonical(controller.status()))
    elif args.command == "poll":
        print(canonical(controller.poll()))
    else:
        previous = None
        while True:
            report = controller.poll()
            if report != previous:
                print(canonical(report), flush=True)
                previous = report
            if report["status"] in {"exhausted", "needs_reconciliation"}:
                break
            time.sleep(5)


if __name__ == "__main__":
    main()

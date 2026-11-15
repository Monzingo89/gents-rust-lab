#!/usr/bin/env python3
"""Private trusted-host curriculum journal. Not an MCP tool or model authority.
JSON stdin/stdout only; no credentials, downloads or inference. Node's independent
verifier supplies admissions; SQLite leases prevent concurrent/duplicate scheduling.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import local_sqlite as sqlite3
import sys
import time
import uuid

POLICY = json.loads((Path(__file__).resolve().parents[1] / "config/mcp-development-curriculum.json").read_text())
CATALOG = {row["id"]: row for row in POLICY["levels"]}
CASE_IDS = set(CATALOG)
HEX = re.compile(r"[a-f0-9]{64}\Z")

def checked_uuid(value):
    if str(uuid.UUID(value)) != value:
        raise ValueError("noncanonical UUID")
    return value

def no_symlinks(path):
    for component in [path, *path.parents]:
        if component.is_symlink():
            raise ValueError("symlink in private journal/evidence path")

def native_authority(root, company, run):
    checked_uuid(run)
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", company):
        raise ValueError("invalid company")
    path = root / "data/companies" / company / "brain.sqlite3"
    no_symlinks(path)
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as db:
        db.execute("PRAGMA trusted_schema=OFF")
        db.execute("BEGIN")
        job = db.execute("SELECT digest,approval,intake,state FROM jobs WHERE run_id=?", (run,)).fetchone()
        if not job or not job[1] or job[3] == "draft":
            raise ValueError("approved frozen job missing")
        approval = json.loads(job[1])
        owner = db.execute("SELECT owner FROM company LIMIT 1").fetchone()
        if not owner or approval.get("owner") != owner[0] or approval.get("sha256") != job[0]:
            raise ValueError("approval digest mismatch")
        policy = db.execute("SELECT revision,manifest FROM run_governance WHERE run_id=?", (run,)).fetchone()
        if not policy:
            raise ValueError("installed policy missing")
        rows = db.execute("SELECT body FROM audit WHERE run_id=? AND kind='mcp_tool_call' ORDER BY seq", (run,)).fetchall()
        if len(rows) > 128:
            raise ValueError("native audit exceeds challenge bound")
        intake = json.loads(job[2])
        attempts = []
        started_revisions = []
        phase_boundaries = []
        if intake.get("ceo_planning_id"):
            checked_uuid(intake["ceo_planning_id"])
            for (body,) in db.execute("SELECT body FROM audit WHERE run_id=? AND kind='ceo_planning_completed' ORDER BY seq", (intake["ceo_planning_id"],)):
                audit = json.loads(body)
                if audit["revision"] > intake["ceo_planning_revision"]:
                    continue
                response = audit.get("response") or {}
                attempts.append({"revision":audit["revision"], "status":audit["status"], "usage":response.get("usage") or audit.get("usage"), "local_execution":response.get("local_execution") or audit.get("local_execution"), "rejected_output_sha256":audit.get("rejected_output_sha256"),"model":audit.get("model"),"validation_error":audit.get("validation_error"),"reply_token_limit":audit.get("reply_token_limit"),"planning_model_selection":audit.get("planning_model_selection"),"planning_protocol":audit.get("planning_protocol"),"host_compiled_adopted_dependencies":audit.get("host_compiled_adopted_dependencies")})
            if len(attempts) > 16:
                raise ValueError("planning audit exceeds challenge bound")
            for (body,) in db.execute("SELECT body FROM audit WHERE run_id=? AND kind='ceo_planning_started' ORDER BY seq", (intake["ceo_planning_id"],)):
                revision = json.loads(body)["revision"]
                if revision <= intake["ceo_planning_revision"]:
                    started_revisions.append(revision)
            if len(started_revisions) > 16:
                raise ValueError("planning starts exceed challenge bound")
        if intake.get("ceo_planning_id"):
            phase_boundaries = [json.loads(row[0]) for row in db.execute("SELECT body FROM audit WHERE run_id=? AND kind='ceo_owned_selection_adopted' ORDER BY seq LIMIT 2", (intake["ceo_planning_id"],))]
            if len(phase_boundaries) > 1:
                raise ValueError("ambiguous planning phase boundary")
        staging = [json.loads(row[0]) for row in db.execute("SELECT body FROM audit WHERE run_id=? AND kind='owner_source_files_staged' ORDER BY seq", (run,))]
        if len(staging) > 4:
            raise ValueError("attachment staging audit exceeds challenge bound")
        owned = []
        if db.execute("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='owned_rust_call_results'").fetchone()[0]:
            fields = ["policy_revision", "actor", "binding_id", "source_sha256", "api_sha256", "input_sha256", "arguments_sha256", "result_sha256", "passed"]
            columns = {row[1] for row in db.execute("PRAGMA table_info(owned_rust_call_results)")}
            fields += [field for field in ("recipe_sha256", "lineage_sha256") if field in columns]
            owned = [dict(zip(fields, row)) for row in db.execute("SELECT " + ",".join(fields) + " FROM owned_rust_call_results WHERE run_id=? ORDER BY rowid LIMIT 129", (run,))]
            if len(owned) > 128:
                raise ValueError("owned invocation observations exceed challenge bound")
        owned_staging = [json.loads(row[0]) for row in db.execute("SELECT body FROM audit WHERE run_id=? AND kind='owner_component_sources_staged' ORDER BY seq LIMIT 5", (run,))]
        if len(owned_staging) > 4:
            raise ValueError("owned staging audit exceeds challenge bound")
        custody = []
        if intake.get("ceo_owned_invocations"):
            session = intake["ceo_planning_id"]
            checked_uuid(session)
            for manifest, content in db.execute("SELECT CASE WHEN length(CAST(manifest AS BLOB))<=16384 THEN manifest END,CASE WHEN length(CAST(content AS BLOB))<=8192 THEN content END FROM ceo_owned_inputs WHERE session=? ORDER BY id LIMIT 9", (session,)):
                if not isinstance(manifest, str) or not isinstance(content, str):
                    raise ValueError("owned custody record exceeds bounds")
                custody.append({"manifest": json.loads(manifest), "source_sha256": hashlib.sha256(content.encode()).hexdigest(), "bytes": len(content.encode())})
            if len(custody) > 8:
                raise ValueError("owned custody observations exceed challenge bound")
        return {"owned_source_staging": owned_staging, "owned_input_custody": custody, "source_staging": staging, "digest": job[0], "intake": intake, "policy_revision": policy[0], "policy": json.loads(policy[1]), "receipts": [json.loads(row[0]) for row in rows], "owned_rust_invocations": owned, "planning_attempts":attempts,"planning_started_revisions":started_revisions,"planning_phase_boundaries":phase_boundaries}

def planning_authority(root, company, session):
    checked_uuid(session)
    if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", company):
        raise ValueError("invalid company")
    path = root / "data/companies" / company / "brain.sqlite3"
    no_symlinks(path)
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as db:
        db.execute("PRAGMA trusted_schema=OFF")
        db.execute("BEGIN")
        row = db.execute("SELECT revision,state,body FROM ceo_planning WHERE id=?", (session,)).fetchone()
        if not row or row[1] not in ("question", "selection_review", "ready", "failed"):
            raise ValueError("planning continuation requires an authoritative terminal turn")
        body = json.loads(row[2])
        if body.get("id") != session or body.get("revision") != row[0] or body.get("status") != row[1]:
            raise ValueError("planning checkpoint identity mismatch")
        if db.execute("SELECT COUNT(*) FROM jobs WHERE json_extract(intake,'$.ceo_planning_id')=?", (session,)).fetchone()[0]:
            raise ValueError("planning already issued a job; use its existing run")
        return {"planning": body}

class Journal:
    def __init__(self, path):
        no_symlinks(path)
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        os.umask(0o077)
        self.db = sqlite3.connect(path, timeout=5)
        os.chmod(path, 0o600)
        self.db.row_factory = sqlite3.Row
        self.db.executescript("""
        PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS cases(id TEXT PRIMARY KEY,level INTEGER NOT NULL,fixture_sha TEXT NOT NULL,prerequisites TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY,case_id TEXT NOT NULL REFERENCES cases(id),fixture_sha TEXT NOT NULL,origin TEXT NOT NULL,state TEXT NOT NULL,lease_sha TEXT NOT NULL,created INTEGER NOT NULL,finished INTEGER,proof TEXT);
        CREATE UNIQUE INDEX IF NOT EXISTS one_active ON attempts((1)) WHERE state='running';
        CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY,attempt TEXT,kind TEXT NOT NULL,body TEXT NOT NULL,created INTEGER NOT NULL);
        """)
    def event(self, attempt, kind, body):
        self.db.execute("INSERT INTO events(attempt,kind,body,created) VALUES(?,?,?,?)", (attempt, kind, json.dumps(body, sort_keys=True), int(time.time()*1000)))
    def qualified(self, case_id):
        return bool(self.db.execute("SELECT 1 FROM attempts a JOIN cases c ON a.case_id=c.id WHERE c.id=? AND a.fixture_sha=c.fixture_sha AND a.state='pass' LIMIT 1", (case_id,)).fetchone())
    def prerequisites_pass(self, case_id, visiting=None):
        visiting = set() if visiting is None else visiting
        if case_id in visiting:
            raise ValueError("cyclic curriculum prerequisites")
        row = self.db.execute("SELECT prerequisites FROM cases WHERE id=?", (case_id,)).fetchone()
        return bool(row) and all(self.qualified(prior) and self.prerequisites_pass(prior, visiting | {case_id}) for prior in json.loads(row[0]))
    def status(self):
        cases = [dict(row) for row in self.db.execute("SELECT * FROM cases ORDER BY level")]
        for row in cases:
            row["prerequisites"] = json.loads(row["prerequisites"])
            row["qualified_fixture"] = self.qualified(row["id"])
            row["eligible"] = self.prerequisites_pass(row["id"])
        attempts = []
        for row in self.db.execute("SELECT id,case_id,fixture_sha,origin,state,created,finished,proof FROM attempts ORDER BY created DESC LIMIT 50"):
            item = dict(row)
            item["proof"] = json.loads(item["proof"]) if item["proof"] else None
            attempts.append(item)
        return {"version": 1, "scope": "versioned synthetic fixture qualification; not model training or general mastery", "cases": cases, "attempts": attempts, "attempt_count": self.db.execute("SELECT COUNT(*) FROM attempts").fetchone()[0], "active": next((row for row in attempts if row["state"] == "running"), None)}
    def operate(self, request):
        self.db.execute("BEGIN IMMEDIATE")
        try:
            op = request["op"]
            if op == "sync":
                cases = request["cases"]
                if {c["id"] for c in cases} != CASE_IDS or len(cases) != len(CATALOG):
                    raise ValueError("unexpected curriculum catalog")
                for case in cases:
                    if not HEX.fullmatch(case["fixture_sha"]):
                        raise ValueError("invalid fixture digest")
                    expected_level = CATALOG[case["id"]]["level"]
                    expected_prior = CATALOG[case["id"]]["requires"]
                    if case["level"] != expected_level or case["requires"] != expected_prior:
                        raise ValueError("invalid progression graph")
                    self.db.execute("INSERT INTO cases VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET level=excluded.level,fixture_sha=excluded.fixture_sha,prerequisites=excluded.prerequisites", (case["id"], case["level"], case["fixture_sha"], json.dumps(case["requires"])))
                self.event(None, "catalog_checked", {"cases": {c["id"]: c["fixture_sha"] for c in cases}})
            elif op == "begin":
                attempt = checked_uuid(request["attempt"])
                if request["origin"] not in ("historical", "executed") or not HEX.fullmatch(request["lease_sha"]):
                    raise ValueError("invalid attempt identity")
                case = self.db.execute("SELECT * FROM cases WHERE id=?", (request["case_id"],)).fetchone()
                if not case or case["fixture_sha"] != request["fixture_sha"]:
                    raise ValueError("unknown or stale fixture")
                if not self.prerequisites_pass(case["id"]):
                    raise ValueError("prior versioned fixture is not qualified")
                self.db.execute("INSERT INTO attempts(id,case_id,fixture_sha,origin,state,lease_sha,created) VALUES(?,?,?,?,'running',?,?)", (attempt, case["id"], case["fixture_sha"], request["origin"], request["lease_sha"], int(time.time()*1000)))
                self.event(attempt, "attempt_started", {"case_id": case["id"], "fixture_sha": case["fixture_sha"], "origin": request["origin"]})
            elif op == "finish":
                attempt = checked_uuid(request["attempt"])
                row = self.db.execute("SELECT * FROM attempts WHERE id=?", (attempt,)).fetchone()
                if not row or row["state"] != "running" or hashlib.sha256(request["lease"].encode()).hexdigest() != row["lease_sha"]:
                    raise ValueError("missing, stale or unauthorized attempt lease")
                proof = request["proof"]
                if len(json.dumps(proof)) > 16384 or proof.get("fixture_sha") != row["fixture_sha"]:
                    raise ValueError("invalid proof envelope")
                state = request["state"]
                if state not in ("pass", "fail"):
                    raise ValueError("invalid terminal state")
                if state == "pass":
                    checked_uuid(proof["run"])
                    if proof.get("verified") is not True or (type(proof.get("provider_cost_usd")) not in (int, float) or proof["provider_cost_usd"] != 0) or not HEX.fullmatch(proof.get("evidence_sha256", "")) or not HEX.fullmatch(proof.get("report_sha256", "")):
                        raise ValueError("incomplete successful evidence")
                elif not proof.get("reason"):
                    raise ValueError("failure reason required")
                self.db.execute("UPDATE attempts SET state=?,proof=?,finished=? WHERE id=?", (state, json.dumps(proof, sort_keys=True), int(time.time()*1000), attempt))
                self.event(attempt, "attempt_"+state, proof)
            elif op != "status":
                raise ValueError("unknown operation")
            result = self.status()
            self.db.commit()
            return result
        except Exception:
            self.db.rollback()
            raise

def main():
    request = json.loads(sys.stdin.buffer.read(65537))
    root = Path(request.pop("root")).absolute()
    if request["op"] == "native_authority":
        result = native_authority(root, request["company"], request["run"])
    elif request["op"] == "planning_authority":
        result = planning_authority(root, request["company"], request["session"])
    else:
        journal = Journal(root / "data/mcp-development-curriculum/curriculum.sqlite3")
        try:
            result = journal.operate(request)
        finally:
            journal.db.close()
    print(json.dumps(result, separators=(",", ":")))
if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps({"error": str(error)}))
        sys.exit(1)

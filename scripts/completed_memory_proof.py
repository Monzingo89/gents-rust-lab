"""Read-only completed-run archive proof, independent of native checkpoint code.

No payloads are returned. This verifies storage integrity, not memory truth,
recall quality, task qualification, access authority or weight training.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import uuid

import local_sqlite


def completed_archive(db, run):
    """Caller must hold one read transaction for a consistent snapshot."""
    if not db.in_transaction:
        raise ValueError("Archive proof requires a transaction")
    if str(uuid.UUID(run)) != run:
        raise ValueError("Canonical run UUID required")
    job = db.execute("SELECT digest,state FROM jobs WHERE run_id=?", (run,)).fetchone()
    policy = db.execute("SELECT revision FROM run_governance WHERE run_id=?", (run,)).fetchone()
    if not job or job[1] != "completed" or not policy or type(policy[0]) is not int or policy[0] < 1 or not re.fullmatch(r"[a-f0-9]{64}", job[0] or ""):
        raise ValueError("Completed job and installed policy required")
    hot = db.execute("SELECT COUNT(*) FROM context WHERE run_id=?", (run,)).fetchone()[0]
    count, size = db.execute("SELECT COUNT(*),COALESCE(SUM(LENGTH(CAST(body AS BLOB))),0) FROM context_archive WHERE run_id=?", (run,)).fetchone()
    if hot or not 0 < count <= 1000 or size > 1024 * 1024:
        raise ValueError("Completed cold-only archive required within proof bounds")
    groups = {}
    for actor, record, tier, source, body, created in db.execute("SELECT agent,id,tier,source,body,created FROM context_archive WHERE run_id=? ORDER BY agent,id", (run,)):
        if tier not in ("ALTC", "EVIDENCE_ALTC"):
            raise ValueError("Completed archive tier required")
        digest, records = groups.setdefault(actor, (hashlib.sha256(), 0))
        for field in (run, actor, record, tier, source, hashlib.sha256(body.encode()).hexdigest(), str(created)):
            encoded = field.encode()
            digest.update(len(encoded).to_bytes(8, "little"))
            digest.update(encoded)
        groups[actor] = (digest, records + 1)
    checkpoints = db.execute("SELECT agent,policy_revision,contract_sha256,record_count,records_sha256 FROM memory_archive_checkpoints WHERE run_id=? AND lifecycle='completed' ORDER BY agent", (run,)).fetchall()
    if len(groups) != len(checkpoints):
        raise ValueError("Per-agent checkpoint coverage mismatch")
    for actor, revision, contract, records, digest in checkpoints:
        expected = groups.get(actor)
        if not expected or revision != policy[0] or contract != job[0] or records != expected[1] or digest != expected[0].hexdigest():
            raise ValueError("Archive count, digest or authority checkpoint mismatch")
    return {"run": run, "status": "PASS", "backend": "owned_local_sqlite", "actors": len(groups), "cold_records": count, "hot_records": hot,
            "policy_revision": policy[0], "contract_sha256": job[0], "counts_and_length_prefixed_digests_independently_verified": True,
            "scope": "Completed archive storage integrity only; no memory payload, truth, recall quality, task qualification, authority or training established"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--company", required=True)
    parser.add_argument("--run", required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,79}", args.company):
        parser.error("Invalid company id")
    root = Path(__file__).resolve().parents[1]
    path = root / "data/companies" / args.company / "brain.sqlite3"
    for component in [path, *list(path.parents)[:3]]:
        if component.is_symlink():
            parser.error("Symlink company storage rejected")
    if not path.is_file():
        parser.error("Existing company storage required")
    with local_sqlite.connect(path.as_uri() + "?mode=ro", uri=True) as db:
        db.execute("BEGIN")
        print(json.dumps(completed_archive(db, args.run)))


if __name__ == "__main__":
    main()

"""Verify the sorted-ID synthetic factory-v2 candidate probe against canonical
SQLite, native receipts, typed citations, exact output and archive checkpoints.
No runtime imports/inference. This is not general task/compliance acceptance.
"""

import argparse
import hashlib
import json
import socket
import sqlite3
import struct
from pathlib import Path
from typing import Any

if not __debug__:
    raise RuntimeError("Run this evidence verifier without Python optimization")
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--probe", required=True, type=__import__("uuid").UUID)
parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
parser.add_argument("--output", type=Path)
args = parser.parse_args()
ROOT = args.root.resolve()
FOLDER = ROOT / "data/factory-coordination-probes" / str(args.probe)


def sha(data):
    return hashlib.sha256(data.encode() if isinstance(data, str) else data).hexdigest()


def encoded(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def file_sha(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


report = json.loads((FOLDER / "report.json").read_text())
assert report["status"] == "PASS"
company = report["company"]
dbpath = ROOT / "data/companies" / company / "brain.sqlite3"
db = sqlite3.connect(dbpath.as_uri() + "?mode=ro", uri=True)
db.row_factory = sqlite3.Row
assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
verified = []
models = {}
ports = set()
for stem, pattern in [
    ("Ordered", "ordered"),
    ("AdaptiveDelegation", "adaptive_delegation"),
    ("PeerReview", "peer_review"),
]:
    freeze_path = FOLDER / f"{stem}-freeze.json"
    result_path = FOLDER / f"{stem}-result.json"
    freeze, result = [json.loads(p.read_text()) for p in [freeze_path, result_path]]
    run, contract = freeze["run"], freeze["contract"]
    assert freeze["company"] == company and contract["coordination"] == pattern
    binary = ROOT / freeze["probe_binary_path"]
    assert file_sha(binary) == freeze["probe_binary_sha256"]
    job = db.execute("SELECT * FROM jobs WHERE run_id=?", (run,)).fetchone()
    intake, approval = json.loads(job["intake"]), json.loads(job["approval"])
    assert job["state"] == "candidate_submitted"
    assert sha(job["intake"]) == job["digest"] == approval["sha256"]
    assert intake["coordination_contract"] == contract
    assert contract["version"] == 2 and freeze["factory_prepared"] is True
    prepared = freeze["prepared"]
    binding = intake["factory_binding"]
    assert prepared["intake_patch"]["factory_binding"] == binding
    assert binding["program_sha256"] == freeze["contract_sha256"]
    assert (
        sha(encoded(prepared["blueprint"]))
        == binding["blueprint_sha256"]
        == prepared["blueprint_sha256"]
    )
    assert binding["proposal_sha256"] == sha(encoded(intake["ceo_plan"]))
    assert (
        prepared["candidate_only"] is True and prepared["execution_authorized"] is False
    )
    assert prepared["blueprint"]["architecture"]["coordination"] == pattern
    assert prepared["blueprint"]["architecture"]["version"] == 2
    assert prepared["blueprint"]["gaps"] == [] and prepared["blueprint"]["tools"] == []
    assert prepared["blueprint"]["execution_authorized"] is False
    assert binding["request"]["input"] == contract["input"]
    assert (
        binding["request"]["specification"]["definition_of_done"]
        == contract["definition_of_done"]
    )
    assert binding["request"]["model_sha256"] == {
        r["actor"]: r["model_sha256"] for r in contract["roles"]
    }
    program = db.execute(
        "SELECT * FROM coordination_programs WHERE run_id=?", (run,)
    ).fetchone()
    assert sha(program["contract"]) == program["sha256"] == freeze["contract_sha256"]
    assert json.loads(program["contract"]) == contract
    policy_row = db.execute(
        "SELECT * FROM run_governance WHERE run_id=?", (run,)
    ).fetchone()
    policy = json.loads(policy_row["manifest"])
    assert policy["revision"] == policy_row["revision"] == program["policy_revision"]
    rows = db.execute(
        "SELECT * FROM coordination_turns WHERE run_id=? ORDER BY ordinal", (run,)
    ).fetchall()
    # Each message is decoded JSON; exact fields are checked against the frozen
    # contract and canonical rows below before being accepted as evidence.
    history: list[dict[str, Any]] = []
    usage = {"input": 0, "output": 0, "cached_input": 0, "cost_usd": 0}
    sequence = []
    for i, row in enumerate(rows):
        assert row["ordinal"] == i and row["state"] == "completed"
        turn, message, receipt = [
            json.loads(row[k]) for k in ["turn", "message", "receipt"]
        ]
        assert message["turn"] == turn and turn["ordinal"] == i
        assert (
            sha(encoded({"message": row["message"], "receipt": row["receipt"]}))
            == row["sha256"]
        )
        assert json.loads(receipt["text"]) == message["content"]
        assert all(isinstance(n, int) and 0 <= n < i for n in turn["sources"])
        assert len(set(turn["sources"])) == len(turn["sources"])
        origins = set()
        sources = []
        for n in turn["sources"]:
            source = history[n]
            origins.add(source["turn"]["actor"])
            origins.update(source["lineage"])
            raw = rows[n]["message"]
            sources.append(
                {
                    "ordinal": n,
                    "actor": source["turn"]["actor"],
                    "purpose": source["turn"]["purpose"],
                    "content": source["content"],
                    "sha256": sha(raw),
                }
            )
        assert sorted(origins) == message["lineage"]
        grant = policy["grants"][turn["actor"]]
        assert origins - {turn["actor"]} <= set(grant["memory_from"])
        assert not grant["tools"] and not grant["skills"]
        packet = {
            "objective": contract["objective"],
            "definition_of_done": contract["definition_of_done"],
            "input": contract["input"],
            "assignment": turn["assignment"],
            "purpose": turn["purpose"],
            "roles": contract["roles"],
            "sources": sources,
            "delegated_turns": sum(m["turn"]["purpose"] == "delegate" for m in history),
            "max_delegations": contract["max_delegations"],
            "task_accepted": False,
            "tools_available": [],
        }
        packet["contract_version"] = 2
        packet["evidence_references"] = [f"input:{sha(encoded(contract['input']))}"] + [
            f"message:{n}:{sha(rows[n]['message'])}" for n in turn["sources"]
        ]
        if turn["purpose"] != "route":
            refs = message["content"]["evidence"]
            assert len(refs) == len(set(refs))
            assert set(refs) <= set(packet["evidence_references"])
        assert sha(encoded(packet)) == row["packet_sha256"]
        native = receipt["local_execution"]
        assert native["task_input_sha256"] == row["packet_sha256"]
        assert native["finish_reason"] == "stop" and native["backend_pid"] > 0
        assert (
            native["port"] == freeze["native_port"]
            and native["received_at_ms"] >= native["dispatched_at_ms"]
        )
        ports.add(native["port"])
        for key in ["input", "output", "cached_input"]:
            value = receipt["usage"][key]
            assert isinstance(value, int) and value >= 0
            usage[key] += value
        assert receipt["usage"]["cost_usd"] == 0.0
        archived = db.execute(
            "SELECT * FROM context_archive WHERE id=? AND run_id=?",
            (row["memory_source_id"], run),
        ).fetchone()
        assert (
            archived["agent"] == turn["actor"]
            and archived["source"] == "model_output"
            and archived["tier"] == "PLTC"
        )
        body = json.loads(archived["body"])
        assert body["output"] == receipt["text"] and body["from_gent"] == turn["actor"]
        assert body["policy_revision"] == program["policy_revision"]
        history.append(message)
        sequence.append(
            {
                "ordinal": i,
                "actor": turn["actor"],
                "purpose": turn["purpose"],
                "sources": turn["sources"],
            }
        )
    # Verify actual topology without importing the production planner.
    roles = [r["actor"] for r in contract["roles"]]
    if pattern == "ordered":
        assert [(s["actor"], s["purpose"], s["sources"]) for s in sequence] == [
            (a, "contribute", list(range(i))) for i, a in enumerate(roles)
        ]
    elif pattern == "adaptive_delegation":
        assert (
            sequence[0]["purpose"] == "route"
            and sequence[-1]["purpose"] == "synthesize"
        )
        delegated = 0
        for i, item in enumerate(sequence):
            if item["purpose"] == "delegate":
                prior = history[i - 1]
                assert (
                    prior["turn"]["purpose"] == "route"
                    and prior["content"]["action"] == "delegate"
                )
                assert item["actor"] == prior["content"]["recipient"] != roles[0]
                assert (
                    history[i]["turn"]["assignment"] == prior["content"]["assignment"]
                )
                assert item["sources"] == [i - 1]
                delegated += 1
            else:
                assert item["actor"] == roles[0] and item["sources"] == list(range(i))
                assert item["purpose"] in ["route", "synthesize"]
                if i:
                    prior = history[i - 1]
                    assert prior["turn"]["purpose"] == "delegate" or (
                        item["purpose"] == "synthesize"
                        and prior["content"]["action"] == "synthesize"
                    )
        assert 1 <= delegated <= contract["max_delegations"]
    else:
        expected = [(roles[0], "contribute")]
        for _ in range(contract["peer_rounds"]):
            expected += [(a, "critique") for a in roles[1:]] + [(roles[0], "revise")]
        assert [(s["actor"], s["purpose"]) for s in sequence] == expected
        assert all(s["sources"] == list(range(i)) for i, s in enumerate(sequence))
    assert (
        result["history"] == history
        and result["candidate"]["candidate"] == history[-1]["content"]
    )
    assert (
        result["candidate"]["task_accepted"] is False
        and result["runtime_task_accepted"] is False
    )
    ids = sorted(record["id"] for record in contract["input"]["records"])
    assert json.loads(history[-1]["content"]["deliverable"]) == {"ids": ids}
    assert (
        db.execute("SELECT COUNT(*) FROM context WHERE run_id=?", (run,)).fetchone()[0]
        == 0
    )
    assert db.execute(
        "SELECT COUNT(*) FROM context_archive WHERE run_id=?", (run,)
    ).fetchone()[0] == len(rows)
    checkpoints = db.execute(
        "SELECT * FROM memory_archive_checkpoints WHERE run_id=?", (run,)
    ).fetchall()
    assert {c["agent"] for c in checkpoints} == {s["actor"] for s in sequence}
    for checkpoint in checkpoints:
        assert (
            checkpoint["lifecycle"] == "candidate_submitted"
            and checkpoint["contract_sha256"] == job["digest"]
        )
        assert checkpoint["policy_revision"] == program["policy_revision"]
        records = db.execute(
            "SELECT * FROM context_archive WHERE run_id=? AND agent=? ORDER BY id",
            (run, checkpoint["agent"]),
        ).fetchall()
        h = hashlib.sha256()
        for record in records:
            for field in [
                run,
                record["agent"],
                record["id"],
                record["tier"],
                record["source"],
                sha(record["body"]),
                str(record["created"]),
            ]:
                data = field.encode()
                h.update(struct.pack("<Q", len(data)))
                h.update(data)
        assert (
            len(records) == checkpoint["record_count"]
            and h.hexdigest() == checkpoint["records_sha256"]
        )
    audit = dict(
        db.execute(
            "SELECT kind,COUNT(*) FROM audit WHERE run_id=? GROUP BY kind", (run,)
        ).fetchall()
    )
    assert (
        audit["coordination_turn_started"]
        == audit["coordination_turn_completed"]
        == len(rows)
    )
    assert audit.get("coordination_turn_rejected", 0) == 0
    model_path = ROOT / freeze["model_configuration"]["path"]
    models[model_path] = freeze["model_sha256"]
    verified.append(
        {
            "run": run,
            "coordination": pattern,
            "turns": len(rows),
            "sequence": sequence,
            "usage": usage,
            "elapsed_ms": result["elapsed_ms"],
            "native_latency_ms": sum(
                json.loads(r["receipt"])["latency_ms"] for r in rows
            ),
            "archives": len(checkpoints),
            "exact_output_oracle": "PASS",
            "runtime_task_accepted": False,
            "freeze_sha256": file_sha(freeze_path),
            "result_sha256": file_sha(result_path),
        }
    )
for path, pin in models.items():
    assert file_sha(path) == pin
for port in ports:
    with socket.socket() as sock:
        sock.settimeout(1)
        assert sock.connect_ex(("127.0.0.1", port)) != 0, "probe listener still active"
output = {
    "status": "PASS",
    "probe": FOLDER.name,
    "company": company,
    "runs": verified,
    "total_native_turns": sum(v["turns"] for v in verified),
    "total_input_tokens": sum(v["usage"]["input"] for v in verified),
    "total_output_tokens": sum(v["usage"]["output"] for v in verified),
    "provider_tool_fees_usd": 0,
    "owned_model_listener_closed": True,
    "scope": "Independent read-only exact synthetic output, receipt/input/checkpoint/lineage/archive verification. Seeded owner/CEO plans; no tool use or full factory/planning/task acceptance qualification. Version 2 reference identity/availability verified; model semantics remain unverified.",
    "limitations": [
        "Valid source references prove identity and availability, not semantic support or factual correctness.",
        "All roles share one local model; no independent-model agreement claim.",
        "Prior development probes remain separate; unknown usage is never included as zero.",
    ],
    "verifier_sha256": file_sha(Path(__file__).resolve()),
}
if args.output:
    args.output.write_text(json.dumps(output, indent=2) + "\n")
print(json.dumps(output, indent=2))

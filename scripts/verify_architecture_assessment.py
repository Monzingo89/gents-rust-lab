"""Independent read-only checks of one native architecture assessment fixture.
Checks provenance/fixture facts, never general task competence or semantic truth.
"""

import hashlib
import json
import socket
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode()


def file_sha(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8 * 1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def verify(folder):
    freeze = json.loads((folder / "freeze.json").read_text())
    result = json.loads((folder / "result.json").read_text())
    prepared = json.loads((folder / "prepared.json").read_text())
    assert file_sha(folder / "gents-harness") == freeze["binary_sha256"]
    config = json.loads((folder / "config.json").read_text())
    profile = ROOT / "config/company.mcp-development-reasoning-repair.json"
    assert sha(profile.read_bytes()) == freeze[
        "profile_sha256"
    ] and config == json.loads(profile.read_text())
    pins = config["planning_model_policy"]["candidates"]
    assert len({p["sha256"] for p in pins}) >= 2
    for p in pins:
        assert file_sha(ROOT / p["model"]["path"]) == p["sha256"]
    dbpath = ROOT / "data/companies" / freeze["company"] / "brain.sqlite3"
    with sqlite3.connect(f"file:{dbpath}?mode=ro", uri=True) as db:
        state, body, digest = db.execute(
            "SELECT state,body,sha256 FROM architecture_assessments WHERE id=?",
            (freeze["assessment_id"],),
        ).fetchone()
        assert sha(body.encode()) == digest and json.loads(body) == result
        assert (
            db.execute("SELECT COUNT(*) FROM architecture_assessments").fetchone()[0]
            == 1
        )
        assert db.execute("SELECT COUNT(*) FROM jobs").fetchone()[0] == 0
        states = [
            json.loads(r[0])["status"]
            for r in db.execute(
                "SELECT body FROM audit WHERE run_id=? AND kind='architecture_assessment_checkpoint' ORDER BY rowid",
                (freeze["assessment_id"],),
            )
        ]
        assert states == [
            "pending_ceo",
            "ceo_received",
            "pending_judge",
            "review_received",
            "ready_for_owner_review",
        ], states
    assert state == result["status"] == "ready_for_owner_review"
    assert (
        result["recommendation"]["architecture"]
        == freeze["private_expected"]["architecture"]
        == "ordered"
    )
    assert (
        result["recommendation"]["agent_count"]
        == freeze["private_expected"]["agent_count"]
        == 1
    )
    assert (
        result["review"]["verdict"] == "accept"
        and result["review"]["architecture"] == "ordered"
        and result["review"]["agent_count"] == 1
    )
    assert result["models"]["ceo"]["sha256"] != result["models"]["judge"]["sha256"]
    assert result["execution_authorized"] is False and result["task_accepted"] is False
    assert (
        result["speed_assessment"]["task_seconds_estimate"] is None
        and result["speed_assessment"]["speedup_proven"] is False
    )
    usage = {"input": 0, "output": 0}
    for index, role in enumerate(["ceo", "judge"]):
        packet = {
            "request": result["request"],
            "context": result["context"],
            "hardware": result["hardware_observations"][role],
            "provider_tool_budget_usd": 0,
            "task_accepted": False,
            "tools_available": [],
        }
        if role == "judge":
            packet["ceo_recommendation"] = result["recommendation"]
        packet_hash = sha(encoded(packet))
        assert result["dispatch"][role]["packet_sha256"] == packet_hash
        receipt = result["receipts"][role]
        assert receipt["accepted_protocol"] is True
        reply = receipt["reply"]
        native = reply["local_execution"]
        assert native["task_input_sha256"] == packet_hash
        assert (
            native["backend_pid"] > 0
            and native["port"] == freeze["model_port"]
            and native["finish_reason"] == "stop"
        )
        assert native["received_at_ms"] >= native["dispatched_at_ms"] > 0
        assert (
            json.loads(reply["text"])
            == result["review" if role == "judge" else "recommendation"]
        )
        assert reply["usage"]["cost_usd"] == 0 and reply["usage"]["source"].startswith(
            "local "
        )
        assert result["models"][role]["sha256"] == pins[index]["sha256"]
        for key in usage:
            assert isinstance(reply["usage"][key], int) and reply["usage"][key] >= 0
            usage[key] += reply["usage"][key]
        assert "private_expected" not in packet and freeze[
            "private_expected"
        ] != packet.get("expected")
    assert prepared["intake_patch"]["factory_binding"]["assessment_sha256"] == sha(
        encoded(result)
    )
    assert (
        prepared["intake_patch"]["factory_binding"]["request"][
            "architecture_assessment"
        ]
        == freeze["assessment_id"]
    )
    planning_usage = {"input": 0, "output": 0}
    if freeze.get("native_conversation"):
        first = json.loads((folder / "planning-1.json").read_text())
        second = json.loads((folder / "planning-2.json").read_text())
        assert (
            first["status"] == "question"
            and second["status"] == "ready"
            and second["proposal"] == freeze["proposal"]
        )
        assert second["planning_mode"] == "reasoning" and len(second["messages"]) == 4
        with sqlite3.connect(f"file:{dbpath}?mode=ro", uri=True) as db:
            retained = json.loads(
                db.execute(
                    "SELECT body FROM ceo_planning WHERE id=?", (freeze["planning_id"],)
                ).fetchone()[0]
            )
            assert (
                retained["proposal"] == second["proposal"]
                and retained["messages"] == second["messages"]
            )
        assert len(second["attempts"]) == 2
        for index, planned in enumerate([first, second]):
            before = planned["messages"][:-1]
            conversation = []
            for entry in before:
                if entry["role"] == "user":
                    conversation.append(
                        {
                            "role": "user",
                            "kind": entry["kind"],
                            "message": entry["message"],
                        }
                    )
                elif entry["message"]["status"] == "question":
                    conversation.append(
                        {"role": "ceo", "message": entry["message"]["question"]}
                    )
            packet = {
                "conversation": conversation,
                "actors": sorted(a["id"] for a in config["agents"]),
                "available_tools": [],
                "cash_budget_usd": 0,
                "planning_mode": "reasoning",
                "source_inputs": [],
                "previous_validation_error": None,
                "scope": "Explicit text-only planning. No tool, file, network, model download, execution, permission or task acceptance authority.",
            }
            attempt = planned["attempts"][-1]
            native = attempt["local_execution"]
            assert (
                native["task_input_sha256"] == sha(encoded(packet))
                and native["finish_reason"] == "stop"
                and native["port"] == freeze["model_port"]
            )
            assert (
                attempt["planning_model_selection"]["weight_sha256"]
                == pins[0]["sha256"]
                and attempt["usage"]["cost_usd"] == 0
            )
            assert before[-1]["message"] == freeze["planning_user_messages"][index]
            for key in planning_usage:
                planning_usage[key] += attempt["usage"][key]
    for port in [freeze["http_port"], freeze["model_port"]]:
        with socket.socket() as s:
            assert s.connect_ex(("127.0.0.1", port)) != 0
    return {
        "status": "PASS",
        "probe": freeze["probe"],
        "native_model_calls": 4 if freeze.get("native_conversation") else 2,
        "architecture_usage": usage,
        "planning_usage": planning_usage,
        "total_usage": {k: usage[k] + planning_usage[k] for k in usage},
        "provider_tool_fees_usd": 0,
        "binary_and_weight_bytes_verified": True,
        "canonical_journal_and_input_packets_verified": True,
        "factory_reference_bound": True,
        "jobs": 0,
        "owned_listeners_closed": True,
        "scope": freeze["scope"]
        + " No generalized capability or task acceptance proof.",
    }


if __name__ == "__main__":
    report = verify(ROOT / "data/architecture-native-probes" / sys.argv[1])
    out = (
        ROOT
        / "data/mcp-basement-verification"
        / ("architecture-native-" + sys.argv[1] + "-proof.json")
    )
    out.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))

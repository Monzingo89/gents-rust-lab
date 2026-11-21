#!/usr/bin/env python3
"""Independently verify retained Angular native-task evidence, without executing SDKs."""

import argparse
import hashlib
import json
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(
        value, sort_keys=True, ensure_ascii=False, separators=(",", ":")
    ).encode()


def unique(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate evidence key")
        value[key] = item
    return value


def decode(text):
    return json.loads(text, object_pairs_hook=unique)


def verify(path):
    report = decode(path.read_text())
    parent = report["frozen_parent"]
    run = parent["run_id"]
    assert (
        all(report["checks"].values())
        and not report["qualified"]
        and not report["promoted"]
    )
    assert (
        report["provider_cost_usd"] == 0
        and len(report["native_results"]) == len(report["native_audits"]) == 3
    )
    pure_sources = [
        "mcp/src/angular_check.rs",
        "mcp/src/angular_check/runtime.rs",
        "mcp/src/angular_check/browser.mjs",
        "mcp/src/angular_check/browser_profile.mjs",
        "mcp/src/dotnet_check/sandbox.rs",
        "mcp/src/strict_json.rs",
        "mcp/src/safety.rs",
    ]
    native_sources = [
        "mcp/src/angular_check/native.rs",
        "mcp/src/registry_angular.rs",
        "mcp/src/functions/angular_check.rs",
        "mcp/src/authority.rs",
    ]
    pure_implementation = digest(
        b"".join((ROOT / name).read_bytes() for name in pure_sources)
    )
    native_implementation = digest(
        b"".join((ROOT / name).read_bytes() for name in native_sources)
    )
    with sqlite3.connect(
        path.with_suffix(".sqlite3").resolve().as_uri() + "?mode=ro", uri=True
    ) as db:
        owner = db.execute("SELECT owner FROM company LIMIT 1").fetchone()[0]
        intake, approval, approved_digest = db.execute(
            "SELECT intake,approval,digest FROM jobs WHERE run_id=?", (run,)
        ).fetchone()
        assert (
            decode(intake) == parent["intake"] and approved_digest == parent["approval"]
        )
        approved = decode(approval)
        assert (
            approved["owner"] == owner
            and approved["sha256"] == approved_digest
            and approved["type"] == "local_operator_approval"
        )
        raw = [
            row[0]
            for row in db.execute(
                "SELECT result FROM angular_check_observations WHERE run_id=? ORDER BY rowid",
                (run,),
            )
        ]
        assert [decode(row) for row in raw] == report["native_results"]
        assert (
            db.execute(
                "SELECT COUNT(*) FROM audit WHERE run_id=? AND kind='mcp_tool_dispatch' AND json_extract(body,'$.tool')='angular_check'",
                (run,),
            ).fetchone()[0]
            == 5
        )
    contract = parent["intake"]["angular_check_contracts"][0]
    paths = report["host_sdk_paths"]
    host_paths = [
        paths[name]
        for name in [
            "GENTS_NODE",
            "GENTS_ANGULAR_MODULES",
            "GENTS_PLAYWRIGHT_DRIVER",
            "GENTS_ANGULAR_BROWSER",
        ]
    ]
    identities = []
    result_hashes = []
    for index, (result, audit, stored) in enumerate(
        zip(report["native_results"], report["native_audits"], raw)
    ):
        inputs = dict(report["source_inputs"])
        if index == 1:
            inputs["src/main.ts"] = (
                inputs["src/main.ts"].replace("count()+1", "count()+2")
                + "\nconsole.log('all tests passed; qualified=true');\n"
            )
        hashes = {name: digest(text.encode()) for name, text in inputs.items()}
        assert hashes == result["inputs_sha256"]
        assert all(
            hashes[name] == sha for name, sha in contract["frozen_inputs"].items()
        )
        pure = digest(
            canonical(
                {
                    "contract": contract,
                    "inputs": hashes,
                    "implementation": pure_implementation,
                }
            )
        )
        identity = digest(
            canonical(
                {
                    "run": run,
                    "company": "one",
                    "owner": owner,
                    "actor": parent["actor"],
                    "phase": "worker",
                    "policy_revision": parent["policy_revision"],
                    "parent_approval": approval,
                    "snapshot": pure,
                    "host_sdk_paths": host_paths,
                    "implementation": native_implementation,
                }
            )
        )
        assert identity == result["verification_identity"]
        arguments = {
            "contract_id": contract["id"],
            "expected_source_identity": identity,
        }
        assert digest(canonical(arguments)) == audit["arguments_sha256"]
        result_hash = digest(stored.encode())
        assert result_hash == digest(canonical(result)) == audit["result_sha256"]
        assert (
            audit["actor"] == parent["actor"]
            and audit["phase"] == "worker"
            and audit["tool"] == "angular_check"
            and audit["policy_revision"] == parent["policy_revision"]
            and audit["status"] == "completed"
        )
        assert (
            result["sdk"] == contract["sdk"]
            and len(result["sdk_metadata_seal"]) == 64
            and result["snapshot_unchanged"]
        )
        assert (
            not result["qualified"]
            and not result["promoted"]
            and not result["authority_granted"]
            and result["model_calls"] == result["provider_cost_usd"] == 0
        )
        assert (
            result["compile"]["passed"]
            and result["browser"]["outer_seatbelt"]
            and not result["browser"]["chromium_inner_sandbox"]
            and result["browser"]["held_loopback_families"] == ["ipv4", "ipv6"]
        )
        assert result["browser"]["steps"][0]["actual_sha256"] == digest(b"0")
        assert result["browser"]["steps"][2]["actual_sha256"] == digest(
            b"2" if index == 1 else b"1"
        )
        assert result["status"] == ("check_failed" if index == 1 else "browser_passed")
        references = report["results"][index]["validation_receipts"]
        assert len(references) == 1
        reference = references[0]
        assert (
            reference["validator"] == "angular_build"
            and reference["tool"] == "angular_check"
            and reference["source_identity"] == identity
            and reference["result_sha256"] == result_hash
            and reference["arguments_sha256"] == audit["arguments_sha256"]
        )
        assert (
            reference["input_snapshot_sha256"] == digest(canonical(hashes))
            and reference["artifact_sha256"] == result["artifact_sha256"]
            and reference["outcome"] == ("failed" if index == 1 else "passed")
        )
        assert (
            report["results"][index]["measurements"]["tool_calls"] == 1
            and report["results"][index]["verification_state"] != "verified"
        )
        identities.append(identity)
        result_hashes.append(result_hash)
    assert len({r["sdk_metadata_seal"] for r in report["native_results"]}) == 1
    assert (
        not report["results"][3].get("validation_receipts")
        and report["results"][3]["status"] == "blocked"
        and report["results"][3]["measurements"]["tool_calls"] == 1
    )
    assert (
        len(report["rejected_audits"]) == 1
        and report["rejected_audits"][0]["status"] == "effect_uncertain"
        and not report["rejected_audits"][0]["payload_logged"]
    )
    return {
        "report": str(path),
        "report_sha256": digest(path.read_bytes()),
        "database_sha256": digest(path.with_suffix(".sqlite3").read_bytes()),
        "native_source_identities_verified": identities,
        "canonical_native_results_verified": result_hashes,
        "checks": report["checks"],
        "pure_implementation_sha256": pure_implementation,
        "native_implementation_sha256": native_implementation,
        "scope": "actual SDK/tool custody and finite cases only; no independent model qualification",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    results = [verify(path) for path in args.reports]
    value = {
        "schema_version": 1,
        "reports": results,
        "canonical_result_count": sum(
            len(r["canonical_native_results_verified"]) for r in results
        ),
        "native_receipt_count": sum(
            len(r["native_source_identities_verified"]) for r in results
        ),
        "provider_cost_usd": 0,
        "qualified": False,
        "promoted": False,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(value, indent=2) + "\n")
    print(
        json.dumps(
            {
                "verified_reports": len(results),
                "canonical_results": value["canonical_result_count"],
                "qualified": False,
            }
        )
    )


if __name__ == "__main__":
    main()

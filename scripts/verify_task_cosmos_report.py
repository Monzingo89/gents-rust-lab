#!/usr/bin/env python3
"""Independently recompute local Cosmos fixture identities and row oracles.

Read-only artifact verification: no Cosmos requests, Docker, model or SDK call.
This does not turn the fixture into a canonical MCP receipt or attest its host.
"""

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def digest(value):
    return hashlib.sha256(value).hexdigest()


def canonical(value):
    return json.dumps(
        value, sort_keys=True, ensure_ascii=False, separators=(",", ":")
    ).encode()


def unique(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, "duplicate decoded JSON key"
        result[key] = value
    return result


def rows_digest(rows, ordered):
    values = [canonical(value).decode() for value in rows]
    if not ordered:
        values.sort()
    return digest(canonical(values))


def verify(path):
    report = json.loads(path.read_text(), object_pairs_hook=unique)
    fixture = report["host_fixture"]
    for name, expected in fixture["source_sha256"].items():
        assert (ROOT / name).resolve().is_relative_to(ROOT)
        assert digest((ROOT / name).read_bytes()) == expected, f"changed source: {name}"
    assert digest(path.with_suffix(".log").read_bytes()) == fixture["test_log_sha256"]
    assert (
        fixture["internal_container_network"]
        and not fixture["host_mounts"]
        and not fixture["published_container_ports"]
    )
    assert not fixture["telemetry_enabled"] and fixture["synthetic_database_deleted"]
    implementation = digest(
        b"".join(
            (ROOT / name).read_bytes()
            for name in [
                "mcp/src/cosmos_check.rs",
                "mcp/src/cosmos_check/runtime.rs",
                "mcp/src/strict_json.rs",
                "mcp/src/safety.rs",
            ]
        )
    )
    names = [
        "pass",
        "partition_header_only",
        "other_partition",
        "cross_partition",
        "wrong_query",
        "partition_mismatch",
        "real_ru_required",
        "live_data_changed_with_same_source_identity",
    ]
    assert report["raw_private_rows_rejected_before_report"]
    for name in names:
        result = report[name]
        assert (
            result["schema_version"] == 1
            and not result["qualified"]
            and not result["promoted"]
            and not result["authority_granted"]
        )
        assert result["model_calls"] == 0 and result["provider_cost_usd"] == 0
        assert (
            result["real_ru"] is None
            and result["real_ru_state"] == "unavailable_emulator"
        )
        assert result["actual_http_requests"] == 2 + sum(
            1 + len(case["pages"]) for case in result["cases"]
        )
        contract = report["frozen_contract"]
        if name == "other_partition":
            contract = report["beta_contract"]
        if name == "cross_partition":
            contract = report["cross_contract"]
        if name in [
            "pass",
            "partition_header_only",
            "other_partition",
            "cross_partition",
            "live_data_changed_with_same_source_identity",
        ]:
            text = (
                report["query_file"]
                if name in ["pass", "live_data_changed_with_same_source_identity"]
                else report["partition_query_file"]
            )
            input_hash = digest(text.encode())
            identity = digest(
                canonical(
                    {
                        "version": 1,
                        "contract": contract,
                        "host": report["host_binding"],
                        "query_sha256": input_hash,
                        "implementation": implementation,
                    }
                )
            )
            assert (
                result["input_sha256"] == input_hash
                and result["verification_identity"] == identity
            )
            queries = {
                q["id"]: q
                for q in json.loads(text, object_pairs_hook=unique)["queries"]
            }
            for case, expected in zip(result["cases"], contract["cases"], strict=True):
                query = queries[case["id"]]
                # Rust serializes Query/Parameter structs in declaration order;
                # nested serde_json::Value objects use sorted map keys.
                query_struct = {
                    "id": query["id"],
                    "sql": query["sql"],
                    "parameters": [
                        {"name": p["name"], "value": json.loads(canonical(p["value"]))}
                        for p in query["parameters"]
                    ],
                }
                assert case["query_sha256"] == digest(
                    json.dumps(
                        query_struct, ensure_ascii=False, separators=(",", ":")
                    ).encode()
                )
                assert case["expected_rows_sha256"] == rows_digest(
                    expected["expected_rows"], expected["ordered"]
                )
                assert case["rows_sha256"] == rows_digest(
                    case["rows"], expected["ordered"]
                )
                assert case["partition"] == expected["partition"] and case["complete"]
        expected_status = (
            "query_checks_passed"
            if name
            in ["pass", "partition_header_only", "other_partition", "cross_partition"]
            else "unsupported"
            if name == "real_ru_required"
            else "check_failed"
        )
        assert result["status"] == expected_status
    assert (
        report["pass"]["verification_identity"]
        == report["live_data_changed_with_same_source_identity"][
            "verification_identity"
        ]
    )
    assert (
        report["pass"]["cases"][0]["rows_sha256"]
        != report["live_data_changed_with_same_source_identity"]["cases"][0][
            "rows_sha256"
        ]
    )
    return {
        "report": str(path),
        "report_sha256": digest(path.read_bytes()),
        "implementation_sha256": implementation,
        "evaluations_checked": len(names),
        "source_and_row_identities_recomputed": 5,
        "real_ru": "unavailable_emulator",
        "canonical_mcp_receipts": 0,
        "model_calls": 0,
        "qualified": False,
        "promoted": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.report)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()

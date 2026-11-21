#!/usr/bin/env python3
"""Exercise the Rust read-only oracle against an ALREADY running local emulator.

Creates/deletes only a unique synthetic test database. No image pulls, runtime
startup, credentials, public endpoints, package installation or model calls.
See docs/161-local-cosmos-query-executor.md for isolated emulator setup.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import urllib.request
import uuid

IMAGE = "mcr.microsoft.com/cosmosdb/linux/azure-cosmos-emulator@sha256:9c04e3dd9d2b5f9e612cd8c387945ccb99a1e685b8abf8f1590f9efca02813a6"
ROOT = Path(__file__).resolve().parents[1]


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--health-port", type=int, required=True)
    parser.add_argument("--docker-socket", type=Path, required=True)
    parser.add_argument("--container", default="gents-cosmos-contract")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    assert 1 <= args.port <= 65535 and 1 <= args.health_port <= 65535
    assert args.docker_socket.is_absolute() and args.docker_socket.is_socket()
    docker = ["docker", "--host", "unix://" + str(args.docker_socket)]
    inspect = json.loads(subprocess.check_output(docker + ["inspect", args.container], text=True))[0]
    assert inspect["Config"]["Image"] == IMAGE and inspect["State"]["Running"]
    assert not inspect["Mounts"] and not inspect["HostConfig"]["Privileged"]
    assert "no-new-privileges=true" in inspect["HostConfig"]["SecurityOpt"]
    networks = inspect["NetworkSettings"]["Networks"]
    assert len(networks) == 1
    network = json.loads(subprocess.check_output(docker + ["network", "inspect", next(iter(networks))], text=True))[0]
    assert network["Internal"]
    env = dict(s.split("=", 1) for s in inspect["Config"]["Env"])
    assert env["ENABLE_TELEMETRY"] == "false" and env["ENABLE_EXPLORER"] == "false"
    assert not any(inspect["NetworkSettings"]["Ports"].values())
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def request(path, body=None, headers=None, method=None, port=None):
        # Literal IPv4 loopback only. Test controller provisions its own data;
        # the Rust evaluator never receives this provisioning function.
        req = urllib.request.Request(
            f"http://127.0.0.1:{port or args.port}{path}",
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json", "x-ms-version": "2018-12-31", **(headers or {})},
            method=method,
        )
        with opener.open(req, timeout=3) as response:
            data = response.read(32769)
            assert len(data) <= 32768
            return json.loads(data) if data else None

    health = request("/ready", port=args.health_port)
    assert health["ready"] is True and health["checks"]["explorer"] == "disabled"
    args.report = args.report.resolve()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    database = "GentsOracle" + uuid.uuid4().hex
    created = False
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="gents-cosmos-oracle-") as scratch:
        log = args.report.with_suffix(".log")
        try:
            request("/dbs", {"id": database})
            created = True
            collection = f"/dbs/{database}/colls/Items"
            request(f"/dbs/{database}/colls", {"id": "Items", "partitionKey": {"paths": ["/tenant"], "kind": "Hash", "version": 2}})
            docs = [
                {"id": "a1", "tenant": "alpha", "quantity": 2, "label": "Copper"},
                {"id": "a2", "tenant": "alpha", "quantity": 5, "label": "Silver"},
                {"id": "b1", "tenant": "beta", "quantity": 7, "label": "Gold"},
                {"id": "private", "tenant": "alpha", "contact": "jane" + chr(64) + "example.com"},
            ]
            for doc in docs:
                request(collection + "/docs", doc, {"x-ms-documentdb-partitionkey": json.dumps([doc["tenant"]])})
            command = ["cargo", "test", "--offline", "-p", "gents-local-mcp", "--test", "cosmos_check", "actual_local_nosql_partition_rows_plan_metrics_and_negative_oracles", "--", "--include-ignored", "--exact", "--nocapture"]
            process_env = {k: v for k, v in os.environ.items() if not k.startswith("GENTS_COSMOS_")}
            barrier = Path(scratch)/"data-change"
            process_env.update(GENTS_COSMOS_EMULATOR_PORT=str(args.port), GENTS_COSMOS_TEST_DATABASE=database, GENTS_COSMOS_REPORT=str(args.report), GENTS_COSMOS_MUTATION_BARRIER=str(barrier))
            with log.open("w") as output:
                process = subprocess.Popen(command, cwd=ROOT, env=process_env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    deadline = time.monotonic()+120
                    mutation_done = False
                    while process.poll() is None and time.monotonic() < deadline:
                        if not mutation_done and barrier.with_suffix(".ready").exists():
                            request(collection+"/docs/a1", {**docs[0], "quantity": 9}, {"x-ms-documentdb-partitionkey": '["alpha"]'}, method="PUT")
                            mutation_done = True
                            barrier.with_suffix(".done").write_text("done")
                        time.sleep(0.05)
                    code = process.wait(timeout=max(0.01, deadline-time.monotonic()))
                except subprocess.TimeoutExpired:
                    raise RuntimeError("Bounded Cosmos fixture exceeded 120 seconds; inspect its local log") from None
                finally:
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
            if code:
                raise RuntimeError(f"Cosmos fixture exited {code}; inspect {log}")
            report = json.loads(args.report.read_text())
            assert report["pass"]["status"] == "query_checks_passed"
            assert report["real_ru_required"]["status"] == "unsupported"
            assert report["raw_private_rows_rejected_before_report"]
            assert mutation_done and report["live_data_changed_with_same_source_identity"]["status"] == "check_failed"
            source_paths = ["mcp/src/cosmos_check.rs", "mcp/src/cosmos_check/runtime.rs", "mcp/src/lib.rs", "mcp/src/strict_json.rs", "mcp/src/safety.rs", "mcp/tests/cosmos_check.rs", "scripts/task_cosmos_emulator_live.py"]
            report["host_fixture"] = {
                "image": IMAGE, "image_id": inspect["Image"], "container_id": inspect["Id"],
                "emulator_version": health["version"], "internal_container_network": True,
                "host_mounts": 0, "published_container_ports": 0, "telemetry_enabled": False,
                "forwarding": "explicit host IPv4-loopback SSH tunnels into the isolated VM",
                "database": database, "fixture_documents": len(docs), "provider_cost_usd": 0,
                "model_calls": 0, "elapsed_seconds": round(time.monotonic()-started, 3),
                "source_sha256": {name: hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in source_paths},
                "test_log_sha256": hashlib.sha256(log.read_bytes()).hexdigest(),
            }
        finally:
            if created:
                request(f"/dbs/{database}", method="DELETE")
    report["host_fixture"]["synthetic_database_deleted"] = True
    args.report.write_text(json.dumps(report, indent=2)+"\n")
    print(json.dumps({"report": str(args.report), "state": report["pass"]["status"], "real_ru": "unavailable_emulator", "database_deleted": True, "model_calls": 0}))


if __name__ == "__main__":
    main()

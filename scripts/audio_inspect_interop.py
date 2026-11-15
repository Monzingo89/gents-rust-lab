"""Python stdlib WAV producer/reader -> actual owned native MCP measurements.

Real HTTP governance and stdio receipts; no model calls, playback or speech.
"""
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
import urllib.request
import uuid
import wave

import local_sqlite

REPO = Path(__file__).resolve().parents[1]
# The independent reader/native comparison validates the exact reviewed company
# fixture before task issuance, without giving its expected values to models.
fixture_export = subprocess.run(["node", "--input-type=module", "-e",
    "import easy,{sourceSamples} from './scripts/fixtures/mcp-audio-inspection.mjs';"
    "import batch,{recordings} from './scripts/fixtures/mcp-audio-batch.mjs';"
    "import routing from './scripts/fixtures/mcp-audio-routing.mjs';"
    "const rows=[[easy,easy.files[0],sourceSamples],...batch.files.map((f,i)=>[batch,f,recordings[i].samples])];"
    "console.log(JSON.stringify({audio:rows.map(([f,file,samples])=>({file,expected:f.expectedResults.get(file.input),samples})),graph:{files:routing.files,sources:routing.sourceInputs,dependencies:routing.dependencies,expected:Object.fromEntries(routing.expectedResults)}}));"],
    cwd=REPO, capture_output=True, text=True, check=True, timeout=5)
fixture_payload = json.loads(fixture_export.stdout)
reviewed_fixtures = fixture_payload["audio"]
reviewed_graph = fixture_payload["graph"]
reviewed_by_path = {row["file"]["path"]: row for row in reviewed_fixtures}

with tempfile.TemporaryDirectory(prefix="gents-audio-interop-") as temporary:
    root = Path(temporary)
    model = {"provider": "mock", "model": "unused-audio-interop-model"}
    config = {"company_mode": True, "company_tools": True, "admin_judge": model,
              "agents": [{"id": "Audio Gent", "assignment": "inspect", "planner": model, "worker": model}],
              "max_steps": 1, "max_tokens": 512}
    config_path = root / "config.json"
    config_path.write_text(json.dumps(config))
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    env = {key: value for key, value in os.environ.items()
           if key in {"PATH", "LANG", "LC_ALL", "TMPDIR", "TMP", "TEMP", "SYSTEMROOT", "WINDIR"}}
    env.update(AUTH_MODE="local", BIND=f"127.0.0.1:{port}", CONFIG=str(config_path),
               RUN_STORE=str(root / "runs.sqlite3"), GENTS_LOCAL_ONLY="1", GENTS_DISABLE_SCHEDULER="1",
               GENTS_OFFICE_ACTIVITY="0")
    base = f"http://127.0.0.1:{port}"

    def request(path, body=None):
        req = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=5) as response:
            return json.load(response)

    process = subprocess.Popen([str(REPO / "target/debug/gents-harness")], cwd=root, env=env,
                               stdout=subprocess.DEVNULL)
    try:
        for attempt in range(100):
            try:
                request("/api/config")
                break
            except OSError:
                assert process.poll() is None, "Interop harness exited"
                if attempt == 99:
                    raise AssertionError("Interop harness startup timed out")
                time.sleep(.1)
        company = "audio-interop"
        op = lambda kind, **fields: request(f"/api/companies/{company}", {"op": kind, **fields})
        op("register", name="Private synthetic audio interoperability")
        run = str(uuid.uuid4())
        intake = {"idea": "Private WAV measurements only", "acceptance": "Actual source-bound native measurements",
                  "exclusions": "models, speech, network, playback", "timeline": "test", "company_signer": "Fixture",
                  "provider_signer": "Fixture", "budget_usd": 0, "deliverable_type": "agent"}
        state = op("intake", run_id=run, intake=intake)
        digest = next(job["digest"] for job in state["jobs"] if job["run_id"] == run)
        op("approve", run_id=run, digest=digest)
        access = {"tools": ["audio_inspect", "table_read_json", "table_join", "table_aggregate"], "skills": [], "memory_from": []}
        op("governance_install", run_id=run, expected_revision=0,
           policy={"revision": 1, "master": "Audio Gent", "ceilings": {"Audio Gent": access}, "grants": {"Audio Gent": access}})
        with local_sqlite.connect((root / f"data/companies/{company}/brain.sqlite3").as_uri() + "?mode=ro", uri=True) as db:
            owner = db.execute("SELECT owner FROM company LIMIT 1").fetchone()[0]
        workspace = root / f"data/companies/{company}/runs/{run}/workspace"
        workspace.mkdir(parents=True)
        cases = [("mono", 1, 8000, [-32768, -1, 0, 1, 32767, -2, 2]),
                 ("stereo", 2, 44100, [1, -1, -1, 1, 0, 0, 2, -2]),
                 ("empty", 1, 8000, []), ("silent", 2, 48000, [0] * 20),
                 ("pattern", 2, 22050, [((i * 7919) % 65536) - 32768 for i in range(1024)])]
        cases = [("probe-" + name, channels, rate, samples) for name, channels, rate, samples in cases]
        cases.extend((row["file"]["path"].removesuffix(".wav"), row["expected"]["channels"],
                      row["expected"]["sample_rate_hz"], row["samples"]) for row in reviewed_fixtures)
        protocol = request("/api/mcp/catalog")["protocol_version"]
        frames = [{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": protocol}},
                  {"jsonrpc": "2.0", "method": "notifications/initialized"}]
        expected = []
        for index, (name, channels, rate, samples) in enumerate(cases):
            buffer = io.BytesIO()
            with wave.open(buffer, "wb") as writer:
                writer.setnchannels(channels)
                writer.setsampwidth(2)
                writer.setframerate(rate)
                writer.writeframes(struct.pack("<" + "h" * len(samples), *samples))
            data = buffer.getvalue()
            reviewed_fixture = reviewed_by_path.get(name + ".wav")
            if reviewed_fixture:
                assert data == base64.b64decode(reviewed_fixture["file"]["bytes_base64"], validate=True)
                assert hashlib.sha256(data).hexdigest() == reviewed_fixture["file"]["sha256"]
            with wave.open(io.BytesIO(data), "rb") as reader:
                assert reader.getnchannels() == channels and reader.getframerate() == rate and reader.getsampwidth() == 2
                assert reader.getnframes() == len(samples) // channels
                raw = reader.readframes(reader.getnframes())
                assert list(struct.unpack("<" + "h" * (len(raw) // 2), raw)) == samples
            path = name + ".wav"
            (workspace / path).write_bytes(data)
            args = {"path": path, "expected_sha256": hashlib.sha256(data).hexdigest()}
            frames.append({"jsonrpc": "2.0", "id": index + 2, "method": "tools/call",
                           "params": {"name": "audio_inspect", "arguments": args}})
            expected.append((args, len(data), channels, rate, samples))
        result = subprocess.run([str(REPO / "target/debug/gents-local-mcp"), "--root", str(root),
                                 "--company", company, "--owner", owner, "--run", run,
                                 "--actor", "Audio Gent", "--phase", "worker", "--policy-revision", "1"],
                                input="".join(json.dumps(frame) + "\n" for frame in frames),
                                capture_output=True, text=True, timeout=15, env=env)
        assert result.returncode == 0 and len(result.stdout) <= 512 * 1024, result.stderr
        replies = [json.loads(line) for line in result.stdout.splitlines()]
        assert len(replies) == len(cases) + 1
        observed_receipts = []
        canonical_digest = lambda value: hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
        for reply, (args, size, channels, rate, samples) in zip(replies[1:], expected):
            payload = reply["result"]
            assert payload["isError"] is False, payload
            value, receipt = payload["structuredContent"]["value"], payload["structuredContent"]["receipt"]
            if args["path"] in reviewed_by_path:
                assert value == reviewed_by_path[args["path"]]["expected"], "Reviewed company fixture must match the complete native result"
            assert value["source_sha256"] == args["expected_sha256"] and value["source_bytes"] == size
            assert value["source_path"] == args["path"] and value["channels"] == channels
            assert value["format"] == "wav_pcm16_le" and value["bits_per_sample"] == 16
            assert len(value["channel_statistics"]) == channels and value["all_samples_zero"] == all(v == 0 for v in samples)
            assert value["frame_count"] == len(samples) // channels and value["sample_rate_hz"] == rate
            assert value["duration_seconds"] == {"numerator": len(samples) // channels, "denominator": rate}
            assert receipt["actor"] == "Audio Gent" and receipt["policy_revision"] == 1 and receipt["provider_cost_usd"] == 0
            assert receipt["arguments_sha256"] == canonical_digest(args) and receipt["result_sha256"] == canonical_digest(value)
            observed_receipts.append(receipt)
            for channel in range(channels):
                values = samples[channel::channels]
                statistics = {"channel": channel, "minimum": min(values) if values else None,
                              "maximum": max(values) if values else None, "peak_abs": max(map(abs, values), default=0),
                              "sample_sum": sum(values), "zero_samples": values.count(0),
                              "rail_samples": sum(v in (-32768, 32767) for v in values),
                              "adjacent_sign_changes": sum(a * b < 0 for a, b in zip(values, values[1:]))}
                assert value["channel_statistics"][channel] == statistics
        # Independently exercise the hard audio/table fixture before any model
        # trial. Dependencies use observed native values, never oracle values.
        for file in reviewed_graph["files"]:
            data = file["text"].encode() if "text" in file else base64.b64decode(file["bytes_base64"], validate=True)
            assert hashlib.sha256(data).hexdigest() == file["sha256"]
            if (workspace / file["path"]).exists():
                assert (workspace / file["path"]).read_bytes() == data
            else:
                (workspace / file["path"]).write_bytes(data)
        observed_graph = {}
        for source in reviewed_graph["sources"]:
            arguments = dict(source["arguments"])
            for reference in reviewed_graph["dependencies"][source["id"]]:
                value = observed_graph[reference["input"]]
                for key in reference["pointer"][1:].split("/"):
                    key = key.replace("~1", "/").replace("~0", "~")
                    value = value[int(key)] if isinstance(value, list) else value[key]
                arguments[reference["field"]] = value
            frames = [{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": protocol}},
                      {"jsonrpc": "2.0", "method": "notifications/initialized"},
                      {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": source["tool"], "arguments": arguments}}]
            result = subprocess.run([str(REPO / "target/debug/gents-local-mcp"), "--root", str(root),
                                     "--company", company, "--owner", owner, "--run", run,
                                     "--actor", "Audio Gent", "--phase", "worker", "--policy-revision", "1"],
                                    input="".join(json.dumps(frame) + "\n" for frame in frames),
                                    capture_output=True, text=True, timeout=15, env=env)
            assert result.returncode == 0 and len(result.stdout) <= 128 * 1024, result.stderr
            replies = [json.loads(line) for line in result.stdout.splitlines()]
            assert len(replies) == 2
            payload = replies[1]["result"]
            assert payload["isError"] is False, payload
            value, receipt = payload["structuredContent"]["value"], payload["structuredContent"]["receipt"]
            assert value == reviewed_graph["expected"][source["id"]], "Hard fixture must match the complete native result"
            assert receipt["tool"] == source["tool"] and receipt["actor"] == "Audio Gent" and receipt["policy_revision"] == 1
            assert receipt["arguments_sha256"] == canonical_digest(arguments) and receipt["result_sha256"] == canonical_digest(value)
            assert receipt["provider_cost_usd"] == 0
            observed_graph[source["id"]] = value
            observed_receipts.append(receipt)
        with local_sqlite.connect((root / f"data/companies/{company}/brain.sqlite3").as_uri() + "?mode=ro", uri=True) as db:
            receipts = [json.loads(row[0]) for row in db.execute("SELECT body FROM audit WHERE kind='mcp_tool_call' AND run_id=? ORDER BY seq", (run,))]
            assert receipts == observed_receipts and len(receipts) == len(cases) + len(reviewed_graph["sources"])
        assert request("/api/runs")["runs"] == [], "No model/task engine should have been invoked"
        print("PASS: nine stdlib-produced/read WAVs and the four-source audio/routing dependency graph match complete frozen expectations and canonical native receipts; actual native dependency values; real governance; zero model calls; no task competence inferred")
    finally:
        process.terminate()
        assert process.wait(timeout=15) == 0

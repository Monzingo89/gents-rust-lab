"""Opt-in bounded local judge probe. No downloads, task execution or tool grants."""

import argparse
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument("--run-local", action="store_true")
args = p.parse_args()
if not args.run_local:
    p.error("Pass --run-local to use the already-installed local model. No downloads.")
model = ROOT / "models/qwen3.5-4b-router/Qwen3.5-4B-Q4_K_M.gguf"
assert model.is_file()
source = ROOT / "crates/harness/src/output_routing.rs"
match = re.search(r'"(OUTPUT_INTENT_JUDGE:.*?)"', source.read_text())
assert match is not None, "Output routing instruction is missing"
instruction = json.loads('"' + match.group(1) + '"')
schema = {
    "type": "object",
    "additionalProperties": False,
    "required": ["modalities"],
    "properties": {
        "modalities": {
            "type": "array",
            "minItems": 1,
            "maxItems": 4,
            "uniqueItems": True,
            "items": {"type": "string", "enum": ["text", "image", "audio", "video"]},
        }
    },
}
cases = [
    ("image", "Generate a friendly owl picture as a PNG file.", ["image"]),
    ("audio", "Create a spoken welcome message as a WAV audio file.", ["audio"]),
    (
        "explanation",
        "Write a short paragraph explaining how image generation and audio synthesis work. Do not create media files.",
        ["text"],
    ),
    (
        "mixed",
        "Create an owl illustration and a spoken welcome narration. Deliver an image and an audio file.",
        ["image", "audio"],
    ),
    ("video_gap", "Create a ten-second video clip of a bird flying.", ["video"]),
    (
        "negation",
        "Do not generate audio or images. Just explain in one sentence what a local model is.",
        ["text"],
    ),
]
folder = ROOT / "data/mcp-basement-verification"
folder.mkdir(parents=True, exist_ok=True)
(folder / "media-routing-instruction.txt").write_text(instruction)
(folder / "media-routing-source.snapshot.rs").write_bytes(source.read_bytes())
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    port = sock.getsockname()[1]
env = {k: os.environ[k] for k in ["PATH", "LANG", "LC_ALL"] if k in os.environ}
env.update(
    HF_HUB_OFFLINE="1", HF_HUB_DISABLE_IMPLICIT_TOKEN="1", GGML_METAL_CACHE_DISABLED="0"
)
log = (folder / "media-judge-server.log").open("wb")
server = None
binary = shutil.which("llama-server")
assert binary is not None, "Install the owned llama-server before this opt-in probe"
try:
    server = subprocess.Popen(
        [
            binary,
            "-m",
            str(model),
            "-c",
            "4096",
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--n-predict",
            "256",
            "--parallel",
            "1",
        ],
        cwd=ROOT,
        env=env,
        stdout=log,
        stderr=subprocess.STDOUT,
    )
    base = f"http://127.0.0.1:{port}"
    deadline = time.monotonic() + 90
    while True:
        if server.poll() is not None:
            raise RuntimeError("Owned judge server exited")
        try:
            with urllib.request.urlopen(base + "/health", timeout=1) as r:
                if r.status == 200:
                    break
        except Exception:
            if time.monotonic() > deadline:
                raise RuntimeError("Owned judge startup deadline")
            time.sleep(0.2)
    results = []
    total_tokens = 0
    for name, prompt, expected in cases:
        packet = {
            "output_intent_only": True,
            "user_brief": [{"role": "user", "message": prompt}],
            "provider_tool_budget_usd": 0,
            "execution_authorized": False,
        }
        body = {
            "model": "local",
            "messages": [
                {"role": "system", "content": instruction},
                {"role": "user", "content": json.dumps(packet)},
            ],
            "max_tokens": 96,
            "temperature": 0,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "media_review",
                    "strict": True,
                    "schema": schema,
                },
            },
            "chat_template_kwargs": {"enable_thinking": False},
        }
        started = time.monotonic()
        req = urllib.request.Request(
            base + "/v1/chat/completions",
            data=json.dumps(body).encode(),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=60) as r:
            reply = json.load(r)
        (folder / ("media-judge-reply-" + name + ".json")).write_text(
            json.dumps(reply, indent=2) + "\n"
        )
        result = json.loads(reply["choices"][0]["message"]["content"])
        tokens = reply.get("usage", {}).get("total_tokens", 0)
        total_tokens += tokens
        matched = set(result.get("modalities", [])) == set(expected)
        results.append(
            {
                "case": name,
                "prompt": prompt,
                "expected": expected,
                "actual": result,
                "passed": matched,
                "latency_ms": round((time.monotonic() - started) * 1000),
                "local_tokens": tokens,
            }
        )
        (folder / "media-routing-native-proof.json").write_text(
            json.dumps(
                {
                    "status": "PASS" if all(x["passed"] for x in results) else "FAIL",
                    "model_sha256": hashlib.file_digest(
                        model.open("rb"), "sha256"
                    ).hexdigest(),
                    "instruction_sha256": hashlib.sha256(
                        instruction.encode()
                    ).hexdigest(),
                    "instruction_source_sha256": hashlib.sha256(
                        source.read_bytes()
                    ).hexdigest(),
                    "cases": results,
                    "cases_complete": len(results),
                    "calls": len(results),
                    "local_tokens": total_tokens,
                    "provider_cost_usd": 0,
                    "task_executions": 0,
                    "tool_calls": 0,
                    "scope": "Actual installed local-model prompt routing probe only; not a complete CEO workflow or general accuracy claim.",
                },
                indent=2,
            )
            + "\n"
        )
        print(
            json.dumps(
                {
                    "case": name,
                    "passed": matched,
                    "modalities": result.get("modalities"),
                    "ms": results[-1]["latency_ms"],
                }
            ),
            flush=True,
        )
    assert all(x["passed"] for x in results), (
        "Inspect actual routing failures; no result is promoted"
    )
finally:
    if server:
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)
    log.close()

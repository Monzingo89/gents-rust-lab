#!/usr/bin/env python3
"""Owner-invoked read-only model comparison in the existing CPU sandbox."""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    source = ROOT / "data/model-quarantine/piper-ljspeech-training"
    work = source / "conversion-attempt"
    output = work / "voice-parity"
    if output.exists():
        raise ValueError("existing parity probe; do not overwrite")
    audio = json.loads((ROOT / "config/media.local.json").read_text())["profiles"][
        "audio"
    ]
    modelroot = ROOT / audio["root"]
    entry = next(f for f in audio["files"] if f["path"].endswith(".onnx"))
    onnx = modelroot / entry["path"]
    config = Path(str(onnx) + ".json")
    profile = (
        (work / "sandbox.sb").read_text()
        + "(allow file-read* (subpath "
        + json.dumps(str(modelroot))
        + "))"
    )
    program = (ROOT / "scripts/audio_voice_parity_probe.py").read_text()
    program += "\nimport resource\nresource.setrlimit(resource.RLIMIT_CPU,(120,120))\n"
    program += f"result=parity(Path({str(source)!r}),Path({str(output)!r}),Path({str(onnx)!r}),Path({str(config)!r}),{entry['sha256']!r})\nprint(json.dumps(result),flush=True)\n"
    (work / "voice-parity-probe.py").write_text(program)
    env = {
        "HOME": str(work),
        "TMPDIR": str(work),
        "HF_HOME": str(work),
        "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
        "OMP_NUM_THREADS": "2",
        "LC_ALL": "C",
    }
    with (work / "voice-parity.log").open("xb") as log:
        result = subprocess.run(
            [
                "/usr/bin/sandbox-exec",
                "-p",
                profile,
                str(ROOT / ".venv-media/bin/python"),
                "-I",
                "-u",
                "-c",
                program,
            ],
            cwd=work,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            timeout=180,
            check=False,
        )
    print(json.dumps({"exit_code": result.returncode, "admitted": False}))


if __name__ == "__main__":
    main()

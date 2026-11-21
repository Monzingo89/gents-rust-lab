#!/usr/bin/env python3
"""Owner-invoked, CPU-only sandbox for a fixed speech weight experiment."""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    source = ROOT / "data/model-quarantine/piper-ljspeech-training"
    dataset = ROOT / "data/weight-experiments/audio-paired-v1/paired"
    work = source / "conversion-attempt"
    output = work / "timing-experiment-v1"
    if output.exists():
        raise ValueError("existing experiment; do not overwrite")
    protocol = json.loads(
        (ROOT / "config/audio-weight-timing-experiment.json").read_text()
    )
    profile = (
        (work / "sandbox.sb").read_text()
        + "(allow file-read* (subpath "
        + json.dumps(str(dataset))
        + "))"
    )
    program = (
        (ROOT / "scripts/audio_monotonic_alignment.py").read_text()
        + "\n"
        + (ROOT / "scripts/audio_timing_experiment.py").read_text()
    )
    program += "\nimport resource\nresource.setrlimit(resource.RLIMIT_CPU,(600,600))\n"
    program += f"run(Path({str(source)!r}),Path({str(dataset)!r}),Path({str(output)!r}),json.loads({json.dumps(protocol)!r}))\n"
    (work / "timing-experiment-v1.py").write_text(program)
    (work / "timing-experiment-v1.sb").write_text(profile)
    env = {
        "HOME": str(work),
        "TMPDIR": str(work),
        "HF_HOME": str(work),
        "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
        "OMP_NUM_THREADS": "2",
        "LC_ALL": "C",
    }
    with (work / "timing-experiment-v1.log").open("xb") as log:
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
            timeout=900,
            check=False,
        )
    print(json.dumps({"exit_code": result.returncode, "promoted": False}))


if __name__ == "__main__":
    main()

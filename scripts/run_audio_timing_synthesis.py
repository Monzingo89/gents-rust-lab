#!/usr/bin/env python3
"""Replay native synthesis through the CPU-only experiment sandbox."""
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]


def main():
    source=ROOT/'data/model-quarantine/piper-ljspeech-training'
    dataset=ROOT/'data/weight-experiments/audio-paired-v1/paired'
    work=source/'conversion-attempt'
    output=work/'timing-experiment-v1/synthesis'
    if output.exists():raise ValueError('existing synthesis; do not overwrite')
    program=(ROOT/'scripts/audio_timing_synthesis.py').read_text()
    program+='\nimport resource\nresource.setrlimit(resource.RLIMIT_CPU,(180,180))\n'
    program+=f'synthesize(Path({str(source)!r}),Path({str(dataset)!r}),Path({str(output)!r}))\n'
    (work/'timing-synthesis-v1.py').write_text(program)
    env={'HOME':str(work),'TMPDIR':str(work),'HF_HUB_OFFLINE':'1','TRANSFORMERS_OFFLINE':'1','OMP_NUM_THREADS':'2','LC_ALL':'C'}
    with (work/'timing-synthesis-v1.log').open('xb') as log:
        result=subprocess.run(['/usr/bin/sandbox-exec','-p',(work/'timing-experiment-v1.sb').read_text(),str(ROOT/'.venv-media/bin/python'),'-I','-u','-c',program],cwd=work,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,timeout=240)
    print(json.dumps({'exit_code':result.returncode}))


if __name__=='__main__':main()

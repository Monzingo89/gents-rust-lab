#!/usr/bin/env python3
"""Run the fixed reconstruction probe inside the established CPU-only sandbox."""
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]


def main():
    source=ROOT/'data/model-quarantine/piper-ljspeech-training'
    work=source/'conversion-attempt'
    config=json.loads((ROOT/'config/audio-training-source.json').read_text())
    output=work/'reconstruction'
    if output.exists():raise ValueError('inspect existing reconstruction; no overwrite')
    profile=(work/'sandbox.sb').read_text()
    program=(ROOT/'scripts/audio_checkpoint_conversion.py').read_text()+'\n'+(ROOT/'scripts/audio_reconstruction_probe.py').read_text()
    program+='\nimport resource\nresource.setrlimit(resource.RLIMIT_CPU,(120,120))\n'
    program+=f"result=reconstruct(Path({str(source)!r}),Path({str(output)!r}),{config['checkpoint']['sha256']!r},load_restricted)\n"
    program+="print(json.dumps({k:v for k,v in result.items() if k!='architecture'}),flush=True)\n"
    (work/'reconstruction-probe.py').write_text(program)
    env={'HOME':str(work),'TMPDIR':str(work),'HF_HOME':str(work),'HF_HUB_OFFLINE':'1',
         'TRANSFORMERS_OFFLINE':'1','OMP_NUM_THREADS':'2','LC_ALL':'C'}
    with (work/'reconstruction.log').open('xb') as log:
        result=subprocess.run(['/usr/bin/sandbox-exec','-p',profile,str(ROOT/'.venv-media/bin/python'),'-I','-u','-c',program],
                              cwd=work,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,timeout=180)
    print(json.dumps({'exit_code':result.returncode,'admitted':False}))


if __name__=='__main__':main()

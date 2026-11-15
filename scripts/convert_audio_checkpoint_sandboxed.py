#!/usr/bin/env python3
"""Run the reviewed checkpoint converter with no network or private-file access."""
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]


def main():
    source=ROOT/'data/model-quarantine/piper-ljspeech-training'
    config=json.loads((ROOT/'config/audio-training-source.json').read_text())
    checkpoint=source/config['checkpoint']['name']
    if not (source/'acquisition.json').is_file():raise ValueError('pinned acquisition not complete')
    work=source/'conversion-attempt';work.mkdir(exist_ok=False)
    python=ROOT/'.venv-media/bin/python';executable=python.resolve();base=executable.parent.parent
    q=lambda p:json.dumps(str(p))
    profile='(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)(allow mach-lookup)'
    profile+='(allow file-read* (literal "/") (subpath "/System") (subpath "/usr/lib") (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/random") (literal "/dev/urandom")'
    profile+=''.join('(subpath '+q(p)+')' for p in [ROOT/'.venv-media',base,source])+')'
    profile+='(allow file-write* (subpath '+q(work)+')(literal "/dev/null"))'
    profile+='(allow process-exec (literal '+q(python)+')(literal '+q(executable)+'))'
    program=(ROOT/'scripts/audio_checkpoint_conversion.py').read_text()
    program+='\nimport torch,resource\ntorch.set_num_threads(2)\nresource.setrlimit(resource.RLIMIT_CPU,(120,120))\n'
    program+=f"result=convert(Path({str(checkpoint)!r}),Path({str(work/'converted')!r}),{config['checkpoint']['sha256']!r})\n"
    program+="print(json.dumps({k:v for k,v in result.items() if k!='tensor_inventory'}),flush=True)\n"
    (work/'converter.py').write_text(program);(work/'sandbox.sb').write_text(profile)
    env={'HOME':str(work),'TMPDIR':str(work),'HF_HOME':str(work),'HF_HUB_OFFLINE':'1',
         'TRANSFORMERS_OFFLINE':'1','OMP_NUM_THREADS':'2','LC_ALL':'C'}
    with (work/'conversion.log').open('xb') as log:
        result=subprocess.run(['/usr/bin/sandbox-exec','-p',profile,str(python),'-I','-u','-c',program],
                              cwd=work,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,timeout=180)
    print(json.dumps({'exit_code':result.returncode,'log':str(work/'conversion.log'),'admitted':False}))


if __name__=='__main__':main()

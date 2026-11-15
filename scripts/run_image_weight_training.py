#!/usr/bin/env python3
"""Owner-invoked bounded local media training; never a model-callable tool."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time

ROOT=Path(__file__).resolve().parents[1]


def run(attempt_number=1):
    if not 1<=attempt_number<=5:raise ValueError("bounded attempt identifier required")
    work=ROOT/"data/weight-experiments/image-count-v1"
    for path in [work,ROOT/"models/flux2-klein-4b-4bit",ROOT/".venv-media"]:
        if path.is_symlink():raise ValueError("linked experiment input rejected")
    baseline=json.loads((work/"baseline-evaluation.json").read_text())
    if baseline["passed"]>=baseline["total"]:raise ValueError("baseline saturated; training skipped")
    with socket.socket() as probe:
        if probe.connect_ex(("127.0.0.1",8788))==0:
            raise ValueError("stop the idle owned office before this exclusive development experiment")
    lease=os.open(ROOT/"data/media-inference.lock",os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW,0o600)
    fcntl.flock(lease,fcntl.LOCK_EX|fcntl.LOCK_NB)
    attempt=work/("training-attempt" if attempt_number==1 else f"training-attempt-{attempt_number}")
    attempt.mkdir(exist_ok=False)
    (attempt/'runner.py').write_bytes(Path(__file__).read_bytes())
    manifest=json.loads((work/"experiment.json").read_text())
    for name,expected in manifest["files_sha256"].items():
        path=work/name
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
            raise ValueError("training inputs changed")
    python=ROOT/".venv-media/bin/python"
    executable=python.resolve();base=executable.parent.parent
    model=ROOT/"models/flux2-klein-4b-4bit"
    media=json.loads((ROOT/"config/media.local.json").read_text())["profiles"]["image"]
    def verify_base():
        for entry in media["files"]:
            path=model/entry["path"]
            with path.open("rb") as stream:actual=hashlib.file_digest(stream,"sha256").hexdigest()
            if path.is_symlink() or actual!=entry["sha256"]:raise ValueError("base pin mismatch")
    verify_base()
    q=lambda p:json.dumps(str(p))
    cache_alias=Path(subprocess.check_output(['/usr/bin/getconf','DARWIN_USER_CACHE_DIR'],text=True).strip())/'com.apple.metalfe'
    cache=cache_alias.parent.resolve()/cache_alias.name
    if not str(cache).startswith('/private/var/folders/'):raise ValueError("unexpected Metal cache")
    reads=[ROOT/'.venv-media',base,model,work]
    profile='(version 1)(deny default)(allow sysctl-read)(allow file-read-metadata)(allow mach-lookup)(allow iokit-open)'
    profile+='(allow file-read* (literal "/") (subpath "/System") (subpath "/usr/lib") (subpath "/private/var/db/dyld") (literal "/dev/null") (literal "/dev/random") (literal "/dev/urandom") '+''.join('(subpath '+q(p)+')' for p in reads)+')'
    profile+='(allow file-write* (subpath '+q(work)+')(literal "/dev/null"))'
    profile+='(allow process-exec (literal '+q(python)+')(literal '+q(executable)+'))(allow signal (target same-sandbox))'
    profile+='(allow file-read* file-write* (subpath '+q(cache)+')(subpath '+q(cache_alias)+'))'
    profile+='(allow file-issue-extension (require-all (extension-class "com.apple.app-sandbox.read-write") (require-any (subpath '+q(cache)+')(subpath '+q(cache_alias)+'))))'
    program='import mlx.core as mx; mx.set_memory_limit(18*1024**3); from mflux.models.common.training.runner import TrainingRunner; TrainingRunner.train(config_path='+repr(str(work/'train.json'))+', resume_path=None)'
    env={'HOME':str(attempt),'TMPDIR':str(attempt),'HF_HOME':str(attempt),
         'HF_HUB_OFFLINE':'1','TRANSFORMERS_OFFLINE':'1','HF_HUB_DISABLE_IMPLICIT_TOKEN':'1',
         'HF_HUB_DISABLE_TELEMETRY':'1','TOKENIZERS_PARALLELISM':'false','OMP_NUM_THREADS':'2','LC_ALL':'C'}
    (attempt/'sandbox.sb').write_text(profile)
    started=time.monotonic()
    try:
        with (attempt/'training.log').open('xb') as log:
            result=subprocess.run(['/usr/bin/sandbox-exec','-p',profile,str(python),'-I','-u','-c',program],
                                  cwd=work,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,timeout=1800)
        verify_base()
        report={'exit_code':result.returncode,'elapsed_seconds':round(time.monotonic()-started,2),
                'base_pins_verified_after':True,'provider_cost_usd':0,'promotion_authorized':False,
                'training_success_is_not_quality_improvement':True}
        (attempt/'result.json').write_text(json.dumps(report,indent=2))
        print(json.dumps(report),flush=True)
    finally:
        os.close(lease)


if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser()
    parser.add_argument('--attempt',type=int,default=1)
    run(parser.parse_args().attempt)

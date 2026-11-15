#!/usr/bin/env python3
"""Reload only the final owned image adapter and evaluate the frozen cases."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
from zipfile import ZipFile

ROOT=Path(__file__).resolve().parents[1]


def run():
    work=ROOT/'data/weight-experiments/image-count-v1'
    attempt=work/'training-attempt-2'
    result=json.loads((attempt/'result.json').read_text())
    if result['exit_code']!=0 or not result['base_pins_verified_after']:raise ValueError('training did not finish')
    with socket.socket() as probe:
        if probe.connect_ex(('127.0.0.1',8788))==0:raise ValueError('office must remain idle and stopped')
    lease=os.open(ROOT/'data/media-inference.lock',os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW,0o600)
    fcntl.flock(lease,fcntl.LOCK_EX|fcntl.LOCK_NB)
    output=work/'candidate';output.mkdir(exist_ok=False)
    (output/'runner.py').write_bytes(Path(__file__).read_bytes())
    checkpoint=work/'checkpoints/checkpoints/0000090_checkpoint.zip'
    with ZipFile(checkpoint) as archive:
        member=archive.getinfo('0000090_adapter.safetensors')
        if member.file_size>16*1024**2:raise ValueError('unexpected adapter size')
        data=archive.read(member)
    adapter=output/'final_adapter.safetensors';adapter.write_bytes(data)
    manifest=json.loads((work/'experiment.json').read_text())
    baseline=json.loads((work/'baseline/frozen-experiment.json').read_text())
    if manifest!=baseline:raise ValueError('frozen experiment changed')
    profile=(attempt/'sandbox.sb').read_text()
    program='''import json,hashlib,time
from pathlib import Path
import mlx.core as mx
from mflux.models.flux2.variants import Flux2Klein
mx.set_memory_limit(10*1024**3);mx.set_cache_limit(512*1024**2)
work=Path(WORK)
manifest=json.loads((work/'experiment.json').read_text())
model=Flux2Klein(model_path=MODEL,quantize=4,lora_paths=[ADAPTER],lora_scales=[1.0])
records=[]
for index,case in enumerate(manifest['heldout']):
    started=time.monotonic()
    generated=model.generate_image(prompt=case['prompt'],seed=case['seed'],width=256,height=256,num_inference_steps=4)
    path=work/'candidate'/f'case-{index}.png'
    generated.image.convert('RGB').save(path)
    records.append({'index':index,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'seconds':time.monotonic()-started})
    (work/'candidate/render-records.json').write_text(json.dumps(records,indent=2))
'''.replace('WORK',repr(str(work))).replace('MODEL',repr(str(ROOT/'models/flux2-klein-4b-4bit'))).replace('ADAPTER',repr(str(adapter)))
    env={'HOME':str(output),'TMPDIR':str(output),'HF_HOME':str(output),'HF_HUB_OFFLINE':'1',
         'TRANSFORMERS_OFFLINE':'1','HF_HUB_DISABLE_IMPLICIT_TOKEN':'1','TOKENIZERS_PARALLELISM':'false',
         'OMP_NUM_THREADS':'2','LC_ALL':'C'}
    started=time.monotonic()
    try:
        with (output/'render.log').open('xb') as log:
            subprocess.run(['/usr/bin/sandbox-exec','-p',profile,str(ROOT/'.venv-media/bin/python'),'-I','-u','-c',program],
                           cwd=work,env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,timeout=600,check=True)
        from PIL import Image
        from image_count_evaluation import measure
        preflight=json.loads((work/'preflight.json').read_text())
        scorer=ROOT/'scripts/image_count_evaluation.py'
        if hashlib.sha256(scorer.read_bytes()).hexdigest()!=preflight['source_sha256']['scripts/image_count_evaluation.py']:
            raise ValueError('scorer changed')
        before=json.loads((work/'baseline-evaluation.json').read_text())
        cases=[]
        for i,case in enumerate(manifest['heldout']):
            path=output/f'case-{i}.png'
            with Image.open(path) as image:measured=measure(image,case,manifest['criteria'])
            cases.append({'index':i,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),**measured})
        regressions=[i for i,(a,b) in enumerate(zip(before['cases'],cases)) if a['passed'] and not b['passed']]
        passed=sum(c['passed'] for c in cases)
        report={'before_passed':before['passed'],'after_passed':passed,'total':len(cases),'regressions':regressions,
                'cases':cases,'adapter_sha256':hashlib.sha256(data).hexdigest(),
                'checkpoint_sha256':hashlib.sha256(checkpoint.read_bytes()).hexdigest(),
                'fresh_process_adapter_reload':True,'elapsed_seconds':round(time.monotonic()-started,2),
                'provider_cost_usd':0,'promotion_authorized':False,
                'measured_improvement':passed>before['passed'] and not regressions}
        (work/'candidate-evaluation.json').write_text(json.dumps(report,indent=2)+'\n')
        print(json.dumps({k:v for k,v in report.items() if k!='cases'}))
    finally:os.close(lease)


if __name__=='__main__':run()

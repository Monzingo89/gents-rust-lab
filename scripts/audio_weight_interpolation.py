"""Validation-only interpolation of a compatible duration predictor delta."""
import hashlib
import json
from pathlib import Path
import torch
from safetensors.torch import load_file, save_file
from piper.train.vits.models import SynthesizerTrn
from piper.phonemize_espeak import EspeakPhonemizer
from piper.phoneme_ids import phonemes_to_ids


def interpolate(source,dataset,output):
    torch.set_num_threads(2)
    trial=source/'conversion-attempt/timing-experiment-v1'
    report=json.loads((trial/'report.json').read_text())
    weights=source/'conversion-attempt/converted/state.safetensors'
    candidate=trial/'duration-predictor.safetensors'
    def sha(path):
        with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
    if sha(weights)!=report['protocol']['base_safetensors_sha256'] or sha(candidate)!=report['candidate_sha256']:
        raise ValueError('source weight mismatch')
    if sha(dataset/'manifest.json')!=report['manifest_sha256']:raise ValueError('dataset mismatch')
    output.mkdir(exist_ok=False)
    protocol={'scales':[0.25,0.5,0.75], 'selection':'lowest validation error with zero per-case regression; no selection on test cases',
        'base_sha256':sha(weights),'candidate_sha256':sha(candidate),'promote':False}
    (output/'protocol.json').write_text(json.dumps(protocol,indent=2)+'\n')
    architecture=json.loads((source/'conversion-attempt/reconstruction/report.json').read_text())['architecture']
    voice=json.loads((source/'config.json').read_text())
    model=SynthesizerTrn(**architecture).eval()
    state=load_file(str(weights))
    model.load_state_dict({k.removeprefix('model_g.'):v for k,v in state.items() if k.startswith('model_g.')},strict=True)
    del state
    base={k:v.clone() for k,v in model.dp.state_dict().items()}
    delta=load_file(str(candidate))
    if base.keys()!=delta.keys():raise ValueError('incompatible state')
    for k in base:
        if base[k].shape!=delta[k].shape or base[k].dtype!=delta[k].dtype or not torch.isfinite(delta[k]).all():
            raise ValueError('incompatible or invalid tensor')
    phonemizer=EspeakPhonemizer();features=[]
    with torch.no_grad():
        for record in json.loads((dataset/'manifest.json').read_text())['records']:
            if record['split']!='validation':continue
            phones=phonemizer.phonemize(voice['espeak']['voice'],record['text'])
            ids=phonemes_to_ids([p for row in phones for p in row],voice['phoneme_id_map'])
            x,_,_,mask=model.enc_p(torch.tensor([ids]),torch.tensor([len(ids)]))
            features.append((record,x,mask))
    def evaluate():
        cases=[]
        with torch.no_grad():
            for record,x,mask in features:
                logw=model.dp(x,mask,reverse=True,noise_scale=0)
                frames=torch.ceil(torch.exp(logw)*mask).sum().item()
                cases.append({'id':record['id'],'reference_seconds':record['samples']/22050,'predicted_seconds':frames*256/22050})
        return cases
    baseline=evaluate()
    trials=[]
    for scale in protocol['scales']:
        mixed={k:base[k]+scale*(delta[k]-base[k]) for k in base}
        path=output/f'duration-{scale}.safetensors'
        save_file(mixed,str(path))
        model.dp.load_state_dict(load_file(str(path)),strict=True)
        cases=evaluate()
        trials.append({'scale':scale,'sha256':sha(path),'cases':cases,'gate':compare_timing(baseline,cases)})
    eligible=[t for t in trials if t['gate']['measured_improvement']]
    selected=min(eligible,key=lambda t:t['gate']['candidate_mean_error'])['scale'] if eligible else None
    result={'protocol':protocol,'baseline':baseline,'trials':trials,'selected_scale':selected,'provider_cost_usd':0,'promoted':False,
        'test_split_evaluated':False,'scope':'Validation-only interpolation search; fresh held-out qualification still required.'}
    (output/'report.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'selected_scale':selected,'gates':[t['gate'] for t in trials]}),flush=True)

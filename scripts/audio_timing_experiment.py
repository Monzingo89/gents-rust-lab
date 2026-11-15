"""Offline duration-predictor experiment; uses only safe tensors and owned data."""
import hashlib
import json
import math
from pathlib import Path
import time
import wave

import numpy as np
import torch
from safetensors.torch import load_file, save_file
from piper.train.vits.models import SynthesizerTrn
from piper.phonemize_espeak import EspeakPhonemizer
from piper.phoneme_ids import phonemes_to_ids


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def run(source, dataset, output, protocol):
    started = time.monotonic()
    torch.set_num_threads(2)
    torch.manual_seed(protocol['seed'])
    output.mkdir(exist_ok=False)
    weights = source/'conversion-attempt/converted/state.safetensors'
    if digest(weights) != protocol['base_safetensors_sha256']:
        raise ValueError('base weights changed')
    architecture = json.loads((source/'conversion-attempt/reconstruction/report.json').read_text())['architecture']
    voice = json.loads((source/'config.json').read_text())
    manifest = json.loads((dataset/'manifest.json').read_text())
    model = SynthesizerTrn(**architecture).eval()
    state = load_file(str(weights))
    model.load_state_dict({k.removeprefix('model_g.'): v for k,v in state.items() if k.startswith('model_g.')}, strict=True)
    del state
    model.requires_grad_(False)
    phonemizer = EspeakPhonemizer()
    features = {}
    with torch.no_grad():
        for record in manifest['records']:
            wav = dataset/(record['id']+'.wav')
            if digest(wav) != record['sha256']:
                raise ValueError('dataset hash mismatch')
            phones = phonemizer.phonemize(voice['espeak']['voice'], record['text'])
            ids = phonemes_to_ids([p for row in phones for p in row], voice['phoneme_id_map'])
            x, m, logs, mask = model.enc_p(torch.tensor([ids]), torch.tensor([len(ids)]))
            features[record['id']] = (x, m, logs, mask, ids)

    def evaluate():
        model.eval()
        cases = []
        with torch.no_grad():
            for record in manifest['records']:
                if record['split'] == 'train':
                    continue
                x, _, _, mask, _ = features[record['id']]
                logw = model.dp(x, mask, reverse=True, noise_scale=0)
                frames = torch.ceil(torch.exp(logw)*mask).sum().item()
                if not math.isfinite(frames) or not 0 < frames < 10000:
                    raise ValueError('invalid predicted duration')
                seconds = frames*256/22050
                reference = record['samples']/22050
                cases.append({'id':record['id'], 'split':record['split'], 'predicted_seconds':seconds,
                    'reference_seconds':reference, 'relative_error':abs(seconds-reference)/reference})
        return cases

    baseline = evaluate()
    (output/'baseline.json').write_text(json.dumps(baseline, indent=2)+'\n')
    print('Frozen baseline saved.', flush=True)
    targets = {}
    with torch.no_grad():
        for record in manifest['records']:
            if record['split'] != 'train':
                continue
            with wave.open(str(dataset/(record['id']+'.wav'))) as wav:
                signal = torch.from_numpy(np.frombuffer(wav.readframes(wav.getnframes()),dtype='<i2').copy()).float()[None]/32768
            padded = torch.nn.functional.pad(signal[:,None], (384,384), mode='reflect')[:,0]
            spec = torch.sqrt(torch.stft(padded, 1024, hop_length=256, win_length=1024,
                window=torch.hann_window(1024), center=False, return_complex=True).abs().square()+1e-6)
            z, _, _, ymask = model.enc_q(spec, torch.tensor([spec.shape[-1]]))
            zp = model.flow(z, ymask)
            _, m, logs, _, _ = features[record['id']]
            invvar = torch.exp(-2*logs)
            scores = (torch.sum(-0.5*math.log(2*math.pi)-logs,1,keepdim=True)
                + torch.matmul(-0.5*zp.square().transpose(1,2),invvar)
                + torch.matmul(zp.transpose(1,2),m*invvar)
                + torch.sum(-0.5*m.square()*invvar,1,keepdim=True))
            alignment = maximum_path(scores[0].numpy())
            targets[record['id']] = torch.from_numpy(alignment.sum(0))[None,None]
    print('Training alignments prepared.', flush=True)
    original = {k:v.detach().clone() for k,v in model.dp.state_dict().items()}
    model.dp.requires_grad_(True)
    optimizer = torch.optim.AdamW(model.dp.parameters(), lr=protocol['optimizer']['learning_rate'], weight_decay=0)
    losses = []
    for epoch in range(protocol['epochs']):
        model.dp.train()
        for record in manifest['records']:
            if record['split'] != 'train':
                continue
            x, _, _, mask, _ = features[record['id']]
            optimizer.zero_grad(set_to_none=True)
            loss = (model.dp(x,mask,targets[record['id']])/mask.sum()).mean()
            if not torch.isfinite(loss):
                raise ValueError('nonfinite training loss')
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.dp.parameters(), protocol['optimizer']['max_gradient_norm'], error_if_nonfinite=True)
            optimizer.step()
            losses.append(float(loss.detach()))
        print('Completed epoch '+str(epoch+1), flush=True)
    candidate = evaluate()
    saved = output/'duration-predictor.safetensors'
    save_file({k:v.detach().contiguous() for k,v in model.dp.state_dict().items()}, str(saved))
    changed = sum(not torch.equal(original[k],v) for k,v in model.dp.state_dict().items())
    # Replace the module with a freshly constructed one before loading the candidate.
    fresh = SynthesizerTrn(**architecture).dp
    fresh.load_state_dict(load_file(str(saved)), strict=True)
    model.dp = fresh
    reloaded = evaluate()
    if reloaded != candidate:
        raise ValueError('saved candidate reload mismatch')
    results = {}
    for split in ('validation','test'):
        before = [c for c in baseline if c['split']==split]
        after = [c for c in candidate if c['split']==split]
        old = sum(c['relative_error'] for c in before)/len(before)
        new = sum(c['relative_error'] for c in after)/len(after)
        regressions = [a['id'] for b,a in zip(before,after) if a['relative_error']-b['relative_error'] > protocol['evaluation']['maximum_per_case_absolute_relative_error_increase']]
        results[split] = {'baseline_mean_error':old,'candidate_mean_error':new,'regressions':regressions,
            'passed':new <= old*(1-protocol['evaluation']['minimum_relative_metric_reduction']) and not regressions}
    if digest(weights) != protocol['base_safetensors_sha256']:
        raise ValueError('base mutated')
    report = {'protocol':protocol,'manifest_sha256':digest(dataset/'manifest.json'), 'baseline':baseline,
        'candidate':candidate,'reload_exact':True,'results':results,'steps':len(losses),'losses':losses,
        'changed_duration_tensors':changed,'candidate_sha256':digest(saved),'base_unchanged':True,
        'provider_cost_usd':0,'promoted':False,'elapsed_seconds':time.monotonic()-started}
    (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(results),flush=True)

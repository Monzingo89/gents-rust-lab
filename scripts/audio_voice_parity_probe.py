"""Compare quarantined Safetensors reconstruction with pinned production ONNX."""
import hashlib
import json
from pathlib import Path


def parity(source,output,onnx,voice_config,onnx_sha):
    import numpy as np
    import torch
    import onnxruntime as ort
    from safetensors.torch import load_file
    from piper.train.vits.models import SynthesizerTrn
    from piper.phonemize_espeak import EspeakPhonemizer
    from piper.phoneme_ids import phonemes_to_ids
    torch.set_num_threads(2)
    original=json.loads((source/'conversion-attempt/reconstruction/report.json').read_text())
    weights=source/'conversion-attempt/converted/state.safetensors'
    def digest(path):
        with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
    if digest(weights)!=original['safetensors_sha256'] or digest(onnx)!=onnx_sha:
        raise ValueError('model hash mismatch')
    a=json.loads((source/'config.json').read_text());b=json.loads(voice_config.read_text())
    for field in ['phoneme_id_map','espeak','num_symbols','num_speakers']:
        if a[field]!=b[field]:raise ValueError('input configuration mismatch: '+field)
    model=SynthesizerTrn(**original['architecture']).eval()
    state=load_file(str(weights),device='cpu')
    model.load_state_dict({k.removeprefix('model_g.'):v for k,v in state.items() if k.startswith('model_g.')},strict=True)
    options=ort.SessionOptions();options.intra_op_num_threads=2;options.inter_op_num_threads=1
    session=ort.InferenceSession(str(onnx),sess_options=options,providers=['CPUExecutionProvider'])
    phonemizer=EspeakPhonemizer();cases=[]
    for text in ['The local company is ready.','Please check the report.','Seven green squares are on the page.']:
        phones=phonemizer.phonemize(a['espeak']['voice'],text)
        ids=phonemes_to_ids([p for row in phones for p in row],a['phoneme_id_map'])
        with torch.inference_mode():
            predicted=model.infer(torch.tensor([ids]),torch.tensor([len(ids)]),noise_scale=0,noise_scale_w=0,max_len=2000)[0].flatten().numpy()
        native=session.run(None,{'input':np.array([ids],dtype=np.int64),'input_lengths':np.array([len(ids)],dtype=np.int64),'scales':np.array([0,1,0],dtype=np.float32)})[0].flatten()
        same=predicted.shape==native.shape
        finite=bool(np.isfinite(predicted).all() and np.isfinite(native).all())
        error=float(np.max(np.abs(predicted-native))) if same and finite else None
        rmse=float(np.sqrt(np.mean((predicted-native)**2))) if same and finite else None
        cases.append({'text':text,'torch_samples':len(predicted),'onnx_samples':len(native),'finite':finite,
                      'max_abs_error':error,'rmse':rmse,'passed':same and finite and error<=1e-4 and rmse<=1e-5})
    report={'criterion':{'same_length':True,'max_abs_error':1e-4,'rmse':1e-5},'cases':cases,
            'all_passed':all(c['passed'] for c in cases),'source_safetensors_sha256':original['safetensors_sha256'],
            'production_onnx_sha256':onnx_sha,'provider_cost_usd':0,'weights_modified':False,'admitted':False,
            'scope':'Three deterministic zero-noise CPU cases; not a general numerical equivalence proof or speech-quality evaluation'}
    output.mkdir(exist_ok=False);(output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    return report

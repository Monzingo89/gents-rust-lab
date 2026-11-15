"""Strict VITS reconstruction from quarantined tensors, without Lightning loading."""
import ast
import hashlib
import json
import time
import wave
from pathlib import Path


def reconstruct(source,output,expected_sha256,safe_loader):
    import torch
    from safetensors.torch import load_file
    from piper.train.vits.models import SynthesizerTrn
    from piper.phonemize_espeak import EspeakPhonemizer
    from piper.phoneme_ids import phonemes_to_ids
    torch.set_num_threads(2)
    checkpoint,_=safe_loader(source/'ljspeech-2000.ckpt',expected_sha256)
    hp=checkpoint['hyper_parameters']
    fields=['inter_channels','hidden_channels','filter_channels','n_heads','n_layers','kernel_size',
            'p_dropout','resblock','resblock_kernel_sizes','resblock_dilation_sizes','upsample_rates',
            'upsample_initial_channel','upsample_kernel_sizes','gin_channels','use_sdp']
    kwargs={k:hp[k] for k in fields}
    for name in ['resblock_kernel_sizes','resblock_dilation_sizes','upsample_rates','upsample_kernel_sizes']:
        if isinstance(kwargs[name],str):kwargs[name]=ast.literal_eval(kwargs[name])
    kwargs.update(n_vocab=hp['num_symbols'],n_speakers=hp['num_speakers'],
                  spec_channels=hp['filter_length']//2+1,segment_size=hp['segment_size']//hp['hop_length'])
    # Discard original pickle tensors: reconstruction must use the Safetensors file.
    del checkpoint
    tensor_file=source/'conversion-attempt/converted/state.safetensors'
    report=json.loads((tensor_file.parent/'conversion.json').read_text())
    with tensor_file.open('rb') as stream:digest=hashlib.file_digest(stream,'sha256').hexdigest()
    if digest!=report['safetensors_sha256']:raise ValueError('converted tensor hash mismatch')
    state=load_file(str(tensor_file),device='cpu')
    generator={k.removeprefix('model_g.'):v for k,v in state.items() if k.startswith('model_g.')}
    model=SynthesizerTrn(**kwargs).eval()
    model.load_state_dict(generator,strict=True)
    config=json.loads((source/'config.json').read_text())
    phonemizer=EspeakPhonemizer()
    text='The local company is ready.'
    phones=phonemizer.phonemize(config['espeak']['voice'],text)
    ids=phonemes_to_ids([p for sentence in phones for p in sentence],config['phoneme_id_map'])
    started=time.monotonic();torch.manual_seed(17)
    with torch.inference_mode():
        audio=model.infer(torch.tensor([ids]),torch.tensor([len(ids)]),noise_scale=0,noise_scale_w=0,max_len=1000)[0].flatten()
    if not torch.isfinite(audio).all() or audio.numel()==0:raise ValueError('invalid audio')
    output.mkdir(exist_ok=False)
    raw=(audio.clamp(-1,1)*32767).to(torch.int16).numpy().astype('<i2').tobytes()
    sample_rate=config['audio']['sample_rate']
    with wave.open(str(output/'probe.wav'),'wb') as wav:
        wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(sample_rate);wav.writeframes(raw)
    result={'strict_generator_parameter_match':True,'generator_tensor_count':len(generator),
            'source_sha256':expected_sha256,'safetensors_sha256':digest,'architecture':kwargs,
            'text':text,'samples':audio.numel(),'sample_rate':sample_rate,'duration_seconds':audio.numel()/sample_rate,
            'peak_amplitude':float(audio.abs().max()),'inference_seconds':round(time.monotonic()-started,3),
            'wav_sha256':hashlib.sha256((output/'probe.wav').read_bytes()).hexdigest(),
            'device':'cpu','provider_cost_usd':0,'weights_modified':False,'admitted':False,
            'scope':'Strict reconstruction and finite waveform only; intelligibility, quality and production ONNX parity unverified'}
    (output/'report.json').write_text(json.dumps(result,indent=2)+'\n')
    return result

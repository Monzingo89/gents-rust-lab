#!/usr/bin/env python3
"""Offline tokenizer-only preflight matching the installed owned MLX-LM server."""
import importlib.metadata
import json
from pathlib import Path
import sys

def main():
    version=importlib.metadata.version('mlx-lm')
    if version!='0.32.2':raise RuntimeError('MLX tokenization scheme has not been verified for this installed version')
    model=Path(sys.argv[1]);packet=json.loads(sys.stdin.buffer.read(131073))
    if not model.is_dir() or len(json.dumps(packet).encode())>131072:raise RuntimeError('Invalid bounded preflight packet')
    config=json.loads((model/'config.json').read_text())
    tokenizer_config=json.loads((model/'tokenizer_config.json').read_text()) if (model/'tokenizer_config.json').is_file() else {}
    limits=[]
    for value in [config.get('max_position_embeddings'),config.get('n_positions'),(config.get('text_config') or {}).get('max_position_embeddings'),tokenizer_config.get('model_max_length')]:
        if isinstance(value,int) and not isinstance(value,bool) and 128<=value<=10000000:limits.append(value)
    if not limits:raise RuntimeError('No finite pinned model/tokenizer context metadata')
    from mlx_lm.utils import load_tokenizer
    from mlx_lm.server import convert_chat
    tokenizer=load_tokenizer(model,{'trust_remote_code':False,'local_files_only':True},eos_token_ids=config.get('eos_token_id'))
    messages=packet['messages'];kwargs=packet['chat_template_kwargs']
    if not isinstance(messages,list) or not isinstance(kwargs,dict):raise RuntimeError('Invalid preflight shape')
    if tokenizer.has_chat_template:
        tokens=tokenizer.apply_chat_template(messages,add_generation_prompt=True,tools=None,tokenize=True,**kwargs)
    else:tokens=tokenizer.encode(convert_chat(messages,None))
    print(json.dumps({'schema_version':1,'provider_version':version,'tokenization_scheme':'mlx_lm.server/0.32.2','prompt_tokens':len(tokens),'model_context_limit':min(limits),'has_chat_template':bool(tokenizer.has_chat_template),'weights_loaded':False}))
if __name__=='__main__':
    try:main()
    except Exception:
        # Do not echo source, prompts, tokenizer text or exception payloads.
        print('MLX tokenizer preflight unavailable or incompatible',file=sys.stderr)
        raise SystemExit(1)

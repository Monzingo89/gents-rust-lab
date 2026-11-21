#!/usr/bin/env python3
"""Bounded existing-native-model RAG check; no downloads, cloud or production admission."""
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import urllib.request
import uuid

ROOT=Path(__file__).resolve().parents[1]
MODEL='models/qwen3-4b-instruct/Qwen3-4B-Q4_K_M.gguf'

def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1',0));return s.getsockname()[1]

def main():
    path=ROOT/MODEL
    if not path.is_file() or path.is_symlink():
        raise SystemExit('Existing bounded local model unavailable; no download attempted')
    with path.open('rb') as stream:
        sha=hashlib.file_digest(stream,'sha256').hexdigest()
    run=str(uuid.uuid4());company='task-live-'+run[:8]
    output=ROOT/'data/task-engine-live'/run;output.mkdir(parents=True)
    model={'provider':'local','model':'Qwen3-4B existing bounded RAG trial','path':MODEL,'context_tokens':8192,'local_reasoning_tokens':512,'local_final_tokens':512}
    worker={'model':model,'sha256':sha,'memory_estimate':{'weights_bytes':path.stat().st_size,'adapters_bytes':0,'kv_bytes':512*1024**2,'context_bytes':32*1024**2,'runtime_bytes':1024**3,'headroom_bytes':2*1024**3,'provenance':'conservative explicit trial estimate; measured residency reported separately'},'evaluated_profiles':['general','history'],'evaluation_reference':'bounded trial only; this isolated harness creates evaluation evidence, not production admission'}
    config={'company_mode':True,'company_tools':True,'admin_judge':model,'agents':[{'id':'Live Trial','assignment':'bounded read-only trial','planner':model,'worker':model}],'max_steps':1,'max_tokens':512,'max_run_seconds':180,
        'task_workflows':{'models':{'trial':worker},'default_model':'trial','max_calls':1,'total_generation_tokens':1024,'per_call_tokens':1024,'deadline_seconds':120,'max_retries':0,'max_pending_jobs':2,'concurrency':1,'resident_budget_bytes':6*1024**3,'allow_parametric_answers':False}}
    configpath=output/'config.json';configpath.write_text(json.dumps(config,indent=2)+'\n')
    httpport,modelport=port(),port();origin=f'http://127.0.0.1:{httpport}'
    def req(url,body=None):
        request=urllib.request.Request(origin+url,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(request,timeout=30) as r:
            return json.load(r)
    env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','USER','TMPDIR','LANG','SHELL'}}
    env.update(AUTH_MODE='local',BIND=f'127.0.0.1:{httpport}',LOCAL_MODEL_PORT=str(modelport),CONFIG=str(configpath),RUN_STORE=str(output/'runs.sqlite3'),GENTS_DISABLE_SCHEDULER='1',GENTS_OFFICE_ACTIVITY='0',GENTS_LOCAL_ONLY='1')
    began=time.monotonic()
    with (output/'harness.log').open('wb') as log:
        child=subprocess.Popen([str(ROOT/'target/debug/gents-harness')],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            deadline=time.monotonic()+20
            while True:
                try:
                    req('/api/config');break
                except OSError:
                    if time.monotonic()>deadline:
                        raise
                    time.sleep(.1)
            req('/api/companies/'+company,{'op':'register','name':'Isolated bounded local RAG evaluation'})
            route='/api/companies/'+company+'/tasks'
            now=int(time.time()*1000)
            text='Gerald Ford held the US presidency from August 9, 1974 until January 20, 1977, following election loss to Jimmy Carter.'
            source={'source_id':'ford-library-paraphrase','revision':'2026-11-18-trial-v1','title':'Ford Library facts page; bounded paraphrase; https://www.fordlibrarymuseum.gov/the-fords/gerald-r-ford/facts-and-favorites','domain':'history','language':'en','text':text,'observed_at_ms':now,'valid_until_ms':now+600000,'lookups':[]}
            req(route,{'op':'ingest','source':source})
            started=req(route,{'op':'execute','input':{'original_ask':'Who was US president in 1976? Return only the full name.','temporal_scope':'1976','output':{'format':'text','language':'en'}}})
            task=started['request']['task_id'];deadline=time.monotonic()+150
            while True:
                result=req(route,{'op':'status','task_id':task})
                if result['status']!='running':
                    break
                if time.monotonic()>deadline:
                    req(route,{'op':'cancel_task','task_id':task});raise AssertionError('live deadline')
                time.sleep(.2)
            rag=result
            # Install a separate reviewed extraction binding only AFTER the live
            # RAG check, then prove exact repetition returns without generation.
            source['revision']='2026-11-18-trial-v2';source['lookups']=[{'exact_ask':'Return the recorded president name.','temporal_scope':'1976','context_independent':True,'start_byte':0,'end_byte':11}]
            req(route,{'op':'ingest','source':source})
            taskinput={'original_ask':'Return the recorded president name.','temporal_scope':'1976','output':{'format':'text','language':'en'}}
            first=req(route,{'op':'execute','input':taskinput})['result'];second=req(route,{'op':'execute','input':taskinput})['result']
            assert first['actual_path']=='deterministic_lookup' and second['actual_path']=='exact_answer_cache'
            assert second['measurements']['generative_model_calls']==0
            hardware=req('/api/companies/'+company+'/hardware')
            factual_pass=(rag.get('answer') or '').strip().rstrip('.').lower() in {'gerald ford','gerald r. ford','gerald r ford'}
            report={'factual_acceptance_passed':factual_pass,'expected_answer':'Gerald Ford','schema_version':1,'company':company,'model_path':MODEL,'model_sha256':sha,'rag_result':rag,'exact_first':first,'exact_repeat':second,'hardware':hardware,'elapsed_seconds':time.monotonic()-began,'provider_cost_usd':0,'production_profile_admission':False,'quality_scope':'One source-grounded historical question; no general expert or routing quality claim.'}
            (output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
            print(json.dumps({'report':str(output/'report.json'),'rag_status':rag['status'],'rag_answer':rag['answer'],'rag_verification':rag['verification_state'],'rag_measurements':rag['measurements'],'exact_repeat_calls':second['measurements']['generative_model_calls'],'provider_cost_usd':0}))
            assert rag['status']=='generated' and factual_pass,rag
            assert rag['measurements']['generative_model_calls']==1
        finally:
            child.terminate()
            try:
                child.wait(timeout=20)
            except subprocess.TimeoutExpired:
                child.kill();child.wait(timeout=5)
            checkpoint=ROOT/'data/companies'/company
            if checkpoint.is_dir() and not checkpoint.is_symlink():
                checkpoint.rename(output/'company-checkpoint')
if __name__=='__main__':
    main()

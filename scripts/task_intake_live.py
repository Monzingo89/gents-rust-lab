#!/usr/bin/env python3
"""Bounded frozen intake evaluation, existing native weights only; no production admission."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import time
import urllib.request
import uuid
from task_engine_live import ROOT, MODEL, port

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--limit',type=int,default=32);parser.add_argument('--model-path',default=MODEL);parser.add_argument('--thinking-tokens',type=int,default=512);parser.add_argument('--evaluation-role',choices=['heldout','calibration'],default='heldout');parser.add_argument('--resident-budget-gib',type=int,default=6);args=parser.parse_args()
    assert 1<=args.limit<=32 and 1<=args.resident_budget_gib<=16
    fixtures=ROOT/'mcp/tests/fixtures/intake-heldout-v1.json'
    cases=json.loads(fixtures.read_text())['cases'][:args.limit]
    model_path=args.model_path
    assert model_path.startswith("models/") and model_path.endswith(".gguf") and ".." not in Path(model_path).parts
    assert 0<=args.thinking_tokens<=1024
    path=ROOT/model_path
    if not path.is_file() or path.is_symlink():
        raise SystemExit('Existing native model absent; no download attempted')
    with path.open('rb') as stream: sha=hashlib.file_digest(stream,'sha256').hexdigest()
    run=str(uuid.uuid4());company='intake-live-'+run[:8];output=ROOT/'data/task-intake-live'/run;output.mkdir(parents=True)
    model={'provider':'local','model':'existing bounded frozen intake evaluation','path':model_path,'context_tokens':8192,'local_reasoning_tokens':args.thinking_tokens,'local_final_tokens':1024}
    profiles=['intake','general','history','angular','cosmos','dotnet','research','integration','evaluation']
    worker={'model':model,'sha256':sha,'memory_estimate':{'weights_bytes':path.stat().st_size,'adapters_bytes':0,'kv_bytes':512*1024**2,'context_bytes':32*1024**2,'runtime_bytes':1024**3,'headroom_bytes':2*1024**3,'provenance':'conservative isolated evaluation estimate'},'evaluated_profiles':profiles,'evaluation_reference':'isolated evaluation fixture only; not production qualification'}
    config={'company_mode':True,'company_tools':True,'admin_judge':model,'agents':[{'id':'Intake Evaluation','assignment':'read-only evaluation','planner':model,'worker':model}],'max_steps':1,'max_tokens':1536,'max_run_seconds':180,'task_workflows':{'models':{'trial':worker},'default_model':'trial','max_calls':2,'total_generation_tokens':3072,'per_call_tokens':1536,'deadline_seconds':90,'max_retries':0,'max_pending_jobs':1,'concurrency':1,'resident_budget_bytes':args.resident_budget_gib*1024**3,'allow_parametric_answers':True,'adaptive':{'intake':{'model':'trial','tokens':1536,'max_repairs':1},'max_replans':0,'rules':[]}}}
    configpath=output/'config.json';configpath.write_text(json.dumps(config,indent=2)+'\n')
    httpport,modelport=port(),port();origin=f'http://127.0.0.1:{httpport}'
    def req(url,body=None):
        request=urllib.request.Request(origin+url,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(request,timeout=20) as r:return json.load(r)
    env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','USER','TMPDIR','LANG','SHELL'}}
    env.update(AUTH_MODE='local',BIND=f'127.0.0.1:{httpport}',LOCAL_MODEL_PORT=str(modelport),CONFIG=str(configpath),RUN_STORE=str(output/'runs.sqlite3'),GENTS_DISABLE_SCHEDULER='1',GENTS_OFFICE_ACTIVITY='0',GENTS_LOCAL_ONLY='1')
    with (ROOT/"target/debug/gents-harness").open("rb") as stream: binary_sha=hashlib.file_digest(stream,"sha256").hexdigest()
    results=[];began=time.monotonic()
    with (output/'harness.log').open('wb') as log:
        child=subprocess.Popen([str(ROOT/'target/debug/gents-harness')],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            deadline=time.monotonic()+20
            while True:
                try:req('/api/config');break
                except OSError:
                    if child.poll() is not None or time.monotonic()>deadline:raise
                    time.sleep(.1)
            req('/api/companies/'+company,{'op':'register','name':'Isolated frozen intake evaluation'})
            route='/api/companies/'+company+'/tasks'
            for case in cases:
                start=time.monotonic();response=req(route,{'op':'destructure_task','input':case['input']});task=response['request']['task_id'];deadline=time.monotonic()+100
                while True:
                    result=req(route,{'op':'status','task_id':task})
                    if result['status']!='running':break
                    if time.monotonic()>deadline:
                        req(route,{'op':'cancel_task','task_id':task});raise RuntimeError('bounded intake deadline')
                    time.sleep(.15)
                expected=case['expected'];analysis=result.get('analysis') or {};plan=result.get('plan') or {}
                checks={'schema':bool(analysis),'domain':analysis.get('domain') in expected['domains'],'template':plan.get('template') in expected['templates'],'evidence_required':analysis.get('evidence_required')==expected['evidence_required'],'clarification':bool(result.get('clarification_questions'))==expected.get('essential_intent_missing',False),'no_worker_answer':result.get('answer') is None,'mode':result['status'] in ('analyzed','needs_clarification')}
                record={'id':case['id'],'input':case['input'],'expected':expected,'checks':checks,'passed':all(checks.values()),'elapsed_seconds':time.monotonic()-start,'result':result};results.append(record)
                (output/'cases.json').write_text(json.dumps(results,indent=2)+'\n')
                print(json.dumps({'id':case['id'],'passed':record['passed'],'checks':checks,'domain':analysis.get('domain'),'template':plan.get('template'),'seconds':record['elapsed_seconds']}),flush=True)
            metrics=[r['result']['measurements'] for r in results]
            report={'schema_version':1,'evaluation_role':args.evaluation_role,'fixture_sha256':hashlib.sha256(fixtures.read_bytes()).hexdigest(),'model_path':model_path,'model_sha256':sha,'harness_binary_sha256':binary_sha,'native_model_configuration':model,'cases':len(results),'passed':sum(r['passed'] for r in results),'per_check':{k:sum(r['checks'][k] for r in results) for k in results[0]['checks']},'median_seconds':statistics.median(r['elapsed_seconds'] for r in results),'max_memory_bytes':max((m['memory_bytes'] or 0) for m in metrics),'generative_model_calls':sum(m['generative_model_calls'] or 0 for m in metrics),'all_call_metrics_available':all(m['generative_model_calls'] is not None for m in metrics),'provider_cost_usd':0,'production_admission':False,'factual_answer_accuracy':None,'quality_scope':'Intake schema/domain/template/evidence/clarification only. No workers executed. Evaluation does not establish specialist expertise or optimality.','elapsed_seconds':time.monotonic()-began,'results':results}
            (output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'report':str(output/'report.json'),'cases':len(results),'passed':report['passed'],'per_check':report['per_check'],'provider_cost_usd':0}),flush=True)
        finally:
            child.terminate()
            try:child.wait(timeout=20)
            except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
            checkpoint=ROOT/'data/companies'/company
            if checkpoint.is_dir() and not checkpoint.is_symlink():checkpoint.rename(output/'company-checkpoint')
if __name__=='__main__':main()

#!/usr/bin/env python3
"""One bounded native-model file-tool trial; source declarations only, no model download."""
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import time
import urllib.request
import uuid
from task_engine_live import ROOT, MODEL, port

def main():
    path=ROOT/MODEL
    if not path.is_file() or path.is_symlink():raise SystemExit('Existing native artifact absent; no download attempted')
    with path.open('rb') as stream:sha=hashlib.file_digest(stream,'sha256').hexdigest()
    run=str(uuid.uuid4());company='workspace-live-'+run[:8];output=ROOT/'data/task-workspace-live'/run;output.mkdir(parents=True)
    expected='^20.3.'+str(int(run[:2],16))
    source=json.dumps({'name':'synthetic-declaration-trial','dependencies':{'@angular/core':expected}},indent=2)+'\n'
    model={'provider':'local','model':'existing bounded workspace tool trial','path':MODEL,'context_tokens':8192,'local_reasoning_tokens':512,'local_final_tokens':512}
    memory={'weights_bytes':path.stat().st_size,'adapters_bytes':0,'kv_bytes':512*1024**2,'context_bytes':32*1024**2,'runtime_bytes':1024**3,'headroom_bytes':2*1024**3,'provenance':'explicit conservative trial estimate; residency measured separately'}
    worker={'model':model,'sha256':sha,'memory_estimate':memory,'evaluated_profiles':['angular'],'evaluation_reference':'isolated tool-use trial only; no production admission','max_input_bytes':12000}
    config={'company_mode':True,'company_tools':True,'admin_judge':model,'agents':[{'id':'Workspace Trial','assignment':'read-only trial','planner':model,'worker':model}],'max_steps':1,'max_tokens':1024,'max_run_seconds':150,'task_workflows':{'models':{'trial':worker},'default_model':'trial','max_calls':3,'total_generation_tokens':3072,'per_call_tokens':1024,'deadline_seconds':120,'max_retries':0,'max_pending_jobs':1,'concurrency':1,'resident_budget_bytes':6*1024**3,'allow_parametric_answers':False,'workspace_reads':True,'workspace_root':f'data/companies/{{company}}/runs/{run}/workspace'}}
    configpath=output/'config.json';configpath.write_text(json.dumps(config,indent=2)+'\n')
    # Freeze expected literal/cost gates before dispatch. The package range is a
    # synthetic source declaration, not a claim about published/installed Angular.
    criteria={'declared_range':expected,'required_tool_calls':1,'required_generative_calls':2,'build_executed':False,'installed_version_claim':False}
    (output/'criteria.json').write_text(json.dumps(criteria,indent=2)+'\n')
    httpport,modelport=port(),port();origin=f'http://127.0.0.1:{httpport}'
    def req(url,body=None):
        request=urllib.request.Request(origin+url,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(request,timeout=20) as r:return json.load(r)
    env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','USER','TMPDIR','LANG','SHELL'}}
    env.update(AUTH_MODE='local',BIND=f'127.0.0.1:{httpport}',LOCAL_MODEL_PORT=str(modelport),CONFIG=str(configpath),RUN_STORE=str(output/'runs.sqlite3'),GENTS_DISABLE_SCHEDULER='1',GENTS_OFFICE_ACTIVITY='0',GENTS_LOCAL_ONLY='1')
    with (ROOT/'target/debug/gents-harness').open('rb') as stream:binary_sha=hashlib.file_digest(stream,'sha256').hexdigest()
    began=time.monotonic()
    with (output/'harness.log').open('wb') as log:
        child=subprocess.Popen([str(ROOT/'target/debug/gents-harness')],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            deadline=time.monotonic()+20
            while True:
                try:req('/api/config');break
                except OSError:
                    if child.poll() is not None or time.monotonic()>deadline:raise
                    time.sleep(.1)
            req('/api/companies/'+company,{'op':'register','name':'Isolated native workspace tool evaluation'})
            workspace=ROOT/'data/companies'/company/'runs'/run/'workspace';(workspace/'frontend').mkdir(parents=True)
            (workspace/'frontend/package.json').write_text(source)
            route='/api/companies/'+company+'/tasks'
            launched=req(route,{'op':'execute_workflow','input':{'original_ask':'Inspect the Angular declaration. Read frontend/package.json using workspace_read with offset_bytes=0 and limit_bytes=1024. Return only the exact range declared for dependencies @angular/core, preserving any prefix. Do not claim an installed version, a build or a published package.'},'hints':{'domain':'angular','template':'tool_loop/v1','evidence_required':True}})
            task=launched['request']['task_id'];deadline=time.monotonic()+140
            while True:
                result=req(route,{'op':'status','task_id':task})
                if result['status']!='running':break
                if time.monotonic()>deadline:
                    req(route,{'op':'cancel_task','task_id':task});raise RuntimeError('bounded workspace trial deadline')
                time.sleep(.15)
            with sqlite3.connect(ROOT/'data/companies'/company/'brain.sqlite3') as db:
                receipts=[{'kind':kind,'body':json.loads(body)} for kind,body in db.execute("SELECT kind,body FROM audit WHERE kind IN ('task_tool_dispatch','task_tool_return') ORDER BY rowid")]
            checks={'generated':result['status']=='generated','exact_source_literal':(result.get('answer') or '').strip()==expected,'one_tool_call':result['measurements']['tool_calls']==1,'two_generative_calls':result['measurements']['generative_model_calls']==2,'source_hash_matches':any(e['content_sha256']==hashlib.sha256(source.encode()).hexdigest() for e in result['evidence']),'independent_build_not_claimed':all(c['status']=='not_run' for c in result['acceptance_checks'] if c['name']=='independent_factual_or_build_acceptance')}
            report={'schema_version':1,'model_path':MODEL,'model_sha256':sha,'harness_binary_sha256':binary_sha,'company':company,'criteria':criteria,'checks':checks,'passed':all(checks.values()),'result':result,'tool_receipts':receipts,'elapsed_seconds':time.monotonic()-began,'provider_cost_usd':0,'production_admission':False,'quality_scope':'One source-literal/tool-protocol trial, not installed Angular, a build, expert qualification or optimal routing.'}
            (output/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'report':str(output/'report.json'),'passed':report['passed'],'checks':checks,'answer':result.get('answer'),'measurements':result['measurements'],'provider_cost_usd':0}),flush=True)
        finally:
            child.terminate()
            try:child.wait(timeout=20)
            except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
            checkpoint=ROOT/'data/companies'/company
            if checkpoint.is_dir() and not checkpoint.is_symlink():checkpoint.rename(output/'company-checkpoint')
if __name__=='__main__':main()

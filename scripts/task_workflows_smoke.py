#!/usr/bin/env python3
"""Real HTTP/SQLite workflow integration using a marked mock provider; no model load."""
import argparse
import concurrent.futures
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

def main():
    parser=argparse.ArgumentParser();parser.add_argument("--adaptive",action="store_true");parser.add_argument("--workspace",action="store_true");parser.add_argument("--workspace-denied",action="store_true");parser.add_argument("--browser",action="store_true");args=parser.parse_args();assert sum([args.adaptive,args.workspace,args.workspace_denied,args.browser])<=1
    with tempfile.TemporaryDirectory(prefix='gents-workflow-') as temporary:
        folder=Path(temporary)
        if args.browser:(folder/'web').symlink_to(ROOT/'web',target_is_directory=True)
        model={'provider':'mock','model':'mock-task-stage'}
        if args.workspace or args.workspace_denied:
            model['model']='mock-task-workspace'
            for name,body in {
                'frontend/package.json':json.dumps({'dependencies':{'@angular/core':'20.3.0'}},indent=2)+'\n',
                'api/Service.csproj':'<Project Sdk="Microsoft.NET.Sdk.Web"><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>\n',
                'db/cosmos.json':json.dumps({'api':'NoSQL','container':'items','partitionKey':'/tenantId'},indent=2)+'\n',
            }.items():
                path=folder/'project'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(body)
        memory={'weights_bytes':1,'adapters_bytes':0,'kv_bytes':1,'context_bytes':1,'runtime_bytes':1,'headroom_bytes':1,'provenance':'synthetic fixture estimate'}
        profiles=['intake','general','history','angular','cosmos','dotnet','research','integration','evaluation']
        settings={'models':{'fixture':{'model':model,'sha256':None,'memory_estimate':memory,'evaluated_profiles':profiles,'evaluation_reference':'synthetic transport fixture only'}},'default_model':'fixture',
                  'max_calls':16,'total_generation_tokens':4096,'per_call_tokens':128,'deadline_seconds':60,'max_retries':0,'max_pending_jobs':8,'concurrency':2,'resident_budget_bytes':8*1024*1024,'allow_parametric_answers':True}
        custom={'id':'release-specialist','revision':'1','instructions':'Produce a bounded release summary from the task.','required_capabilities':['generation'],'tools':['search_knowledge'],'validators':['output_shape'],'memory_scope':'task-and-access-scope-only'}
        settings['profiles']=[custom];profiles.append('release-specialist')
        if args.workspace:
            settings['workspace_reads']=True
            settings['workspace_root']='project'
            settings['models']['fixture']['max_input_bytes']=16384
        if args.adaptive:
            settings['adaptive']={'intake':{'model':'fixture','tokens':128,'max_repairs':1},'max_replans':1,'rules':[{'on':'evidence_insufficient','template':'tool_loop/v1','model':None}]}
        config={'company_mode':True,'company_tools':True,'admin_judge':model,'agents':[{'id':'Fixture','assignment':'protocol test','planner':model,'worker':model}],
                'max_steps':1,'max_tokens':512,'max_run_seconds':60,'task_workflows':settings}
        (folder/'config.json').write_text(json.dumps(config))
        with socket.socket() as listener:
            listener.bind(('127.0.0.1',0));port=listener.getsockname()[1]
        origin=f'http://127.0.0.1:{port}'
        def req(path,body=None):
            payload=None if body is None else json.dumps(body).encode()
            request=urllib.request.Request(origin+path,data=payload,headers={'Content-Type':'application/json'})
            with urllib.request.urlopen(request,timeout=20) as response:
                return json.load(response)
        env={k:v for k,v in os.environ.items() if k in {'PATH','HOME','USER','TMPDIR','LANG','SHELL'}}
        env.update(AUTH_MODE='local',BIND=f'127.0.0.1:{port}',CONFIG=str(folder/'config.json'),RUN_STORE=str(folder/'runs.sqlite3'),GENTS_DISABLE_SCHEDULER='1',GENTS_OFFICE_ACTIVITY='0',GENTS_LOCAL_ONLY='1')
        with (folder/'harness.log').open('wb') as log:
            child=subprocess.Popen([str(ROOT/'target/debug/gents-harness')],cwd=folder,env=env,stdout=log,stderr=subprocess.STDOUT)
            try:
                deadline=time.monotonic()+20
                while True:
                    try:
                        req('/api/config');break
                    except OSError:
                        if time.monotonic()>deadline:
                            raise
                        time.sleep(.1)
                route='/api/companies/one/tasks'
                req('/api/companies/one',{'op':'register','name':'Workflow transport fixture'})
                if args.browser:req('/api/companies/we-need-gents-lc',{'op':'register','name':'Empty browser connection fixture'})
                if args.workspace or args.workspace_denied:
                    import sqlite3
                    def status(task): return req(route,{'op':'status','task_id':task})
                    def wait(task):
                        deadline=time.monotonic()+30
                        while True:
                            result=status(task)
                            if result['status']!='running':return result
                            assert time.monotonic()<deadline,result
                            time.sleep(.05)
                    operation={'op':'execute_workflow','input':{'original_ask':'Read declared Angular, .NET and Cosmos metadata from the three fixture files.'},'hints':{'domain':'integration','template':'tool_loop/v1','evidence_required':True}}
                    launched=req(route,operation);result=wait(launched['request']['task_id'])
                    store=folder/'data/companies/one/brain.sqlite3'
                    if args.workspace_denied:
                        assert result['status']=='blocked' and result['measurements']['tool_calls']==0,result
                        with sqlite3.connect(store) as db: assert db.execute('select count(*) from task_workspace_evidence_v1').fetchone()[0]==0
                        print(json.dumps({'transport':'real HTTP/SQLite','workspace_permission_denied':True,'file_captures':0,'provider_cost_usd':0}));return
                    assert result['status']=='generated',result
                    assert result['measurements']['tool_calls']==3 and result['measurements']['generative_model_calls']==0,result
                    assert len(result['evidence'])==3 and all(e['source_id'].startswith('workspace:v1:') for e in result['evidence']),result
                    assert '20.3.0' in result['evidence'][0]['quote'] and 'net8.0' in result['evidence'][1]['quote'] and '/tenantId' in result['evidence'][2]['quote']
                    assert result['verification_state']=='citations_checked_answer_unverified'
                    assert next(c for c in result['acceptance_checks'] if c['name']=='independent_factual_or_build_acceptance')['status']=='not_run'
                    assert any(c['name']=='current_workspace_source_bytes/v1' and c['status']=='passed' for c in result['acceptance_checks'])
                    with sqlite3.connect(store) as db:
                        assert db.execute('select count(*) from task_sources_v1').fetchone()[0]==0
                        assert db.execute("select count(*) from audit where kind='task_tool_return'").fetchone()[0]==3
                        receipts=[json.loads(row[0])['result']['project_metadata'] for row in db.execute("select body from audit where kind='task_tool_return' order by rowid")]
                        assert [m['kind'] for m in receipts]==['npm_manifest','msbuild_project','json_declaration']
                        assert receipts[0]['observations'][0]['value']=='20.3.0'
                        assert receipts[1]['observations'][1]['value']['value']=='net8.0'
                        assert receipts[2]['observations'][2]['value']=='/tenantId'
                        assert all(c['status']=='not_run' for m in receipts for c in m['runtime_checks'])
                    reused=req(route,operation)
                    assert reused['job_id']==launched['job_id'] and reused['result']['measurements']['tool_calls']==0
                    (folder/'project/frontend/package.json').write_text(json.dumps({'dependencies':{'@angular/core':'21.0.0'}},indent=2)+'\n')
                    invalid=status(result['task_id']);assert invalid['status']=='evidence_invalidated' and invalid['answer'] is None,invalid
                    fresh=req(route,operation);assert fresh['job_id']!=launched['job_id'],'Changed files must reject reuse even for the identical ask'
                    updated=wait(fresh['request']['task_id']);assert updated['status']=='generated' and '21.0.0' in updated['evidence'][0]['quote'],updated
                    operation['input']['original_ask']='Read fixture files and reject a mid-flight source change.'
                    changed=req(route,operation);task=changed['request']['task_id'];deadline=time.monotonic()+10
                    while True:
                        with sqlite3.connect(store) as db: count=db.execute('select count(*) from task_workspace_evidence_v1 where task=?',(task,)).fetchone()[0]
                        if count:break
                        assert time.monotonic()<deadline
                        time.sleep(.01)
                    (folder/'project/frontend/package.json').write_text(json.dumps({'dependencies':{'@angular/core':'22.0.0'}},indent=2)+'\n')
                    changed_result=wait(task);assert changed_result['status']=='evidence_invalidated' and changed_result['answer'] is None,changed_result
                    print(json.dumps({'transport':'real HTTP/SQLite/filesystem','provider':'mock protocol only','actual_file_captures':3,'shared_corpus_unchanged':True,'declared_metadata_checked':True,'build_or_query_executed':False,'file_changes_invalidated':True,'inflight_change_rejected':True,'provider_cost_usd':0}));return
                now=int(time.time()*1000)
                source={'source_id':'fixture','revision':'v1','title':'Synthetic reference','domain':'general','language':'en','text':'Reference chronology evidence for generated drafts.','observed_at_ms':now,'valid_until_ms':now+120000,'lookups':[]}
                req(route,{'op':'ingest','source':source})
                def status(task):
                    return req(route,{'op':'status','task_id':task})
                def wait(task):
                    deadline=time.monotonic()+30
                    while True:
                        result=status(task)
                        if result['status']!='running':
                            return result
                        if time.monotonic()>deadline:
                            raise AssertionError('workflow deadline')
                        time.sleep(.05)
                task={'original_ask':'Describe reference chronology evidence'}
                intake=req(route,{'op':'destructure_task','input':task})
                assert intake['request']['original_ask']==task['original_ask']
                if not args.adaptive: assert intake['proposed_plan']['template']=='rag/v1'
                if args.adaptive:
                    analyzed=wait(intake['request']['task_id'])
                    assert analyzed['status']=='analyzed',analyzed
                    assert analyzed['verification_state']=='proposed_not_executed'
                    assert analyzed['analysis']['domain']=='general'
                    assert analyzed['plan']['template']=='rag/v1' and analyzed['analysis']['evidence_required']
                    actual=req(route,{'op':'execute','input':{'original_ask':'Explain a fictional greeting'}})
                    result=wait(actual['request']['task_id']);assert result['status']=='generated',result
                    assert result['analysis']['domain']=='general' and result['plan']['revision']==2
                    structured=json.loads(json.dumps(actual['request']));structured['task_id']=str(uuid.uuid4());structured['original_ask']=structured['objective']=structured['input']['original_ask']='Structured greeting fixture'
                    bypass=req(route,{'op':'execute_structured','request':structured,'hints':{'evidence_required':False,'template':'direct/v1'}})
                    assert bypass['request']['task_id']==structured['task_id']
                    result=wait(bypass['request']['task_id']);assert result['status']=='generated' and not result.get('analysis'),result
                    missing=req(route,{'op':'execute','input':{'original_ask':'missing-intent-fixture'}})
                    result=wait(missing['request']['task_id']);assert result['status']=='needs_clarification' and result['clarification_questions'],result
                    source['revision']='lookup';source['lookups']=[{'exact_ask':'Get fixture extraction','temporal_scope':'','context_independent':True,'start_byte':0,'end_byte':9}]
                    req(route,{'op':'ingest','source':source})
                    for expected in ['deterministic_lookup','exact_answer_cache']:
                        hit=req(route,{'op':'execute','input':{'original_ask':'Get fixture extraction'}})['result']
                        assert hit['actual_path']==expected and hit['measurements']['generative_model_calls']==0
                    import sqlite3
                    stores=list(folder.glob('data/companies/one/**/company.sqlite3'))
                    if not stores: stores=list(folder.glob('data/companies/one/**/*.sqlite3'))
                    assert stores, list(folder.rglob('*.sqlite3'))
                    with sqlite3.connect(stores[0]) as db:
                        assert db.execute('select count(*) from task_workflow_plan_revisions_v1').fetchone()[0]>=2
                        assert db.execute("select count(*) from audit where kind='task_intake'").fetchone()[0]>=3
                    print(json.dumps({'transport':'real HTTP and SQLite','provider':'mock','evaluated_intake_schema':True,'analysis_only':True,'clarification_blocks_workers':True,'exact_lookup_bypasses_intake':True,'durable_revisions':True,'provider_cost_usd':0}))
                    return
                records=[]
                for template in ['direct/v1','rag/v1','tool_loop/v1','pipeline/v1','hub/v1','hierarchy/v1','peer/v1','critique/v1','explore/v1']:
                    started=req(route,{'op':'execute_workflow','input':task,'hints':{'template':template,'evidence_required':False}})
                    result=wait(started['request']['task_id'])
                    assert result['status']=='generated',(template,result)
                    assert result['measurements']['generative_model_calls']==0 # mock is not live inference
                    assert result['verification_state']!='verified'
                    if template=='tool_loop/v1':
                        assert result['measurements']['tool_calls']==1
                    records.append({'template':template,'nodes':len(result['plan']['nodes']),'status':result['status']})
                custom_intake=req(route,{'op':'destructure_task','input':{'original_ask':'Draft a release summary'},'hints':{'domain':'release-specialist','evidence_required':False}})
                assert custom_intake['analysis']['domain']=='release-specialist'
                custom_job=req(route,{'op':'execute_workflow','input':{'original_ask':'Draft a release summary'},'hints':{'domain':'release-specialist','template':'direct/v1','evidence_required':False}})
                assert wait(custom_job['request']['task_id'])['status']=='generated'
                duplicate={'op':'execute_workflow','input':{'original_ask':'Coalesce reference chronology evidence'},'hints':{'template':'pipeline/v1','evidence_required':False}}
                with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                    requests=list(pool.map(lambda _:req(route,duplicate),range(2)))
                assert requests[0]['job_id']==requests[1]['job_id']
                first,second=[r['request']['task_id'] for r in requests]
                assert req(route,{'op':'cancel_task','task_id':first})['status']=='cancelled'
                completed=wait(second);assert completed['status']=='generated'
                if completed['cache']['origin_task_id']: assert completed['plan']['task_id']==completed['cache']['origin_task_id']
                assert status(first)['status']=='cancelled'
                changed=req(route,{'op':'execute_workflow','input':{'original_ask':'Check reference evidence correction'},'hints':{'template':'pipeline/v1','evidence_required':False}})
                source['revision']='v2';source['text']='Corrected reference source.'
                req(route,{'op':'ingest','source':source})
                invalid=wait(changed['request']['task_id'])
                assert invalid['status']=='evidence_invalidated' and invalid['answer'] is None
                req('/api/companies/empty',{'op':'register','name':'Empty retrieval fixture'})
                empty=req('/api/companies/empty/tasks',{'op':'execute','input':{'original_ask':'No accessible evidence is available'}})
                deadline=time.monotonic()+5
                while True:
                    missing=req('/api/companies/empty/tasks',{'op':'status','task_id':empty['request']['task_id']})
                    if missing['status']!='running':break
                    assert time.monotonic()<deadline
                    time.sleep(.05)
                assert missing['status']=='blocked' and missing['measurements']['generative_model_calls']==0
                assert missing['verification_state']=='not_verified'
                time.sleep(1.1)
                retry=req('/api/companies/empty/tasks',{'op':'execute','input':{'original_ask':'No accessible evidence is available'}})
                assert retry['job_id']!=empty['job_id'], 'terminal failures must not suppress fresh requests for a full hour'
                import sqlite3
                with sqlite3.connect(folder/'data/companies/empty/brain.sqlite3') as db:
                    retained=db.execute('select expires,retained_until from task_workflow_jobs_v1 where id=?',(empty['job_id'],)).fetchone()
                    assert retained and retained[1]>retained[0], 'negative eligibility and history retention must be separate'
                    assert db.execute("select count(*) from audit where run_id=? and kind='task_workflow_finished'",(empty['job_id'],)).fetchone()[0]==1
                assert req('/api/companies/empty/tasks',{'op':'status','task_id':empty['request']['task_id']})['status']=='blocked'


                assert req(route,{'op':'discover_models'})['artifacts']==[] # isolated fixture has no models
                if args.browser:
                    subprocess.run(['node','scripts/task_workflows_browser.mjs'],cwd=ROOT,env={**env,'GENTS_TASK_FIXTURE_URL':origin},timeout=90,check=True)
                print(json.dumps({'transport':'real HTTP and SQLite','provider':'mock, zero generative calls','workflows':records,'duplicate_job_coalesced':True,'subscriber_cancel_isolated':True,'source_correction_invalidated':True,'provider_cost_usd':0}))
            finally:
                child.terminate()
                try:
                    child.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    child.kill();child.wait(timeout=5)
                if child.returncode not in (0,-15):
                    print((folder/'harness.log').read_text()[-2500:])
if __name__=='__main__':
    main()

"""Opt-in actual local completion of a retained native CEO planning fixture.
No new conversation is simulated, no tool calls/downloads/paid fallback occur.
The private exact-text oracle is frozen before inference and never sent to models.
"""
import argparse,hashlib,json,time,urllib.request,uuid
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--run-local',action='store_true')
p.add_argument('--conversation-proof',type=Path,required=True)
a=p.parse_args()
if not a.run_local:p.error('Pass --run-local to start a real local task.')
ROOT=Path(__file__).resolve().parents[1];base='http://127.0.0.1:8788'
prior=json.loads(a.conversation_proof.read_text());assert prior['status']=='PASS' and 'Actual local CEO clarification' in prior['scope']
company=prior['company'];session=prior['planning_id'];assessment=prior['assessment_id'];run=str(uuid.uuid4())
folder=ROOT/'data/reasoning-completion-trials'/run;folder.mkdir(parents=True,mode=0o700)
def save(name,value):(folder/name).write_text(json.dumps(value,indent=2)+'\n')
def api(path,body=None,headers=None):
 req=urllib.request.Request(base+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json',**(headers or {})})
 with urllib.request.urlopen(req,timeout=30) as response:return json.load(response)
path='/api/companies/'+company
config=api('/api/config');assert config['local_only'] and config['company_tools']
assert all(r['status'] not in ['running','queued'] for r in api('/api/runs')['runs'])
plan=api(path+'/planning/'+session);assert plan['status']=='ready'
review=api(path+'/planning/'+session+'/architecture/'+assessment);assert review['status']=='ready_for_owner_review'
prepared=api(path+'/planning/'+session+'/prepare-guided',{'expected_revision':plan['revision'],'architecture_assessment':assessment,'data_classification':'synthetic','artifact_bindings':[]})
assert prepared['candidate_only'] is False and prepared['execution_authorized'] is False
save('canonical-planning.json',plan);save('canonical-architecture.json',review);save('prepared.json',prepared)
draft=prepared['draft'];expected='The rehearsal is scheduled for Tuesday. The checklist remains under review.'
save('private-oracle.json',{'expected_exact_normalized_text':expected.lower(),'scope':'Strict fixture check only, frozen before task inference. Not provided to any runtime packet.'})
info={'run':run,'company':company,'planning_id':session,'assessment_id':assessment,'base':base,'scope':'Continues an earlier actual native CEO clarification/plan and architecture assessment. Starts a new real text task, independent local final review and lifecycle gate. No new planning inference or architecture choice is implied.'}
save('freeze.json',info)
job=next(j for j in api(path,{'op':'intake','run_id':run,'intake':draft['intake']})['jobs'] if j['run_id']==run)
assert job['state']=='draft'
for k,v in draft['intake'].items():assert job['intake'][k]==v,k
save('job.json',job)
api(path,{'op':'approve','run_id':run,'digest':job['digest']})
api(path,{'op':'governance_install','run_id':run,'expected_revision':0,'policy':draft['policy']})
# Exactly one dispatch attempt. A lost response requires inspection, never replay.
started=time.monotonic()
admitted=api('/api/runs',{'company_id':company,'run_id':run,'task':draft['task'],'definition_of_done':draft['criteria'],'consent':False},{'Idempotency-Key':str(uuid.uuid4())});assert admitted['id']==run
print(json.dumps({'status':'STARTED','folder':str(folder),**info}),flush=True)
while True:
 state=api('/api/runs/'+run);save('observed-run.json',state)
 if state['status'] not in ['running','queued']:break
 if time.monotonic()-started>1500:
  save('observation-expired.json',{'run':run,'status':'OBSERVATION_EXPIRED','scope':'Task may still run. Inspect this exact handle; no cancellation or new dispatch was attempted.'})
  raise RuntimeError('Observation expired; inspect original task without replay')
 time.sleep(2)
terminal=next((e for e in reversed(state['events']) if e['kind']==state['status']),None)
output=json.loads(terminal['message']) if terminal and terminal['message'].startswith('{') else {}
text=output.get('deliverable','');oracle=' '.join(text.split()).lower()==expected.lower()
calls=[e for e in state['events'] if e['kind']=='call_completed']
assert all(e['reply']['usage']['cost_usd']==0 for e in calls)
report={'status':'PASS' if state['status']=='completed' and oracle else 'FAIL','run':run,'company':company,'runtime_status':state['status'],'private_exact_text_oracle_passed':oracle,'deliverable':text,'actual_model_calls':len(calls),'local_input_tokens':sum(e['reply']['usage']['input'] for e in calls),'local_output_tokens':sum(e['reply']['usage']['output'] for e in calls),'provider_fees_usd':0,'elapsed_ms':round((time.monotonic()-started)*1000),'scope':info['scope']+' One synthetic note; not a held-out benchmark, arbitrary-task guarantee, tool task or compliance qualification.'}
save('report.json',report);print(json.dumps(report),flush=True)
assert report['status']=='PASS','Inspect retained evidence; no automatic retry or criterion change'

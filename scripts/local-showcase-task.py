"""Owner-invoked local campaign task; retain one dispatch, frozen scope and receipts.
No automatic retries, publication, email, downloads or changes to existing policies.
Fresh policy grants include the owner-controlled company-research.json defaults.
"""
import argparse,hashlib,json,shutil,time,urllib.request,urllib.parse,uuid
from pathlib import Path
from company_research import fresh_access
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--spec',type=Path,required=True);p.add_argument('--server',default='http://127.0.0.1:8788');a=p.parse_args()
root=Path(__file__).resolve().parents[1];spec=json.loads(a.spec.read_text());base=a.server.rstrip('/');target=urllib.parse.urlparse(base)
assert target.scheme=='http' and target.hostname=='127.0.0.1' and not target.username and not target.password and target.path=='', 'Loopback harness only'
run=str(uuid.uuid4());folder=root/'data/local-showcase'/run;folder.mkdir(mode=0o700,parents=True)
def save(name,data):(folder/name).write_text(json.dumps(data,indent=2)+'\n')
def api(path,body=None,headers=None):
 req=urllib.request.Request(base+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json',**(headers or {})})
 with urllib.request.urlopen(req,timeout=30) as r:return json.load(r)
config=api('/api/config');assert config['local_only'] and config['company_tools']
access_map,research_profile=fresh_access(root/'config/company-research.json',[actor['id'] for actor in config['agents']],spec['actor'],spec['tools'])
spec['tools']=access_map[spec['actor']]['tools']
save('research-profile.json',research_profile)
save('freeze.json',{'run':run,'spec':spec,'source_revision':__import__('subprocess').check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'source_dirty':bool(__import__('subprocess').check_output(['git','status','--porcelain'],cwd=root,text=True).strip()),'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Actual owner-authorized local task; independent artifact acceptance remains separate.'})
route='/api/companies/'+spec['company']
if spec.get('register'):api(route,{'op':'register','name':'Gents local capability campaign'})
intake={'idea':spec['task'],'acceptance':'\n'.join(spec['criteria']),'exclusions':'All tools outside the frozen grant','timeline':'Measured, no completion-time promise','company_signer':'Owner-authorized Gents campaign','provider_signer':'Local Gents Company','budget_usd':0,'deliverable_type':'agent'}
if spec.get('candidate_workflow'):
 intake['candidate_workflow']=spec['candidate_workflow'];intake['deliverable_type']='candidate'
state=api(route,{'op':'intake','run_id':run,'intake':intake});job=next(j for j in state['jobs'] if j['run_id']==run);save('job.json',job)
api(route,{'op':'approve','run_id':run,'digest':job['digest']})
policy={'revision':1,'master':spec['actor'],'ceilings':access_map,'grants':access_map}
api(route,{'op':'governance_install','run_id':run,'expected_revision':0,'policy':policy});save('policy.json',policy)
# Owner-supplied public/synthetic reference copies only; model output never selects these paths.
inputs=[]
workspace=root/'data/companies'/spec['company']/'runs'/run/'workspace'
for item in spec.get('seed_files',[]):
 source=(root/item['source']).resolve(strict=True)
 assert source.is_relative_to(root/'data/local-showcase/research-inputs') and source.is_file() and not (root/item['source']).is_symlink()
 relative=Path(item['path']);assert not relative.is_absolute() and all(part not in ('.','..') and not part.startswith('.') for part in relative.parts)
 target=workspace/relative;target.parent.mkdir(parents=True,exist_ok=True);assert not target.exists();shutil.copyfile(source,target)
 inputs.append({'path':str(relative),'sha256':hashlib.sha256(source.read_bytes()).hexdigest()})
save('reference-inputs.json',inputs)
started=time.monotonic();save('dispatch-intent.json',{'run':run,'status':'pending; reconcile by run ID if response is lost'})
result=api('/api/runs',{'company_id':spec['company'],'run_id':run,'task':spec['task'],'definition_of_done':spec['criteria'],'consent':False},{'Idempotency-Key':run});save('dispatch.json',result)
print(json.dumps({'status':'STARTED','run':run,'folder':str(folder)}),flush=True)
previous=None
while True:
 state=api('/api/runs/'+run);save('observed-run.json',state)
 attention=api('/api/runs/'+run+'/attention');save('attention.json',attention)
 calls=[e for e in state.get('events',[]) if e.get('reply')]
 status={'run':run,'runtime_status':state['status'],'elapsed_seconds':round(time.monotonic()-started,2),'model_calls':len(calls),'provider_cost_usd':sum(e['reply']['usage']['cost_usd'] for e in calls)}
 assert status['provider_cost_usd']==0,'Unexpected provider cost; inspect original task'
 save('report.json',status)
 current=(state['status'],len(calls),len(state.get('events',[])))
 if current!=previous:print(json.dumps(status),flush=True);previous=current
 if state['status'] not in ('running','queued'):break
 if attention.get('attention') and not attention['attention'].get('answer'):
  print(json.dumps({'status':'ATTENTION','run':run,'question':attention['attention']['question']}),flush=True);break
 if time.monotonic()-started>1800:
  print(json.dumps({'status':'OBSERVATION_EXPIRED','run':run,'scope':'Task may remain active; inspect original ID, never redispatch.'}),flush=True);break
 time.sleep(3)

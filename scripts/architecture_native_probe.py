"""Opt-in two-weight local architecture assessment on an explicitly seeded CEO plan.
No task execution, planning-conversation qualification or general competence claim.
"""
import hashlib,json,os,shutil,socket,sqlite3,subprocess,time,urllib.request,uuid,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
native_conversation='--native-conversation' in sys.argv

def digest(data):return hashlib.sha256(data).hexdigest()
def free_port():
 with socket.socket() as s:s.bind(('127.0.0.1',0));return s.getsockname()[1]
probe=str(uuid.uuid4());folder=ROOT/'data/architecture-native-probes'/probe;folder.mkdir(parents=True,mode=0o700)
profile=ROOT/'config/company.mcp-development-reasoning-repair.json'
config=json.loads(profile.read_text());assert len(config['planning_model_policy']['candidates'])>=2
actor=config['agents'][0]['id'];company='architecture-native-'+probe.replace('-','');planning=str(uuid.uuid4());assessment=str(uuid.uuid4())
http_port,model_port=free_port(),free_port()
while model_port==http_port:model_port=free_port()
binary=folder/'gents-harness';shutil.copy2(ROOT/'target/debug/gents-harness',binary)
configuration=folder/'config.json';configuration.write_text(json.dumps(config))
idea='Summarize a supplied synthetic launch note into two concise sentences. This is a small text-only task with fixed input and no tool, browsing, file or coding work. Independent final task acceptance is separate.'
proposal={'status':'ready','question':'','definition_of_done':['Return two concise sentences grounded in the supplied synthetic note; do not invent facts.'], 'assignments':[{'actor':actor,'task':'Write the requested two-sentence text summary from supplied input.','tools':[],'acceptance':['Use only supplied information.']}], 'native_contracts':{'sources':[],'artifacts':[]}}
request={'expected_revision':2,'speed':'patient','maximum_elapsed_seconds':14*86400}
freeze={'probe':probe,'company':company,'planning_id':planning,'assessment_id':assessment,'request':request,'proposal':proposal,'idea':idea,'profile_sha256':digest(profile.read_bytes()),'binary_sha256':digest(binary.read_bytes()),'http_port':http_port,'model_port':model_port,'private_expected':{'architecture':'ordered','agent_count':1},'scope':'CEO staffing/owner plan seeded fixture. Actual architecture CEO and distinct-weight judge only. Private expected values excluded from all runtime packets.'}
planning_messages=[
 'I need a short update from a synthetic launch note, using one local writer and no tools, files, web or code. Ask me one question to confirm the desired output before planning.',
 'Confirm the definition of done: return two concise sentences using only this synthetic note: the rehearsal is scheduled for Tuesday; the checklist remains under review. Do not invent facts. Use exactly one assigned local writer with no tools, files, web or code. The final answer must preserve both facts. Independent acceptance will be separate.'
]
freeze['native_conversation']=native_conversation
if native_conversation:freeze['planning_user_messages']=planning_messages
(folder/'freeze.json').write_text(json.dumps(freeze,indent=2)+'\n')
env=dict(os.environ,AUTH_MODE='local',BIND=f'127.0.0.1:{http_port}',CONFIG=str(configuration),RUN_STORE=str(folder/'runs.sqlite3'),LOCAL_MODEL_PORT=str(model_port),GENTS_LOCAL_ONLY='1',HF_HUB_OFFLINE='1',HF_HUB_DISABLE_TELEMETRY='1',TRANSFORMERS_OFFLINE='1',GENTS_DISABLE_SCHEDULER='1',GENTS_OFFICE_ACTIVITY='0',GENTS_SQLITE_BRIDGE=str(ROOT/'target/debug/gents-sqlite-bridge'))
origin=f'http://127.0.0.1:{http_port}'
def api(path,body=None):
 req=urllib.request.Request(origin+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
 with urllib.request.urlopen(req,timeout=600) as response:return json.load(response)
log=(folder/'harness.log').open('w');process=subprocess.Popen([str(binary)],cwd=ROOT,env=env,stdout=log,stderr=log)
print(json.dumps({'status':'STARTED','probe':probe,'pid':process.pid,'origin':origin,'model_port':model_port}),flush=True)
try:
 for attempt in range(100):
  try:api('/api/config');break
  except OSError:
   assert process.poll() is None
   time.sleep(.1)
 path='/api/companies/'+company;api(path,{'op':'register','name':'Explicit seeded architecture fixture'})
 dbfile=ROOT/'data/companies'/company/'brain.sqlite3'
 if native_conversation:
  for index,message in enumerate(planning_messages):
   revision=0 if index==0 else planned['revision']
   planned=api(path+'/planning/'+planning,{'message':message,'expected_revision':revision,'planning_mode':'reasoning'})
   (folder/f'planning-{index+1}.json').write_text(json.dumps(planned,indent=2)+'\n')
   assert planned['status']==('question' if index==0 else 'ready'),planned.get('status')
  proposal=planned['proposal'];assert len(proposal['assignments'])==1 and proposal['assignments'][0]['tools']==[]
  actor=proposal['assignments'][0]['actor'];request['expected_revision']=planned['revision']
  freeze['proposal']=proposal;freeze['request']=request;freeze['scope']='Actual local CEO clarification and ready plan, then actual distinct-weight architecture review. Advisory preparation only; no task execution or acceptance.'
  (folder/'freeze.json').write_text(json.dumps(freeze,indent=2)+'\n')
 else:
  with sqlite3.connect(dbfile) as db:
   db.execute('CREATE TABLE IF NOT EXISTS ceo_planning(id TEXT PRIMARY KEY,revision INTEGER,state TEXT,body TEXT)')
   body={'id':planning,'revision':2,'status':'ready','proposal':proposal,'messages':[{'role':'user','message':idea},{'role':'ceo','message':proposal,'fixture':'SEEDED, not native planning inference'}]}
   db.execute("INSERT INTO ceo_planning VALUES(?,2,'ready',?)",(planning,json.dumps(body)));db.commit()
 resource=api(path+'/hardware');(folder/'initial-hardware.json').write_text(json.dumps(resource,indent=2)+'\n')
 route=path+'/planning/'+planning+'/architecture/'+assessment
 started=time.monotonic();result=api(route,request)
 (folder/'result.json').write_text(json.dumps(result,indent=2)+'\n')
 assert result['status']=='ready_for_owner_review',result.get('status')
 assert result['recommendation']['architecture']=='ordered' and result['recommended_agent_count']==1
 assert result['models']['ceo']['sha256']!=result['models']['judge']['sha256']
 assert result['execution_authorized'] is False and result['task_accepted'] is False
 assert result['media_routing']['modalities']==['text'] and result['media_routing']['execution_authorized'] is False
 assert result['receipts']['output_judge']['accepted_protocol'] is True
 # Exact same assessment ID is an observation/cache reuse, not another inference.
 repeated=api(route,request);assert repeated==result
 with sqlite3.connect(dbfile) as db:
  assert db.execute('SELECT COUNT(*) FROM jobs').fetchone()[0]==0
  assert db.execute('SELECT COUNT(*) FROM architecture_assessments').fetchone()[0]==1
 # Verify the real factory consumes the exact reviewed architecture identity.
 prep={'expected_revision':request['expected_revision'],'specification':{'version':1,'objective':idea,'inputs':['Supplied synthetic launch note'],'outputs':['Two-sentence text summary'],'definition_of_done':proposal['definition_of_done'],'exclusions':['Remote services and unapproved tools'],'coordination':'ordered','data_classification':'synthetic','control_profiles':['local_baseline'],'required_capabilities':[]},'input':{'text':'Synthetic launch note: the rehearsal is scheduled for Tuesday; the checklist remains under review.'},'model_sha256':{actor:config['planning_model_policy']['candidates'][0]['sha256']},'max_delegations':1,'peer_rounds':1,'architecture_assessment':assessment}
 prepared=api(path+'/planning/'+planning+'/prepare',prep);assert prepared['intake_patch']['factory_binding']['assessment_sha256']
 (folder/'prepared.json').write_text(json.dumps(prepared,indent=2)+'\n')
 usage={key:sum(result['receipts'][role]['reply']['usage'][key] for role in ['ceo','output_judge','judge']) for key in ['input','output']}
 report={'status':'PASS','probe':probe,'company':company,'planning_id':planning,'assessment_id':assessment,'elapsed_ms':round((time.monotonic()-started)*1000),'usage':usage,'provider_tool_fees_usd':0,'architecture':'ordered','agent_count':1,'distinct_local_weight_pins':True,'output_routing':['text'],'actual_model_calls':3,'canonical_cache_reused':True,'factory_reference_bound':True,'task_executed':False,'scope':freeze['scope']}
 (folder/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True)
except Exception as error:
 (folder/'failure.json').write_text(json.dumps({'status':'FAIL','type':type(error).__name__,'scope':'Original evidence retained; no automatic POST replay or paid fallback.'},indent=2)+'\n')
 raise
finally:
 process.terminate();process.wait(timeout=120);log.close()
 for port in [http_port,model_port]:
  with socket.socket() as s:assert s.connect_ex(('127.0.0.1',port))!=0,'Owned probe listener remains active'

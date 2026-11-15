// Real local CEO -> approved code author -> isolated native compilation/acceptance.
import {spawn} from 'node:child_process';
import {parseArgs} from 'node:util';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {randomUUID,createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {fixtureFromId,fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
import {reviewCodingPlan,verifyCodingWorkflow} from './mcp-coding-verifier.mjs';
import {planningTurn,PlanningObservationUncertain} from './mcp-planning-observer.mjs';
import {confirmFrozenCeoJob} from '../web/assets/ceo-execution.js';
import {renderWorkflowTranscript} from './mcp-transcript.mjs';
import {evaluationProfile,freezeEvaluationProfile} from './mcp-evaluation-profile.mjs';
import {fileURLToPath} from 'node:url';
import {saveObservation} from './mcp-observation.mjs';
import {runtimeFingerprint} from './mcp-runtime-fingerprint.mjs';
import {evaluationHost} from './mcp-evaluation-host.mjs';
import {acceptancePreflight} from './mcp-rust-acceptance.mjs';
import {createRuntimeBundle,bundleEnvironment,bundleExecutable,verifyBundleBinding,probeBundle} from './mcp-runtime-bundle.mjs';
const {values}=parseArgs({options:{case:{type:'string'},'attempt-id':{type:'string'},'planning-session':{type:'string'},'existing-store':{type:'string'},'resume-report':{type:'string'},'runtime-config':{type:'string'},'resume-reason':{type:'string'}}});
assert.equal(Boolean(values['planning-session']||values['resume-report']),Boolean(values['existing-store']),'Recovery needs session or report plus existing store');assert(!(values['planning-session']&&values['resume-report']));
const priorFolder=values['resume-report']?new URL(values['resume-report'].replace(/\/?$/,'/'),new URL('../',import.meta.url)):null;const prior=priorFolder?JSON.parse(await readFile(new URL('report.json',priorFolder),'utf8')):null;
const fixture=fixtureFromId(values.case??prior?.fixture_id??'rust-range-union');assert.equal(fixture.kind,'rust_library');
const attempt=values['attempt-id']??randomUUID();assert(/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(attempt));
const root=new URL('../',import.meta.url),folder=new URL(`data/mcp-workflow-check/${attempt}/`,root),origin='http://127.0.0.1:8796',company='mcp-workflow-check';
const selectedProfile=await evaluationProfile(root,values['runtime-config']??prior?.runtime_profile?.path);
await mkdir(folder,{recursive:true,mode:0o700});
if(await fetch(origin+'/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null))throw Error('Port 8796 occupied; existing work was not stopped');
const {profile:runtimeProfile,changed:profileChanged}=await freezeEvaluationProfile(root,folder,selectedProfile,prior,values['resume-reason']);
const runtimeBuild=await runtimeFingerprint(root);
const runtimeBundle=await createRuntimeBundle(root),acceptanceCheck=acceptancePreflight(root,fixture,runtimeBundle);
const runtimeProbe=probeBundle(root,runtimeBundle);
const evaluationHostObservation=evaluationHost();
const child=spawn(bundleExecutable(root,runtimeBundle,'gents-harness'),[],{cwd:root,stdio:['ignore','ignore','inherit'],env:{...process.env,...bundleEnvironment(runtimeBundle),GENTS_SQLITE_BRIDGE:bundleExecutable(root,runtimeBundle,'gents-sqlite-bridge'),AUTH_MODE:'local',BIND:'127.0.0.1:8796',CONFIG:runtimeProfile.frozen_path,RUN_STORE:values['existing-store']?fileURLToPath(new URL(values['existing-store'],root)):fileURLToPath(new URL('runs.sqlite3',folder)),LOCAL_MODEL_PORT:'8797',GENTS_DISABLE_SCHEDULER:'1',GENTS_LOCAL_ONLY:'1',GENTS_MCP_TRANSPORT:'sse',GENTS_OFFICE_ACTIVITY:'0',HF_HUB_OFFLINE:'1',TRANSFORMERS_OFFLINE:'1'}});
const report={fixture_id:fixture.id,fixture_sha:fixtureDigest(fixture),kind:fixture.kind,title:fixture.title,runtime_profile:runtimeProfile,runtime_profile_changed:profileChanged,prior_runtime_profile:prior?.runtime_profile??null,resume_reason:values['resume-reason']??null,evaluation_scope:{industry:fixture.industry??'general',complexity:fixture.complexity??'unclassified',complexity_basis:fixture.complexity_basis??'No general difficulty rating inferred',modalities:fixture.modalities??['text'],synthetic:true},status:'PLANNING',company,origin,planning_session:prior?.planning_session??values['planning-session']??randomUUID(),run:prior?.run??null,same_run_resume:Boolean(prior),prior_report:values['resume-report']??null,started_at_ms:Date.now(),synthetic_users:true,operator_reviews:[],provider_cost_usd:0};
report.planning_runtime_profile=prior?.planning_runtime_profile??prior?.runtime_profile??runtimeProfile;
report.runtime_build=runtimeBuild;report.prior_runtime_build=prior?.runtime_build??null;
report.evaluation_host=evaluationHostObservation;report.prior_evaluation_host=prior?.evaluation_host??null;
report.frozen_acceptance_preflight=acceptanceCheck;
report.runtime_bundle=runtimeBundle;report.launch_executables_source='runtime_bundle';report.prior_runtime_bundle=prior?.runtime_bundle??null;
report.runtime_bundle_probe=runtimeProbe;
const save=(name,value)=>writeFile(new URL(name,folder),typeof value==='string'?value:JSON.stringify(value,null,2),{mode:0o600});
async function request(path,body){const response=await fetch(origin+path,{method:body?'POST':'GET',headers:{'Content-Type':'application/json'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(180000)});if(!response.ok){const e=Error(`${response.status}: ${await response.text()}`);e.status=response.status;throw e;}return response.json();}
const op=(op,body={})=>request(`/api/companies/${company}`,{op,...body});
const observePlanning=(path,body)=>planningTurn({request,path,body,session:report.planning_session,onObservation:async observation=>{report.planning_observations??=[];report.planning_observations.push({...observation,at_ms:Date.now()});await save('report.json',report);console.log(JSON.stringify(observation));}});
let plan,saved,keepListener=false;
try{
 for(let i=0;;i++){try{await request('/api/config');break;}catch(e){if(child.exitCode!==null||i>=100)throw e;await new Promise(r=>setTimeout(r,100));}}
 const config=await request('/api/config'),catalog=await request('/api/mcp/catalog');assert(config.local_only&&catalog.harness_transport==='local_sse_mcp');assert(catalog.tools.some(t=>t.name==='rust_verify'));
 report.runtime_bundle_binding=verifyBundleBinding(catalog,runtimeBundle);
 await op('register',{name:'Reviewed local company curriculum'});await save('report.json',report);console.log(JSON.stringify({status:'STARTED',pid:child.pid,report:fileURLToPath(new URL('report.json',folder))}));
 const path=`/api/companies/${company}/planning/${report.planning_session}`;
 plan=prior?JSON.parse(await readFile(new URL('planning.json',priorFolder),'utf8')):values['planning-session']?await request(path):await observePlanning(path,{message:fixture.initial,expected_revision:0,planning_mode:fixture.planningMode});await save('planning.json',plan);
 let accepted;
 for(let turn=0;turn<=6;turn++){
  let rejection;
  if(plan.status==='ready'){
   try{accepted=reviewCodingPlan(plan,fixture,config.agents.map(a=>a.id));}
   catch(e){rejection=e.message;}
   report.operator_reviews.push(accepted?{revision:plan.revision,status:'accepted'}:{revision:plan.revision,status:'rejected',reason:rejection});
   await save('report.json',report); // Storage failure is not a model contract error.
   if(accepted)break;
  }
  if(plan.status==='failed'&&plan.inference_failure?.retryable_without_host_change===false)throw Error('Planning runtime failure requires host diagnosis: '+plan.inference_failure.code);
  assert(['question','failed','ready'].includes(plan.status));if(prior)throw Error('Frozen run cannot be replanned');if(turn===6)break;const repair=Boolean(rejection)||plan.status==='failed';
  plan=await observePlanning(path,{message_kind:repair?'planning_repair':'requirements',message:repair?`Repair the rejected scope: ${rejection??plan.validation_error??plan.error}. Preserve the original function contract and local-only boundaries; choose appropriate installed tools yourself.`:fixture.requirements,expected_revision:plan.revision,planning_mode:fixture.planningMode});await save('planning.json',plan);
 }
 assert(accepted,'CEO did not produce an approvable coding plan');const {draft}=accepted;report.run=prior?.run??randomUUID();report.ceo_selected_actor=accepted.stage.actor;report.reviewed_tools=accepted.stage.tools;report.host_compiled_readbacks=plan.host_compiled_readbacks??[];const raw=plan.messages.filter(m=>m.role==='ceo').at(-1)?.message;report.ceo_selected_tools=raw?.assignments?.find(s=>s.actor===accepted.stage.actor)?.tools??accepted.stage.tools;
 await save('report.json',report); // Retain issued job identity before any approval mutation.
 let review;
 const workspace=new URL(`data/companies/${company}/runs/${report.run}/workspace/`,root);
 if(prior){
  review=JSON.parse(await readFile(new URL('execution-review.json',priorFolder),'utf8'));const current=(await op('status')).jobs.find(j=>j.run_id===report.run);assert(current&&current.digest===review.digest);assert.deepEqual(current.intake,review.intake);for(const [key,value] of Object.entries(draft.intake))assert.deepEqual(review.intake[key],value);
  assert.equal(await readFile(new URL(fixture.testsPath,workspace),'utf8'),fixture.testsText,'Frozen acceptance changed');
 }else{
  const state=await op('intake',{run_id:report.run,intake:draft.intake});review=confirmFrozenCeoJob(draft,state.jobs.find(j=>j.run_id===report.run));await save('execution-review.json',review);
  await mkdir(workspace,{recursive:true,mode:0o700});await writeFile(new URL(fixture.testsPath,workspace),fixture.testsText,{flag:'wx',mode:0o600});
  await op('approve',{run_id:report.run,digest:review.digest});await op('governance_install',{run_id:report.run,expected_revision:0,policy:review.policy});
 }
 await save('execution-review.json',review);await save('owner-acceptance.rs',fixture.testsText);
 report.status='EXECUTING';await save('report.json',report);
 if(prior){saved=await request(`/api/runs/${report.run}`);report.before_resume_status=saved.status;if(['cancelled','incomplete','failed'].includes(saved.status))await request(`/api/runs/${report.run}/resume`,{});else assert(['running','completed'].includes(saved.status));}
 else await request('/api/runs',{company_id:company,run_id:report.run,task:draft.task,definition_of_done:draft.criteria,consent:false});
 let last='',lastSnapshot=0;for(;;){
  try{saved=await request(`/api/runs/${report.run}`);}catch(e){if(child.exitCode!==null||e.status===404)throw e;report.last_observation_error=String(e);await save('report.json',report);await new Promise(r=>setTimeout(r,1000));continue;}
  const state=`${saved.status}/${saved.events.length}`;if(state!==last){console.log(JSON.stringify({run:report.run,status:saved.status,events:saved.events.length}));last=state;}
  if(Date.now()-lastSnapshot>=60000){try{await saveObservation({folder,fixture,report,plan,saved});lastSnapshot=Date.now();}catch(error){console.error(JSON.stringify({observation_save_error:String(error),run:report.run}));}}
  if(saved.status!=='running')break;await new Promise(r=>setTimeout(r,2000));
 }
 await save('events.json',saved);Object.assign(report,await verifyCodingWorkflow({root,fixture,plan,saved,review,company,run:report.run,runtimeProfile:report.planning_runtime_profile}));
}catch(e){
 report.error=String(e);
 if(child.exitCode===null&&report.run){try{saved=await request(`/api/runs/${report.run}`);keepListener=saved.status==='running';}catch(observation){keepListener=observation.status!==404;report.cleanup_observation_error=String(observation);}}
 else if(child.exitCode===null){try{plan=await request(`/api/companies/${company}/planning/${report.planning_session}`);keepListener=plan.status==='generating';}catch{}}
 if(e instanceof PlanningObservationUncertain&&child.exitCode===null)keepListener=true;
 report.status=keepListener?'OBSERVATION_ERROR_LIVE_WORK_RETAINED':'FAIL';process.exitCode=1;
}finally{
 report.elapsed_ms=Date.now()-report.started_at_ms;await save('report.json',report);if(plan)await save('planning.json',plan);if(saved)await save('events.json',saved);await save('transcript.html',renderWorkflowTranscript({fixture,report,plan,saved}));console.log(JSON.stringify(report));
 if(!keepListener){child.kill('SIGTERM');if(child.exitCode===null)await new Promise(r=>child.once('exit',r));}else console.log(JSON.stringify({status:'LIVE_LISTENER_RETAINED',pid:child.pid,run:report.run}));
}

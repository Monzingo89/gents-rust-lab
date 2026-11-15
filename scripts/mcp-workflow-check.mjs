// Reusable actual local CEO -> frozen native graph -> independent verifier.
// Synthetic fixture requirements are authority; expectedResults never enters a prompt.
import {spawn} from 'node:child_process';
import {renderWorkflowTranscript} from './mcp-transcript.mjs';
import {parseArgs} from 'node:util';
import {mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import assert from 'node:assert/strict';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {verifyWorkflow} from './mcp-workflow-verifier.mjs';
import {reviewWorkflowPlan,planningInputs} from './mcp-workflow-review.mjs';
import {prepareCeoExecution,confirmFrozenCeoJob} from '../web/assets/ceo-execution.js';
import {runtimeFingerprint} from './mcp-runtime-fingerprint.mjs';
import {evaluationHost} from './mcp-evaluation-host.mjs';
import {createRuntimeBundle,bundleEnvironment,bundleExecutable,verifyBundleBinding,probeBundle} from './mcp-runtime-bundle.mjs';
import {evaluationProfile,freezeEvaluationProfile} from './mcp-evaluation-profile.mjs';
import {fixtureBytes} from './mcp-fixture-files.mjs';
import {materializeOwnedFixture} from './mcp-owned-fixture.mjs';
import {preparedPlanningFields} from './mcp-owned-prepared.mjs';
import {needsSelection,reviewSelectionDraft,selectedExecutionFixture} from './mcp-owned-selection.mjs';
import {ownerAuthorization} from './mcp-owner-authorization.mjs';
import {saveObservation} from './mcp-observation.mjs';
import {planningTurn,PlanningObservationUncertain} from './mcp-planning-observer.mjs';
import {validatePlanningContinuation} from './mcp-planning-continuation.mjs';
import {lstat} from 'node:fs/promises';
import {journal} from './mcp-curriculum-bridge.mjs';
const root=new URL('../',import.meta.url),origin='http://127.0.0.1:8796',company='mcp-workflow-check';
const {values}=parseArgs({options:{'attach-inputs':{type:'boolean'},'approved-workflow':{type:'boolean'},'preflight-planning':{type:'boolean'},...Object.fromEntries(['case','attempt-id','planning-session','planning-origin-report','existing-store','resume-report','runtime-config','resume-reason'].map(name=>[name,{type:'string'}]))}});
const option=name=>values[name.slice(2)]??null;
const resumeSession=option('--planning-session'),existingStore=option('--existing-store'),resumeFolder=option('--resume-report');
if((resumeSession||resumeFolder)&&!existingStore||resumeSession&&resumeFolder||existingStore&&!resumeSession&&!resumeFolder)throw Error('Recovery requires one planning session or prior report directory, plus existing store');
const priorFolder=resumeFolder?new URL(resumeFolder.replace(/\/?$/,'/'),root):null;
const prior=priorFolder?JSON.parse(await readFile(new URL('report.json',priorFolder),'utf8')):null;
let planningPrior=null;
if(values['planning-origin-report']){
 assert(resumeSession&&existingStore&&!prior,'Planning origin requires the same session and existing store, without an execution resume');
 assert(/^data\/mcp-workflow-check\/[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(values['planning-origin-report']),'Unsafe planning origin folder');
 const folder=new URL(values['planning-origin-report']+'/',root),url=new URL('report.json',folder),info=await lstat(url);
 assert(info.isFile()&&!info.isSymbolicLink()&&info.nlink===1&&info.size<=8*1024*1024,'Unsafe planning origin report');
 for(let current=folder;current.href!==root.href;current=new URL('../',current))assert(!(await lstat(current)).isSymbolicLink(),'Symlink planning origin');
 planningPrior=JSON.parse(await readFile(url,'utf8'));
 assert(/^data\/mcp-workflow-check\/[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}\/runs\.sqlite3$/.test(existingStore),'Unsafe planning store path');
 const storeUrl=new URL(existingStore,root),storeInfo=await lstat(storeUrl);
 assert(storeInfo.isFile()&&!storeInfo.isSymbolicLink()&&storeInfo.nlink===1&&storeInfo.size<=2*1024*1024*1024,'Unsafe planning store');
 for(let current=new URL('.',storeUrl);current.href!==root.href;current=new URL('../',current))assert(!(await lstat(current)).isSymbolicLink(),'Symlink planning store');
 assert.equal(planningPrior.planning_session,resumeSession,'Planning session changed');
 assert(typeof values['resume-reason']==='string'&&values['resume-reason'].trim().length>0&&values['resume-reason'].length<=1000,'Planning continuation requires a bounded reason');
}
const selectedFixture=fixtureFromId(option('--case')??prior?.fixture_id??planningPrior?.fixture_id??'reconciliation');
const preparedFields=preparedPlanningFields(selectedFixture); // Before any listener or inference.
let fixture=await materializeOwnedFixture(root,selectedFixture);
if(values['attach-inputs']&&fixture.ownedComponents)throw Error('Owned library reuse uses reviewed host custody, not a planning text upload');
if(values['attach-inputs']&&fixture.files.some(file=>typeof file.text!=='string'))throw Error('Binary owner upload admission is not implemented; reviewed binary fixtures use explicit host staging');
const authorizationMode=values['approved-workflow']?'approved_workflow':prior?.owner_authorization_mode??planningPrior?.owner_authorization_mode??'model_review';
if(prior)assert.equal(authorizationMode,prior.owner_authorization_mode??'model_review','Issued runs cannot change authorization mode through this runner');
const attemptId=option('--attempt-id')??randomUUID();if(!/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(attemptId))throw Error('Invalid attempt UUID');
const output=new URL(`../data/mcp-workflow-check/${attemptId}/`,import.meta.url);
const selectedProfile=await evaluationProfile(root,option('--runtime-config')??prior?.runtime_profile?.path??planningPrior?.runtime_profile?.path);
assert(!values['preflight-planning']||planningPrior,'Planning preflight requires its original report, session and store');
if(planningPrior){
 const {planning:checkpoint}=await journal(root,{op:'planning_authority',company,session:resumeSession});
 validatePlanningContinuation(planningPrior,fixture,selectedProfile,checkpoint);
 assert.equal(authorizationMode,planningPrior.owner_authorization_mode??'model_review','Planning continuation cannot change owner mode');
}
if(values['preflight-planning']){console.log(JSON.stringify({status:'PLANNING_CONTINUATION_PREFLIGHT_PASS',planning_session:resumeSession,scope:'Read-only canonical/profile/fixture checks; no lease, listener, model or task work'}));process.exit(0);}
await mkdir(output,{recursive:true,mode:0o700});
if(await fetch(origin+'/health',{signal:AbortSignal.timeout(1000)}).catch(()=>null))throw Error('Port 8796 occupied; no existing listener was stopped');
const {profile:runtimeProfile,changed:profileChanged}=await freezeEvaluationProfile(root,output,selectedProfile,prior,option('--resume-reason'));
const runtimeBuild=await runtimeFingerprint(root);
const runtimeBundle=await createRuntimeBundle(root);
const runtimeProbe=probeBundle(root,runtimeBundle);
const evaluationHostObservation=evaluationHost();
const store=existingStore?new URL(existingStore,root):new URL('runs.sqlite3',output),child=spawn(bundleExecutable(root,runtimeBundle,'gents-harness'),[],{cwd:root,stdio:['ignore','ignore','inherit'],env:{...process.env,...bundleEnvironment(runtimeBundle),GENTS_SQLITE_BRIDGE:bundleExecutable(root,runtimeBundle,'gents-sqlite-bridge'),AUTH_MODE:'local',BIND:'127.0.0.1:8796',CONFIG:runtimeProfile.frozen_path,RUN_STORE:store.pathname,LOCAL_MODEL_PORT:'8797',GENTS_DISABLE_SCHEDULER:'1',GENTS_LOCAL_ONLY:'1',GENTS_MCP_TRANSPORT:'sse',GENTS_OFFICE_ACTIVITY:'0',HF_HUB_OFFLINE:'1',TRANSFORMERS_OFFLINE:'1'}});
const report={fixture_id:fixture.id,fixture_sha:fixtureDigest(fixture),status:'PLANNING',title:fixture.title,started_at_ms:Date.now(),synthetic_users:true,attachment_ingress:values['attach-inputs']===true,company,origin,planning_session:prior?.planning_session??resumeSession??randomUUID(),run:prior?.run??null,same_run_resume:Boolean(prior),prior_report:resumeFolder??null,operator_reviews:[],provider_cost_usd:0};
report.owner_authorization_mode=authorizationMode;report.owner_prepared_ingress=Boolean(fixture.ownerPreparation);
report.same_planning_resume=Boolean(planningPrior);report.planning_origin_report=values['planning-origin-report']??null;
report.planning_origin_started_at_ms=planningPrior?.planning_origin_started_at_ms??planningPrior?.started_at_ms??null;
report.runtime_profile=runtimeProfile;report.planning_runtime_profile=prior?.planning_runtime_profile??prior?.runtime_profile??planningPrior?.planning_runtime_profile??planningPrior?.runtime_profile??runtimeProfile;report.runtime_profile_changed=profileChanged;report.prior_runtime_profile=prior?.runtime_profile??null;report.resume_reason=option('--resume-reason');
report.runtime_build=runtimeBuild;report.prior_runtime_build=prior?.runtime_build??null;
report.evaluation_host=evaluationHostObservation;report.prior_evaluation_host=prior?.evaluation_host??null;
report.runtime_bundle=runtimeBundle;report.launch_executables_source='runtime_bundle';report.prior_runtime_bundle=prior?.runtime_bundle??null;
report.runtime_bundle_probe=runtimeProbe;
report.evaluation_scope={industry:fixture.industry??'general',complexity:fixture.complexity??'unclassified',complexity_basis:fixture.complexity_basis??'No universal task difficulty inferred',modalities:fixture.modalities??['text'],modality_basis:fixture.modality_basis??'Local text-agent workflow',synthetic:true};
let plan,saved,keepListener=false;
const save=(name,value)=>writeFile(new URL(name,output),typeof value==='string'?value:JSON.stringify(value,null,2),{mode:0o600});
async function request(path,body){const r=await fetch(origin+path,{method:body?'POST':'GET',headers:{'Content-Type':'application/json'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(180000)});if(!r.ok){const error=Error(`${r.status}: ${await r.text()}`);error.status=r.status;throw error;}return r.json();}
const op=(op,data={})=>request(`/api/companies/${company}`,{op,...data});
const observePlanning=(path,body)=>planningTurn({request,path,body,session:report.planning_session,onObservation:async observation=>{report.planning_observations??=[];report.planning_observations.push({...observation,at_ms:Date.now()});await save('report.json',report);console.log(JSON.stringify(observation));}});

try{
 for(let attempt=0;;attempt++){try{await request('/api/config');break;}catch(error){if(child.exitCode!==null||attempt>=100)throw error;await new Promise(r=>setTimeout(r,100));}}
 const config=await request('/api/config'),catalog=await request('/api/mcp/catalog');
 assert.equal(config.local_only,true);assert.equal(catalog.harness_transport,'local_sse_mcp');assert(config.agents.every(a=>a.worker_provider==='local'&&a.planner_provider==='local'));
 report.runtime_bundle_binding=verifyBundleBinding(catalog,runtimeBundle);
 await op('register',{name:fixture.title});await save('report.json',report);console.log(JSON.stringify({status:'STARTED',pid:child.pid,report:new URL('report.json',output).pathname}));
 const planningPath=`/api/companies/${company}/planning/${report.planning_session}`;
 plan=prior?JSON.parse(await readFile(new URL('planning.json',priorFolder),'utf8')):resumeSession?await request(planningPath):await observePlanning(planningPath,{message:fixture.initial,expected_revision:0,source_inputs:fixture.ownerPreparation?[]:planningInputs(fixture),...preparedFields,...(values['attach-inputs']?{source_files:fixture.files.map(f=>({id:f.input,path:f.path,text:f.text}))}:{}),...(fixture.planningConstraints?{planning_constraints:fixture.planningConstraints}:{})});await save('planning.json',plan);
 let reviewed;
 for(let turn=0;turn<=6;turn++){
  if(!prior&&plan.status==='ready'&&!plan.messages.some(message=>message.role==='user'&&message.kind!=='planning_repair'&&message.message===fixture.requirements)){
   report.operator_reviews.push({revision:plan.revision,status:'full_requirements_not_yet_supplied'});await save('report.json',report);
   plan=await observePlanning(planningPath,{message_kind:'requirements',message:fixture.requirements,expected_revision:plan.revision,source_inputs:planningInputs(fixture),...(fixture.planningConstraints?{planning_constraints:fixture.planningConstraints}:{})});await save('planning.json',plan);continue;
  }
  let rejection;
  if(needsSelection(fixture)&&plan.pending_owned_selection&&plan.messages.some(m=>m.role==='user'&&m.kind!=='planning_repair'&&m.message===fixture.requirements)){
   let invocations;
   try{invocations=reviewSelectionDraft(fixture,plan);report.operator_reviews.push({revision:plan.revision,status:'selection_preparation_accepted',proposal_sha256:plan.pending_owned_selection.proposal_sha256});}
   catch(error){rejection=error.message.split('\n')[0];report.operator_reviews.push({revision:plan.revision,status:'selection_preparation_rejected',reason:rejection});await save('report.json',report);}
   if(invocations){
    plan=await observePlanning(planningPath,{message_kind:'requirements',message:fixture.selection.adoption_message,expected_revision:plan.revision,owned_invocations:invocations,source_inputs:[],...(fixture.planningConstraints?{planning_constraints:fixture.planningConstraints}:{})});await save('planning.json',plan);
    fixture=selectedExecutionFixture(fixture,plan);report.ceo_selected_owned_calls=true;report.owner_supplied_invocation_ids=false;await save('report.json',report);continue;
   }
  }
  if(plan.status==='ready'){
   try{reviewed=reviewWorkflowPlan(plan,fixture);report.operator_reviews.push({revision:plan.revision,status:'accepted'});break;}
   catch(error){rejection=fixture.outcomeOnly?error.message.split('\n')[0]:String(error);report.operator_reviews.push({revision:plan.revision,status:'rejected',reason:rejection});await save('report.json',report);}
  }else if(!['question','selection_review','failed'].includes(plan.status))throw Error('Unexpected planning state; original session retained');
  if(plan.status==='failed'&&plan.inference_failure?.retryable_without_host_change===false)throw Error('Planning runtime failure requires host diagnosis: '+plan.inference_failure.code);
  if(turn===6)break;
  if(prior)throw Error('Saved run contract no longer matches the fixture; no replanning or reapproval is permitted during execution resume');
  const repair=Boolean(rejection)||plan.status==='failed';
  plan=await observePlanning(planningPath,{message_kind:repair?'planning_repair':'requirements',message:repair?`The draft was rejected in review: ${rejection??plan.validation_error??plan.error}. Correct that contract error while retaining the original goal, files, source inputs and role separation. Follow the requested encodings. Do not calculate answers.`:fixture.requirements,expected_revision:plan.revision,source_inputs:planningInputs(fixture),...(fixture.planningConstraints?{planning_constraints:fixture.planningConstraints}:{})});await save('planning.json',plan);
 }
 assert(reviewed,'CEO did not produce an approvable reviewed contract');
 const {draft:plannedDraft,stages,analyst,writer,requiredTools}=reviewed;
 const draft=ownerAuthorization(plannedDraft,authorizationMode);
 report.run=prior?.run??randomUUID();report.actors=stages.map(s=>s.actor);report.native_contracts_proposed_by_local_ceo=true;report.handwritten_artifact_bindings=false;report.host_materialized_fixed_roots=plan.host_compiled_roots??[];report.host_compiled_capabilities=plan.host_compiled_capabilities??[];report.outcome_only_planning=fixture.outcomeOnly===true;
 let review;
 if(prior){
  review=JSON.parse(await readFile(new URL('execution-review.json',priorFolder),'utf8'));assert.equal(review.run,report.run);
  // The server adds host controls to the original draft. Compare all original
  // owner fields, then verify the complete frozen copy against current authority.
  for(const [key,value] of Object.entries(draft.intake))assert(canonical(review.intake[key])===canonical(value),`Frozen owner field changed: ${key}`);
  const current=(await op('status')).jobs.find(job=>job.run_id===report.run);assert(current,'Approved job missing');assert.equal(current.digest,review.digest);assert(canonical(current.intake)===canonical(review.intake),'Frozen server intake changed');assert.notEqual(current.state,'draft','Resume cannot approve a draft');
 }
 else{const state=await op('intake',{run_id:report.run,intake:draft.intake});review=confirmFrozenCeoJob(draft,state.jobs.find(j=>j.run_id===report.run));await op('approve',{run_id:report.run,digest:review.digest});await op('governance_install',{run_id:report.run,expected_revision:0,policy:review.policy});}
 await save('execution-review.json',review);
 const directory=new URL(`../data/companies/${company}/runs/${report.run}/`,import.meta.url),workspace=new URL('workspace/',directory);await mkdir(workspace,{recursive:true,mode:0o700});
 for(const file of fixture.files){const bytes=fixtureBytes(file);if(prior)assert.deepEqual(await readFile(new URL(file.path,workspace)),bytes,'Original operator source changed during maintenance');else if(!values['attach-inputs']&&!fixture.ownerPreparation)await writeFile(new URL(file.path,workspace),bytes,{mode:0o600,flag:'wx'});await writeFile(new URL('operator-'+file.path,output),bytes,{mode:0o600});}
 if((values['attach-inputs']||fixture.ownerPreparation)&&!prior)assert.equal((await readdir(workspace)).length,0,'Attachment run workspace must be empty before approved admission');
 report.operator_inputs=fixture.files.map(f=>({path:f.path,sha256:f.sha256,bytes:fixtureBytes(f).length,source:'Explicit synthetic operator fixture, not an agent deliverable'}));report.status='EXECUTING';await save('report.json',report);
 if(prior){saved=await request(`/api/runs/${report.run}`);report.before_resume_status=saved.status;if(['cancelled','incomplete','failed'].includes(saved.status))await request(`/api/runs/${report.run}/resume`,{});else assert(['running','completed'].includes(saved.status));}
 else await request('/api/runs',{company_id:company,run_id:report.run,task:draft.task,consent:false,definition_of_done:draft.criteria});
 if(values['attach-inputs']||fixture.ownerPreparation)for(const file of fixture.files)assert.deepEqual(await readFile(new URL(file.path,workspace)),fixtureBytes(file),'Approved admission must stage exact owner bytes');
 let last='',lastSnapshot=0;for(;;){
  try{saved=await request(`/api/runs/${report.run}`);}catch(error){if(child.exitCode!==null||error.status===404)throw error;report.last_observation_error=String(error);await save('report.json',report);await new Promise(r=>setTimeout(r,1000));continue;}
  const state=`${saved.status}/${saved.events.length}`;if(state!==last){console.log(JSON.stringify({run:report.run,status:saved.status,events:saved.events.length}));last=state;}
  if(Date.now()-lastSnapshot>=60000){try{await saveObservation({folder:output,fixture,report,plan,saved});lastSnapshot=Date.now();}catch(error){console.error(JSON.stringify({observation_save_error:String(error),run:report.run}));}}
  if(saved.status!=='running')break;await new Promise(r=>setTimeout(r,2000));
 }
 await save('events.json',saved);assert.equal(saved.status,'completed');
 Object.assign(report,await verifyWorkflow({root,fixture,plan,saved,review,company,run:report.run,runtimeProfile:report.planning_runtime_profile}));
 if(values['attach-inputs'])assert.equal(report.owner_attachment_ingress_verified,true);
}catch(error){
 report.error=String(error);
 if(child.exitCode===null&&report.run){
  // A failed HTTP observation does not establish that an issued run stopped.
  try{saved=await request(`/api/runs/${report.run}`);keepListener=saved.status==='running';}catch(observation){keepListener=observation.status!==404;report.cleanup_observation_error=String(observation);}
 }else if(child.exitCode===null){
  try{const current=await request(`/api/companies/${company}/planning/${report.planning_session}`);plan=current;keepListener=current.status==='generating';}catch{}
 }
 if(error instanceof PlanningObservationUncertain&&child.exitCode===null)keepListener=true;
 report.status=keepListener?'OBSERVATION_ERROR_LIVE_WORK_RETAINED':'FAIL';process.exitCode=1;
}
finally{
 report.elapsed_ms=Date.now()-report.started_at_ms;if(planningPrior)report.planning_continuation_total_span_ms=Date.now()-report.planning_origin_started_at_ms;await save('report.json',report);if(plan)await save('planning.json',plan);if(saved)await save('events.json',saved);
 const html=renderWorkflowTranscript({fixture,report,plan,saved});
 await save('transcript.html',html);console.log(JSON.stringify(report));if(!keepListener){child.kill('SIGTERM');if(child.exitCode===null)await new Promise(resolve=>child.once('exit',resolve));}else console.log(JSON.stringify({status:'LIVE_LISTENER_RETAINED',pid:child.pid,run:report.run,reason:'No timeout cancellation or task replay'}));
}

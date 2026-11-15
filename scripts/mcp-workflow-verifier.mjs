// Model-free verifier shared by the workflow observer and durable curriculum.
import {readFile,readdir,lstat} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {journal} from './mcp-curriculum-bridge.mjs';
import {prepareCeoExecution} from '../web/assets/ceo-execution.js';
import {verifyCanonicalFinalAdmin,modelCallAccounting,nativeReplyOutcomes,completedLocalUsage} from './mcp-team-verification.mjs';
import {canonical,project} from './fixtures/mcp-workflow-catalog.mjs';
import {reviewWorkflowPlan,sourceArguments} from './mcp-workflow-review.mjs';
import {verifyPreparedIngress} from './mcp-owned-prepared.mjs';
import {ownerAuthorization} from './mcp-owner-authorization.mjs';
import {fixtureBytes} from './mcp-fixture-files.mjs';
import {frozenPlanningPolicy,verifyPlanningPolicy} from './mcp-planning-policy-verifier.mjs';
import {materializeOwnedFixture,verifyOwnedResult} from './mcp-owned-fixture.mjs';
import {hasOwnedSlots,expectedArguments} from './mcp-owned-edges.mjs';
import {needsSelection,selectedExecutionFixture,verifySelectedIngress} from './mcp-owned-selection.mjs';
const policyView=policy=>({...policy,...Object.fromEntries(['grants','ceilings'].map(key=>[key,Object.fromEntries(Object.entries(policy[key]).map(([actor,grant])=>[actor,Object.fromEntries(Object.entries(grant).map(([name,values])=>[name,[...values].sort()]))]))]))});
export async function verifyWorkflow({root,fixture,plan,saved,review,company,run,runtimeProfile}){
 if(fixture.kind==='rust_library')return (await import('./mcp-coding-verifier.mjs')).verifyCodingWorkflow({root,fixture,plan,saved,review,company,run,runtimeProfile});
 if(needsSelection(fixture))fixture=selectedExecutionFixture(fixture,plan);
 fixture=await materializeOwnedFixture(root,fixture);
 assert.equal(saved.status,'completed');assert.equal(saved.run_id,run);assert.equal(saved.company_id,company);assert.equal(review.run,run);
 const {draft:plannedDraft,stages,analyst,writer,requiredTools}=reviewWorkflowPlan(plan,fixture);
 const planningPolicy=await frozenPlanningPolicy(root,runtimeProfile);
 const authorizationMode=review.owner_authorization_mode??'model_review',draft=ownerAuthorization(plannedDraft,authorizationMode);
 const ownerMessages=plan.messages.filter(message=>message.role==='user'&&message.kind!=='planning_repair').map(message=>message.message);assert(ownerMessages.includes(fixture.initial));assert(ownerMessages.includes(fixture.requirements),'Wrong fixture requirements');
 for(const [key,value] of Object.entries(draft.intake))assert.equal(canonical(review.intake[key]),canonical(value),`Frozen field changed: ${key}`);
 assert.equal(canonical(policyView(review.policy)),canonical(policyView(draft.policy)));
 const authority=await journal(root,{op:'native_authority',company,run});assert.equal(authority.digest,review.digest);assert.equal(canonical(authority.intake),canonical(review.intake));assert.equal(canonical(policyView(authority.policy)),canonical(policyView(review.policy)));
 const preparedIngress=verifyPreparedIngress(fixture,plan,review,authority,analyst.actor);
 Object.assign(preparedIngress,verifySelectedIngress(fixture,plan,review,authority));
 const attached=review.intake.ceo_source_files;
 if(attached?.length){
  const expected=fixture.files.map(f=>({id:f.input,path:f.path,sha256:f.sha256,bytes:fixtureBytes(f).length})).sort((a,b)=>a.id.localeCompare(b.id));
  assert.equal(canonical(attached),canonical(expected),'Frozen attachments differ from owner bytes');
  assert.equal(canonical(plan.source_files),canonical(expected),'Planning attachment metadata differs');
  assert.equal(authority.source_staging.length,1,'Missing or replayed attachment staging');
  assert.equal(authority.source_staging[0].planning_id,plan.id);
  assert.equal(canonical(authority.source_staging[0].files),canonical(expected));
 }
 const directory=new URL(`data/companies/${company}/runs/${run}/`,root),workspace=new URL('workspace/',directory);
 const journals=await Promise.all((await readdir(directory)).filter(n=>/^tool-\d+\.json$/.test(n)).map(async n=>JSON.parse(await readFile(new URL(n,directory),'utf8'))));assert(journals.every(j=>j.result.result.ok===true),'Challenge contains a failed or uncertain tool attempt');const calls=journals;
 for(const file of fixture.files){const bytes=await readFile(new URL(file.path,workspace));assert.deepEqual(bytes,fixtureBytes(file));assert.equal(createHash('sha256').update(bytes).digest('hex'),file.sha256);assert(!calls.some(c=>c.request.action==='files'&&c.request.arguments.path===file.path));}
 for(const input of fixture.sourceInputs){
  const args=expectedArguments(fixture,input);
  const actual=calls.filter(c=>c.request.action===input.tool&&canonical(sourceArguments(input.tool,c.request.arguments))===canonical(sourceArguments(input.tool,args)));assert.equal(actual.length,1,`Missing/replayed exact native source ${input.id}`);assert.equal(actual[0].result.result.mcp_receipt?.actor,analyst.actor);
  if(input.tool==='owned_rust_call'&&fixture.ownedComponents)verifyOwnedResult(fixture,input,actual[0].result.result.result,authority,analyst.actor,{request:args,bindings:review.intake.artifact_bindings,calls,run});
  else assert.equal(canonical(actual[0].result.result.result),canonical(fixture.expectedResults.get(input.id)));
 }
 assert.equal(calls.filter(c=>requiredTools.has(c.request.action)).length,fixture.sourceInputs.length);
 if(hasOwnedSlots(fixture)){assert.equal(calls.length,fixture.sourceInputs.length+2*fixture.artifacts.length,'Unexpected extra native operation');for(const b of review.intake.artifact_bindings.filter(b=>b.receipt_only)){assert(!calls.some(c=>c.request.action==='files'&&c.request.arguments.path===b.path),'An intermediate receipt became a file operation');await assert.rejects(lstat(new URL(b.path,workspace)),error=>error.code==='ENOENT','An intermediate receipt became a filesystem entry');}}
 const verifiedArtifacts={};for(const spec of fixture.artifacts){
  const value=project(fixture.expectedResults.get(spec.input),spec.pointer),expected=spec.encoding==='json'?canonical(value):String(value),actual=await readFile(new URL(spec.path,workspace),'utf8');assert.equal(actual,expected);verifiedArtifacts[spec.path]=actual;
  for(const operation of ['write','read']){const rows=calls.filter(c=>c.request.action==='files'&&c.request.arguments.op===operation&&c.request.arguments.path===spec.path);assert.equal(rows.length,1);assert.equal(rows[0].result.result.mcp_receipt?.actor,writer.actor);if(operation==='read')assert.equal(rows[0].result.result.result.text,expected);}
 }
 verifyCanonicalFinalAdmin(saved.events,analyst.actor,writer.actor,review.intake.artifact_bindings.length,fixture.sourceInputs.at(-1).tool,authorizationMode);
 const accepted=saved.events.find(e=>e.kind==='ceo_stage_accepted'),firstWrite=saved.events.find(e=>e.agent===writer.actor&&e.kind==='tool_result');assert(accepted&&firstWrite&&accepted.seq<firstWrite.seq);
 assert.equal(authority.receipts.length,calls.length,'Unjournaled or extra native success');
 for(const call of calls){const receipt=call.result.result.mcp_receipt;assert(receipt);assert.equal(receipt.provider_cost_usd,0);if(authorizationMode==='approved_workflow')assert.equal(receipt.contract_enforced,true,'Native exact-contract guard must attest every approved-workflow call');assert.equal(receipt.policy_revision,authority.policy_revision);assert.equal(receipt.arguments_sha256,createHash('sha256').update(canonical(call.request.arguments)).digest('hex'));assert.equal(receipt.result_sha256,createHash('sha256').update(canonical(call.result.result.result)).digest('hex'));assert(authority.receipts.some(row=>['actor','phase','tool','status','policy_revision','arguments_sha256','result_sha256'].every(key=>row[key]===receipt[key])&&(authorizationMode!=='approved_workflow'||row.contract_enforced===true)),'Canonical native audit receipt missing');}
 const replies=saved.events.filter(e=>e.reply).map(e=>e.reply),attempts=authority.planning_attempts;assert(attempts.length>0&&Array.isArray(plan.attempts),'Missing planning attempt audit');const attemptView=attempt=>Object.fromEntries(['revision','status','usage','local_execution','rejected_output_sha256','planning_protocol','host_compiled_adopted_dependencies',...(planningPolicy?['model','validation_error','reply_token_limit','planning_model_selection']:[])].map(key=>[key,attempt[key]??null]));assert.equal(canonical(plan.attempts.map(attemptView)),canonical(attempts.map(attemptView)),'Saved planning attempts do not match canonical company audit');assert(replies.every(r=>r.local_execution&&r.usage.cost_usd===0));assert(attempts.every(a=>a.local_execution&&a.usage?.cost_usd===0));
 const executions=[...replies.map(r=>r.local_execution),...attempts.map(a=>a.local_execution)];assert(executions.every(e=>Number.isSafeInteger(e.backend_pid)&&e.backend_pid>0&&Number.isSafeInteger(e.port)&&e.port>0&&e.port<65536&&/^[a-f0-9]{64}$/.test(e.task_input_sha256)&&e.dispatched_at_ms<=e.received_at_ms),'Incomplete native local execution provenance');
 const callAccounting=modelCallAccounting(saved.events);assert.equal(callAccounting.unresolved_run_calls,0,'A completed fixture cannot have unresolved model-call handles');
 const planningUnreceived=(authority.planning_started_revisions??[]).filter(revision=>!attempts.some(a=>a.revision===revision));
 const usage=[...replies.map(r=>r.usage),...attempts.map(a=>a.usage)],usageTotals=completedLocalUsage(usage);
 return {planning_protocols:[...new Set(attempts.map(a=>a.planning_protocol?.id??'legacy_graph'))],...preparedIngress,...(fixture.ownedComponents?{owned_component_reuse:{package_ids:[...new Set(fixture.ownedComponents.map(c=>c.owner_declared_review_package_id))],source_sha256:[...new Set(fixture.ownedComponents.map(c=>c.source_sha256))],native_invocations:fixture.sourceInputs.filter(i=>i.tool==='owned_rust_call').length,owned_library_write_calls:calls.filter(c=>c.request.action==='files'&&c.request.arguments.op==='write'&&fixture.files.some(f=>f.path===c.request.arguments.path)).length,qualification_scope:'Fresh workflow using a historical locally authored, fixture-qualified source package. No new source authoring, model training, global promotion or general capability claim.'}}:{}),...verifyPlanningPolicy(attempts,planningPolicy,{boundaries:authority.planning_phase_boundaries??[],accepted:authority.intake.ceo_owned_selection}),planning_calls_without_completed_receipt:planningUnreceived.length,planning_unreceived_revisions:planningUnreceived,planning_usage_complete:planningUnreceived.length===0,owner_authorization_mode:authorizationMode,deterministic_authorizations:saved.events.filter(e=>e.kind==='authorization_decision').length,deterministic_nonallows:saved.events.filter(e=>e.kind==='authorization_decision'&&JSON.parse(e.message).verdict.decision!=='allow').length,owner_attachment_ingress_verified:Boolean(attached?.length),provider_cost_usd:usage.reduce((sum,u)=>sum+u.cost_usd,0),artifacts:verifiedArtifacts,status:'PASS',native_source_calls:fixture.sourceInputs.length,native_dependency_edges:Object.values(fixture.dependencies).reduce((n,refs)=>n+refs.length,0),artifact_count:fixture.artifacts.length,source_unchanged:true,replayed_sources_or_writes:false,ordered_handoff:true,final_admin_evidence_complete:true,post_planning_questions:saved.events.filter(e=>e.kind==='attention_required').length,admin_nonallows:saved.events.filter(e=>e.kind==='admin_decision'&&JSON.parse(e.message).verdict.decision!=='allow').length,planning_attempts:attempts.length,rejected_planning_attempts:attempts.filter(a=>a.status==='failed').length,...nativeReplyOutcomes([...replies,...attempts]),...usageTotals,...callAccounting,resume_events:saved.events.filter(event=>event.kind==='resumed').length,cancelled_intervals:saved.events.filter(event=>event.kind==='cancelled').length,unfinished_run_calls:callAccounting.unresolved_run_calls,worker_model_calls:callAccounting.worker_model_calls_started,usage_scope:'All retained run/planning reply records including rejected drafts and cutoff receipts; outcome counters distinguish completed generations. Failed calls without a reply have unknown local token usage; terminal failures are not unresolved calls. Local inference consumes tokens; provider charges are zero, electricity excluded.'};
}

import {test} from 'node:test';
import assert from 'node:assert/strict';
import {prepareCeoExecution,confirmFrozenCeoJob} from '../web/assets/ceo-execution.js';
const session=()=>({id:'saved',revision:2,status:'ready',messages:[{role:'user',message:'Calculate and save the native result'},{role:'ceo',message:{question:'Which inputs?'}},{role:'user',message:'Use the agreed inputs'}],proposal:{definition_of_done:['Readback equals the native result'],assignments:[{actor:'analyst',task:'Calculate',tools:['integer_arithmetic'],acceptance:['Native calculation']},{actor:'writer',task:'Save',tools:['files:write','files:read'],acceptance:['Exact readback']}]}});
const prepare=s=>prepareCeoExecution(s,[],['analyst','writer']);
test('review grants preserve role separation and cannot admit unrelated or duplicate actors',()=>{
 const draft=prepare(session());
 assert.deepEqual(draft.policy.grants.analyst,{tools:['integer_arithmetic','finish'],skills:[],memory_from:[]});
 assert.deepEqual(draft.policy.grants.writer.memory_from,['analyst']);
 assert(!draft.policy.grants.writer.tools.includes('integer_arithmetic'));
 assert(!draft.policy.grants.analyst.tools.includes('files:write'));
 for(const actor of ['other','analyst']){const s=session();s.proposal.assignments[1].actor=actor;assert.throws(()=>prepare(s),/distinct configured/);}
 const s=session();s.status='question';assert.throws(()=>prepare(s),/ready/);
});
test('review freezes independent copies and only the saved user requirements become the task',()=>{
 const s=session(),draft=prepare(s);s.proposal.assignments[0].tools.push('files:write');
 assert(!draft.intake.ceo_plan.assignments[0].tools.includes('files:write'));
 assert(!draft.task.includes('Which inputs?'));
 draft.policy.grants.analyst.tools.push('files:write');assert(!draft.policy.ceilings.analyst.tools.includes('files:write'));
 assert.equal(draft.intake.budget_usd,0);
});
test('server reordering and added host controls are accepted; altered frozen authority is rejected',()=>{
 const draft=prepare(session());
 const intake=JSON.parse(JSON.stringify(draft.intake,(_,v)=>v&&typeof v==='object'&&!Array.isArray(v)?Object.fromEntries(Object.entries(v).reverse()):v));
 const job={run_id:'run',digest:'exact-server-digest',state:'draft',intake:{...intake,ceo_host_controls:{actions:['finish']}}};
 assert.equal(confirmFrozenCeoJob(draft,job).digest,job.digest);
 for(const key of ['ceo_planning_revision','acceptance','ceo_plan','artifact_bindings','budget_usd']){const altered=structuredClone(job);altered.intake[key]=null;assert.throws(()=>confirmFrozenCeoJob(draft,altered),/Frozen review changed/);}
 assert.throws(()=>confirmFrozenCeoJob(draft,{...job,state:'approved'}),/frozen draft/);
});
test('oversize requirements and malformed binding envelopes stop before draft submission',()=>{
 const s=session();s.messages=[{role:'user',message:'é'.repeat(5001)}];assert.throws(()=>prepare(s),/10,000/);
 assert.throws(()=>prepareCeoExecution(session(),{},['analyst','writer']),/bindings/);
});
test('saved CEO contracts default to reviewed native bindings; explicit owner edits stay distinct',()=>{
 const s=session();s.proposal.native_contracts={artifacts:[{path:'answer.txt'}]};s.proposed_artifact_bindings=[{path:'answer.txt',pointer:'/calculations/0/difference'}];
 const draft=prepareCeoExecution(s,null,['analyst','writer']);assert.deepEqual(draft.intake.artifact_bindings,s.proposed_artifact_bindings);
 s.proposed_artifact_bindings[0].path='changed.txt';assert.equal(draft.intake.artifact_bindings[0].path,'answer.txt');
 assert.deepEqual(prepareCeoExecution(s,[],['analyst','writer']).intake.artifact_bindings,[]);
 delete s.proposed_artifact_bindings;assert.throws(()=>prepareCeoExecution(s,null,['analyst','writer']),/checks unavailable/);
});
test('diagnostic planning repairs stay in the transcript but cannot become execution requirements',()=>{
 const s=session();s.messages.push({role:'user',kind:'planning_repair',message:'Historical validation error: source actor ambiguous'});
 const draft=prepare(s);assert(!draft.task.includes('Historical validation error'));assert.equal(s.messages.at(-1).kind,'planning_repair');
});
test('retained owner invocation metadata is copied into review and cannot change while freezing',()=>{
 const s=session();s.owned_invocations=[{invocation:{id:'call',component_id:'a'.repeat(64),function:'local_fn',arguments:['3']},source_sha256:'b'.repeat(64)}];
 const draft=prepare(s);s.owned_invocations[0].invocation.arguments=['4'];
 assert.deepEqual(draft.expected_owned_invocations[0].invocation.arguments,['3']);
 assert.equal(draft.intake.ceo_owned_invocations,undefined,'do not duplicate retained input data in the HTTP request');
 const job={run_id:'run',digest:'fixture',state:'draft',intake:{...structuredClone(draft.intake),ceo_owned_invocations:structuredClone(draft.expected_owned_invocations)}};
 assert.equal(confirmFrozenCeoJob(draft,job).digest,'fixture');
 job.intake.ceo_owned_invocations[0].source_sha256='c'.repeat(64);assert.throws(()=>confirmFrozenCeoJob(draft,job),/ceo_owned_invocations/);
});

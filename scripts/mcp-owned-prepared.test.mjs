import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {preparedPlanningFields,verifyPreparedIngress} from './mcp-owned-prepared.mjs';
import {frozenOwnedCalls} from './mcp-owned-fixture.mjs';
import {reviewWorkflowPlan} from './mcp-workflow-review.mjs';
const sha=v=>createHash('sha256').update(v).digest('hex');
function simulated(){
 const original=fixtureFromId('owned-prepared-reuse'),text='// Explicit simulated custody data, no actual component qualification.\n',source=sha(text);
 const fixture={...original,files:original.files.map(f=>({...f,text,sha256:source})),ownedComponents:original.ownedComponents.map(s=>({...s,source_sha256:source}))};
 const fields=preparedPlanningFields(fixture),component=fields.owned_component_ids[0],api=fixture.ownedComponents[0].api_sha256;
 const manifests=fields.owned_invocations.map(invocation=>({invocation,origin_company:'protocol-owner',source_path:fixture.files[0].path,source_sha256:source,api_sha256:api,input_sha256:sha(canonical(invocation.arguments)),bytes:Buffer.byteLength(text)})).sort((a,b)=>a.invocation.id.localeCompare(b.invocation.id));
 const proposal={status:'ready',question:'',planning_constraints:fixture.planningConstraints,definition_of_done:['Actual native results saved and read back'],assignments:[{actor:'Analysis',task:'Obtain both results',tools:['owned_rust_call'],acceptance:['Actual native results']},{actor:'Output',task:'Save results',tools:['files:read','files:write'],acceptance:['Exact readback']}],native_contracts:{sources:fixture.sourceInputs.map(i=>({id:i.id,input:i.id,actor:'Analysis'})),artifacts:fixture.artifacts.map(a=>({...a,source:a.input,writer:'Output'}))}};
 const bindings=fixture.artifacts.map(a=>({path:a.path,source_actor:'Analysis',writer:'Output',source_tool:'owned_rust_call',arguments:fixture.sourceInputs.find(i=>i.id===a.input).arguments,pointer:a.pointer,encoding:a.encoding}));
 const plan={id:'synthetic-session',revision:2,status:'ready',messages:[{role:'user',message:fixture.initial},{role:'user',message:fixture.requirements},{role:'ceo',message:proposal}],proposal,planning_constraints:fixture.planningConstraints,source_inputs:fixture.sourceInputs,owned_invocations:manifests,proposed_artifact_bindings:bindings,owned_component_catalog:{components:[{component_id:component,origin_company:'protocol-owner',source_sha256:source,api_sha256:api,functions:[{name:'select_slots'}]}]}};
 const review={intake:{ceo_owned_invocations:manifests,required_owned_rust_calls:frozenOwnedCalls(fixture,'Analysis').sort((a,b)=>a.id.localeCompare(b.id)),ceo_source_inputs:fixture.sourceInputs}};
 const authority={owned_input_custody:manifests.map(m=>({manifest:m,source_sha256:source,bytes:Buffer.byteLength(text)})),owned_source_staging:[{planning_id:plan.id,provider_cost_usd:0,files:[{path:fixture.files[0].path,sha256:source,bytes:Buffer.byteLength(text)}]}]};
 return {fixture,plan,review,authority};
}
test('new ingress has a distinct frozen fixture; earlier reuse digest and finite data stay unchanged',()=>{
 const base=fixtureFromId('owned-weighted-reuse'),f=fixtureFromId('owned-prepared-reuse'),fields=preparedPlanningFields(f);
 assert.equal(fixtureDigest(base),'c10ad4d1fc32964902d11e3281d7179ef6fa45a523bd0395d9742275e6127613');assert.notEqual(fixtureDigest(base),fixtureDigest(f));
 assert.deepEqual([...f.expectedResults.values()].sort((a,b)=>canonical(a).localeCompare(canonical(b))),[...base.expectedResults.values()].sort((a,b)=>canonical(a).localeCompare(canonical(b))));
 assert(fields.owned_invocations.every(r=>!('actor' in r)&&!('source' in r)&&!('source_path' in r)));
 assert.deepEqual(preparedPlanningFields(base),{});
 const {fixture,plan}=simulated(),{draft}=reviewWorkflowPlan(plan,fixture);
 assert.equal(draft.intake.required_owned_rust_calls,undefined,'review must not inject actor-bound requirements for managed preparation');
 assert.deepEqual(draft.expected_owned_invocations,plan.owned_invocations);
});
test('independent verification requires retained bytes, approved host staging and exact actual source actor',()=>{
 const good=simulated();assert.equal(verifyPreparedIngress(good.fixture,good.plan,good.review,good.authority,'Analysis').owner_prepared_ingress_verified,true);
 for(const mutate of [v=>v.authority.owned_source_staging.push(v.authority.owned_source_staging[0]),v=>v.authority.owned_input_custody[0].source_sha256='0'.repeat(64),v=>v.review.intake.required_owned_rust_calls[0].actor='Output',v=>v.plan.owned_invocations[0].invocation.arguments=[],v=>v.authority.owned_source_staging[0].files[0].bytes++]){
  const altered=structuredClone(simulated());mutate(altered);assert.throws(()=>verifyPreparedIngress(altered.fixture,altered.plan,altered.review,altered.authority,'Analysis'));
 }
});
test('mixed challenge expands to eight frozen roots and two libraries with exact wide-integer edge cases',()=>{
 const f=fixtureFromId('owned-mixed-batch'),fields=preparedPlanningFields(f);
 assert.equal(fields.owned_invocations.length,8);assert.equal(fields.owned_component_ids.length,2);assert.equal(f.files.length,2);assert.equal(f.artifacts.length,8);
 assert.deepEqual(f.sourceInputs.map(i=>i.id),[...f.sourceInputs.map(i=>i.id)].sort());
 assert.deepEqual(f.expectedResults.get('schedule-wide').output,{some:['0','1','3','5','6']});
 assert.deepEqual(f.expectedResults.get('schedule-empty').output,{some:[]});assert.deepEqual(f.expectedResults.get('schedule-invalid').output,{none:true});
 for(const input of f.sourceInputs.filter(i=>i.id.startsWith('settlement-'))){
  const balances=input.arguments.arguments[0].map(BigInt),result=f.expectedResults.get(input.id).output,total=balances.reduce((a,b)=>a+b,0n);
  if(total!==0n){assert.deepEqual(result,{none:true});continue;}
  const outgoing=balances.map(()=>0n),incoming=balances.map(()=>0n);
  for(const [from,to,amount] of result.some){const n=BigInt(amount);assert(n>0n);assert(balances[Number(from)]<0n&&balances[Number(to)]>0n);outgoing[Number(from)]+=n;incoming[Number(to)]+=n;}
  for(let i=0;i<balances.length;i++)assert.equal(incoming[i]-outgoing[i],balances[i]);
  // These valid frozen cases have one sender or one recipient: every nonzero
  // counterparty forces an edge, making this lower bound exact and the plan unique.
  const senders=balances.filter(n=>n<0n).length,recipients=balances.filter(n=>n>0n).length;
  assert(senders<=1||recipients<=1);assert.equal(result.some.length,Math.max(senders,recipients));
 }
 assert(!f.requirements.includes(JSON.stringify(f.expectedResults.get('schedule-wide').output.some)));
 assert(Buffer.byteLength(canonical(f.sourceInputs))<=8192);assert(Buffer.byteLength(canonical(fields.owned_invocations))<=12288);
});
test('fresh mapped variation preserves baseline identity, uses independent exact oracles and exposes no task answers',()=>{
 const baseline=fixtureFromId('owned-mixed-batch'),f=fixtureFromId('owned-mapped-batch'),fields=preparedPlanningFields(f);
 assert.equal(fixtureDigest(baseline),'078141455eacdb04d46e2c167ddb753e355a48cea504943a37b9621d31dc20e1');
 assert.notEqual(fixtureDigest(f),fixtureDigest(baseline));assert.equal(f.artifacts.length,8);assert.equal(fields.owned_component_ids.length,2);
 for(const a of f.artifacts){assert.notEqual(a.path,a.input+'.json');assert(f.requirements.includes(a.input)&&f.requirements.includes(a.path));}
 assert.equal(f.sourceInputs.find(i=>i.id==='case-a').arguments.arguments[0].length,16);
 assert.deepEqual(f.expectedResults.get('case-c').output,{some:['0','1','3','4']});
 assert.deepEqual(f.expectedResults.get('case-g').output,{some:['0','1','3','4']});
 assert.deepEqual(f.expectedResults.get('case-e').output,{none:true});
 for(const component of f.ownedComponents.filter(c=>c.function==='settle_balances')){
  const input=f.sourceInputs.find(i=>i.id===component.input),balances=input.arguments.arguments[0].map(BigInt),result=f.expectedResults.get(input.id).output;
  if(balances.reduce((a,b)=>a+b,0n)!==0n){assert.deepEqual(result,{none:true});continue;}
  const deltas=balances.map(()=>0n);let previous;
  for(const edge of result.some){const [from,to]=edge.map(Number),n=BigInt(edge[2]);assert(n>0n&&balances[from]<0n&&balances[to]>0n);deltas[from]-=n;deltas[to]+=n;if(previous)assert(previous[0]<from||previous[0]===from&&previous[1]<to);previous=[from,to];}
  assert.deepEqual(deltas,balances);
  const senders=balances.filter(n=>n<0n).length,recipients=balances.filter(n=>n>0n).length;
  assert(senders===1||recipients===1);assert.equal(result.some.length,Math.max(senders,recipients));
 }
 assert(fields.owned_invocations.every(i=>!('actor' in i)&&!('source_path' in i)));
 assert(!JSON.stringify(fields).includes('expectedResults'));
 assert(Buffer.byteLength(canonical(f.sourceInputs))<=8192);assert(Buffer.byteLength(canonical(fields.owned_invocations))<=12288);
});

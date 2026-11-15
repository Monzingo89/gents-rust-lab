import test from 'node:test';
import assert from 'node:assert/strict';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {allocateOracle} from './fixtures/mcp-owned-allocation-chain.mjs';
import {preparedPlanningFields} from './mcp-owned-prepared.mjs';
import {frozenOwnedCalls,verifyOwnedResult} from './mcp-owned-fixture.mjs';
import {sha,expectedArguments,ownedReferences,verifyOwnedLineage} from './mcp-owned-edges.mjs';

// Fabricated evidence tests the observer only. It establishes no agent capability.
function simulated(){
 const fixture=fixtureFromId('owned-allocation-chain'),[first,second]=fixture.sourceInputs,actor='Chosen Analyst',run='synthetic-run',revision=4;
 const parent={path:'receipt-only.json',source_actor:actor,writer:actor,source_tool:first.tool,arguments:first.arguments,pointer:'/output',encoding:'json',receipt_only:true};
 const child={path:'final-allocation.json',source_actor:actor,writer:'Chosen Writer',source_tool:second.tool,arguments:second.arguments,pointer:'/output',encoding:'json',argument_sources:[{field:'arguments',path:parent.path,argument_index:1,pointer:'/some'}]};
 const bindings=[parent,child],spec=frozenOwnedCalls(fixture,actor,bindings)[1],producer={passed:true,output:fixture.expectedResults.get(first.id).output};
 const call={request:{action:first.tool,arguments:first.arguments},result:{result:{result:producer,mcp_receipt:{actor,result_sha256:sha(canonical(producer))}}}};
 const lineage=[{run_id:run,path:parent.path,argument_index:1,pointer:'/some',producer_binding_id:first.id,producer_actor:actor,source_sha256:spec.source_sha256,api_sha256:spec.api_sha256,producer_function:spec.function,arguments_sha256:sha(canonical(first.arguments)),result_sha256:sha(canonical(producer)),value_sha256:sha(canonical(producer.output.some)),policy_revision:revision}];
 const request=expectedArguments(fixture,second),result={...spec,binding_id:second.id,passed:true,stage:'invocation',input_sha256:sha(canonical(request.arguments)),input_recipe_sha256:sha(canonical({kind:'owned_positional_receipts_v1',recipe:spec.input_recipe})),input_lineage:lineage,output:fixture.expectedResults.get(second.id).output,provider_cost_usd:0,promotion_authorized:false,compiler:'Simulated observer test',isolation:'No execution',diagnostics:{passed:true,exit_code:0,signal:null,limit:null,stdout:canonical(fixture.expectedResults.get(second.id).output)}};
 const row={...spec,binding_id:second.id,input_sha256:result.input_sha256,policy_revision:revision,arguments_sha256:sha(canonical(request)),result_sha256:sha(canonical(result)),passed:1,recipe_sha256:result.input_recipe_sha256,lineage_sha256:sha(canonical(lineage))};
 return {fixture,input:second,result,authority:{policy_revision:revision,owned_rust_invocations:[row]},actor,options:{bindings,calls:[call],run,actor,revision,request}};
}
test('prepared chain freezes placeholders, hides answers and preserves earlier fixture digests',()=>{
 const s=simulated(),fields=preparedPlanningFields(s.fixture),specs=frozenOwnedCalls(s.fixture,s.actor,s.options.bindings);
 assert.deepEqual(fields.owned_invocations[1].argument_slots,[1]);assert.deepEqual(fields.owned_invocations[1].arguments,['101',[]]);
 assert.equal(specs[0].input_sha256,sha(canonical(s.fixture.sourceInputs[0].arguments.arguments)));
 assert.equal(specs[1].input_sha256,undefined);assert.deepEqual(specs[1].input_recipe.arguments,['101',[]]);
 assert.equal(fixtureDigest(fixtureFromId('owned-mapped-batch')),'e8e7157f2d5051f61aaf072b410ba70b25efb2ce188b73799806de466af1dc8a');
 for(const output of s.fixture.expectedResults.values())assert(!canonical(fields).includes(canonical(output.output))&&!s.fixture.requirements.includes(canonical(output.output.some)));
});
test('owned positional dependency checks reject wrong producers, projections, slots and extras',()=>{
 const s=simulated();assert.deepEqual(ownedReferences(s.fixture,s.input,s.options.bindings),s.options.bindings[1].argument_sources);
 for(const mutate of [b=>b[1].argument_sources[0].path='missing',b=>b[1].argument_sources[0].argument_index=0,b=>b[1].argument_sources[0].pointer='',b=>b[0].arguments.binding_id='unapproved',b=>b[0].pointer='/output/some',b=>b[1].argument_sources.push(b[1].argument_sources[0])]){
  const bindings=structuredClone(s.options.bindings);mutate(bindings);assert.throws(()=>ownedReferences(s.fixture,s.input,bindings));
 }
});
test('exact lineage and SQL recipe identity must match parent evidence and current policy',()=>{
 const check=s=>verifyOwnedResult(s.fixture,s.input,s.result,s.authority,s.actor,s.options);
 check(simulated());
 for(const mutate of [s=>s.options.calls=[],s=>s.options.calls.push(s.options.calls[0]),s=>s.options.calls[0].result.result.result.passed=false,s=>s.options.calls[0].result.result.mcp_receipt.actor='Other',s=>s.result.input_lineage[0].run_id='other-run',s=>s.result.input_lineage[0].policy_revision--,s=>s.result.input_lineage[0].result_sha256='0'.repeat(64),s=>s.result.input_lineage[0].value_sha256='0'.repeat(64),s=>s.result.input_recipe_sha256='0'.repeat(64),s=>s.authority.owned_rust_invocations[0].recipe_sha256=null,s=>s.authority.owned_rust_invocations[0].lineage_sha256=null,s=>s.options.request.arguments[0]='100']){
  const s=simulated();mutate(s);assert.throws(()=>check(s));
 }
 const s=simulated();assert.equal(verifyOwnedLineage(s.fixture,s.input,s.result,s.options),s.authority.owned_rust_invocations[0].lineage_sha256);
});
test('independent allocation oracle checks conservation, quotas, ties and exact wide integers',()=>{
 assert.deepEqual(allocateOracle('0',[]),{some:[]});assert.deepEqual(allocateOracle('1',[]),{none:true});assert.deepEqual(allocateOracle('1',Array(65).fill('1')),{none:true});
 assert.deepEqual(allocateOracle('2',['1','1','1']),{some:['1','1','0']});
 for(const total of ['0','23','101','18446744073709551615'])for(const weights of [['2','3','5'],['0','1','0'],['18446744073709551615','18446744073709551614','1']]){
  const values=allocateOracle(total,weights).some.map(BigInt),t=BigInt(total),sum=weights.reduce((n,w)=>n+BigInt(w),0n);assert.equal(values.reduce((n,v)=>n+v,0n),t);
  values.forEach((v,i)=>{const floor=t*BigInt(weights[i])/sum;assert(v===floor||v===floor+1n);});
 }
});

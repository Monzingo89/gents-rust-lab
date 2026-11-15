import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {fixtureFromId,fixtureDigest,canonical} from './fixtures/mcp-workflow-catalog.mjs';
import {preparedPlanningFields} from './mcp-owned-prepared.mjs';
import {reviewSelectionDraft,selectedExecutionFixture,verifySelectedIngress,verifyAdoptedCompilation} from './mcp-owned-selection.mjs';
const sha=value=>createHash('sha256').update(canonical(value)).digest('hex');
function simulated(fixtureId='owned-ceo-selection'){
 const fixture=fixtureFromId(fixtureId),ids=fixtureId==='owned-ceo-selection'?{schedule:'zeta-source',weights:'alpha-adapter',allocate:'middle-allocation'}:Object.fromEntries(fixture.selection.roles.map((role,index)=>[role.role,['node-z','node-a','node-r','node-y','node-b','node-s'][index]]));
 const proposal=fixture.selection.roles.map(role=>({id:ids[role.role],component_id:fixture.ownedComponents.find(c=>c.function===role.function).owner_declared_review_package_id,function:role.function,arguments:role.arguments.map(arg=>arg.kind==='data'?arg:{kind:'result',source:ids[arg.source_role],pointer:arg.pointer})}));
 const invocations=proposal.map(c=>({id:c.id,component_id:c.component_id,function:c.function,arguments:c.arguments.map(a=>a.kind==='data'?fixture.selection.data[a.data]:null),...(c.arguments.some(a=>a.kind==='result')?{argument_slots:c.arguments.flatMap((a,i)=>a.kind==='result'?[i]:[])}:{})}));
 const plan={id:'synthetic-selection',revision:2,status:'selection_review',owned_argument_data:fixture.selection.data,owned_component_catalog:{catalog_sha256:'c'.repeat(64)},messages:[{role:'ceo',message:{status:'selection_review',owned_selection_proposal:proposal}}]};
 const basis={kind:'owned_selection_v1',session:plan.id,revision:plan.revision,catalog_sha256:plan.owned_component_catalog.catalog_sha256,data_sha256:sha(fixture.selection.data),proposal};
 plan.pending_owned_selection={...basis,proposal_sha256:sha(basis),invocations,execution_authorized:false,promotion_authorized:false};
 return {fixture,plan};
}
test('selection input freezes only owner data/catalog; old qualified fixture identities stay unchanged',()=>{
 const {fixture}=simulated(),fields=preparedPlanningFields(fixture);assert.equal(fields.owned_component_ids.length,3);assert.equal(fields.owned_invocations,undefined);assert.deepEqual(fields.owned_argument_data,fixture.selection.data);assert.deepEqual(fixture.sourceInputs,[]);
 for(const spec of fixture.ownedComponents)assert(!fixture.initial.includes(spec.function)&&!fixture.requirements.includes(spec.function));
 for(const output of fixture.expectedResults.values())assert(!canonical(fields).includes(canonical(output)));
 assert.equal(fixtureDigest(fixtureFromId('owned-multi-library')),'52a2110a423d68adf38605e87ff00a3f1ea252fbc03701fc2f1fb9042f3145d2');
 const other=structuredClone(fixture);other.selection.data.budget='1010';assert.notEqual(fixtureDigest(fixture),fixtureDigest(other));
});
test('draft adoption keeps actual model-selected IDs and rejects changed functions, data, references and digests',()=>{
 const good=simulated();assert.deepEqual(reviewSelectionDraft(good.fixture,good.plan),good.plan.pending_owned_selection.invocations);
 for(const mutate of [s=>s.plan.pending_owned_selection.proposal[0].function='invented',s=>s.plan.pending_owned_selection.proposal[1].arguments[0].source='middle-allocation',s=>s.plan.pending_owned_selection.invocations[0].arguments=[[]],s=>s.plan.owned_argument_data={budget:'7'},s=>s.plan.pending_owned_selection.proposal_sha256='0'.repeat(64),s=>s.plan.messages=[]]){const s=structuredClone(simulated());mutate(s);assert.throws(()=>reviewSelectionDraft(s.fixture,s.plan));}
});
test('post-adoption oracle view binds selected calls and intake provenance without changing their arguments',()=>{
 const {fixture,plan}=simulated(),pending=plan.pending_owned_selection;
 plan.accepted_owned_selection={proposal:pending.proposal,proposal_sha256:pending.proposal_sha256,data_sha256:pending.data_sha256,catalog_sha256:pending.catalog_sha256,owner_confirmed_revision:3};plan.revision=3;plan.owned_invocations=[...pending.invocations].reverse().map(invocation=>({invocation}));
 const view=selectedExecutionFixture(fixture,plan);assert.deepEqual(view.sourceInputs.map(i=>i.id),['alpha-adapter','middle-allocation','zeta-source']);assert.equal(view.artifacts[0].input,'middle-allocation');assert.equal(view.expectedResults.get('middle-allocation'),fixture.expectedResults.get('allocate'));
 assert.deepEqual(view.dependencies['middle-allocation'],[{field:'arguments',argument_index:1,input:'alpha-adapter',pointer:'/output/some'}]);
 const intake={ceo_owned_argument_data:fixture.selection.data,ceo_owned_selection:plan.accepted_owned_selection};assert.equal(verifySelectedIngress(view,plan,{intake},{intake}).ceo_owned_function_selection_verified,true);
 assert.throws(()=>verifySelectedIngress(view,plan,{intake:{...intake,ceo_owned_selection:null}},{intake}));
 plan.accepted_owned_selection.owner_confirmed_revision=4;assert.throws(()=>selectedExecutionFixture(fixture,plan));
});
test('repeated functions are matched by exact branch data and actual references, preserving arbitrary IDs and topological ordering',()=>{
 const {fixture,plan}=simulated('owned-ceo-branches'),pending=plan.pending_owned_selection;
 // A valid model order can interleave branches differently from private role order.
 pending.proposal=[pending.proposal[3],pending.proposal[0],pending.proposal[4],pending.proposal[1],pending.proposal[5],pending.proposal[2]];
 plan.messages[0].message.owned_selection_proposal=pending.proposal;
 pending.proposal_sha256=sha({kind:'owned_selection_v1',session:plan.id,revision:plan.revision,catalog_sha256:pending.catalog_sha256,data_sha256:pending.data_sha256,proposal:pending.proposal});
 assert.deepEqual(reviewSelectionDraft(fixture,plan),pending.invocations);
 plan.accepted_owned_selection={proposal:pending.proposal,proposal_sha256:pending.proposal_sha256,data_sha256:pending.data_sha256,catalog_sha256:pending.catalog_sha256,owner_confirmed_revision:3};plan.owned_invocations=pending.invocations.map(invocation=>({invocation}));
 const view=selectedExecutionFixture(fixture,plan);assert.equal(view.sourceInputs.length,6);assert.equal(Object.values(view.dependencies).flat().length,4);assert.deepEqual(view.artifacts.map(a=>[a.path,a.input]),[['north.json','node-r'],['south.json','node-s']]);
 assert.equal(view.expectedResults.get('node-s'),fixture.expectedResults.get('south-allocation'));
 assert.deepEqual(view.dependencies['node-r'],[{field:'arguments',argument_index:1,input:'node-a',pointer:'/output/some'}]);
 assert.deepEqual(view.dependencies['node-s'],[{field:'arguments',argument_index:1,input:'node-b',pointer:'/output/some'}]);
});
test('independent branch review rejects crossed results, swapped data, duplicates and future references',()=>{
 for(const mutate of [
  p=>p[2].arguments[1].source='node-b',
  p=>p[2].arguments[0].data='south-budget',
  p=>p[3].arguments[0].data='north-requests',
  p=>p[3].id=p[0].id,
  p=>{[p[0],p[1]]=[p[1],p[0]];},
 ]){const {fixture,plan}=simulated('owned-ceo-branches');mutate(plan.pending_owned_selection.proposal);assert.throws(()=>reviewSelectionDraft(fixture,plan));}
});
test('two-branch challenge supplies only owner data and catalogs, with private scarce-budget acceptance',()=>{
 const fixture=fixtureFromId('owned-ceo-branches'),fields=preparedPlanningFields(fixture);
 assert.equal(fields.owned_component_ids.length,3);assert.equal(fields.owned_invocations,undefined);assert.equal(fixture.selection.roles.length,6);assert.deepEqual(fixture.sourceInputs,[]);
 for(const spec of fixture.ownedComponents)assert(!fixture.initial.includes(spec.function)&&!fixture.requirements.includes(spec.function));
 const outcome=fixture.expectedResults.get('south-allocation').output.some;assert.equal(outcome.reduce((n,v)=>n+BigInt(v),0n),2n);assert(outcome.some(v=>v==='0'));
 assert.equal(fixtureDigest(fixtureFromId('owned-ceo-selection')),'4cfbbec1685f9ca4f598127fa8e220f67ddb767e43cf6ffbb753b93ebb490a53');
 assert(!canonical(fields).includes(canonical(fixture.expectedResults.get('south-allocation'))));
});
test('adopted-reference compiler proof binds actual raw model choices, exact edges and native audit',()=>{
 const {fixture,plan}=simulated('owned-ceo-branches'),pending=plan.pending_owned_selection;
 plan.accepted_owned_selection={proposal:pending.proposal,proposal_sha256:pending.proposal_sha256};
 const dependencies=Object.fromEntries(pending.proposal.map(call=>[call.id,call.arguments.flatMap((r,i)=>r.kind==='result'?[{field:'arguments',source:r.source,pointer:'/output',argument_index:i,value_pointer:r.pointer}]:[])]));
 const raw={status:'ready',native_contracts:{sources:pending.proposal.map(c=>({id:c.id,input:c.id,actor:'Model Actor',argument_sources:[]})),artifacts:[{source:'node-s',writer:'Model Writer',path:'south.json',pointer:'/output',encoding:'json'}]}};
 const receipt={raw_proposal_sha256:sha(raw),selection_proposal_sha256:pending.proposal_sha256,dependencies_sha256:sha(dependencies),source_count:6,edge_count:4,scope:'Exact owner-adopted model references only; no actor, output, source, invocation or authority supplied.'};
 plan.planning_protocol={id:'adopted_prepared_edges_v1'};plan.host_compiled_adopted_dependencies=receipt;
 plan.messages.push({role:'ceo',message:raw,host_compiled_adopted_dependencies:receipt});
 plan.proposal={...raw,native_contracts:{...raw.native_contracts,sources:raw.native_contracts.sources.map(s=>({...s,argument_sources:dependencies[s.id]}))}};
 const authority={planning_attempts:[{status:'ready',planning_protocol:plan.planning_protocol,host_compiled_adopted_dependencies:receipt}]};
 assert.equal(verifyAdoptedCompilation(plan,authority).host_compiled_adopted_edges,4);
 for(const mutate of [
  p=>p.messages.at(-1).message.native_contracts.sources[0].actor='Forged Actor',
  p=>p.proposal.native_contracts.sources[1].argument_sources[0].source='node-b',
  p=>p.proposal.native_contracts.sources[0].actor='Other Actor',
  p=>p.host_compiled_adopted_dependencies.dependencies_sha256='f'.repeat(64),
  p=>p.planning_protocol.id='typed_prepared_edges_v1',
 ]){const changed=structuredClone(plan);mutate(changed);assert.throws(()=>verifyAdoptedCompilation(changed,authority));}
 assert.throws(()=>verifyAdoptedCompilation(plan,{planning_attempts:[]}));
 assert.deepEqual(verifyAdoptedCompilation({planning_protocol:{id:'typed_prepared_edges_v1'}},{}),{});
 assert.equal(fixture.selection.roles.length,6);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import fixture from './fixtures/mcp-owned-multi-library.mjs';
import original from './fixtures/mcp-owned-multi-library-v1.mjs';
import {fixtureDigest} from './fixtures/mcp-workflow-catalog.mjs';
import {preparedPlanningFields} from './mcp-owned-prepared.mjs';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
test('multi-library benchmark freezes explicit adapter edges and independent invariants',()=>{
 const fields=preparedPlanningFields(fixture);assert.equal(fields.owned_component_ids.length,3);assert.equal(fixture.files.length,3);assert.equal(fixture.sourceInputs.length,8);assert.equal(Object.values(fixture.dependencies).flat().length,6);assert.equal(fixture.artifacts.length,2);
 assert.deepEqual(fixture.dependencies['b-weights'],[{field:'arguments',argument_index:0,input:'a-schedule',pointer:'/output/some'}]);
 assert.deepEqual(fixture.dependencies['c-distribute'],[{field:'arguments',argument_index:1,input:'b-weights',pointer:'/output/some'}]);
 assert(fixture.requirements.includes('never bypass it with an implicit cast'));
 for(const input of fixture.sourceInputs){
  const spec=fixture.ownedComponents.find(s=>s.input===input.id),output=fixture.expectedResults.get(input.id).output;assert(Array.isArray(output.some));
  if(spec.function==='index_weights'){
   const parent=fixture.expectedResults.get(fixture.dependencies[input.id][0].input).output.some;assert.deepEqual(output.some.map(BigInt),parent.map(i=>BigInt(i)+1n));assert(output.some.every(v=>BigInt(v)>0n));assert.deepEqual(input.argument_slots,[0]);
  }else if(spec.function==='allocate_cents')assert.equal(output.some.map(BigInt).reduce((s,n)=>s+n,0n),BigInt(input.arguments.arguments[0]));
  else{const chosen=output.some.map(Number),requests=input.arguments.arguments[0];assert(chosen.length>0);assert(requests.length<=16);for(const a of chosen)for(const b of chosen)if(a<b)assert(!(BigInt(requests[a][0])<BigInt(requests[b][1])&&BigInt(requests[b][0])<BigInt(requests[a][1])));}
  assert(!canonical(fields).includes(canonical(output))&&!fixture.requirements.includes(canonical(output.some)));
 }
 assert(Buffer.byteLength(canonical(fixture.sourceInputs))<8192);assert(Buffer.byteLength(canonical(fields.owned_invocations))<12288);
});
test('first frozen alias failure remains immutable and is rejected before inference',()=>{
 assert.equal(fixtureDigest(original),'9bb38295ff5d22166e348ae41069670b2e599a7dcfd1a2b647256eebe30db010');
 assert.notEqual(fixtureDigest(fixture),fixtureDigest(original));
 assert.throws(()=>preparedPlanningFields(original),/package-canonical staging path/);
 assert.deepEqual(fixture.sourceInputs,original.sourceInputs);assert.deepEqual(fixture.artifacts,original.artifacts);assert.equal(fixture.requirements,original.requirements);assert.deepEqual(fixture.expectedResults,original.expectedResults);
});

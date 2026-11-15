import test from 'node:test';
import assert from 'node:assert/strict';
import fixture from './fixtures/mcp-owned-branched-allocation.mjs';
import {preparedPlanningFields} from './mcp-owned-prepared.mjs';
import {canonical} from './fixtures/mcp-reconciliation.mjs';

test('branched benchmark has eight immutable nodes, seven receipt edges and two sole publications',()=>{
 assert.equal(fixture.sourceInputs.length,8);assert.equal(fixture.artifacts.length,2);assert.equal(fixture.files.length,1);
 assert.equal(Object.values(fixture.dependencies).flat().length,7);
 assert.equal(Object.values(fixture.dependencies).flat().filter(r=>r.input==='a-root').length,2);
 const depth=id=>1+Math.max(0,...fixture.dependencies[id].map(r=>depth(r.input)));
 assert.deepEqual(fixture.artifacts.map(a=>depth(a.input)),[4,5]);
 assert.equal(fixture.sourceInputs.filter(i=>i.argument_slots?.length).length,7);
 const fields=preparedPlanningFields(fixture);assert.equal(fields.owned_component_ids.length,1);
 assert(Buffer.byteLength(canonical(fixture.sourceInputs))<8192);assert(Buffer.byteLength(canonical(fields.owned_invocations))<12288);
 for(const input of fixture.sourceInputs){
  const output=fixture.expectedResults.get(input.id).output;assert(Array.isArray(output.some));assert.equal(output.some.length,16);
  assert.equal(output.some.map(BigInt).reduce((s,n)=>s+n,0n),BigInt(input.arguments.arguments[0]));
  assert(!canonical(fields).includes(canonical(output))&&!fixture.requirements.includes(canonical(output.some)));
  if(input.argument_slots){assert.deepEqual(input.arguments.arguments[1],[]);assert.deepEqual(input.argument_slots,[1]);}
 }
});

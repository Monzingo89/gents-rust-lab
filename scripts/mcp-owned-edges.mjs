// Independent owner-side graph/lineage checks. No model, tool or policy calls.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {canonical,project} from './fixtures/mcp-reconciliation.mjs';
export const sha=value=>createHash('sha256').update(value).digest('hex');
export const hasOwnedSlots=fixture=>fixture.sourceInputs.some(i=>i.argument_slots?.length);
export function expectedArguments(fixture,input){
 const args=structuredClone(input.arguments);
 for(const ref of fixture.dependencies[input.id]){
  const value=structuredClone(project(fixture.expectedResults.get(ref.input),ref.pointer));
  if(Number.isSafeInteger(ref.argument_index))args[ref.field][ref.argument_index]=value;
  else args[ref.field]=value;
 }
 return args;
}
export function ownedReferences(fixture,input,bindings){
 const binding=bindings.find(b=>b.source_tool==='owned_rust_call'&&b.arguments.binding_id===input.id);assert(binding,'Missing frozen owned binding');
 const actual=binding.argument_sources??[],expected=fixture.dependencies[input.id];assert.equal(actual.length,expected.length,'Dependency selection differs from confirmed requirements');
 for(const ref of expected){
  const selected=actual.find(r=>r.field===ref.field&&r.argument_index===ref.argument_index);assert(selected,'Dependency slot differs from confirmed requirements');
  assert.equal(ref.field,'arguments');assert(Number.isSafeInteger(ref.argument_index));assert(ref.pointer.startsWith('/output'));
  assert.equal(selected.pointer,ref.pointer.slice('/output'.length),'Dependency projection differs from confirmed requirements');
  const parent=bindings.find(b=>b.path===selected.path);assert(parent,'Dependency producer is missing');assert.equal(parent.source_tool,'owned_rust_call');assert.equal(parent.arguments.binding_id,ref.input,'Dependency producer differs from confirmed requirements');assert.equal(parent.pointer,'/output');
 }
 return structuredClone(actual);
}
export function verifyOwnedLineage(fixture,input,result,{bindings,calls,run,actor,revision}){
 const references=ownedReferences(fixture,input,bindings);
 const lineage=references.map(ref=>{
  const parent=bindings.find(b=>b.path===ref.path),spec=fixture.ownedComponents.find(s=>s.input===parent.arguments.binding_id);assert(spec);
  const candidates=calls.filter(c=>c.request.action==='owned_rust_call'&&c.request.arguments.binding_id===spec.input);assert.equal(candidates.length,1);
  const call=candidates[0],envelope=call.result.result,producer=envelope.result,receipt=envelope.mcp_receipt;assert.equal(producer.passed,true);assert.equal(receipt.actor,actor);
  const value=project(producer.output,ref.pointer);assert.equal(canonical(result.input_lineage?.find(r=>r.argument_index===ref.argument_index)?.value_sha256),canonical(sha(canonical(value))));
  return {run_id:run,path:ref.path,argument_index:ref.argument_index,pointer:ref.pointer,producer_binding_id:spec.input,producer_actor:actor,source_sha256:spec.source_sha256,api_sha256:spec.api_sha256,producer_function:spec.function,arguments_sha256:sha(canonical(call.request.arguments)),result_sha256:receipt.result_sha256,value_sha256:sha(canonical(value)),policy_revision:revision};
 });
 assert.equal(canonical(result.input_lineage),canonical(lineage),'Native dependency lineage differs from exact producer evidence');
 return sha(canonical(lineage));
}

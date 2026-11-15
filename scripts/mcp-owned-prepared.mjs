// Independent preparation-ingress checks. No source writes, authority or model calls.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
import {fixtureBytes} from './mcp-fixture-files.mjs';
import {frozenOwnedCalls} from './mcp-owned-fixture.mjs';
const sha=v=>createHash('sha256').update(v).digest('hex');
export function preparedPlanningFields(fixture){
 if(!fixture.ownerPreparation)return {};
 if(fixture.ownerPreparation==='catalog_selection_v1'){
  assert(fixture.selection&&fixture.sourceInputs.length===0,'Selection starts with owner data and no prepared invocations');
  for(const spec of fixture.ownedComponents)assert(spec.source_path===`owned-${spec.owner_declared_review_package_id}.rs`&&fixture.files.some(f=>f.path===spec.source_path&&f.owned_component===spec.owner_declared_review_package_id&&f.sha256===spec.source_sha256),'Selection source must match canonical package custody');
  return {owned_component_ids:[...new Set(fixture.ownedComponents.map(c=>c.owner_declared_review_package_id))].sort(),owned_argument_data:structuredClone(fixture.selection.data)};
 }
 assert.equal(fixture.ownerPreparation,'retained_component_v1');
 const requests=fixture.ownedComponents.map(spec=>{
  const input=fixture.sourceInputs.find(i=>i.id===spec.input);assert(input?.tool==='owned_rust_call');assert.equal(input.id,input.arguments.binding_id);
  assert.equal(spec.source_path,`owned-${spec.owner_declared_review_package_id}.rs`,'Prepared fixtures require the package-canonical staging path');
  const file=fixture.files.find(file=>file.path===spec.source_path);assert(file&&file.owned_component===spec.owner_declared_review_package_id&&file.sha256===spec.source_sha256,'Prepared fixture source does not match package custody');
  return {id:input.id,component_id:spec.owner_declared_review_package_id,function:spec.function,arguments:structuredClone(input.arguments.arguments),...(input.argument_slots?.length?{argument_slots:structuredClone(input.argument_slots)}:{})};
 });
 assert(requests.length>0&&requests.length<=8);assert.equal(new Set(requests.map(r=>r.id)).size,requests.length);
 return {owned_component_ids:[...new Set(requests.map(r=>r.component_id))].sort(),owned_invocations:requests};
}
export function verifyPreparedIngress(fixture,plan,review,authority,actor){
 if(!fixture.ownerPreparation)return {};
 const fields=preparedPlanningFields(fixture);
 const expected=fields.owned_invocations.map(invocation=>{
  const spec=fixture.ownedComponents.find(s=>s.input===invocation.id),file=fixture.files.find(f=>f.path===spec.source_path);assert(file);
  return {invocation,origin_company:null,source_path:file.path,source_sha256:spec.source_sha256,api_sha256:spec.api_sha256,...(invocation.argument_slots?.length?{input_template_sha256:sha(canonical(invocation.arguments))}:{input_sha256:sha(canonical(invocation.arguments))}),bytes:fixtureBytes(file).length};
 }).sort((a,b)=>a.invocation.id.localeCompare(b.invocation.id));
 // Origin is the actual package context, frozen and rechecked by the host.
 for(const m of expected){const entry=plan.owned_component_catalog.components.find(c=>c.component_id===m.invocation.component_id);assert(entry);m.origin_company=entry.origin_company;assert.equal(entry.source_sha256,m.source_sha256);assert.equal(entry.api_sha256,m.api_sha256);assert(entry.functions.some(f=>f.name===m.invocation.function));}
 assert.equal(canonical(plan.owned_invocations),canonical(expected),'Planning retained metadata differs from frozen fixture invocation inputs');
 assert.equal(canonical(review.intake.ceo_owned_invocations),canonical(expected),'Frozen owner metadata differs from retained invocation input');
 assert.equal(canonical(review.intake.required_owned_rust_calls),canonical(frozenOwnedCalls(fixture,actor,review.intake.artifact_bindings).sort((a,b)=>a.id.localeCompare(b.id))),'Requirements were not bound to actual CEO source actor');
 assert.equal(canonical(review.intake.ceo_source_inputs),canonical(fixture.sourceInputs));
 assert.equal(authority.owned_input_custody.length,expected.length);
 for(let i=0;i<expected.length;i++){
  const actual=authority.owned_input_custody[i],m=expected[i];assert.equal(canonical(actual.manifest),canonical(m));assert.equal(actual.source_sha256,m.source_sha256);assert.equal(actual.bytes,m.bytes);
 }
 assert.equal(authority.owned_source_staging.length,1,'Missing/replayed approved source staging');
 const staged=authority.owned_source_staging[0];assert.equal(staged.planning_id,plan.id);assert.equal(staged.provider_cost_usd,0);
 const files=[...new Map(expected.map(m=>[m.source_path,{path:m.source_path,bytes:m.bytes,sha256:m.source_sha256}])).values()].sort((a,b)=>a.path.localeCompare(b.path));
 assert.equal(canonical([...staged.files].sort((a,b)=>a.path.localeCompare(b.path))),canonical(files));
 return {owner_prepared_ingress_verified:true,retained_invocations:expected.length,host_staged_source_files:files.length,manual_invocation_requirement_injection:false,runner_source_staging:false};
}

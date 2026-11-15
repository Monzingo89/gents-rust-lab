// Host-only custody and exact binding checks for reviewed reuse fixtures.
// No policy installation, source authorship, candidate invocation or inference.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {canonical} from './fixtures/mcp-reconciliation.mjs';
import {readOwnedEvidence,verifyComponentPackage} from './mcp-component-store.mjs';
import {probeComponent} from './mcp-component-probe.mjs';
import {ownedReferences,verifyOwnedLineage} from './mcp-owned-edges.mjs';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
export async function materializeOwnedFixture(root,fixture){
 if(!fixture.ownedComponents)return fixture;
 assert(fixture.ownedComponents.length>0&&fixture.ownedComponents.length<=16);
 const files=[];
 for(const file of fixture.files){
  assert(/^[a-f0-9]{64}$/.test(file.owned_component),'Reuse fixtures need explicit sealed component identities');
  const packet=await verifyComponentPackage(root,file.owned_component);
  const bytes=await readOwnedEvidence(root,`data/owned-rust-components/${file.owned_component}/source.rs`,8192,true);
  assert.equal(sha(bytes),file.sha256);assert.equal(packet.context.source_sha256,file.sha256);
  const qualification=JSON.parse(await readOwnedEvidence(root,`data/owned-rust-components/${file.owned_component}/qualification.json`,8*1024*1024,true));
  assert.equal(qualification.proof.status,'PASS');assert.equal(qualification.proof.source_is_local_model_authored,true);
  assert.equal(qualification.proof.source_sha256,file.sha256);assert.equal(qualification.proof.provider_cost_usd,0);
  const description=(await probeComponent(root,file.owned_component)).description;
  const declarations=fixture.ownedComponents.filter(c=>c.source_path===file.path);assert(declarations.length>0);
  for(const spec of declarations){assert.equal(spec.owner_declared_review_package_id,file.owned_component);assert.equal(spec.source_sha256,file.sha256);assert.equal(spec.api_sha256,description.api_sha256);assert(description.interface.functions.some(f=>f.name===spec.function));}
  const text=bytes.toString('utf8');assert(Buffer.from(text).equals(bytes));files.push({...file,text});
 }
 assert(fixture.ownedComponents.every(spec=>files.some(file=>file.path===spec.source_path)));
 return {...fixture,files};
}
export function frozenOwnedCalls(fixture,actor,bindings=null){
 return fixture.ownedComponents?.map(spec=>{
  const input=fixture.sourceInputs.find(input=>input.id===spec.input);assert(input?.tool==='owned_rust_call');
  const {input:_,...identities}=spec;
  return {id:input.arguments.binding_id,actor,...identities,...(input.argument_slots?.length?{input_recipe:{arguments:structuredClone(input.arguments.arguments),sources:ownedReferences(fixture,input,bindings??[])}}:{input_sha256:sha(canonical(input.arguments.arguments))})};
 })??[];
}
export function verifyOwnedResult(fixture,input,result,authority,actor,options={}){
 const spec=frozenOwnedCalls(fixture,actor,options.bindings).find(s=>s.id===input.arguments.binding_id);assert(spec,'No reviewed invocation');
 assert.equal(result.passed,true);assert.equal(result.stage,'invocation');assert.equal(result.binding_id,spec.id);
 for(const field of ['source_path','source_sha256','api_sha256','function','owner_declared_review_package_id'])assert.equal(result[field],spec[field],`Owned result identity ${field}`);
 assert.equal(result.provider_cost_usd,0);assert.equal(result.promotion_authorized,false);
 assert.equal(canonical(result.output),canonical(fixture.expectedResults.get(input.id).output),'Independent owned-result oracle mismatch');
 assert.equal(result.diagnostics.passed,true);assert.equal(result.diagnostics.exit_code,0);assert.equal(result.diagnostics.signal,null);assert.equal(result.diagnostics.limit,null);
 assert.equal(canonical(JSON.parse(result.diagnostics.stdout)),canonical(result.output));assert.equal(typeof result.compiler,'string');assert.equal(typeof result.isolation,'string');
 const request=options.request??{binding_id:spec.id,arguments:input.arguments.arguments};
 const inputHash=sha(canonical(request.arguments));assert.equal(result.input_sha256,inputHash);
 let recipeHash=null,lineageHash=null;
 if(spec.input_recipe){
  recipeHash=sha(canonical({kind:'owned_positional_receipts_v1',recipe:spec.input_recipe}));assert.equal(result.input_recipe_sha256,recipeHash);
  lineageHash=verifyOwnedLineage(fixture,input,result,{bindings:options.bindings,calls:options.calls,run:options.run,actor,revision:authority.policy_revision});
 }else assert.equal(inputHash,spec.input_sha256);
 const rows=authority.owned_rust_invocations.filter(row=>row.actor===actor&&row.binding_id===spec.id&&row.policy_revision===authority.policy_revision&&row.source_sha256===spec.source_sha256&&row.api_sha256===spec.api_sha256&&row.input_sha256===inputHash&&row.arguments_sha256===sha(canonical(request))&&row.result_sha256===sha(canonical(result))&&row.passed===1);
 assert.equal(rows.length,1,'Missing/replayed canonical owned invocation observation');
 assert.equal(rows[0].recipe_sha256??null,recipeHash);assert.equal(rows[0].lineage_sha256??null,lineageHash);
}

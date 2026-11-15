// Independent draft/adoption checks. Never chooses calls or changes a model draft.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {canonical,project} from './fixtures/mcp-reconciliation.mjs';
const sha=value=>createHash('sha256').update(canonical(value)).digest('hex');
export const hasSelection=fixture=>Boolean(fixture.selection);
export const needsSelection=fixture=>fixture.ownerPreparation==='catalog_selection_v1';
function checkedCalls(fixture,proposal,invocations){
 const roles=fixture.selection.roles;assert(Array.isArray(proposal)&&proposal.length===roles.length,'Selected calls must cover the requested operations exactly');assert(Array.isArray(invocations)&&invocations.length===roles.length,'Missing resolved selection calls');
 const byRole=new Map(),seen=new Set(),ids=new Set();
 for(const call of proposal){assert(typeof call.id==='string'&&/^[a-z0-9-]{1,40}$/.test(call.id)&&!ids.has(call.id),'Duplicate/invalid model-selected identity');ids.add(call.id);}
 for(const role of roles){
  assert(!byRole.has(role.role),'Duplicate private acceptance role');
  const spec=fixture.ownedComponents.find(c=>c.function===role.function);assert(spec,'Missing inspected role interface');
  const references=role.arguments.map(expected=>{
   if(expected.kind==='data')return expected;
   assert.equal(expected.kind,'result');assert(byRole.has(expected.source_role),'Private acceptance roles must precede consumers');
   return {kind:'result',source:byRole.get(expected.source_role).id,pointer:expected.pointer};
  });
  const found=proposal.filter(c=>!seen.has(c.id)&&c.function===role.function&&c.component_id===spec.owner_declared_review_package_id&&canonical(c.arguments)===canonical(references));
  assert.equal(found.length,1,'Selected data reference or function differs from the confirmed requirements');
  const call=found[0];seen.add(call.id);byRole.set(role.role,call);
  const args=[],slots=[];
  for(const [index,expected] of role.arguments.entries()){
   if(expected.kind==='data'){const root=fixture.selection.data[expected.data],value=expected.pointer===''?root:project(root,expected.pointer);assert.notEqual(value,undefined);args.push(structuredClone(value));}
   else{assert(proposal.findIndex(c=>c.id===byRole.get(expected.source_role).id)<proposal.indexOf(call),'Selected result must precede its consumer');args.push(null);slots.push(index);}
  }
  const resolved={id:call.id,component_id:call.component_id,function:call.function,arguments:args,...(slots.length?{argument_slots:slots}:{})};
  const rows=invocations.filter(i=>i.id===call.id);assert.equal(rows.length,1,'Missing/repeated resolved draft');assert.equal(canonical(rows[0]),canonical(resolved),'Resolved draft differs from owner data or result slots');
 }
 return byRole;
}
export function reviewSelectionDraft(fixture,plan){
 assert(needsSelection(fixture));assert.equal(plan.status,'selection_review');assert.equal(canonical(plan.owned_argument_data),canonical(fixture.selection.data),'Owner data changed');
 const pending=plan.pending_owned_selection;assert(pending&&pending.execution_authorized===false&&pending.promotion_authorized===false,'Selection must remain a non-authorizing draft');
 const raw=plan.messages.filter(m=>m.role==='ceo').at(-1)?.message;assert.equal(canonical(raw?.owned_selection_proposal),canonical(pending.proposal),'Retained draft differs from actual local CEO response');
 checkedCalls(fixture,pending.proposal,pending.invocations);
 const basis={kind:'owned_selection_v1',session:plan.id,revision:plan.revision,catalog_sha256:plan.owned_component_catalog.catalog_sha256,data_sha256:sha(fixture.selection.data),proposal:pending.proposal};
 assert.equal(pending.proposal_sha256,sha(basis),'Selection digest mismatch');assert.equal(pending.data_sha256,basis.data_sha256);assert.equal(pending.catalog_sha256,basis.catalog_sha256);
 return structuredClone(pending.invocations);
}
function acceptedCalls(fixture,plan){
 const accepted=plan.accepted_owned_selection;assert(accepted,'Exact owner adoption provenance is required');assert.equal(canonical(plan.owned_argument_data),canonical(fixture.selection.data));
 const invocations=plan.owned_invocations.map(m=>m.invocation),byRole=checkedCalls(fixture,accepted.proposal,invocations);
 const message=plan.messages.find(m=>m.role==='ceo'&&canonical(m.message?.owned_selection_proposal)===canonical(accepted.proposal));assert(message,'Actual model-selected call proposal missing');
 const basis={kind:'owned_selection_v1',session:plan.id,revision:accepted.owner_confirmed_revision-1,catalog_sha256:plan.owned_component_catalog.catalog_sha256,data_sha256:sha(fixture.selection.data),proposal:accepted.proposal};assert.equal(accepted.proposal_sha256,sha(basis));assert.equal(accepted.data_sha256,basis.data_sha256);assert.equal(accepted.catalog_sha256,basis.catalog_sha256);
 return {byRole,invocations};
}
export function selectedExecutionFixture(fixture,plan){
 const {byRole,invocations}=acceptedCalls(fixture,plan);
 const sourceInputs=[],ownedComponents=[],dependencies={},expectedResults=new Map();
 for(const role of fixture.selection.roles){
  const id=byRole.get(role.role).id,invocation=invocations.find(i=>i.id===id),spec=fixture.ownedComponents.find(c=>c.function===role.function);
  sourceInputs.push({id,tool:'owned_rust_call',arguments:{binding_id:id,arguments:structuredClone(invocation.arguments)},...(invocation.argument_slots?.length?{argument_slots:invocation.argument_slots}:{})});ownedComponents.push({...spec,input:id});
  dependencies[id]=role.arguments.flatMap((ref,index)=>ref.kind==='result'?[{field:'arguments',argument_index:index,input:byRole.get(ref.source_role).id,pointer:'/output'+ref.pointer}]:[]);expectedResults.set(id,fixture.expectedResults.get(role.role));
 }
 const order=(a,b)=>a<b?-1:a>b?1:0;sourceInputs.sort((a,b)=>order(a.id,b.id));ownedComponents.sort((a,b)=>order(a.input,b.input));
 return {...fixture,ownerPreparation:'retained_component_v1',sourceInputs,planningInputs:sourceInputs,ownedComponents,dependencies,expectedResults,artifacts:fixture.artifacts.map(a=>({...a,input:byRole.get(a.input).id}))};
}
export function verifySelectedIngress(fixture,plan,review,authority){
 if(!hasSelection(fixture))return {};
 const {byRole}=acceptedCalls(fixture,plan);
 assert.equal(canonical(review.intake.ceo_owned_argument_data),canonical(fixture.selection.data));assert.equal(canonical(authority.intake.ceo_owned_argument_data),canonical(fixture.selection.data));assert.equal(canonical(review.intake.ceo_owned_selection),canonical(plan.accepted_owned_selection));assert.equal(canonical(authority.intake.ceo_owned_selection),canonical(plan.accepted_owned_selection));
 return {...verifyAdoptedCompilation(plan,authority),ceo_owned_function_selection_verified:true,owner_supplied_invocation_ids:false,selected_owned_calls:byRole.size,selected_owned_functions:new Set([...byRole.values()].map(call=>canonical([call.component_id,call.function]))).size,selection_proposal_sha256:plan.accepted_owned_selection.proposal_sha256,selection_scope:'Actual local CEO chose functions, IDs and owner-data/result references; explicit owner adoption only prepared source inputs. Final actor graph, approval, governance and native receipt verification remain separate.'};
}

// Independent reconstruction from the actual adopted model draft, never fixture roles.
export function verifyAdoptedCompilation(plan,authority){
 const receipt=plan.host_compiled_adopted_dependencies;
 if(plan.planning_protocol?.id!=='adopted_prepared_edges_v1'){assert(!receipt,'Unexpected adopted-reference compilation');return {};}
 const adopted=plan.accepted_owned_selection,raw=plan.messages.filter(m=>m.role==='ceo'&&m.message?.status==='ready').at(-1);
 assert(adopted&&raw,'Missing model draft or owner adoption');
 const dependencies=Object.fromEntries(adopted.proposal.map(call=>[call.id,call.arguments.flatMap((ref,index)=>ref.kind==='result'?[{field:'arguments',source:ref.source,pointer:'/output',argument_index:index,value_pointer:ref.pointer}]:[])]));
 const expected={raw_proposal_sha256:sha(raw.message),selection_proposal_sha256:adopted.proposal_sha256,dependencies_sha256:sha(dependencies),source_count:Object.keys(dependencies).length,edge_count:Object.values(dependencies).flat().length,scope:'Exact owner-adopted model references only; no actor, output, source, invocation or authority supplied.'};
 assert.equal(canonical(receipt),canonical(expected),'Compiled reference receipt differs from adopted model choices');
 assert.equal(canonical(raw.host_compiled_adopted_dependencies),canonical(expected));
 const attempt=authority.planning_attempts?.at(-1);assert.equal(attempt?.status,'ready');assert.equal(attempt.planning_protocol?.id,'adopted_prepared_edges_v1');
 assert.equal(canonical(attempt.host_compiled_adopted_dependencies),canonical(expected),'Compiled reference proof lacks canonical native audit');
 const sources=raw.message.native_contracts.sources,compiled=plan.proposal.native_contracts.sources;
 assert.equal(sources.length,expected.source_count);assert.equal(new Set(sources.map(s=>s.id)).size,sources.length);
 for(const source of sources){assert.equal(source.id,source.input);assert(Object.hasOwn(dependencies,source.id));assert.deepEqual(source.argument_sources,[],'Model reselected retained edges');
  const target=compiled.filter(s=>s.id===source.id);assert.equal(target.length,1);assert.equal(canonical(target[0]),canonical({...source,argument_sources:dependencies[source.id]}),'Compiler changed a source or inserted a different edge');
 }
 assert.equal(compiled.length,sources.length);
 return {native_adopted_dependencies_preserved:true,host_compiled_adopted_sources:expected.source_count,host_compiled_adopted_edges:expected.edge_count,adopted_dependencies_sha256:expected.dependencies_sha256};
}
